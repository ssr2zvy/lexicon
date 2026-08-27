use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use lexicon_core::runtime::{
    RuntimeExecutionMode, RuntimeInvocationEnvelopeV1,
    encode_runtime_invocation,
    invocation::{ProjectInvocationIdentity, SessionInvocationIdentity},
};
use lexicon_core::session::{
    RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE, SafeSessionFailure, SessionFailureCode, SessionFailureKind,
    SessionIdentity, SessionState,
};

use crate::data::error::ForegroundDataExecutionError;
use crate::data::outcome::{ForegroundDataOutcome, ObservedChildTermination};
use crate::data::project::resolve_project_layout;
use crate::data::request::{DataOperation, ForegroundDataRequest};
use crate::data::runtime::{AdmittedBundle, admit_bundle, recheck_executable_integrity};
use crate::data::session::{
    build_coordinator, build_project_identity, load_terminal_session,
    persist_abnormal_termination, select_and_prepare_session,
};
use crate::session::PreparedSessionLaunch;

/// Execute a foreground data acquisition or processing run.
///
/// This is the single public entrypoint for the data command.
pub fn execute_foreground_data(
    request: ForegroundDataRequest,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    // Reject --bg immediately without any session side effects.
    if request.background {
        return Err(ForegroundDataExecutionError::BackgroundModeUnsupported);
    }

    // 1. Project discovery and layout validation.
    let (layout, project_name) = resolve_project_layout(&request.source_name, request.operation)?;

    // 2. Bundle admission.
    let admitted = admit_bundle(&layout, request.operation)?;

    // 3. Build project identity.
    let project_identity = build_project_identity(&project_name)?;

    // 4. Build session coordinator from the layout and the admitted bundle's identity.
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

    // 6. Build invocation envelope.
    let session_id = prepared.session().clone();
    let execution_mode = prepared.record().execution_mode();
    let envelope = build_invocation_envelope(
        &project_name,
        &admitted,
        &session_id,
        execution_mode,
    )?;

    // 7. Encode argv.
    let encoded = encode_runtime_invocation(&envelope, &request.source_arguments)
        .map_err(|e| ForegroundDataExecutionError::InvocationEncoding(e.to_string()))?;

    // 8. Pre-launch executable integrity recheck.
    if let Err(err) = recheck_executable_integrity(&admitted) {
        // Mark session as failed before returning.
        let _ = prepared.fail_launch(
            coordinator.store(),
            SafeSessionFailure::new(
                SessionFailureKind::Runtime,
                SessionFailureCode::LaunchFailed,
                Some("runtime executable integrity check failed before launch".to_owned()),
            ),
        );
        return Err(err);
    }

    // 9. Build argv: argv[0] is the executable, followed by the encoded invocation args.
    let executable = admitted.executable_path().to_path_buf();
    let mut argv: Vec<OsString> = Vec::new();
    argv.extend(encoded.arguments().iter().cloned());

    let context_document = prepared.context_document().to_owned();
    let operation_root = prepared.operation_root().to_path_buf();

    // 10. Launch and wait.
    let termination = launch_and_wait(
        &executable,
        &argv,
        &context_document,
        &layout.protocol_root().to_path_buf(),
        prepared,
        coordinator.store(),
        request.operation,
        &session_id,
    )?;

    // 11. Reconcile termination.
    reconcile_termination(
        termination,
        &operation_root,
        &session_id,
        request.operation,
        &request.source_name,
        &project_name,
        execution_mode,
    )
}

// ---------------------------------------------------------------------------
// Invocation envelope construction
// ---------------------------------------------------------------------------

fn build_invocation_envelope(
    project_name: &str,
    admitted: &AdmittedBundle,
    session_id: &SessionIdentity,
    execution_mode: RuntimeExecutionMode,
) -> Result<RuntimeInvocationEnvelopeV1, ForegroundDataExecutionError> {
    let project_invocation = ProjectInvocationIdentity::new(project_name).map_err(|e| {
        ForegroundDataExecutionError::InvocationConstruction(format!(
            "invalid project identity for invocation: {e}"
        ))
    })?;

    let session_invocation = SessionInvocationIdentity::new(session_id.id()).map_err(|e| {
        ForegroundDataExecutionError::InvocationConstruction(format!(
            "invalid session identity for invocation: {e}"
        ))
    })?;

    // Use the exact compiled RuntimeIdentity from the admitted bundle manifest.
    // The core library parses this with Box::leak for the source name in from_json.
    let runtime_identity = admitted.information_identity();

    RuntimeInvocationEnvelopeV1::new(
        project_invocation,
        runtime_identity,
        session_invocation,
        execution_mode,
        lexicon_core::runtime::RuntimeSupervisionMode::Foreground,
    )
    .map_err(|e| ForegroundDataExecutionError::InvocationConstruction(e.to_string()))
}

// ---------------------------------------------------------------------------
// Process launch
// ---------------------------------------------------------------------------

/// Launch the runtime process and wait for it to complete.
///
/// The supervisor lease is held throughout. On spawn failure the session is
/// transitioned to Failed before returning.
fn launch_and_wait(
    executable: &Path,
    argv: &[OsString],
    context_document: &str,
    working_directory: &std::path::Path,
    prepared: PreparedSessionLaunch,
    store: &lexicon_core::session::SessionStore,
    _operation: DataOperation,
    _session_id: &SessionIdentity,
) -> Result<ObservedChildTermination, ForegroundDataExecutionError> {
    let mut cmd = Command::new(executable);
    cmd.args(argv);
    cmd.env(RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE, context_document);
    cmd.env_remove("LEXICON_SOURCE_DIRECTORY");
    cmd.current_dir(working_directory);

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(spawn_err) => {
            // Persist the launch failure.
            let persistence_failure = prepared.fail_launch(
                store,
                SafeSessionFailure::new(
                    SessionFailureKind::Runtime,
                    SessionFailureCode::LaunchFailed,
                    Some("process launch failed".to_owned()),
                ),
            )
            .err();

            return Err(ForegroundDataExecutionError::ProcessSpawn {
                source: spawn_err,
                persistence_failure,
            });
        }
    };

    // The prepared launch value (and its lease) is retained while we wait.
    // Hold it so it is not dropped until after we get the exit status.
    let _lease_holder = prepared;

    let status = child
        .wait()
        .map_err(ForegroundDataExecutionError::ProcessWait)?;

    // Drop the lease after the child has completed.
    drop(_lease_holder);

    Ok(observe_termination(status))
}

/// Map a `std::process::ExitStatus` to `ObservedChildTermination`.
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
// Termination reconciliation
// ---------------------------------------------------------------------------

fn reconcile_termination(
    termination: ObservedChildTermination,
    operation_root: &std::path::Path,
    session_id: &SessionIdentity,
    operation: DataOperation,
    source_name: &str,
    project_name: &str,
    execution_mode: RuntimeExecutionMode,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    match termination {
        ObservedChildTermination::ExitCode(0) => {
            reconcile_zero_exit(
                operation_root,
                session_id,
                operation,
                source_name,
                project_name,
                execution_mode,
            )
        }
        ObservedChildTermination::ExitCode(code) => {
            reconcile_nonzero_exit(
                code,
                operation_root,
                session_id,
                operation,
                source_name,
            )
        }
        ObservedChildTermination::Signaled { signal } => {
            reconcile_signal(
                signal,
                operation_root,
                session_id,
                operation,
                source_name,
            )
        }
        ObservedChildTermination::UnknownAbnormalTermination => {
            reconcile_signal(
                None,
                operation_root,
                session_id,
                operation,
                source_name,
            )
        }
    }
}

fn reconcile_zero_exit(
    operation_root: &std::path::Path,
    session_id: &SessionIdentity,
    operation: DataOperation,
    source_name: &str,
    project_name: &str,
    execution_mode: RuntimeExecutionMode,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    let record = load_terminal_session(operation_root, session_id)?;

    match record.state() {
        SessionState::Succeeded => {
            // Verify root status agrees.
            Ok(ForegroundDataOutcome {
                project: project_name.to_owned(),
                source: source_name.to_owned(),
                operation,
                session: session_id.clone(),
                execution_mode,
            })
        }
        SessionState::Failed => {
            // Session is failed even though child exited zero.
            let failure = record.failure().cloned();
            Err(ForegroundDataExecutionError::ChildFailed {
                operation: operation.display_name().to_owned(),
                source: source_name.to_owned(),
                session: session_id.id().to_owned(),
                failure_kind: failure
                    .as_ref()
                    .map(|f| format!("{:?}", f.kind()))
                    .unwrap_or_else(|| "unknown".to_owned()),
                failure_code: failure
                    .as_ref()
                    .map(|f| format!("{:?}", f.code()))
                    .unwrap_or_else(|| "unknown".to_owned()),
                exit_code: 0,
            })
        }
        SessionState::Prepared | SessionState::Running => {
            // Abnormal: child exited zero but session didn't reach a terminal state.
            let revision = record.revision();
            match persist_abnormal_termination(
                operation_root,
                session_id,
                revision,
                SessionFailureCode::AbnormalTermination,
                Some("child exited zero without completing the session".to_owned()),
            ) {
                Ok(_) => {}
                Err(e) => {
                    return Err(ForegroundDataExecutionError::AbnormalTerminationPersistence {
                        termination: ObservedChildTermination::ExitCode(0),
                        persistence_failure: e,
                    });
                }
            }
            Err(ForegroundDataExecutionError::ZeroExitSessionIncomplete {
                session: session_id.id().to_owned(),
                operation: operation.display_name().to_owned(),
            })
        }
        SessionState::Abandoned => {
            Err(ForegroundDataExecutionError::ExitSessionDisagreement {
                detail: format!(
                    "session {} is abandoned but child exited zero",
                    session_id.id()
                ),
            })
        }
    }
}

fn reconcile_nonzero_exit(
    exit_code: i32,
    operation_root: &std::path::Path,
    session_id: &SessionIdentity,
    operation: DataOperation,
    source_name: &str,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    let record = load_terminal_session(operation_root, session_id)?;

    match record.state() {
        SessionState::Failed => {
            let failure = record.failure().cloned();
            Err(ForegroundDataExecutionError::ChildFailed {
                operation: operation.display_name().to_owned(),
                source: source_name.to_owned(),
                session: session_id.id().to_owned(),
                failure_kind: failure
                    .as_ref()
                    .map(|f| format!("{:?}", f.kind()))
                    .unwrap_or_else(|| "unknown".to_owned()),
                failure_code: failure
                    .as_ref()
                    .map(|f| format!("{:?}", f.code()))
                    .unwrap_or_else(|| "unknown".to_owned()),
                exit_code,
            })
        }
        SessionState::Succeeded => {
            Err(ForegroundDataExecutionError::ExitSessionDisagreement {
                detail: format!(
                    "session {} is succeeded but child exited {exit_code}",
                    session_id.id()
                ),
            })
        }
        SessionState::Prepared | SessionState::Running => {
            let revision = record.revision();
            match persist_abnormal_termination(
                operation_root,
                session_id,
                revision,
                SessionFailureCode::AbnormalTermination,
                Some(format!("child exited {exit_code} without completing the session")),
            ) {
                Ok(_) => {}
                Err(e) => {
                    return Err(ForegroundDataExecutionError::AbnormalExitPersistence {
                        exit_code,
                        persistence_failure: e,
                    });
                }
            }
            Err(ForegroundDataExecutionError::AbnormalTermination {
                operation: operation.display_name().to_owned(),
                source: source_name.to_owned(),
                session: session_id.id().to_owned(),
                signal: None,
            })
        }
        SessionState::Abandoned => {
            Err(ForegroundDataExecutionError::ExitSessionDisagreement {
                detail: format!(
                    "session {} is abandoned but child exited {exit_code}",
                    session_id.id()
                ),
            })
        }
    }
}

fn reconcile_signal(
    signal: Option<i32>,
    operation_root: &std::path::Path,
    session_id: &SessionIdentity,
    operation: DataOperation,
    source_name: &str,
) -> Result<ForegroundDataOutcome, ForegroundDataExecutionError> {
    let record = load_terminal_session(operation_root, session_id)
        .ok();

    if let Some(record) = record {
        match record.state() {
            // Preserve an already-committed terminal record.
            SessionState::Succeeded | SessionState::Failed => {}
            SessionState::Prepared | SessionState::Running | SessionState::Abandoned => {
                let revision = record.revision();
                let _ = persist_abnormal_termination(
                    operation_root,
                    session_id,
                    revision,
                    SessionFailureCode::AbnormalTermination,
                    signal.map(|s| format!("terminated by signal {s}")),
                );
            }
        }
    } else {
        // Session record not found; try to persist anyway.
        // We don't have a revision, so we can't call transition.
        // Just return the abnormal termination error.
    }

    Err(ForegroundDataExecutionError::AbnormalTermination {
        operation: operation.display_name().to_owned(),
        source: source_name.to_owned(),
        session: session_id.id().to_owned(),
        signal,
    })
}
