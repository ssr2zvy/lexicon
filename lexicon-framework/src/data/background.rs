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
use lexicon_core::session::error::SessionLeaseError;
use lexicon_core::session::model::generate_session_id;
use lexicon_core::session::{SafeSessionFailure, SessionIdentity};

use crate::data::error::{ForegroundDataExecutionError, ProjectDiscoveryError};
use crate::data::foreground::{
    ForegroundRuntimeLauncher, PreparedForegroundExecution, ProcessCommandLauncher,
    spawn_and_supervise,
};
use crate::data::outcome::{BackgroundHandoffOutcome, ForegroundDataOutcome};
use crate::data::project::resolve_project_layout;
use crate::data::request::ForegroundDataRequest;
use crate::data::runtime::admit_bundle;
use crate::data::session::{build_coordinator, build_project_identity, select_and_prepare_session};
use crate::session::SessionCoordinationError;
use crate::supervision::{OperatorHostInvocationEncodingError, OperatorHostInvocationV1};

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
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct HandoffIntentDocumentV1 {
    pub(crate) schema_version: u32,
    pub(crate) session_id: String,
    pub(crate) handoff_token: String,
    pub(crate) initiator_pid: u32,
    pub(crate) created_at_unix_nanos: u64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct HandoffAcknowledgementDocumentV1 {
    pub(crate) schema_version: u32,
    pub(crate) session_id: String,
    pub(crate) handoff_token: String,
    pub(crate) operator_host_pid: u32,
    pub(crate) accepted_at_unix_nanos: u64,
}

pub(crate) fn execute_background_data_with_re_executor_and_timing(
    request: ForegroundDataRequest,
    re_executor: &dyn OperatorHostReExecutor,
    ownership_handoff_timeout: Duration,
    ownership_poll_interval: Duration,
) -> Result<BackgroundHandoffOutcome, ForegroundDataExecutionError> {
    // 1. Project discovery and layout validation.
    let (layout, project_name) = resolve_project_layout(&request.source_name, &request.protocol, request.operation)?;

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
    let session_dir = layout.session_directory(request.operation, session_id.id());
    let intent_path = session_dir.join("handoff_intent.json");
    let ack_path = session_dir.join("handoff_ack.json");

    // 6. Generate unguessable single-use handoff token and write intent document while retaining lease.
    let handoff_token = format!("{}-{}", session_id.id(), generate_session_id());
    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let intent = HandoffIntentDocumentV1 {
        schema_version: 1,
        session_id: session_id.id().to_string(),
        handoff_token: handoff_token.clone(),
        initiator_pid: std::process::id(),
        created_at_unix_nanos: now_nanos,
    };
    let intent_json = serde_json::to_string(&intent).map_err(|e| {
        ForegroundDataExecutionError::OperatorHostEncoding(
            OperatorHostInvocationEncodingError::Serialization(e.to_string()),
        )
    })?;
    if let Err(e) = std::fs::write(&intent_path, intent_json) {
        let _ = prepared.fail_launch(coordinator.store(), SafeSessionFailure::source_failure());
        return Err(ForegroundDataExecutionError::ProjectDiscovery(
            ProjectDiscoveryError::CurrentDirectory(e),
        ));
    }

    // 7. Build the operator-host invocation reference and spawn operator host.
    let reference = OperatorHostInvocationV1::new(
        request.source_name.clone(),
        request.protocol.clone(),
        request.operation,
        session_id.clone(),
        handoff_token.clone(),
    );
    let encoded_reference = reference
        .to_json()
        .map_err(ForegroundDataExecutionError::OperatorHostEncoding)?;

    let mut operator_host_arguments: Vec<OsString> = vec![
        OsString::from("__operator-host"),
        OsString::from(encoded_reference),
        OsString::from("--"),
    ];
    operator_host_arguments.extend(request.source_arguments.iter().cloned());

    let mut operator_host_child = match re_executor
        .spawn_operator_host(&operator_host_arguments, layout.project_root())
    {
        Ok(c) => c,
        Err(e) => {
            let _ = std::fs::remove_file(&intent_path);
            let _ = prepared.fail_launch(coordinator.store(), SafeSessionFailure::source_failure());
            return Err(ForegroundDataExecutionError::OperatorHostReExec(e));
        }
    };

    let expected_child_pid = operator_host_child.id();

    // 8. Wait, bounded, for the operator host to write acknowledgement.
    let deadline = Instant::now() + ownership_handoff_timeout;
    let mut handoff_acknowledged = false;

    loop {
        match operator_host_child.try_wait() {
            Ok(Some(status)) => {
                let _ = std::fs::remove_file(&intent_path);
                let _ = std::fs::remove_file(&ack_path);
                let _ = prepared.fail_launch(coordinator.store(), SafeSessionFailure::source_failure());
                return Err(ForegroundDataExecutionError::OperatorHostExitedBeforeOwnership {
                    exit_code: status.code(),
                });
            }
            Ok(None) => {}
            Err(io_error) => {
                let _ = operator_host_child.kill();
                let _ = operator_host_child.wait();
                let _ = std::fs::remove_file(&intent_path);
                let _ = std::fs::remove_file(&ack_path);
                let _ = prepared.fail_launch(coordinator.store(), SafeSessionFailure::source_failure());
                return Err(ForegroundDataExecutionError::OperatorHostReExec(io_error));
            }
        }

        if ack_path.is_file() {
            let ack_bytes = match std::fs::read_to_string(&ack_path) {
                Ok(b) => b,
                Err(_) => {
                    std::thread::sleep(ownership_poll_interval);
                    continue;
                }
            };
            let ack: HandoffAcknowledgementDocumentV1 = match serde_json::from_str(&ack_bytes) {
                Ok(a) => a,
                Err(e) => {
                    let _ = operator_host_child.kill();
                    let _ = operator_host_child.wait();
                    let _ = std::fs::remove_file(&intent_path);
                    let _ = std::fs::remove_file(&ack_path);
                    let _ = prepared.fail_launch(coordinator.store(), SafeSessionFailure::source_failure());
                    return Err(ForegroundDataExecutionError::OperatorHostAcknowledgementMismatch(
                        format!("malformed acknowledgement JSON: {e}"),
                    ));
                }
            };

            if ack.session_id != session_id.id() {
                let _ = operator_host_child.kill();
                let _ = operator_host_child.wait();
                let _ = std::fs::remove_file(&intent_path);
                let _ = std::fs::remove_file(&ack_path);
                let _ = prepared.fail_launch(coordinator.store(), SafeSessionFailure::source_failure());
                return Err(ForegroundDataExecutionError::OperatorHostAcknowledgementMismatch(
                    format!("mismatched session_id: expected {}, found {}", session_id.id(), ack.session_id),
                ));
            }

            if ack.handoff_token != handoff_token {
                let _ = operator_host_child.kill();
                let _ = operator_host_child.wait();
                let _ = std::fs::remove_file(&intent_path);
                let _ = std::fs::remove_file(&ack_path);
                let _ = prepared.fail_launch(coordinator.store(), SafeSessionFailure::source_failure());
                return Err(ForegroundDataExecutionError::OperatorHostAcknowledgementMismatch(
                    "mismatched handoff token".to_string(),
                ));
            }

            if expected_child_pid != 0 && ack.operator_host_pid != expected_child_pid {
                let _ = operator_host_child.kill();
                let _ = operator_host_child.wait();
                let _ = std::fs::remove_file(&intent_path);
                let _ = std::fs::remove_file(&ack_path);
                let _ = prepared.fail_launch(coordinator.store(), SafeSessionFailure::source_failure());
                return Err(ForegroundDataExecutionError::OperatorHostAcknowledgementMismatch(
                    format!("mismatched operator host pid: expected {expected_child_pid}, found {}", ack.operator_host_pid),
                ));
            }

            handoff_acknowledged = true;
            break;
        }

        if Instant::now() >= deadline {
            let _ = operator_host_child.kill();
            let _ = operator_host_child.wait();
            let _ = std::fs::remove_file(&intent_path);
            let _ = std::fs::remove_file(&ack_path);
            let _ = prepared.fail_launch(coordinator.store(), SafeSessionFailure::source_failure());
            return Err(ForegroundDataExecutionError::OperatorHostOwnershipTimeout);
        }

        std::thread::sleep(ownership_poll_interval);
    }

    if !handoff_acknowledged {
        let _ = operator_host_child.kill();
        let _ = operator_host_child.wait();
        let _ = std::fs::remove_file(&intent_path);
        let _ = std::fs::remove_file(&ack_path);
        let _ = prepared.fail_launch(coordinator.store(), SafeSessionFailure::source_failure());
        return Err(ForegroundDataExecutionError::OperatorHostOwnershipTimeout);
    }

    // Release lease to transfer ownership to the acknowledged operator host.
    let _ = prepared.release_for_handoff();
    let _ = std::fs::remove_file(&intent_path);

    Ok(BackgroundHandoffOutcome {
        project: project_name,
        source: request.source_name,
        operation: request.operation,
        session: session_id,
    })
}

/// Execute the operator-host role: resume the already-`Prepared` session
/// described by `reference`, spawn the runtime, and supervise it to
/// completion.
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
    let protocol = reference.protocol().to_owned();
    let session_id: SessionIdentity = reference.session().clone();
    let handoff_token = reference.handoff_token().to_owned();

    // 1-4. Re-derive the exact same project layout, bundle, identity, and
    // coordinator the initiating process built.
    let (layout, project_name) = resolve_project_layout(&source_name, &protocol, operation)?;
    let admitted = admit_bundle(&layout, operation)?;
    let project_identity = build_project_identity(&project_name)?;
    let runtime_identity = admitted.identity().clone();
    let coordinator = build_coordinator(&layout, project_identity.clone(), runtime_identity.clone(), operation)?;

    let session_dir = layout.session_directory(operation, session_id.id());
    let intent_path = session_dir.join("handoff_intent.json");
    let ack_path = session_dir.join("handoff_ack.json");

    // 5. Verify handoff authorization intent.
    if !intent_path.is_file() {
        return Err(ForegroundDataExecutionError::OperatorHostUnauthorizedHandoff(
            "no handoff intent found for this session".to_string(),
        ));
    }
    let intent_bytes = std::fs::read_to_string(&intent_path).map_err(|e| {
        ForegroundDataExecutionError::OperatorHostUnauthorizedHandoff(format!("failed to read handoff intent: {e}"))
    })?;
    let intent: HandoffIntentDocumentV1 = serde_json::from_str(&intent_bytes).map_err(|e| {
        ForegroundDataExecutionError::OperatorHostUnauthorizedHandoff(format!("corrupt handoff intent: {e}"))
    })?;
    if intent.session_id != session_id.id() || intent.handoff_token != handoff_token {
        return Err(ForegroundDataExecutionError::OperatorHostUnauthorizedHandoff(
            "handoff token mismatch".to_string(),
        ));
    }

    // 6. Write acknowledgement so the initiator can safely release its lease.
    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let ack = HandoffAcknowledgementDocumentV1 {
        schema_version: 1,
        session_id: session_id.id().to_string(),
        handoff_token: handoff_token.clone(),
        operator_host_pid: std::process::id(),
        accepted_at_unix_nanos: now_nanos,
    };
    std::fs::write(&ack_path, serde_json::to_string(&ack).unwrap()).map_err(|e| {
        ForegroundDataExecutionError::ProjectDiscovery(ProjectDiscoveryError::CurrentDirectory(e))
    })?;

    // 7. Resume the prepared session, waiting for initiator to release lease.
    let resume_deadline = Instant::now() + Duration::from_secs(5);
    let prepared = loop {
        match coordinator.resume_prepared_launch(&session_id) {
            Ok(p) => break p,
            Err(SessionCoordinationError::Lease(SessionLeaseError::AlreadyOwned)) => {
                if Instant::now() >= resume_deadline {
                    return Err(ForegroundDataExecutionError::SessionPreparation(
                        SessionCoordinationError::Lease(SessionLeaseError::AlreadyOwned),
                    ));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(ForegroundDataExecutionError::SessionPreparation(e)),
        }
    };

    let owner = PreparedForegroundExecution::new(prepared, operation, project_name, source_name);

    // 8. Shared spawn-and-supervise pipeline, recording Background supervision.
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

    const FAST_TIMEOUT: Duration = Duration::from_millis(2000);
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
            protocol: "http".to_string(),
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
            arguments: &[OsString],
            _working_directory: &Path,
        ) -> Result<Child, std::io::Error> {
            let encoded_ref = arguments[1].to_str().expect("valid reference string");
            let reference = OperatorHostInvocationV1::from_json(encoded_ref).expect("valid ref");
            let session_id = reference.session().clone();
            let handoff_token = reference.handoff_token().to_string();

            let child = spawn_long_running_process();
            let child_pid = child.id();

            // Write acknowledgement matching the spawned child and token
            let session_dir = self
                .coordinator
                .store()
                .operation_root()
                .session_directory(&session_id);
            let ack = HandoffAcknowledgementDocumentV1 {
                schema_version: 1,
                session_id: session_id.id().to_string(),
                handoff_token,
                operator_host_pid: child_pid,
                accepted_at_unix_nanos: 1,
            };
            std::fs::write(
                session_dir.join("handoff_ack.json"),
                serde_json::to_string(&ack).unwrap(),
            )
            .unwrap();

            Ok(child)
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
            resolve_project_layout("example-source", "http", DataOperation::Acquisition)
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
            resolve_project_layout("example-source", "http", DataOperation::Acquisition)
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
            OperatorHostInvocationV1::new("example-source", "http", DataOperation::Acquisition, session_id.clone(), "test-token");

        let session_dir = layout.session_directory(DataOperation::Acquisition, session_id.id());
        let intent = HandoffIntentDocumentV1 {
            schema_version: 1,
            session_id: session_id.id().to_string(),
            handoff_token: "test-token".to_string(),
            initiator_pid: std::process::id(),
            created_at_unix_nanos: 1,
        };
        std::fs::write(session_dir.join("handoff_intent.json"), serde_json::to_string(&intent).unwrap()).unwrap();

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

    #[test]
    fn operator_host_rejects_missing_or_mismatched_handoff_token() {
        let project = build_fake_project("example-source");
        let (layout, project_name) = with_test_cwd(&project.project_root, || {
            resolve_project_layout("example-source", "http", DataOperation::Acquisition)
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
        let _ = prepared.release_for_handoff();

        // Pass an invocation reference without creating the handoff intent file
        let reference = OperatorHostInvocationV1::new(
            "example-source",
            "http",
            DataOperation::Acquisition,
            session_id,
            "unauthorized-token",
        );

        let result = with_test_cwd(&project.project_root, || {
            execute_operator_host_with_launcher(reference, Vec::new(), &ProcessCommandLauncher)
        });

        assert!(matches!(
            result,
            Err(ForegroundDataExecutionError::OperatorHostUnauthorizedHandoff(_))
        ));
    }

    #[test]
    fn processing_background_handoff_succeeds() {
        let project = build_fake_project("example-source");
        let (layout, project_name) = with_test_cwd(&project.project_root, || {
            resolve_project_layout("example-source", "http", DataOperation::Processing)
        })
        .unwrap();
        let admitted = admit_bundle(&layout, DataOperation::Processing).unwrap();
        let project_identity = build_project_identity(&project_name).unwrap();
        let coordinator = build_coordinator(
            &layout,
            project_identity,
            admitted.identity().clone(),
            DataOperation::Processing,
        )
        .unwrap();

        let re_executor = ResumesMostRecentSessionAndHoldsLease::new(&coordinator);
        let mut request = background_request("example-source");
        request.operation = DataOperation::Processing;

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
        assert_eq!(outcome.operation, DataOperation::Processing);
    }

    struct WritesCorruptedAckExecutor;

    impl OperatorHostReExecutor for WritesCorruptedAckExecutor {
        fn spawn_operator_host(
            &self,
            arguments: &[OsString],
            _working_directory: &Path,
        ) -> Result<Child, std::io::Error> {
            let encoded_ref = arguments[1].to_str().expect("valid reference string");
            let reference = OperatorHostInvocationV1::from_json(encoded_ref).expect("valid ref");
            let session_id = reference.session().clone();

            let child = spawn_long_running_process();
            let child_pid = child.id();

            // Write mismatched token in ack
            let session_dir = _working_directory
                .join("sources/example-source/http/get-raw-data/sessions")
                .join(session_id.id());
            let ack = HandoffAcknowledgementDocumentV1 {
                schema_version: 1,
                session_id: session_id.id().to_string(),
                handoff_token: "wrong-token".to_string(),
                operator_host_pid: child_pid,
                accepted_at_unix_nanos: 1,
            };
            std::fs::write(
                session_dir.join("handoff_ack.json"),
                serde_json::to_string(&ack).unwrap(),
            )
            .unwrap();

            Ok(child)
        }
    }

    #[test]
    fn mismatched_acknowledgement_token_fails_handoff() {
        let project = build_fake_project("example-source");
        let request = background_request("example-source");

        let result = with_test_cwd(&project.project_root, || {
            execute_background_data_with_re_executor_and_timing(
                request,
                &WritesCorruptedAckExecutor,
                FAST_TIMEOUT,
                FAST_POLL,
            )
        });

        assert!(matches!(
            result,
            Err(ForegroundDataExecutionError::OperatorHostAcknowledgementMismatch(_))
        ));
    }
}
