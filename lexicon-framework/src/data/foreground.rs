use std::ffi::{OsStr, OsString};
use std::path::Path;

use lexicon_core::runtime::{
    RuntimeExecutionMode, RuntimeInvocationEnvelopeV1,
    encode_runtime_invocation,
    invocation::{ProjectInvocationIdentity, SessionInvocationIdentity},
};
use lexicon_core::session::{
    RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE, SafeSessionFailure, SessionFailureCode, SessionFailureKind,
    SessionIdentity, SessionRecordV1, SessionState,
};

use crate::data::error::{
    ForegroundDataExecutionError, ForegroundInvocationConstructionError,
    ForegroundPreparationError, WaitRecoveryFailure,
};
use crate::data::outcome::{ForegroundDataOutcome, ObservedChildTermination};
use crate::data::project::resolve_project_layout;
use crate::data::request::{DataOperation, ForegroundDataRequest};
use crate::data::runtime::{AdmittedBundle, admit_bundle, recheck_executable_integrity};
use crate::data::session::{
    build_coordinator, build_project_identity, load_and_validate_terminal_session,
    load_terminal_session, persist_abnormal_termination,
    select_and_prepare_session, validate_root_summary_against_record,
};
use crate::session::{PreparedSessionLaunch, SessionCoordinationError};

// ---------------------------------------------------------------------------
// Launcher seam
// ---------------------------------------------------------------------------

/// Narrow ownership-oriented launcher for the foreground runtime process.
///
/// The seam separates preparation from spawning and child ownership.
/// It must not expose shell commands, arbitrary environment maps, or PATH lookup.
pub(crate) trait ForegroundRuntimeLauncher {
    fn spawn(
        &self,
        executable: &Path,
        arguments: &[OsString],
        context_environment: &OsStr,
        working_directory: &Path,
    ) -> Result<std::process::Child, std::io::Error>;
}

/// Production launcher using `std::process::Command`.
pub(crate) struct ProcessCommandLauncher;

impl ForegroundRuntimeLauncher for ProcessCommandLauncher {
    fn spawn(
        &self,
        executable: &Path,
        arguments: &[OsString],
        context_environment: &OsStr,
        working_directory: &Path,
    ) -> Result<std::process::Child, std::io::Error> {
        let mut cmd = std::process::Command::new(executable);
        cmd.args(arguments);
        cmd.env(RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE, context_environment);
        cmd.env_remove("LEXICON_SOURCE_DIRECTORY");
        cmd.current_dir(working_directory);
        cmd.spawn()
    }
}

// ---------------------------------------------------------------------------
// Ownership types
// ---------------------------------------------------------------------------

/// Owns the prepared session lease plus the information needed to spawn the runtime.
///
/// # Ownership invariant
/// Neither `PreparedForegroundExecution` nor `RunningForegroundExecution` is `Clone`.
/// The prepared owner retains the session lease. The lease must not be released
/// before terminal reconciliation completes.
pub(crate) struct PreparedForegroundExecution {
    prepared: PreparedSessionLaunch,
    operation: DataOperation,
    project_name: String,
    source_name: String,
}

impl PreparedForegroundExecution {
    fn new(
        prepared: PreparedSessionLaunch,
        operation: DataOperation,
        project_name: String,
        source_name: String,
    ) -> Self {
        Self { prepared, operation, project_name, source_name }
    }

    fn record(&self) -> &SessionRecordV1 {
        self.prepared.record()
    }

    fn session(&self) -> &SessionIdentity {
        self.prepared.session()
    }

    fn context_document(&self) -> &str {
        self.prepared.context_document()
    }

    fn operation_root(&self) -> &std::path::Path {
        self.prepared.operation_root()
    }
}

/// Owns both the live child handle and the session lease.
///
/// # Ownership invariant
/// While `RunningForegroundExecution` exists, the child may be alive and the supervisor
/// lease remains held. The lease is released only when terminal reconciliation
/// completes or produces its final structured error.
pub(crate) struct RunningForegroundExecution {
    child: std::process::Child,
    prepared: PreparedSessionLaunch,
    operation: DataOperation,
    project_name: String,
    source_name: String,
}

impl RunningForegroundExecution {
    /// Wait for the child to exit and reconcile the session.
    ///
    /// The supervisor lease is held throughout. It is released only at the end
    /// of this method, after reconciliation is complete.
    pub(crate) fn wait_and_reconcile(
        mut self,
    ) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
        // Wait loop: retry on EINTR, recover on other errors.
        let termination = loop {
            match self.child.wait() {
                Ok(status) => break observe_termination(status),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    // Retry: do not release ownership.
                    continue;
                }
                Err(wait_err) => {
                    return self.handle_wait_error(wait_err);
                }
            }
        };

        // The prepared record is needed for validation.
        let prepared_record = self.prepared.record().clone();
        let operation = self.operation;
        let source_name = self.source_name.clone();
        let project_name = self.project_name.clone();
        let operation_root = self.prepared.operation_root().to_path_buf();

        // Lease is still held via self.prepared; drop self (releasing the lease) only after
        // reconciliation below.
        let coordinator_store_path = operation_root.clone();

        // Reconcile and build outcome; lease is released when self is dropped at end of
        // this scope (or when fail_running_session consumes prepared).
        reconcile_termination_with_lease(
            termination,
            self,
            &prepared_record,
            &coordinator_store_path,
            operation,
            &source_name,
            &project_name,
        )
    }

    /// Recovery path when `child.wait()` fails with a non-Interrupted error.
    ///
    /// Attempts to determine child state, kill if running, reap, reconcile session.
    fn handle_wait_error(
        mut self,
        wait_err: std::io::Error,
    ) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
        let mut kill_error: Option<std::io::Error> = None;
        let mut reap_error: Option<std::io::Error> = None;
        let mut reconciliation_error: Option<SessionCoordinationError> = None;

        // Attempt kill.
        if let Err(e) = self.child.kill() {
            kill_error = Some(e);
        }

        // Attempt reap.
        match self.child.wait() {
            Ok(_) => {}
            Err(e) => {
                reap_error = Some(e);
            }
        }

        // Inspect durable session state and reconcile to Failed if non-terminal.
        let operation_root = self.prepared.operation_root().to_path_buf();
        let session_id = self.prepared.session().clone();
        if let Ok(record) = load_terminal_session(&operation_root, &session_id) {
            if matches!(record.state(), SessionState::Prepared | SessionState::Running) {
                if let Err(e) = persist_abnormal_termination(
                    &operation_root,
                    &session_id,
                    record.revision(),
                    SessionFailureCode::AbnormalTermination,
                    Some("wait error during foreground supervision".to_owned()),
                ) {
                    reconciliation_error = Some(e);
                }
            }
        }

        // Release lease (via drop of self.prepared and self.child).
        drop(self);

        Err(ForegroundDataExecutionError::ProcessWaitRecovery(Box::new(
            WaitRecoveryFailure {
                wait_error: wait_err,
                kill_error,
                reap_error,
                reconciliation_error,
            },
        )))
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Execute a foreground data acquisition or processing run.
///
/// This is the single public entrypoint for the data command.
pub fn execute_foreground_data(
    request: ForegroundDataRequest,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    execute_foreground_data_with_launcher(request, &ProcessCommandLauncher)
}

pub(crate) fn execute_foreground_data_with_launcher(
    request: ForegroundDataRequest,
    launcher: &dyn ForegroundRuntimeLauncher,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    if request.background {
        return Err(ForegroundDataExecutionError::BackgroundModeUnsupported);
    }

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

    // 5. Session selection policy (reconcile stale → select → prepare).
    let prepared = select_and_prepare_session(
        &coordinator,
        request.operation,
        request.abandon_past_failure,
        &admitted,
    )?;

    // From here onward, every error must transition the session to Failed before returning.
    let owner = PreparedForegroundExecution::new(
        prepared,
        request.operation,
        project_name,
        request.source_name,
    );

    // 6. Build invocation envelope.
    let session_id = owner.session().clone();
    let execution_mode = owner.record().execution_mode();
    let envelope = match build_invocation_envelope(
        &owner.project_name,
        &admitted,
        &session_id,
        execution_mode,
    ) {
        Ok(e) => e,
        Err(cause) => {
            return Err(fail_prepared_execution(
                owner.prepared,
                coordinator.store(),
                SessionFailureCode::InvocationConstructionFailed,
                ForegroundPreparationError::InvocationConstruction(cause),
            ));
        }
    };

    // 7. Encode argv.
    let encoded = match encode_runtime_invocation(&envelope, &request.source_arguments) {
        Ok(e) => e,
        Err(cause) => {
            return Err(fail_prepared_execution(
                owner.prepared,
                coordinator.store(),
                SessionFailureCode::InvocationEncodingFailed,
                ForegroundPreparationError::InvocationEncoding(cause),
            ));
        }
    };

    // 8. Pre-launch executable integrity recheck.
    if let Err(prep_err) = recheck_executable_integrity_typed(&admitted) {
        return Err(fail_prepared_execution(
            owner.prepared,
            coordinator.store(),
            SessionFailureCode::ExecutableIntegrityFailed,
            prep_err,
        ));
    }

    // 9. Build argv.
    let executable = admitted.executable_path().to_path_buf();
    let mut argv: Vec<OsString> = Vec::new();
    argv.extend(encoded.arguments().iter().cloned());

    let context_document = owner.context_document().to_owned();
    let working_directory = layout.protocol_root().to_path_buf();

    // 10. Spawn.
    let child = match launcher.spawn(
        &executable,
        &argv,
        OsStr::new(&context_document),
        &working_directory,
    ) {
        Ok(c) => c,
        Err(spawn_err) => {
            return Err(fail_prepared_execution(
                owner.prepared,
                coordinator.store(),
                SessionFailureCode::LaunchFailed,
                ForegroundPreparationError::ProcessSpawn(spawn_err),
            ));
        }
    };

    // Transfer ownership from PreparedForegroundExecution to RunningForegroundExecution.
    // Do not alter the child's session record from the parent merely because spawn succeeded.
    let running = RunningForegroundExecution {
        child,
        prepared: owner.prepared,
        operation: owner.operation,
        project_name: owner.project_name,
        source_name: owner.source_name,
    };

    // 11. Wait and reconcile; lease is held throughout and released inside.
    running.wait_and_reconcile()
}

// ---------------------------------------------------------------------------
// Centralized pre-spawn failure handler
// ---------------------------------------------------------------------------

/// Transition the prepared session to Failed and return a typed execution error.
///
/// Required order:
/// 1. Retain lease (held in `prepared`).
/// 2. Transition Prepared to Failed.
/// 3. Update root summary (inside `fail_launch`).
/// 4. Release lease (drop of `prepared`).
/// 5. Return typed error.
fn fail_prepared_execution(
    prepared: PreparedSessionLaunch,
    store: &lexicon_core::session::SessionStore,
    failure_code: SessionFailureCode,
    cause: ForegroundPreparationError,
) -> ForegroundDataExecutionError {
    let failure = SafeSessionFailure::new(
        SessionFailureKind::Runtime,
        failure_code,
        None,
    );
    match prepared.fail_launch(store, failure) {
        Ok(_) => {
            // Lease released; convert cause to the appropriate top-level error.
            preparation_error_to_execution_error(cause)
        }
        Err(persistence_err) => {
            ForegroundDataExecutionError::PreparationFailureAndPersistenceFailure {
                preparation: cause,
                persistence: persistence_err,
            }
        }
    }
}

fn preparation_error_to_execution_error(cause: ForegroundPreparationError) -> ForegroundDataExecutionError {
    match cause {
        ForegroundPreparationError::InvocationConstruction(e) => {
            ForegroundDataExecutionError::InvocationConstruction(e)
        }
        ForegroundPreparationError::InvocationEncoding(e) => {
            ForegroundDataExecutionError::InvocationEncoding(e)
        }
        ForegroundPreparationError::ExecutableIntegrityChanged { path, detail } => {
            ForegroundDataExecutionError::ExecutableIntegrityChanged { path, detail }
        }
        ForegroundPreparationError::ExecutableIntegrityCheck(e) => {
            ForegroundDataExecutionError::ExecutableIntegrityCheck(e)
        }
        ForegroundPreparationError::ProcessSpawn(e) => {
            ForegroundDataExecutionError::ProcessSpawn { source: e, persistence_failure: None }
        }
    }
}

// ---------------------------------------------------------------------------
// Invocation envelope construction
// ---------------------------------------------------------------------------

fn build_invocation_envelope(
    project_name: &str,
    admitted: &AdmittedBundle,
    session_id: &SessionIdentity,
    execution_mode: RuntimeExecutionMode,
) -> Result<RuntimeInvocationEnvelopeV1, ForegroundInvocationConstructionError> {
    let project_invocation = ProjectInvocationIdentity::new(project_name)
        .map_err(ForegroundInvocationConstructionError::InvalidProjectIdentity)?;

    let session_invocation = SessionInvocationIdentity::new(session_id.id())
        .map_err(ForegroundInvocationConstructionError::InvalidSessionIdentity)?;

    let runtime_identity = admitted.information_identity();

    RuntimeInvocationEnvelopeV1::new(
        project_invocation,
        runtime_identity,
        session_invocation,
        execution_mode,
        lexicon_core::runtime::RuntimeSupervisionMode::Foreground,
    )
    .map_err(ForegroundInvocationConstructionError::EnvelopeConstruction)
}

// ---------------------------------------------------------------------------
// Integrity check (returns ForegroundPreparationError)
// ---------------------------------------------------------------------------

fn recheck_executable_integrity_typed(
    admitted: &AdmittedBundle,
) -> Result<(), ForegroundPreparationError> {
    match recheck_executable_integrity(admitted) {
        Ok(()) => Ok(()),
        Err(ForegroundDataExecutionError::ExecutableIntegrityChanged { path, detail }) => {
            Err(ForegroundPreparationError::ExecutableIntegrityChanged { path, detail })
        }
        Err(ForegroundDataExecutionError::ExecutableIntegrityCheck(e)) => {
            Err(ForegroundPreparationError::ExecutableIntegrityCheck(e))
        }
        Err(_) => unreachable!("recheck_executable_integrity only returns these two variants"),
    }
}

// ---------------------------------------------------------------------------
// Termination observation
// ---------------------------------------------------------------------------

fn observe_termination(status: std::process::ExitStatus) -> ObservedChildTermination {
    if let Some(code) = status.code() {
        return ObservedChildTermination::ExitCode(code);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        return ObservedChildTermination::Signaled {
            signal: status.signal(),
        };
    }

    #[cfg(not(unix))]
    ObservedChildTermination::UnknownAbnormalTermination
}

// ---------------------------------------------------------------------------
// Termination reconciliation (lease still held via `running`)
// ---------------------------------------------------------------------------

fn reconcile_termination_with_lease(
    termination: ObservedChildTermination,
    running: RunningForegroundExecution,
    prepared_record: &SessionRecordV1,
    operation_root: &std::path::Path,
    operation: DataOperation,
    source_name: &str,
    project_name: &str,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    let execution_mode = prepared_record.execution_mode();
    let session_id = prepared_record.session().clone();

    match termination {
        ObservedChildTermination::ExitCode(0) => {
            reconcile_zero_exit(
                running,
                prepared_record,
                operation_root,
                operation,
                source_name,
                project_name,
                execution_mode,
            )
        }
        ObservedChildTermination::ExitCode(code) => {
            reconcile_nonzero_exit(
                code,
                running,
                prepared_record,
                operation_root,
                operation,
                source_name,
            )
        }
        ObservedChildTermination::Signaled { signal } => {
            reconcile_signal(signal, running, prepared_record, operation_root, operation, source_name)
        }
        ObservedChildTermination::UnknownAbnormalTermination => {
            reconcile_signal(None, running, prepared_record, operation_root, operation, source_name)
        }
    }
}

fn reconcile_zero_exit(
    running: RunningForegroundExecution,
    prepared_record: &SessionRecordV1,
    operation_root: &std::path::Path,
    operation: DataOperation,
    source_name: &str,
    project_name: &str,
    execution_mode: RuntimeExecutionMode,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    let session_id = prepared_record.session().clone();

    // Load and validate the detailed record identity.
    let record = load_and_validate_terminal_session(operation_root, prepared_record)?;

    match record.state() {
        SessionState::Succeeded => {
            // Validate root summary; attempt rebuild if missing/stale.
            let op_root = lexicon_core::session::SessionOperationRoot::new(operation_root.to_path_buf())
                .map_err(ForegroundDataExecutionError::StaleSessionReconciliation)?;
            let store = lexicon_core::session::SessionStore::open(op_root)
                .map_err(ForegroundDataExecutionError::MissingTerminalSession)?;

            match validate_root_summary_against_record(&store, &record) {
                Ok(()) => {}
                Err(detail) => {
                    // Attempt rebuild.
                    match store.rebuild_status_from_record(&session_id) {
                        Ok(_) => {
                            // Reload and re-validate.
                            match validate_root_summary_against_record(&store, &record) {
                                Ok(()) => {}
                                Err(detail2) => {
                                    // Lease released when running is dropped.
                                    drop(running);
                                    return Err(ForegroundDataExecutionError::RootSummaryReconciliationFailed {
                                        detail: detail2,
                                        rebuild_error: None,
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            drop(running);
                            return Err(ForegroundDataExecutionError::RootSummaryReconciliationFailed {
                                detail,
                                rebuild_error: Some(e),
                            });
                        }
                    }
                }
            }

            // Release lease.
            drop(running);

            Ok(ForegroundDataOutcome {
                project: project_name.to_owned(),
                source: source_name.to_owned(),
                operation,
                session: session_id,
                execution_mode,
            })
        }
        SessionState::Failed => {
            let failure = record.failure().cloned();
            // Release lease.
            drop(running);
            Err(ForegroundDataExecutionError::ChildFailed {
                operation: operation.display_name().to_owned(),
                source: source_name.to_owned(),
                session: session_id.id().to_owned(),
                failure_kind: failure
                    .as_ref()
                    .map(|f| f.kind())
                    .unwrap_or(SessionFailureKind::AbnormalTermination),
                failure_code: failure
                    .as_ref()
                    .map(|f| f.code())
                    .unwrap_or(SessionFailureCode::AbnormalTermination),
                exit_code: 0,
            })
        }
        SessionState::Prepared | SessionState::Running => {
            // Abnormal: zero exit without completion; transition to Failed while holding lease.
            let revision = record.revision();
            match persist_abnormal_termination(
                operation_root,
                &session_id,
                revision,
                SessionFailureCode::ZeroExitWithoutCompletion,
                Some("child exited zero without completing the session".to_owned()),
            ) {
                Ok(_) => {
                    drop(running);
                    Err(ForegroundDataExecutionError::ZeroExitSessionIncomplete {
                        session: session_id.id().to_owned(),
                        operation: operation.display_name().to_owned(),
                    })
                }
                Err(e) => {
                    drop(running);
                    Err(ForegroundDataExecutionError::AbnormalTerminationPersistence {
                        termination: ObservedChildTermination::ExitCode(0),
                        persistence_failure: e,
                    })
                }
            }
        }
        SessionState::Abandoned => {
            drop(running);
            Err(ForegroundDataExecutionError::ExitSessionDisagreement {
                termination: ObservedChildTermination::ExitCode(0),
                durable_state: SessionState::Abandoned,
            })
        }
    }
}

fn reconcile_nonzero_exit(
    exit_code: i32,
    running: RunningForegroundExecution,
    prepared_record: &SessionRecordV1,
    operation_root: &std::path::Path,
    operation: DataOperation,
    source_name: &str,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    let session_id = prepared_record.session().clone();
    let record = load_and_validate_terminal_session(operation_root, prepared_record)?;

    match record.state() {
        SessionState::Failed => {
            let failure = record.failure().cloned();
            // Validate or rebuild root summary while lease is held.
            let _ = validate_or_rebuild_summary_if_needed(operation_root, &session_id, &record);
            drop(running);
            Err(ForegroundDataExecutionError::ChildFailed {
                operation: operation.display_name().to_owned(),
                source: source_name.to_owned(),
                session: session_id.id().to_owned(),
                failure_kind: failure
                    .as_ref()
                    .map(|f| f.kind())
                    .unwrap_or(SessionFailureKind::AbnormalTermination),
                failure_code: failure
                    .as_ref()
                    .map(|f| f.code())
                    .unwrap_or(SessionFailureCode::NonzeroExitWithoutFailureRecord),
                exit_code,
            })
        }
        SessionState::Succeeded => {
            drop(running);
            Err(ForegroundDataExecutionError::ExitSessionDisagreement {
                termination: ObservedChildTermination::ExitCode(exit_code),
                durable_state: SessionState::Succeeded,
            })
        }
        SessionState::Prepared | SessionState::Running => {
            let revision = record.revision();
            match persist_abnormal_termination(
                operation_root,
                &session_id,
                revision,
                SessionFailureCode::NonzeroExitWithoutFailureRecord,
                None,
            ) {
                Ok(_) => {
                    drop(running);
                    Err(ForegroundDataExecutionError::AbnormalTermination {
                        operation: operation.display_name().to_owned(),
                        source: source_name.to_owned(),
                        session: session_id.id().to_owned(),
                        signal: None,
                    })
                }
                Err(e) => {
                    drop(running);
                    Err(ForegroundDataExecutionError::AbnormalExitPersistence {
                        exit_code,
                        persistence_failure: e,
                    })
                }
            }
        }
        SessionState::Abandoned => {
            drop(running);
            Err(ForegroundDataExecutionError::ExitSessionDisagreement {
                termination: ObservedChildTermination::ExitCode(exit_code),
                durable_state: SessionState::Abandoned,
            })
        }
    }
}

fn reconcile_signal(
    signal: Option<i32>,
    running: RunningForegroundExecution,
    prepared_record: &SessionRecordV1,
    operation_root: &std::path::Path,
    operation: DataOperation,
    source_name: &str,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    let session_id = prepared_record.session().clone();

    if let Ok(record) = load_terminal_session(operation_root, &session_id) {
        match record.state() {
            SessionState::Succeeded | SessionState::Failed => {
                // Preserve the existing terminal record.
            }
            SessionState::Prepared | SessionState::Running | SessionState::Abandoned => {
                let revision = record.revision();
                let _ = persist_abnormal_termination(
                    operation_root,
                    &session_id,
                    revision,
                    SessionFailureCode::AbnormalTermination,
                    signal.map(|s| format!("terminated by signal {s}")),
                );
            }
        }
    }

    drop(running);

    Err(ForegroundDataExecutionError::AbnormalTermination {
        operation: operation.display_name().to_owned(),
        source: source_name.to_owned(),
        session: session_id.id().to_owned(),
        signal,
    })
}

// ---------------------------------------------------------------------------
// Root summary helpers
// ---------------------------------------------------------------------------

fn validate_or_rebuild_summary_if_needed(
    operation_root: &std::path::Path,
    session_id: &SessionIdentity,
    record: &SessionRecordV1,
) -> Result<(), ()> {
    let op_root = match lexicon_core::session::SessionOperationRoot::new(operation_root.to_path_buf()) {
        Ok(r) => r,
        Err(_) => return Err(()),
    };
    let store = match lexicon_core::session::SessionStore::open(op_root) {
        Ok(s) => s,
        Err(_) => return Err(()),
    };
    if validate_root_summary_against_record(&store, record).is_ok() {
        return Ok(());
    }
    store.rebuild_status_from_record(session_id).map(|_| ()).map_err(|_| ())
}

