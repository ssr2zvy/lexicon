use lexicon_core::session::{
    NewSessionRecord, ProjectIdentity, RuntimeContextPaths, SafeSessionFailure, SessionIdentity,
    SessionLease, SessionOperation, SessionOperationRoot, SessionRecordV1, SessionStore,
    SessionTransition, encode_runtime_context,
};
use lexicon_core::runtime::{OwnedRuntimeIdentity, RuntimeExecutionMode, RuntimeSupervisionMode};

use super::error::SessionCoordinationError;
use super::selection::{
    CurrentSessionStatus, assess_current_session, validate_resume_selection,
    validate_run_selection,
};

// ---------------------------------------------------------------------------
// PreparedSessionLaunch
// ---------------------------------------------------------------------------

/// The result of a successful session preparation by the coordinator.
///
/// Retains:
/// - The `PreparedSession` record.
/// - The exclusive parent-side lease on the session.
/// - The runtime context environment document (JSON string for the child process).
///
/// The parent lease must be held through child startup. When the child
/// process acquires the lease on its own (via an explicit handoff method to
/// be provided by a later process-launching milestone), the parent drops it.
///
/// Do not drop this value before the child has started unless the preparation
/// should be marked as failed.
pub struct PreparedSessionLaunch {
    record: SessionRecordV1,
    lease: SessionLease,
    context_document: String,
}

impl PreparedSessionLaunch {
    pub fn record(&self) -> &SessionRecordV1 {
        &self.record
    }

    /// The JSON string to set as `LEXICON_RUNTIME_CONTEXT_V1` in the child environment.
    pub fn context_document(&self) -> &str {
        &self.context_document
    }

    /// Mark the prepared session as failed, releasing the lease.
    ///
    /// Use this if the child process launch fails before the session is handed off.
    pub fn fail_launch(
        self,
        store: &SessionStore,
        failure: SafeSessionFailure,
    ) -> Result<SessionRecordV1, SessionCoordinationError> {
        let session_id = self.record.session().clone();
        let revision = self.record.revision();
        let transition_result = store
            .transition(&session_id, revision, SessionTransition::ToFailed { failure })
            .map_err(SessionCoordinationError::Store);
        drop(self);
        transition_result
    }
}

impl std::fmt::Debug for PreparedSessionLaunch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedSessionLaunch")
            .field("session", self.record.session())
            .field("operation", &self.record.operation())
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// SessionCoordinator
// ---------------------------------------------------------------------------

/// Framework-owned coordinator for session preparation and reconciliation.
///
/// Operates on already validated project, source, protocol, operation, runtime,
/// and filesystem identities.
///
/// Does not launch processes.
pub struct SessionCoordinator {
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    operation: SessionOperation,
    store: SessionStore,
    context_paths: RuntimeContextPaths,
}

impl SessionCoordinator {
    /// Construct a coordinator from validated identities and paths.
    pub fn new(
        project: ProjectIdentity,
        runtime: OwnedRuntimeIdentity,
        operation: SessionOperation,
        operation_root: SessionOperationRoot,
        context_paths: RuntimeContextPaths,
    ) -> Result<Self, SessionCoordinationError> {
        let store = SessionStore::open(operation_root).map_err(SessionCoordinationError::Store)?;
        Ok(Self {
            project,
            runtime,
            operation,
            store,
            context_paths,
        })
    }

    /// Prepare a new run session.
    ///
    /// - Rejects an actively owned Prepared or Running current session.
    /// - Rejects an unresolved Failed current session unless abandonment was applied.
    /// - Generates a new session identity.
    /// - Creates session.json in Prepared state.
    /// - Updates session_status.json.
    /// - Acquires and retains the lease in the returned launch value.
    pub fn prepare_run(
        &self,
        supervision: RuntimeSupervisionMode,
    ) -> Result<PreparedSessionLaunch, SessionCoordinationError> {
        let status = self.store.load_status().map_err(SessionCoordinationError::Store)?;
        let current = assess_current_session(&self.store, &status)?;
        validate_run_selection(&current)?;

        self.create_prepared_launch(RuntimeExecutionMode::Run, supervision)
    }

    /// Prepare a resume session.
    ///
    /// - Requires acquisition operation.
    /// - Requires a prior resumable (failed or stale-reconciled) session.
    /// - Rejects a currently live owned session.
    /// - Creates a new Prepared record with execution mode Resume.
    pub fn prepare_resume(
        &self,
        supervision: RuntimeSupervisionMode,
    ) -> Result<PreparedSessionLaunch, SessionCoordinationError> {
        let status = self.store.load_status().map_err(SessionCoordinationError::Store)?;
        let current = assess_current_session(&self.store, &status)?;
        validate_resume_selection(self.operation, &current)?;

        self.create_prepared_launch(RuntimeExecutionMode::Resume, supervision)
    }

    /// Abandon the current failed session.
    ///
    /// This implements the `--abandon-past-fail` CLI flag policy.
    ///
    /// Only abandons:
    /// - Non-live Prepared, Running (stale), or Failed sessions.
    ///
    /// Will not abandon:
    /// - A live leased session.
    /// - A succeeded session.
    /// - An already abandoned session.
    /// - A session belonging to another project, source, protocol, or operation.
    pub fn abandon_current_failure(
        &self,
    ) -> Result<SessionRecordV1, SessionCoordinationError> {
        let status = self.store.load_status().map_err(SessionCoordinationError::Store)?;
        let current = assess_current_session(&self.store, &status)?;

        match current {
            CurrentSessionStatus::Failed(record) => {
                let session_id = record.session().clone();
                let revision = record.revision();
                self.store
                    .transition(&session_id, revision, SessionTransition::ToAbandoned)
                    .map_err(SessionCoordinationError::Store)
            }
            CurrentSessionStatus::StaleReconciled(record) => {
                let session_id = record.session().clone();
                let revision = record.revision();
                self.store
                    .transition(&session_id, revision, SessionTransition::ToAbandoned)
                    .map_err(SessionCoordinationError::Store)
            }
            CurrentSessionStatus::Live(_) => {
                Err(SessionCoordinationError::AbandonmentUnavailable {
                    reason: "session is currently live and cannot be abandoned",
                })
            }
            CurrentSessionStatus::Succeeded => {
                Err(SessionCoordinationError::AbandonmentUnavailable {
                    reason: "succeeded sessions cannot be abandoned",
                })
            }
            CurrentSessionStatus::Abandoned => {
                Err(SessionCoordinationError::AbandonmentUnavailable {
                    reason: "session is already abandoned",
                })
            }
            CurrentSessionStatus::None => {
                Err(SessionCoordinationError::AbandonmentUnavailable {
                    reason: "no current session to abandon",
                })
            }
        }
    }

    /// Reconcile a potentially stale current session.
    ///
    /// If the current session owner is no longer alive, transitions the durable
    /// record to Failed with StaleOwnership and updates the root summary.
    ///
    /// Returns `Ok(Some(record))` if reconciliation occurred, `Ok(None)` if no
    /// reconciliation was needed.
    pub fn reconcile_stale_current_session(
        &self,
    ) -> Result<Option<SessionRecordV1>, SessionCoordinationError> {
        let status = self.store.load_status().map_err(SessionCoordinationError::Store)?;

        let session_id = match status.as_ref().and_then(|s| s.current_session()) {
            Some(id) => id.clone(),
            None => return Ok(None),
        };

        self.store
            .reconcile_stale_current_session(&session_id)
            .map_err(SessionCoordinationError::Store)
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn create_prepared_launch(
        &self,
        execution_mode: RuntimeExecutionMode,
        supervision: RuntimeSupervisionMode,
    ) -> Result<PreparedSessionLaunch, SessionCoordinationError> {
        let input = NewSessionRecord {
            project: self.project.clone(),
            runtime: self.runtime.clone(),
            operation: self.operation,
            execution_mode,
            supervision_mode: supervision,
        };

        let prepared = self
            .store
            .create_prepared(input)
            .map_err(SessionCoordinationError::Store)?;

        let session_id = prepared.record().session().clone();

        let lease = self
            .store
            .acquire_lease(&session_id)
            .map_err(SessionCoordinationError::Lease)?;

        // Build context paths scoped to this session.
        let session_paths = build_session_paths(&self.context_paths, &session_id, self.operation)
            .map_err(SessionCoordinationError::InvalidOperationRoot)?;

        let context_document = encode_runtime_context(
            &self.project,
            &self.runtime,
            &session_id,
            &session_paths,
        )
        .map_err(SessionCoordinationError::ContextEncoding)?;

        Ok(PreparedSessionLaunch {
            record: prepared.into_record(),
            lease,
            context_document,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build `RuntimeContextPaths` scoped to a specific session within the coordinator's paths.
fn build_session_paths(
    coordinator_paths: &RuntimeContextPaths,
    session: &lexicon_core::session::SessionIdentity,
    operation: SessionOperation,
) -> Result<RuntimeContextPaths, lexicon_core::session::RuntimeContextError> {
    let op = operation.to_runtime_operation();
    let session_directory = coordinator_paths
        .operation_root()
        .join("sessions")
        .join(session.id());

    RuntimeContextPaths::new(
        coordinator_paths.project_root().to_path_buf(),
        coordinator_paths.protocol_root().to_path_buf(),
        coordinator_paths.operation_root().to_path_buf(),
        session_directory,
        coordinator_paths.raw_data_directory().to_path_buf(),
        coordinator_paths.processed_data_directory().to_path_buf(),
        op,
        session,
    )
}
