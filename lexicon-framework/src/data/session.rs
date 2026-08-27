use lexicon_core::runtime::{OwnedRuntimeIdentity, RuntimeSupervisionMode};
use lexicon_core::session::{
    ProjectIdentity, SafeSessionFailure, SessionFailureCode, SessionFailureKind,
    SessionIdentity, SessionOperation, SessionOperationRoot, SessionRecordV1, SessionState,
    SessionStore, SessionStoreError, SessionTransition,
};

use crate::data::error::ForegroundDataExecutionError;
use crate::data::project::RuntimeProjectLayout;
use crate::data::request::DataOperation;
use crate::data::runtime::AdmittedBundle;
use crate::session::{PreparedSessionLaunch, SessionCoordinator, SessionCoordinationError};

// ---------------------------------------------------------------------------
// Session operations
// ---------------------------------------------------------------------------

/// Build a `SessionOperation` from a `DataOperation`.
pub fn data_operation_to_session_operation(op: DataOperation) -> SessionOperation {
    match op {
        DataOperation::Acquisition => SessionOperation::Acquisition,
        DataOperation::Processing => SessionOperation::Processing,
    }
}

/// Build the `SessionOperationRoot` for the given operation layout.
pub fn build_operation_root(
    layout: &RuntimeProjectLayout,
    operation: DataOperation,
) -> Result<SessionOperationRoot, ForegroundDataExecutionError> {
    let op_root_path = layout.operation_root(operation);
    SessionOperationRoot::new(op_root_path).map_err(ForegroundDataExecutionError::StaleSessionReconciliation)
}

/// Construct a `ProjectIdentity` from the project name.
pub fn build_project_identity(
    project_name: &str,
) -> Result<ProjectIdentity, ForegroundDataExecutionError> {
    ProjectIdentity::new(project_name).map_err(|e| {
        ForegroundDataExecutionError::ProjectConfiguration(
            crate::data::error::ProjectConfigurationError::Identity(format!(
                "invalid project identity '{}': {e}",
                project_name
            ))
        )
    })
}

// ---------------------------------------------------------------------------
// SessionCoordinator construction
// ---------------------------------------------------------------------------

/// Build a `SessionCoordinator` from validated layout, project, and runtime identity.
pub fn build_coordinator(
    layout: &RuntimeProjectLayout,
    project_identity: ProjectIdentity,
    runtime_identity: OwnedRuntimeIdentity,
    operation: DataOperation,
) -> Result<SessionCoordinator, ForegroundDataExecutionError> {
    let session_operation = data_operation_to_session_operation(operation);
    let op_root = build_operation_root(layout, operation)?;

    SessionCoordinator::new(
        project_identity,
        runtime_identity,
        session_operation,
        op_root,
        layout.project_root().to_path_buf(),
        layout.protocol_root().to_path_buf(),
    )
    .map_err(ForegroundDataExecutionError::SessionPreparation)
}

// ---------------------------------------------------------------------------
// Session selection policy
// ---------------------------------------------------------------------------

/// Apply the session selection policy and return a `PreparedSessionLaunch`.
///
/// For acquisition: supports run, resume (if available), and abandon-then-run.
/// For processing: supports run and abandon-then-run; resume is not supported.
pub fn select_and_prepare_session(
    coordinator: &SessionCoordinator,
    operation: DataOperation,
    abandon_past_failure: bool,
    admitted_bundle: &AdmittedBundle,
) -> Result<PreparedSessionLaunch, ForegroundDataExecutionError> {
    // Reconcile stale ownership first.
    coordinator
        .reconcile_stale_current_session()
        .map_err(|e| ForegroundDataExecutionError::StaleSessionReconciliation(
            session_coordination_error_to_store_error(e)
        ))?;

    match operation {
        DataOperation::Acquisition => {
            select_and_prepare_acquisition(coordinator, abandon_past_failure, admitted_bundle)
        }
        DataOperation::Processing => {
            select_and_prepare_processing(coordinator, abandon_past_failure)
        }
    }
}

fn select_and_prepare_acquisition(
    coordinator: &SessionCoordinator,
    abandon_past_failure: bool,
    admitted_bundle: &AdmittedBundle,
) -> Result<PreparedSessionLaunch, ForegroundDataExecutionError> {
    // Load current status after stale reconciliation.
    let current_status = coordinator_current_status(coordinator)?;

    match current_status {
        CurrentStatus::None | CurrentStatus::Succeeded | CurrentStatus::Abandoned => {
            coordinator
                .prepare_run(RuntimeSupervisionMode::Foreground)
                .map_err(ForegroundDataExecutionError::SessionPreparation)
        }
        CurrentStatus::Live => {
            Err(ForegroundDataExecutionError::SessionSelection(
                SessionCoordinationError::LiveSessionAlreadyActive,
            ))
        }
        CurrentStatus::Failed => {
            if abandon_past_failure {
                // Abandon the failed session, then prepare a new run.
                coordinator
                    .abandon_current_failure()
                    .map_err(ForegroundDataExecutionError::Abandonment)?;

                coordinator
                    .prepare_run(RuntimeSupervisionMode::Foreground)
                    .map_err(ForegroundDataExecutionError::SessionPreparation)
            } else {
                // No abandon flag: try to resume if the handler is registered.
                let has_resume = match admitted_bundle {
                    AdmittedBundle::Acquisition(b) => {
                        crate::data::runtime::acquisition_bundle_has_resume(b)
                    }
                    AdmittedBundle::Processing(_) => false,
                };

                if has_resume {
                    coordinator
                        .prepare_resume(RuntimeSupervisionMode::Foreground)
                        .map_err(ForegroundDataExecutionError::SessionPreparation)
                } else {
                    Err(ForegroundDataExecutionError::ResumeHandlerUnavailable)
                }
            }
        }
    }
}

fn select_and_prepare_processing(
    coordinator: &SessionCoordinator,
    abandon_past_failure: bool,
) -> Result<PreparedSessionLaunch, ForegroundDataExecutionError> {
    let current_status = coordinator_current_status(coordinator)?;

    match current_status {
        CurrentStatus::None | CurrentStatus::Succeeded | CurrentStatus::Abandoned => {
            coordinator
                .prepare_run(RuntimeSupervisionMode::Foreground)
                .map_err(ForegroundDataExecutionError::SessionPreparation)
        }
        CurrentStatus::Live => {
            Err(ForegroundDataExecutionError::SessionSelection(
                SessionCoordinationError::LiveSessionAlreadyActive,
            ))
        }
        CurrentStatus::Failed => {
            if abandon_past_failure {
                coordinator
                    .abandon_current_failure()
                    .map_err(ForegroundDataExecutionError::Abandonment)?;
                coordinator
                    .prepare_run(RuntimeSupervisionMode::Foreground)
                    .map_err(ForegroundDataExecutionError::SessionPreparation)
            } else {
                Err(ForegroundDataExecutionError::SessionSelection(
                    SessionCoordinationError::UnresolvedFailure,
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Post-launch session reconciliation
// ---------------------------------------------------------------------------

/// Load a session record from the operation root after the child has terminated.
pub fn load_terminal_session(
    operation_root_path: &std::path::Path,
    session_id: &SessionIdentity,
) -> Result<SessionRecordV1, ForegroundDataExecutionError> {
    let op_root = SessionOperationRoot::new(operation_root_path.to_path_buf())
        .map_err(ForegroundDataExecutionError::StaleSessionReconciliation)?;
    let store = SessionStore::open(op_root)
        .map_err(ForegroundDataExecutionError::MissingTerminalSession)?;
    store
        .load(session_id)
        .map_err(|e| match e {
            SessionStoreError::MissingSession => {
                ForegroundDataExecutionError::MissingTerminalSession(e)
            }
            _ => ForegroundDataExecutionError::CorruptTerminalSession(e),
        })
}

/// Transition a session to Failed after abnormal termination.
///
/// Returns the transition error if persistence fails, so the caller can
/// produce a combined error.
pub fn persist_abnormal_termination(
    operation_root_path: &std::path::Path,
    session_id: &SessionIdentity,
    revision: u64,
    failure_code: SessionFailureCode,
    diagnostic: Option<String>,
) -> Result<SessionRecordV1, SessionCoordinationError> {
    let op_root = SessionOperationRoot::new(operation_root_path.to_path_buf())
        .map_err(SessionCoordinationError::Store)?;
    let store = SessionStore::open(op_root).map_err(SessionCoordinationError::Store)?;
    let failure = SafeSessionFailure::new(
        SessionFailureKind::AbnormalTermination,
        failure_code,
        diagnostic,
    );
    store
        .transition(
            session_id,
            revision,
            SessionTransition::ToFailed { failure },
        )
        .map_err(SessionCoordinationError::Store)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Simplified view of the current session state for selection purposes.
enum CurrentStatus {
    None,
    Succeeded,
    Abandoned,
    Live,
    Failed,
}

fn coordinator_current_status(
    coordinator: &SessionCoordinator,
) -> Result<CurrentStatus, ForegroundDataExecutionError> {
    let store = coordinator.store();
    let status = store
        .load_status()
        .map_err(|e| ForegroundDataExecutionError::StaleSessionReconciliation(e))?;

    let Some(status) = status else {
        return Ok(CurrentStatus::None);
    };
    let Some(session_id) = status.current_session() else {
        return Ok(CurrentStatus::None);
    };

    let record = match store.load(session_id) {
        Ok(r) => r,
        Err(SessionStoreError::MissingSession) => return Ok(CurrentStatus::None),
        Err(e) => {
            return Err(ForegroundDataExecutionError::StaleSessionReconciliation(e));
        }
    };

    match record.state() {
        SessionState::Succeeded => Ok(CurrentStatus::Succeeded),
        SessionState::Abandoned => Ok(CurrentStatus::Abandoned),
        SessionState::Failed => Ok(CurrentStatus::Failed),
        SessionState::Prepared | SessionState::Running => Ok(CurrentStatus::Live),
    }
}

fn session_coordination_error_to_store_error(
    err: SessionCoordinationError,
) -> SessionStoreError {
    match err {
        SessionCoordinationError::Store(e) => e,
        _ => SessionStoreError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            err.to_string(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Terminal session validation helpers
// ---------------------------------------------------------------------------

/// Load the session record after child termination and verify that its identity
/// fields match the prepared launch.
///
/// Checks: project, runtime, session, operation, execution_mode, supervision_mode.
pub fn load_and_validate_terminal_session(
    operation_root_path: &std::path::Path,
    prepared_record: &SessionRecordV1,
) -> Result<SessionRecordV1, crate::data::error::ForegroundDataExecutionError> {
    let session_id = prepared_record.session();
    let record = load_terminal_session(operation_root_path, session_id)?;

    // Verify identity fields agree with the prepared record.
    if record.project() != prepared_record.project() {
        return Err(crate::data::error::ForegroundDataExecutionError::SessionIdentityDisagreement {
            field: "project",
            expected: format!("{:?}", prepared_record.project()),
            actual: format!("{:?}", record.project()),
        });
    }
    if record.runtime() != prepared_record.runtime() {
        return Err(crate::data::error::ForegroundDataExecutionError::SessionIdentityDisagreement {
            field: "runtime",
            expected: format!("{:?}", prepared_record.runtime()),
            actual: format!("{:?}", record.runtime()),
        });
    }
    if record.session() != prepared_record.session() {
        return Err(crate::data::error::ForegroundDataExecutionError::SessionIdentityDisagreement {
            field: "session",
            expected: prepared_record.session().id().to_owned(),
            actual: record.session().id().to_owned(),
        });
    }
    if record.operation() != prepared_record.operation() {
        return Err(crate::data::error::ForegroundDataExecutionError::SessionIdentityDisagreement {
            field: "operation",
            expected: format!("{:?}", prepared_record.operation()),
            actual: format!("{:?}", record.operation()),
        });
    }
    if record.execution_mode() != prepared_record.execution_mode() {
        return Err(crate::data::error::ForegroundDataExecutionError::SessionIdentityDisagreement {
            field: "execution_mode",
            expected: format!("{:?}", prepared_record.execution_mode()),
            actual: format!("{:?}", record.execution_mode()),
        });
    }
    if record.supervision_mode() != prepared_record.supervision_mode() {
        return Err(crate::data::error::ForegroundDataExecutionError::SessionIdentityDisagreement {
            field: "supervision_mode",
            expected: format!("{:?}", prepared_record.supervision_mode()),
            actual: format!("{:?}", record.supervision_mode()),
        });
    }

    Ok(record)
}

/// Validate that the root `session_status.json` agrees with a terminal session record.
///
/// Checks: schema_version, project, runtime, operation, current_session,
/// current_state, revision.
///
/// Returns `Ok(())` if the summary agrees, or `Err` with a detail string describing
/// the first disagreement found.
pub fn validate_root_summary_against_record(
    store: &SessionStore,
    record: &SessionRecordV1,
) -> Result<(), String> {
    let status = match store.load_status() {
        Ok(Some(s)) => s,
        Ok(None) => return Err("root session_status.json is missing".to_owned()),
        Err(e) => return Err(format!("failed to load session_status.json: {e}")),
    };

    if status.schema_version() != lexicon_core::session::SESSION_SCHEMA_VERSION {
        return Err(format!(
            "schema_version mismatch: expected {}, found {}",
            lexicon_core::session::SESSION_SCHEMA_VERSION,
            status.schema_version()
        ));
    }
    if status.project() != record.project() {
        return Err(format!(
            "project mismatch: summary={:?}, record={:?}",
            status.project(),
            record.project()
        ));
    }
    if status.runtime() != record.runtime() {
        return Err(format!(
            "runtime mismatch: summary={:?}, record={:?}",
            status.runtime(),
            record.runtime()
        ));
    }
    if status.operation() != record.operation() {
        return Err(format!(
            "operation mismatch: summary={:?}, record={:?}",
            status.operation(),
            record.operation()
        ));
    }
    match status.current_session() {
        None => return Err("root summary has no current_session".to_owned()),
        Some(id) if id != record.session() => {
            return Err(format!(
                "current_session mismatch: summary={}, record={}",
                id.id(),
                record.session().id()
            ));
        }
        Some(_) => {}
    }
    match status.current_state() {
        None => return Err("root summary has no current_state".to_owned()),
        Some(s) if s != record.state() => {
            return Err(format!(
                "current_state mismatch: summary={:?}, record={:?}",
                s,
                record.state()
            ));
        }
        Some(_) => {}
    }
    if status.revision() != record.revision() {
        return Err(format!(
            "revision mismatch: summary={}, record={}",
            status.revision(),
            record.revision()
        ));
    }

    Ok(())
}

/// Attempt to rebuild the root summary from the session record, then validate.
///
/// Returns the rebuilt status on success, or a `SessionStoreError` if the rebuild fails.
pub fn rebuild_and_validate_root_summary(
    operation_root_path: &std::path::Path,
    session_id: &SessionIdentity,
    record: &SessionRecordV1,
) -> Result<(), crate::data::error::ForegroundDataExecutionError> {
    let op_root = SessionOperationRoot::new(operation_root_path.to_path_buf())
        .map_err(crate::data::error::ForegroundDataExecutionError::StaleSessionReconciliation)?;
    let store = SessionStore::open(op_root)
        .map_err(crate::data::error::ForegroundDataExecutionError::MissingTerminalSession)?;

    store
        .rebuild_status_from_record(session_id)
        .map_err(crate::data::error::ForegroundDataExecutionError::MissingTerminalSession)?;

    // Reload and re-validate.
    match validate_root_summary_against_record(&store, record) {
        Ok(()) => Ok(()),
        Err(detail) => Err(
            crate::data::error::ForegroundDataExecutionError::RootSummaryReconciliationFailed {
                detail,
                rebuild_error: None,
            },
        ),
    }
}
