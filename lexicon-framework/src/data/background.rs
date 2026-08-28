//! Background data execution: operator-host re-execution and durable session handoff.
//!
//! Implements contract.md section 3 ("Background execution") and specs.md
//! section 8.2. Two distinct roles are implemented here:
//!
//! * [`execute_background_data`] runs in the *initiating* process. It
//!   prepares a session exactly like the foreground path, but with
//!   `RuntimeSupervisionMode::Background`, then hands durable ownership of
//!   that session off to a re-executed `__operator-host` process and returns
//!   once ownership is confirmed. It does not spawn the actual source
//!   runtime itself.
//! * [`execute_operator_host`] runs in the re-executed operator-host process.
//!   It resumes the already-`Prepared` session the initiating process
//!   handed off, then reuses the exact same spawn-and-supervise pipeline the
//!   foreground path uses ([`super::foreground::spawn_and_supervise`]).
//!
//! Per contract.md section 9, raw source arguments are never persisted as
//! part of the handoff. They travel only as the operator-host process's own
//! trailing argv, exactly like the ordinary foreground `-- <source-args>` path.

use std::ffi::OsString;
use std::path::Path;
use std::time::{Duration, Instant};

use lexicon_core::runtime::RuntimeSupervisionMode;
use lexicon_core::session::{SessionIdentity, SessionLeaseState};

use crate::data::error::ForegroundDataExecutionError;
use crate::data::foreground::{
    ForegroundRuntimeLauncher, PreparedForegroundExecution, ProcessCommandLauncher,
    spawn_and_supervise,
};
use crate::data::outcome::{BackgroundHandoffOutcome, ForegroundDataOutcome};
use crate::data::project::resolve_project_layout;
use crate::data::request::ForegroundDataRequest;
use crate::data::runtime::admit_bundle;
use crate::data::session::{build_coordinator, build_project_identity, select_and_prepare_session};
use crate::supervision::OperatorHostInvocationV1;

/// Bounded wait for the operator-host process to acquire durable session ownership.
const OWNERSHIP_HANDOFF_TIMEOUT: Duration = Duration::from_secs(10);
const OWNERSHIP_POLL_INTERVAL: Duration = Duration::from_millis(20);

// ---------------------------------------------------------------------------
// Re-exec seam
// ---------------------------------------------------------------------------

/// Narrow seam for re-executing the current binary in the `__operator-host` role.
///
/// Mirrors [`ForegroundRuntimeLauncher`]: it separates the mechanics of
/// spawning the operator-host process from the ownership-handoff policy, so
/// the handoff loop is testable without launching a real process.
pub(crate) trait OperatorHostReExecutor {
    fn spawn_operator_host(
        &self,
        arguments: &[OsString],
        working_directory: &Path,
    ) -> Result<std::process::Child, std::io::Error>;
}

/// Production re-executor: re-executes `std::env::current_exe()`.
pub(crate) struct ProcessOperatorHostReExecutor;

impl OperatorHostReExecutor for ProcessOperatorHostReExecutor {
    fn spawn_operator_host(
        &self,
        arguments: &[OsString],
        working_directory: &Path,
    ) -> Result<std::process::Child, std::io::Error> {
        let current_exe = std::env::current_exe()?;
        let mut command = std::process::Command::new(current_exe);
        command.args(arguments);
        command.current_dir(working_directory);
        command.spawn()
    }
}

// ---------------------------------------------------------------------------
// Initiating-process background path
// ---------------------------------------------------------------------------

/// Execute a background data acquisition or processing run.
///
/// Prepares the session with `RuntimeSupervisionMode::Background`, hands it
/// off to a re-executed `__operator-host` process, and returns once that
/// process has durably acquired ownership. It does not wait for the
/// operation itself to finish.
pub fn execute_background_data(
    request: ForegroundDataRequest,
) -> Result<BackgroundHandoffOutcome, ForegroundDataExecutionError> {
    execute_background_data_with_re_executor(request, &ProcessOperatorHostReExecutor)
}

pub(crate) fn execute_background_data_with_re_executor(
    request: ForegroundDataRequest,
    re_executor: &dyn OperatorHostReExecutor,
) -> Result<BackgroundHandoffOutcome, ForegroundDataExecutionError> {
    execute_background_data_with_re_executor_and_timing(
        request,
        re_executor,
        OWNERSHIP_HANDOFF_TIMEOUT,
        OWNERSHIP_POLL_INTERVAL,
    )
}

/// Test-only seam: identical to [`execute_background_data_with_re_executor`],
/// but with the ownership-handoff timeout and poll interval injectable so
/// tests do not have to wait out the real 10-second production timeout.
/// `execute_background_data` and `execute_background_data_with_re_executor`
/// always pass the fixed production constants; this function does not change
/// their externally observable behavior.
pub(crate) fn execute_background_data_with_re_executor_and_timing(
    request: ForegroundDataRequest,
    re_executor: &dyn OperatorHostReExecutor,
    ownership_handoff_timeout: Duration,
    ownership_poll_interval: Duration,
) -> Result<BackgroundHandoffOutcome, ForegroundDataExecutionError> {
    // 1. Project discovery and layout validation.
    let (layout, project_name) = resolve_project_layout(&request.source_name, request.operation)?;

    // 2. Bundle admission.
    let admitted = admit_bundle(&layout, request.operation)?;

    // 3. Build project identity.
    let project_identity = build_project_identity(&project_name)?;

    // 4. Build session coordinator.
    let runtime_identity = admitted.identity().clone();
    let coordinator = build_coordinator(
        &layout,
        project_identity.clone(),
        runtime_identity.clone(),
        request.operation,
    )?;

    // 5. Session selection policy, requesting Background supervision.
    let prepared = select_and_prepare_session(
        &coordinator,
        request.operation,
        request.abandon_past_failure,
        &admitted,
        RuntimeSupervisionMode::Background,
    )?;

    let session_id = prepared.session().clone();

    // 6. Release the lease so the operator host can acquire it itself. The
    // durable Prepared record survives; only ownership is released.
    let _record = prepared.release_for_handoff();

    // 7. Build the operator-host invocation reference and re-execute.
    let reference =
        OperatorHostInvocationV1::new(request.source_name.clone(), request.operation, session_id.clone());
    let encoded_reference = reference
        .to_json()
        .map_err(ForegroundDataExecutionError::OperatorHostEncoding)?;

    let mut operator_host_arguments: Vec<OsString> = vec![
        OsString::from("__operator-host"),
        OsString::from(encoded_reference),
        OsString::from("--"),
    ];
    operator_host_arguments.extend(request.source_arguments.iter().cloned());

    let mut operator_host_child = re_executor
        .spawn_operator_host(&operator_host_arguments, layout.project_root())
        .map_err(ForegroundDataExecutionError::OperatorHostReExec)?;

    // 8. Wait, bounded, for the operator host to acquire durable ownership.
    let deadline = Instant::now() + ownership_handoff_timeout;
    loop {
        match operator_host_child.try_wait() {
            Ok(Some(status)) => {
                return Err(ForegroundDataExecutionError::OperatorHostExitedBeforeOwnership {
                    exit_code: status.code(),
                });
            }
            Ok(None) => {}
            Err(io_error) => {
                return Err(ForegroundDataExecutionError::OperatorHostReExec(io_error));
            }
        }

        match coordinator.store().inspect_lease_state(&session_id) {
            Ok(SessionLeaseState::Owned) => break,
            Ok(SessionLeaseState::Available) => {}
            Err(lease_error) => {
                return Err(ForegroundDataExecutionError::OperatorHostOwnershipCheckFailed(
                    lease_error,
                ));
            }
        }

        if Instant::now() >= deadline {
            return Err(ForegroundDataExecutionError::OperatorHostOwnershipTimeout);
        }

        std::thread::sleep(ownership_poll_interval);
    }

    Ok(BackgroundHandoffOutcome {
        project: project_name,
        source: request.source_name,
        operation: request.operation,
        session: session_id,
    })
}

// ---------------------------------------------------------------------------
// Operator-host entrypoint
// ---------------------------------------------------------------------------

/// Execute the operator-host role: resume the already-`Prepared` session
/// described by `reference`, spawn the runtime, and supervise it to
/// completion.
///
/// `source_arguments` are the operator-host process's own trailing argv
/// (after `--`), forwarded to the source implementation exactly as received.
/// They are never read from `reference`.
pub fn execute_operator_host(
    reference: OperatorHostInvocationV1,
    source_arguments: Vec<OsString>,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    execute_operator_host_with_launcher(reference, source_arguments, &ProcessCommandLauncher)
}

pub(crate) fn execute_operator_host_with_launcher(
    reference: OperatorHostInvocationV1,
    source_arguments: Vec<OsString>,
    launcher: &dyn ForegroundRuntimeLauncher,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    let operation = reference.operation();
    let source_name = reference.source_name().to_owned();
    let session_id: SessionIdentity = reference.session().clone();

    // 1-4. Re-derive the exact same project layout, bundle, identity, and
    // coordinator the initiating process built. Nothing here is trusted from
    // `reference` beyond the source name and operation; the coordinator and
    // bundle are independently re-validated.
    let (layout, project_name) = resolve_project_layout(&source_name, operation)?;
    let admitted = admit_bundle(&layout, operation)?;
    let project_identity = build_project_identity(&project_name)?;
    let runtime_identity = admitted.identity().clone();
    let coordinator = build_coordinator(&layout, project_identity.clone(), runtime_identity.clone(), operation)?;

    // 5'. Resume the already-Prepared session instead of creating a new one.
    let prepared = coordinator
        .resume_prepared_launch(&session_id)
        .map_err(ForegroundDataExecutionError::SessionPreparation)?;

    let owner = PreparedForegroundExecution::new(prepared, operation, project_name, source_name);

    // 6-11. Shared spawn-and-supervise pipeline, recording Background supervision.
    spawn_and_supervise(
        owner,
        &layout,
        &admitted,
        &coordinator,
        &source_arguments,
        RuntimeSupervisionMode::Background,
        launcher,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::process::Child;

    use lexicon_core::session::SafeSessionFailure;

    use crate::data::request::DataOperation;
    use crate::data::test_support::{build_fake_project, with_test_cwd};
    use crate::session::{PreparedSessionLaunch, SessionCoordinationError, SessionCoordinator};

    use super::*;

    const FAST_TIMEOUT: Duration = Duration::from_millis(300);
    const FAST_POLL: Duration = Duration::from_millis(10);

    /// Spawn a real, short-lived child process that exits immediately.
    fn spawn_immediately_exiting_process() -> Child {
        #[cfg(windows)]
        {
            std::process::Command::new("cmd")
                .args(["/C", "exit", "0"])
                .spawn()
                .expect("spawn immediately-exiting process")
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new("sh")
                .args(["-c", "exit 0"])
                .spawn()
                .expect("spawn immediately-exiting process")
        }
    }

    /// Spawn a real child process that stays alive for a while, so tests can
    /// observe `try_wait() == Ok(None)` before it is dropped (which reaps and
    /// implicitly terminates it on scope exit for the purposes of this test
    /// binary; the OS is responsible for eventual cleanup regardless).
    fn spawn_long_running_process() -> Child {
        #[cfg(windows)]
        {
            std::process::Command::new("cmd")
                .args(["/C", "ping -n 5 127.0.0.1 >NUL"])
                .spawn()
                .expect("spawn long-running process")
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new("sh")
                .args(["-c", "sleep 5"])
                .spawn()
                .expect("spawn long-running process")
        }
    }

    fn background_request(source_name: &str) -> ForegroundDataRequest {
        ForegroundDataRequest {
            operation: DataOperation::Acquisition,
            source_name: source_name.to_string(),
            abandon_past_failure: false,
            background: true,
            source_arguments: Vec::new(),
        }
    }

    /// A fake re-executor that resumes whatever session the coordinator most
    /// recently prepared (simulating the operator host taking ownership) and
    /// retains the resumed lease for as long as this value lives, so the
    /// caller's polling loop observes `Owned`. Releases the lease when this
    /// executor (and therefore the held `RefCell`) is dropped at the end of
    /// the test, well before the fixture's temp directory is removed.
    struct ResumesMostRecentSessionAndHoldsLease<'a> {
        coordinator: &'a SessionCoordinator,
        held: RefCell<Option<PreparedSessionLaunch>>,
    }

    impl<'a> ResumesMostRecentSessionAndHoldsLease<'a> {
        fn new(coordinator: &'a SessionCoordinator) -> Self {
            Self {
                coordinator,
                held: RefCell::new(None),
            }
        }
    }

    impl OperatorHostReExecutor for ResumesMostRecentSessionAndHoldsLease<'_> {
        fn spawn_operator_host(
            &self,
            _arguments: &[OsString],
            _working_directory: &Path,
        ) -> Result<Child, std::io::Error> {
            let status = self
                .coordinator
                .store()
                .load_status()
                .expect("load status")
                .expect("status exists after preparation");
            let session_id = status.current_session().expect("current session").clone();
            let resumed = self
                .coordinator
                .resume_prepared_launch(&session_id)
                .expect("resume prepared launch");
            *self.held.borrow_mut() = Some(resumed);
            Ok(spawn_long_running_process())
        }
    }

    /// A fake re-executor whose spawned process exits immediately without
    /// ever acquiring the lease.
    struct ExitsWithoutAcquiringLease;

    impl OperatorHostReExecutor for ExitsWithoutAcquiringLease {
        fn spawn_operator_host(
            &self,
            _arguments: &[OsString],
            _working_directory: &Path,
        ) -> Result<Child, std::io::Error> {
            Ok(spawn_immediately_exiting_process())
        }
    }

    /// A fake re-executor whose spawned process never exits and never
    /// acquires the lease, exercising the timeout path.
    struct NeverAcquiresLease;

    impl OperatorHostReExecutor for NeverAcquiresLease {
        fn spawn_operator_host(
            &self,
            _arguments: &[OsString],
            _working_directory: &Path,
        ) -> Result<Child, std::io::Error> {
            Ok(spawn_long_running_process())
        }
    }

    /// A fake re-executor that fails to spawn anything.
    struct FailsToSpawn;

    impl OperatorHostReExecutor for FailsToSpawn {
        fn spawn_operator_host(
            &self,
            _arguments: &[OsString],
            _working_directory: &Path,
        ) -> Result<Child, std::io::Error> {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "fake spawn failure"))
        }
    }

    #[test]
    fn successful_handoff_returns_outcome_once_lease_is_owned() {
        let project = build_fake_project("example-source");

        let (layout, _project_name) = with_test_cwd(&project.project_root, || {
            resolve_project_layout("example-source", DataOperation::Acquisition)
        })
        .unwrap();
        let admitted = admit_bundle(&layout, DataOperation::Acquisition).unwrap();
        let project_identity = build_project_identity("test-project").unwrap();
        let coordinator = build_coordinator(
            &layout,
            project_identity,
            admitted.identity().clone(),
            DataOperation::Acquisition,
        )
        .unwrap();

        let re_executor = ResumesMostRecentSessionAndHoldsLease::new(&coordinator);
        let request = background_request("example-source");

        let outcome = with_test_cwd(&project.project_root, || {
            execute_background_data_with_re_executor_and_timing(
                request,
                &re_executor,
                FAST_TIMEOUT,
                FAST_POLL,
            )
        })
        .unwrap();

        assert_eq!(outcome.source, "example-source");
        assert_eq!(outcome.operation, DataOperation::Acquisition);
    }

    #[test]
    fn operator_host_exiting_before_ownership_is_a_typed_error() {
        let project = build_fake_project("example-source");
        let request = background_request("example-source");

        let result = with_test_cwd(&project.project_root, || {
            execute_background_data_with_re_executor_and_timing(
                request,
                &ExitsWithoutAcquiringLease,
                FAST_TIMEOUT,
                FAST_POLL,
            )
        });

        assert!(matches!(
            result,
            Err(ForegroundDataExecutionError::OperatorHostExitedBeforeOwnership { .. })
        ));
    }

    #[test]
    fn ownership_timeout_is_a_typed_error() {
        let project = build_fake_project("example-source");
        let request = background_request("example-source");

        let result = with_test_cwd(&project.project_root, || {
            execute_background_data_with_re_executor_and_timing(
                request,
                &NeverAcquiresLease,
                FAST_TIMEOUT,
                FAST_POLL,
            )
        });

        assert!(matches!(
            result,
            Err(ForegroundDataExecutionError::OperatorHostOwnershipTimeout)
        ));
    }

    #[test]
    fn re_exec_spawn_failure_is_a_typed_error() {
        let project = build_fake_project("example-source");
        let request = background_request("example-source");

        let result = with_test_cwd(&project.project_root, || {
            execute_background_data_with_re_executor_and_timing(
                request,
                &FailsToSpawn,
                FAST_TIMEOUT,
                FAST_POLL,
            )
        });

        assert!(matches!(
            result,
            Err(ForegroundDataExecutionError::OperatorHostReExec(_))
        ));
    }

    #[test]
    fn operator_host_rejects_a_session_that_is_no_longer_prepared() {
        let project = build_fake_project("example-source");

        let (layout, project_name) = with_test_cwd(&project.project_root, || {
            resolve_project_layout("example-source", DataOperation::Acquisition)
        })
        .unwrap();
        let admitted = admit_bundle(&layout, DataOperation::Acquisition).unwrap();
        let project_identity = build_project_identity(&project_name).unwrap();
        let coordinator = build_coordinator(
            &layout,
            project_identity,
            admitted.identity().clone(),
            DataOperation::Acquisition,
        )
        .unwrap();

        let prepared = coordinator.prepare_run(RuntimeSupervisionMode::Background).unwrap();
        let session_id = prepared.session().clone();
        // Release, then resume-and-fail, so the session is durably advanced
        // out of `Prepared` without going through a real operator host.
        prepared.release_for_handoff();
        let first_resume = coordinator.resume_prepared_launch(&session_id).unwrap();
        first_resume
            .fail_launch(coordinator.store(), SafeSessionFailure::source_failure())
            .unwrap();

        let reference =
            OperatorHostInvocationV1::new("example-source", DataOperation::Acquisition, session_id);

        let result = with_test_cwd(&project.project_root, || {
            execute_operator_host_with_launcher(reference, Vec::new(), &ProcessCommandLauncher)
        });

        assert!(matches!(
            result,
            Err(ForegroundDataExecutionError::SessionPreparation(
                SessionCoordinationError::HandoffSessionNotPrepared { .. }
            ))
        ));
    }
}
