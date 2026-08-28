use lexicon_core::runtime::{OwnedRuntimeIdentity, RuntimeSupervisionMode};
use lexicon_core::session::{
    ProjectIdentity, SafeSessionFailure, SessionFailureCode, SessionFailureKind,
    SessionIdentity, SessionOperation, SessionOperationRoot, SessionRecordV1, SessionState,
    SessionStore, SessionStoreError, SessionTransition,
};

use crate::data::error::{
    ForegroundDataExecutionError, RootSummaryReconciliationError, RootSummaryValidationError,
    TerminalSessionIdentityMismatch,
};
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
///
/// `supervision` controls whether the freshly `Prepared` session records
/// `Foreground` (used by `execute_foreground_data`) or `Background` (used by
/// the initiating process of `execute_background_data`, which then hands the
/// prepared session off to the operator host).
pub fn select_and_prepare_session(
    coordinator: &SessionCoordinator,
    operation: DataOperation,
    abandon_past_failure: bool,
    admitted_bundle: &AdmittedBundle,
    supervision: RuntimeSupervisionMode,
) -> Result<PreparedSessionLaunch, ForegroundDataExecutionError> {
    // Reconcile stale ownership first.
    coordinator
        .reconcile_stale_current_session()
        .map_err(|e| ForegroundDataExecutionError::StaleSessionReconciliation(
            session_coordination_error_to_store_error(e)
        ))?;

    match operation {
        DataOperation::Acquisition => {
            select_and_prepare_acquisition(coordinator, abandon_past_failure, admitted_bundle, supervision)
        }
        DataOperation::Processing => {
            select_and_prepare_processing(coordinator, abandon_past_failure, supervision)
        }
    }
}

fn select_and_prepare_acquisition(
    coordinator: &SessionCoordinator,
    abandon_past_failure: bool,
    admitted_bundle: &AdmittedBundle,
    supervision: RuntimeSupervisionMode,
) -> Result<PreparedSessionLaunch, ForegroundDataExecutionError> {
    // Load current status after stale reconciliation.
    let current_status = coordinator_current_status(coordinator)?;

    match current_status {
        CurrentStatus::None | CurrentStatus::Succeeded | CurrentStatus::Abandoned => {
            coordinator
                .prepare_run(supervision)
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
                    .prepare_run(supervision)
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
                        .prepare_resume(supervision)
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
    supervision: RuntimeSupervisionMode,
) -> Result<PreparedSessionLaunch, ForegroundDataExecutionError> {
    let current_status = coordinator_current_status(coordinator)?;

    match current_status {
        CurrentStatus::None | CurrentStatus::Succeeded | CurrentStatus::Abandoned => {
            coordinator
                .prepare_run(supervision)
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
                    .prepare_run(supervision)
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

pub struct ReconciledTerminalSession {
    record: SessionRecordV1,
}

impl ReconciledTerminalSession {
    pub fn record(&self) -> &SessionRecordV1 {
        &self.record
    }
}

/// Load the session record after child termination and verify that its identity
/// fields match the prepared launch.
pub fn load_and_validate_terminal_session(
    operation_root_path: &std::path::Path,
    prepared_record: &SessionRecordV1,
) -> Result<SessionRecordV1, ForegroundDataExecutionError> {
    let session_id = prepared_record.session();
    let record = load_terminal_session(operation_root_path, session_id)?;
    validate_terminal_session_identity(prepared_record, &record)?;
    Ok(record)
}

pub fn validate_terminal_session_identity(
    prepared_record: &SessionRecordV1,
    record: &SessionRecordV1,
) -> Result<(), ForegroundDataExecutionError> {
    if record.project() != prepared_record.project() {
        return Err(ForegroundDataExecutionError::SessionIdentityDisagreement(
            TerminalSessionIdentityMismatch::Project {
                expected: prepared_record.project().clone(),
                actual: record.project().clone(),
            },
        ));
    }
    if record.runtime() != prepared_record.runtime() {
        return Err(ForegroundDataExecutionError::SessionIdentityDisagreement(
            TerminalSessionIdentityMismatch::Runtime {
                expected: prepared_record.runtime().clone(),
                actual: record.runtime().clone(),
            },
        ));
    }
    if record.session() != prepared_record.session() {
        return Err(ForegroundDataExecutionError::SessionIdentityDisagreement(
            TerminalSessionIdentityMismatch::Session {
                expected: prepared_record.session().clone(),
                actual: record.session().clone(),
            },
        ));
    }
    if record.operation() != prepared_record.operation() {
        return Err(ForegroundDataExecutionError::SessionIdentityDisagreement(
            TerminalSessionIdentityMismatch::Operation {
                expected: prepared_record.operation(),
                actual: record.operation(),
            },
        ));
    }
    if record.execution_mode() != prepared_record.execution_mode() {
        return Err(ForegroundDataExecutionError::SessionIdentityDisagreement(
            TerminalSessionIdentityMismatch::ExecutionMode {
                expected: prepared_record.execution_mode(),
                actual: record.execution_mode(),
            },
        ));
    }
    if record.supervision_mode() != prepared_record.supervision_mode() {
        return Err(ForegroundDataExecutionError::SessionIdentityDisagreement(
            TerminalSessionIdentityMismatch::SupervisionMode {
                expected: prepared_record.supervision_mode(),
                actual: record.supervision_mode(),
            },
        ));
    }

    Ok(())
}

pub fn validate_root_summary_against_record(
    store: &SessionStore,
    record: &SessionRecordV1,
) -> Result<(), RootSummaryValidationError> {
    let status = match store.load_status() {
        Ok(Some(s)) => s,
        Ok(None) => return Err(RootSummaryValidationError::Missing),
        Err(e) => return Err(RootSummaryValidationError::Load(e)),
    };

    if status.schema_version() != lexicon_core::session::SESSION_SCHEMA_VERSION {
        return Err(RootSummaryValidationError::SchemaVersionMismatch {
            expected: lexicon_core::session::SESSION_SCHEMA_VERSION,
            actual: status.schema_version(),
        });
    }
    if status.project() != record.project() {
        return Err(RootSummaryValidationError::ProjectMismatch);
    }
    if status.runtime() != record.runtime() {
        return Err(RootSummaryValidationError::RuntimeMismatch);
    }
    if status.operation() != record.operation() {
        return Err(RootSummaryValidationError::OperationMismatch);
    }
    match status.current_session() {
        None => return Err(RootSummaryValidationError::MissingCurrentSession),
        Some(id) if id != record.session() => {
            return Err(RootSummaryValidationError::SessionMismatch);
        }
        Some(_) => {}
    }
    match status.current_state() {
        None => return Err(RootSummaryValidationError::MissingCurrentState),
        Some(s) if s != record.state() => {
            return Err(RootSummaryValidationError::StateMismatch {
                expected: record.state(),
                actual: s,
            });
        }
        Some(_) => {}
    }
    if status.revision() != record.revision() {
        return Err(RootSummaryValidationError::RevisionMismatch {
            expected: record.revision(),
            actual: status.revision(),
        });
    }

    Ok(())
}

pub fn validate_or_rebuild_root_summary(
    store: &SessionStore,
    record: &SessionRecordV1,
) -> Result<(), RootSummaryReconciliationError> {
    match validate_root_summary_against_record(store, record) {
        Ok(()) => Ok(()),
        Err(_) => {
            store
                .rebuild_status_from_record(record.session())
                .map_err(RootSummaryReconciliationError::Rebuild)?;
            validate_root_summary_against_record(store, record)
                .map_err(RootSummaryReconciliationError::ValidationAfterRebuild)
        }
    }
}

pub fn reconcile_terminal_session(
    prepared_record: &SessionRecordV1,
    record: SessionRecordV1,
    store: &SessionStore,
    expected_state: SessionState,
) -> Result<ReconciledTerminalSession, ForegroundDataExecutionError> {
    validate_terminal_session_identity(prepared_record, &record)?;
    if record.state() != expected_state {
        return Err(ForegroundDataExecutionError::ExitSessionDisagreement {
            termination: crate::data::outcome::ObservedChildTermination::UnknownAbnormalTermination,
            durable_state: record.state(),
        });
    }
    validate_or_rebuild_root_summary(store, &record)
        .map_err(ForegroundDataExecutionError::RootSummaryReconciliationFailed)?;
    Ok(ReconciledTerminalSession { record })
}

#[cfg(test)]
mod tests {
    use super::select_and_prepare_session;
    use crate::data::test_support::{admit_fake_bundle, build_fake_coordinator, build_fake_project};
    use crate::session::SessionCoordinationError;

    use super::{DataOperation, ForegroundDataExecutionError, RuntimeSupervisionMode};

    /// `select_and_prepare_session` records the requested supervision mode
    /// (`Background`) on the resulting `PreparedSessionLaunch`.
    #[test]
    fn records_background_supervision_mode_when_requested() {
        let project = build_fake_project("example-source");
        let coordinator = build_fake_coordinator(&project, DataOperation::Acquisition);
        let admitted = admit_fake_bundle(&project);

        let prepared = select_and_prepare_session(
            &coordinator,
            DataOperation::Acquisition,
            false,
            &admitted,
            RuntimeSupervisionMode::Background,
        )
        .unwrap();

        assert_eq!(prepared.record().supervision_mode(), RuntimeSupervisionMode::Background);
    }

    /// `select_and_prepare_session` records `Foreground` when requested,
    /// exercising the same call path `execute_foreground_data` uses.
    #[test]
    fn records_foreground_supervision_mode_when_requested() {
        let project = build_fake_project("example-source");
        let coordinator = build_fake_coordinator(&project, DataOperation::Acquisition);
        let admitted = admit_fake_bundle(&project);

        let prepared = select_and_prepare_session(
            &coordinator,
            DataOperation::Acquisition,
            false,
            &admitted,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();

        assert_eq!(prepared.record().supervision_mode(), RuntimeSupervisionMode::Foreground);
    }

    /// The processing selection path also threads the supervision mode
    /// through, even though it never inspects the admitted bundle.
    #[test]
    fn processing_operation_records_requested_supervision_mode() {
        let project = build_fake_project("example-source");
        let coordinator = build_fake_coordinator(&project, DataOperation::Processing);
        let admitted = admit_fake_bundle(&project);

        let prepared = select_and_prepare_session(
            &coordinator,
            DataOperation::Processing,
            false,
            &admitted,
            RuntimeSupervisionMode::Background,
        )
        .unwrap();

        assert_eq!(prepared.record().supervision_mode(), RuntimeSupervisionMode::Background);
    }

    /// A second selection attempt against an already-live (Prepared, still
    /// leased) session is rejected, regardless of supervision mode. This
    /// preserves the pre-existing live-session-rejection scenario now that
    /// `select_and_prepare_session` takes an extra parameter.
    #[test]
    fn rejects_selection_when_a_live_session_is_already_active() {
        let project = build_fake_project("example-source");
        let coordinator = build_fake_coordinator(&project, DataOperation::Acquisition);
        let admitted = admit_fake_bundle(&project);

        let _first = select_and_prepare_session(
            &coordinator,
            DataOperation::Acquisition,
            false,
            &admitted,
            RuntimeSupervisionMode::Background,
        )
        .unwrap();

        let second = select_and_prepare_session(
            &coordinator,
            DataOperation::Acquisition,
            false,
            &admitted,
            RuntimeSupervisionMode::Foreground,
        );

        assert!(matches!(
            second,
            Err(ForegroundDataExecutionError::SessionSelection(
                SessionCoordinationError::LiveSessionAlreadyActive
            ))
        ));
    }
}
