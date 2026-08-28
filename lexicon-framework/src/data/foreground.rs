use std::ffi::{OsStr, OsString};
use std::path::Path;

use lexicon_core::runtime::{
    RuntimeExecutionMode, RuntimeInvocationEnvelopeV1, RuntimeSupervisionMode,
    encode_runtime_invocation,
    invocation::{ProjectInvocationIdentity, SessionInvocationIdentity},
};
use lexicon_core::session::{
    RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE, SafeSessionFailure, SessionFailureCode, SessionFailureKind,
    SessionIdentity, SessionRecordV1, SessionState,
};

use crate::data::error::{
    ChildOwnershipUncertainError, ForegroundDataExecutionError, ForegroundInvocationConstructionError,
    ForegroundPreparationError, WaitRecoveryFailure,
};
use crate::data::outcome::{ForegroundDataOutcome, ObservedChildTermination};
use crate::data::project::{RuntimeProjectLayout, resolve_project_layout};
use crate::data::request::{DataOperation, ForegroundDataRequest};
use crate::data::runtime::{AdmittedBundle, admit_bundle, recheck_executable_integrity};
use crate::data::session::{
    build_coordinator, build_project_identity, load_and_validate_terminal_session,
    persist_abnormal_termination, select_and_prepare_session, validate_or_rebuild_root_summary,
    validate_terminal_session_identity,
};
use crate::session::{PreparedSessionLaunch, SessionCoordinator};

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
    /// `pub(crate)` because `background.rs`'s operator-host entrypoint
    /// constructs this owner from a resumed (rather than freshly created)
    /// `PreparedSessionLaunch` before calling the shared `spawn_and_supervise`.
    pub(crate) fn new(
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

enum WaitRecoveryState {
    WaitFailed,
    ChildAlreadyExited,
    TerminationRequested,
    TerminationObserved,
    Reaped,
    OwnershipUncertain,
}

impl RunningForegroundExecution {
    /// Wait for the child to exit and reconcile the session.
    ///
    /// The supervisor lease is held throughout. It is released only at the end
    /// of this method, after reconciliation is complete.
    pub(crate) fn wait_and_reconcile(
        mut self,
    ) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
        let termination = loop {
            match self.child.wait() {
                Ok(status) => break observe_termination(status),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(wait_err) => return self.handle_wait_error(wait_err),
            }
        };

        reconcile_terminal_execution(self, termination)
    }

    /// Recovery path when `child.wait()` fails with a non-Interrupted error.
    ///
    /// Attempts to determine child state, kill if running, reap, reconcile session.
    fn handle_wait_error(
        mut self,
        wait_err: std::io::Error,
    ) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
        let mut _state = WaitRecoveryState::WaitFailed;
        let mut try_wait_error: Option<std::io::Error> = None;
        let mut kill_error: Option<std::io::Error> = None;
        let mut reap_error: Option<std::io::Error> = None;

        let termination = match self.child.try_wait() {
            Ok(Some(status)) => {
                _state = WaitRecoveryState::ChildAlreadyExited;
                Some(observe_termination(status))
            }
            Ok(None) => {
                let mut observed_after_kill_failure: Option<ObservedChildTermination> = None;
                match self.child.kill() {
                    Ok(()) => {
                        _state = WaitRecoveryState::TerminationRequested;
                    }
                    Err(err) => {
                        kill_error = Some(err);
                        match self.child.try_wait() {
                            Ok(Some(status)) => {
                                _state = WaitRecoveryState::TerminationObserved;
                                observed_after_kill_failure = Some(observe_termination(status));
                            }
                            Ok(None) => {
                                _state = WaitRecoveryState::OwnershipUncertain;
                                return Err(self.ownership_uncertain(wait_err, try_wait_error, kill_error, reap_error));
                            }
                            Err(err) => {
                                try_wait_error = Some(err);
                                _state = WaitRecoveryState::OwnershipUncertain;
                                return Err(self.ownership_uncertain(wait_err, try_wait_error, kill_error, reap_error));
                            }
                        }
                    }
                }

                if let Some(termination) = observed_after_kill_failure {
                    _state = WaitRecoveryState::Reaped;
                    Some(termination)
                } else {
                    loop {
                        match self.child.wait() {
                            Ok(status) => {
                                _state = WaitRecoveryState::Reaped;
                                break Some(observe_termination(status));
                            }
                            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                            Err(err) => {
                                reap_error = Some(err);
                                match self.child.try_wait() {
                                    Ok(Some(status)) => {
                                        _state = WaitRecoveryState::Reaped;
                                        break Some(observe_termination(status));
                                    }
                                    Ok(None) => {
                                        _state = WaitRecoveryState::OwnershipUncertain;
                                        return Err(self.ownership_uncertain(wait_err, try_wait_error, kill_error, reap_error));
                                    }
                                    Err(err) => {
                                        try_wait_error = Some(err);
                                        _state = WaitRecoveryState::OwnershipUncertain;
                                        return Err(self.ownership_uncertain(wait_err, try_wait_error, kill_error, reap_error));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(err) => {
                try_wait_error = Some(err);
                _state = WaitRecoveryState::OwnershipUncertain;
                return Err(self.ownership_uncertain(wait_err, try_wait_error, kill_error, reap_error));
            }
        };

        let Some(termination) = termination else {
            _state = WaitRecoveryState::OwnershipUncertain;
            return Err(self.ownership_uncertain(wait_err, try_wait_error, kill_error, reap_error));
        };

        let final_state = match reconcile_wait_recovery_session_state(&mut self, &termination) {
            Ok(state) => Some(state),
            Err(session_error) => {
                let (session_load_error, session_reconciliation_error) = match session_error {
                    ForegroundDataExecutionError::MissingTerminalSession(_)
                    | ForegroundDataExecutionError::CorruptTerminalSession(_)
                    | ForegroundDataExecutionError::SessionIdentityDisagreement(_) => {
                        (Some(Box::new(session_error)), None)
                    }
                    _ => (None, Some(Box::new(session_error))),
                };
                return Err(ForegroundDataExecutionError::ProcessWaitRecovery(Box::new(
                    WaitRecoveryFailure {
                        wait_error: wait_err,
                        kill_error,
                        try_wait_error,
                        reap_error,
                        session_load_error,
                        session_reconciliation_error,
                        final_state: None,
                    },
                )));
            }
        };

        Err(ForegroundDataExecutionError::ProcessWaitRecovery(Box::new(
            WaitRecoveryFailure {
                wait_error: wait_err,
                kill_error,
                try_wait_error,
                reap_error,
                session_load_error: None,
                session_reconciliation_error: None,
                final_state,
            },
        )))
    }

    fn ownership_uncertain(
        mut self,
        wait_error: std::io::Error,
        try_wait_error: Option<std::io::Error>,
        kill_error: Option<std::io::Error>,
        reap_error: Option<std::io::Error>,
    ) -> ForegroundDataExecutionError {
        let (session_load_error, session_reconciliation_error) =
            best_effort_reconcile_when_ownership_uncertain(&mut self);

        ForegroundDataExecutionError::ChildOwnershipUncertain(Box::new(
            ChildOwnershipUncertainError {
                wait_error,
                try_wait_error,
                kill_error,
                reap_error,
                session_load_error,
                session_reconciliation_error,
            },
        ))
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

    // 5. Session selection policy (reconcile stale → select → prepare).
    let prepared = select_and_prepare_session(
        &coordinator,
        request.operation,
        request.abandon_past_failure,
        &admitted,
        RuntimeSupervisionMode::Foreground,
    )?;

    // From here onward, every error must transition the session to Failed before returning.
    let owner = PreparedForegroundExecution::new(
        prepared,
        request.operation,
        project_name,
        request.source_name.clone(),
    );

    // 6-11. Build the invocation envelope, spawn the runtime, and supervise it to
    // completion. Shared with the operator-host caller (see `execute_operator_host_with_launcher`
    // in `background.rs`), parametrized only by the supervision mode.
    spawn_and_supervise(
        owner,
        &layout,
        &admitted,
        &coordinator,
        &request.source_arguments,
        RuntimeSupervisionMode::Foreground,
        launcher,
    )
}

// ---------------------------------------------------------------------------
// Shared spawn-and-supervise pipeline
// ---------------------------------------------------------------------------

/// Build the invocation envelope, spawn the admitted runtime, and supervise it
/// through to terminal reconciliation.
///
/// This is the shared core of both foreground execution and operator-host
/// execution. The only difference between the two callers is the supervision
/// mode recorded in the invocation envelope; the spawn, integrity-recheck, and
/// termination-reconciliation logic is identical and must not be duplicated.
pub(crate) fn spawn_and_supervise(
    owner: PreparedForegroundExecution,
    layout: &RuntimeProjectLayout,
    admitted: &AdmittedBundle,
    coordinator: &SessionCoordinator,
    source_arguments: &[OsString],
    supervision: RuntimeSupervisionMode,
    launcher: &dyn ForegroundRuntimeLauncher,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    // 6. Build invocation envelope.
    let session_id = owner.session().clone();
    let execution_mode = owner.record().execution_mode();
    let envelope = match build_invocation_envelope(
        &owner.project_name,
        admitted,
        &session_id,
        execution_mode,
        supervision,
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
    let encoded = match encode_runtime_invocation(&envelope, source_arguments) {
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
    if let Err(prep_err) = recheck_executable_integrity_typed(admitted) {
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
        ForegroundPreparationError::ExecutableIntegrity(e) => {
            ForegroundDataExecutionError::ExecutableIntegrity(e)
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
    supervision: RuntimeSupervisionMode,
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
        supervision,
    )
    .map_err(ForegroundInvocationConstructionError::EnvelopeConstruction)
}

// ---------------------------------------------------------------------------
// Integrity check (returns ForegroundPreparationError)
// ---------------------------------------------------------------------------

fn recheck_executable_integrity_typed(
    admitted: &AdmittedBundle,
) -> Result<(), ForegroundPreparationError> {
    recheck_executable_integrity(admitted).map_err(ForegroundPreparationError::ExecutableIntegrity)
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

pub fn reconcile_terminal_execution(
    running: RunningForegroundExecution,
    termination: ObservedChildTermination,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    let prepared_record = running.prepared.record().clone();
    let operation_root = running.prepared.operation_root().to_path_buf();
    let operation = running.operation;
    let source_name = running.source_name.clone();
    let project_name = running.project_name.clone();
    let execution_mode = prepared_record.execution_mode();

    let op_root = lexicon_core::session::SessionOperationRoot::new(operation_root.clone())
        .map_err(ForegroundDataExecutionError::StaleSessionReconciliation)?;
    let store = lexicon_core::session::SessionStore::open(op_root)
        .map_err(ForegroundDataExecutionError::MissingTerminalSession)?;

    let record = load_and_validate_terminal_session(&operation_root, &prepared_record)?;
    match termination {
        ObservedChildTermination::ExitCode(0) => reconcile_zero_exit(
            record,
            &prepared_record,
            &store,
            &operation_root,
            operation,
            &source_name,
            &project_name,
            execution_mode,
        ),
        ObservedChildTermination::ExitCode(exit_code) => reconcile_nonzero_exit(
            record,
            &prepared_record,
            &store,
            &operation_root,
            operation,
            &source_name,
            exit_code,
        ),
        ObservedChildTermination::Signaled { signal } => reconcile_abnormal_termination(
            record,
            &prepared_record,
            &store,
            &operation_root,
            operation,
            &source_name,
            ObservedChildTermination::Signaled { signal },
        ),
        ObservedChildTermination::UnknownAbnormalTermination => reconcile_abnormal_termination(
            record,
            &prepared_record,
            &store,
            &operation_root,
            operation,
            &source_name,
            ObservedChildTermination::UnknownAbnormalTermination,
        ),
    }
}

fn reconcile_zero_exit(
    record: SessionRecordV1,
    prepared_record: &SessionRecordV1,
    store: &lexicon_core::session::SessionStore,
    operation_root: &std::path::Path,
    operation: DataOperation,
    source_name: &str,
    project_name: &str,
    execution_mode: RuntimeExecutionMode,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    let session_id = prepared_record.session().clone();
    match record.state() {
        SessionState::Succeeded => {
            validate_or_rebuild_root_summary(store, &record)
                .map_err(ForegroundDataExecutionError::RootSummaryReconciliationFailed)?;
            Ok(ForegroundDataOutcome {
                project: project_name.to_owned(),
                source: source_name.to_owned(),
                operation,
                session: session_id,
                execution_mode,
            })
        }
        SessionState::Failed => {
            validate_or_rebuild_root_summary(store, &record)
                .map_err(ForegroundDataExecutionError::RootSummaryReconciliationFailed)?;
            let failure = record.failure().cloned();
            Err(ForegroundDataExecutionError::ChildFailed {
                operation,
                source: source_name.to_owned(),
                session: session_id,
                failure_kind: failure
                    .as_ref()
                    .map(|f| f.kind())
                    .unwrap_or(SessionFailureKind::AbnormalTermination),
                failure_code: failure
                    .as_ref()
                    .map(|f| f.code())
                    .unwrap_or(SessionFailureCode::AbnormalTermination),
                exit_code: Some(0),
            })
        }
        SessionState::Prepared | SessionState::Running => {
            let transitioned = persist_abnormal_termination(
                operation_root,
                &session_id,
                record.revision(),
                SessionFailureCode::ZeroExitWithoutCompletion,
                Some("child exited zero without completing the session".to_owned()),
            )
            .map_err(|persistence_failure| ForegroundDataExecutionError::AbnormalTerminationPersistence {
                termination: ObservedChildTermination::ExitCode(0),
                persistence_failure,
            })?;
            validate_terminal_session_identity(prepared_record, &transitioned)?;
            validate_or_rebuild_root_summary(store, &transitioned)
                .map_err(ForegroundDataExecutionError::RootSummaryReconciliationFailed)?;
            Err(ForegroundDataExecutionError::ZeroExitSessionIncomplete {
                session: session_id,
                operation,
            })
        }
        SessionState::Abandoned => Err(ForegroundDataExecutionError::ExitSessionDisagreement {
            termination: ObservedChildTermination::ExitCode(0),
            durable_state: SessionState::Abandoned,
        }),
    }
}

fn reconcile_nonzero_exit(
    record: SessionRecordV1,
    prepared_record: &SessionRecordV1,
    store: &lexicon_core::session::SessionStore,
    operation_root: &std::path::Path,
    operation: DataOperation,
    source_name: &str,
    exit_code: i32,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    let session_id = prepared_record.session().clone();
    match record.state() {
        SessionState::Failed => {
            validate_or_rebuild_root_summary(store, &record)
                .map_err(ForegroundDataExecutionError::RootSummaryReconciliationFailed)?;
            let failure = record.failure().cloned();
            Err(ForegroundDataExecutionError::ChildFailed {
                operation,
                source: source_name.to_owned(),
                session: session_id,
                failure_kind: failure
                    .as_ref()
                    .map(|f| f.kind())
                    .unwrap_or(SessionFailureKind::AbnormalTermination),
                failure_code: failure
                    .as_ref()
                    .map(|f| f.code())
                    .unwrap_or(SessionFailureCode::NonzeroExitWithoutFailureRecord),
                exit_code: Some(exit_code),
            })
        }
        SessionState::Prepared | SessionState::Running => {
            let transitioned = persist_abnormal_termination(
                operation_root,
                &session_id,
                record.revision(),
                SessionFailureCode::NonzeroExitWithoutFailureRecord,
                None,
            )
            .map_err(|persistence_failure| ForegroundDataExecutionError::AbnormalExitPersistence {
                exit_code,
                persistence_failure,
            })?;
            validate_terminal_session_identity(prepared_record, &transitioned)?;
            validate_or_rebuild_root_summary(store, &transitioned)
                .map_err(ForegroundDataExecutionError::RootSummaryReconciliationFailed)?;
            Err(ForegroundDataExecutionError::AbnormalTermination {
                operation,
                source: source_name.to_owned(),
                session: session_id,
                signal: None,
            })
        }
        SessionState::Succeeded => Err(ForegroundDataExecutionError::ExitSessionDisagreement {
            termination: ObservedChildTermination::ExitCode(exit_code),
            durable_state: SessionState::Succeeded,
        }),
        SessionState::Abandoned => Err(ForegroundDataExecutionError::ExitSessionDisagreement {
            termination: ObservedChildTermination::ExitCode(exit_code),
            durable_state: SessionState::Abandoned,
        }),
    }
}

fn reconcile_abnormal_termination(
    record: SessionRecordV1,
    prepared_record: &SessionRecordV1,
    store: &lexicon_core::session::SessionStore,
    operation_root: &std::path::Path,
    operation: DataOperation,
    source_name: &str,
    termination: ObservedChildTermination,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    let session_id = prepared_record.session().clone();
    let signal = match termination {
        ObservedChildTermination::Signaled { signal } => signal,
        _ => None,
    };

    match record.state() {
        SessionState::Prepared | SessionState::Running => {
            let transitioned = persist_abnormal_termination(
                operation_root,
                &session_id,
                record.revision(),
                SessionFailureCode::AbnormalTermination,
                signal.map(|s| format!("terminated by signal {s}")),
            )
            .map_err(|persistence_failure| ForegroundDataExecutionError::AbnormalTerminationPersistence {
                termination: termination.clone(),
                persistence_failure,
            })?;
            validate_terminal_session_identity(prepared_record, &transitioned)?;
            validate_or_rebuild_root_summary(store, &transitioned)
                .map_err(ForegroundDataExecutionError::RootSummaryReconciliationFailed)?;
            Err(ForegroundDataExecutionError::AbnormalTermination {
                operation,
                source: source_name.to_owned(),
                session: session_id,
                signal,
            })
        }
        SessionState::Failed => {
            validate_or_rebuild_root_summary(store, &record)
                .map_err(ForegroundDataExecutionError::RootSummaryReconciliationFailed)?;
            let failure = record.failure().cloned();
            Err(ForegroundDataExecutionError::ChildFailed {
                operation,
                source: source_name.to_owned(),
                session: session_id,
                failure_kind: failure
                    .as_ref()
                    .map(|f| f.kind())
                    .unwrap_or(SessionFailureKind::AbnormalTermination),
                failure_code: failure
                    .as_ref()
                    .map(|f| f.code())
                    .unwrap_or(SessionFailureCode::AbnormalTermination),
                exit_code: None,
            })
        }
        SessionState::Succeeded => {
            validate_or_rebuild_root_summary(store, &record)
                .map_err(ForegroundDataExecutionError::RootSummaryReconciliationFailed)?;
            Err(ForegroundDataExecutionError::ExitSessionDisagreement {
                termination,
                durable_state: SessionState::Succeeded,
            })
        }
        SessionState::Abandoned => Err(ForegroundDataExecutionError::ExitSessionDisagreement {
            termination,
            durable_state: SessionState::Abandoned,
        }),
    }
}

fn reconcile_wait_recovery_session_state(
    running: &mut RunningForegroundExecution,
    termination: &ObservedChildTermination,
) -> Result<SessionState, ForegroundDataExecutionError> {
    let prepared_record = running.prepared.record().clone();
    let operation_root = running.prepared.operation_root().to_path_buf();
    let op_root = lexicon_core::session::SessionOperationRoot::new(operation_root.clone())
        .map_err(ForegroundDataExecutionError::StaleSessionReconciliation)?;
    let store = lexicon_core::session::SessionStore::open(op_root)
        .map_err(ForegroundDataExecutionError::MissingTerminalSession)?;
    let record = load_and_validate_terminal_session(&operation_root, &prepared_record)?;

    let final_record = match record.state() {
        SessionState::Prepared | SessionState::Running => persist_abnormal_termination(
            &operation_root,
            prepared_record.session(),
            record.revision(),
            SessionFailureCode::AbnormalTermination,
            Some("wait recovery terminated the runtime process".to_owned()),
        )
        .map_err(|persistence_failure| ForegroundDataExecutionError::AbnormalTerminationPersistence {
            termination: termination.clone(),
            persistence_failure,
        })?,
        _ => record,
    };
    validate_terminal_session_identity(&prepared_record, &final_record)?;
    validate_or_rebuild_root_summary(&store, &final_record)
        .map_err(ForegroundDataExecutionError::RootSummaryReconciliationFailed)?;
    Ok(final_record.state())
}

fn best_effort_reconcile_when_ownership_uncertain(
    running: &mut RunningForegroundExecution,
) -> (
    Option<Box<ForegroundDataExecutionError>>,
    Option<Box<ForegroundDataExecutionError>>,
) {
    let prepared_record = running.prepared.record().clone();
    let operation_root = running.prepared.operation_root().to_path_buf();
    let op_root = match lexicon_core::session::SessionOperationRoot::new(operation_root.clone()) {
        Ok(root) => root,
        Err(err) => {
            return (
                Some(Box::new(ForegroundDataExecutionError::StaleSessionReconciliation(err))),
                None,
            );
        }
    };
    let store = match lexicon_core::session::SessionStore::open(op_root) {
        Ok(store) => store,
        Err(err) => {
            return (
                Some(Box::new(ForegroundDataExecutionError::MissingTerminalSession(err))),
                None,
            );
        }
    };
    match load_and_validate_terminal_session(&operation_root, &prepared_record) {
        Ok(record) => {
            if record.state().is_terminal() {
                if let Err(err) = validate_or_rebuild_root_summary(&store, &record)
                    .map_err(ForegroundDataExecutionError::RootSummaryReconciliationFailed)
                {
                    return (None, Some(Box::new(err)));
                }
            }
            (None, None)
        }
        Err(err) => (Some(Box::new(err)), None),
    }
}
