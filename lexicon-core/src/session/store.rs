use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::session::error::{
    SessionDecodingError, SessionLeaseError, SessionStoreError,
};
use crate::session::lease::SessionLease;
use crate::session::model::{
    NewSessionRecord, SessionClock, SessionIdentity, SessionOperation, SessionRecordV1,
    SessionState, SessionStatusV1, SessionTimestamp, SessionTransition, SystemClock,
    generate_session_id,
};
use crate::session::transition::validate_transition;

// ---------------------------------------------------------------------------
// SessionOperationRoot
// ---------------------------------------------------------------------------

/// Validated path to an operation-level workspace root.
///
/// The root contains `sessions/` and `session_status.json`.
#[derive(Debug, Clone)]
pub struct SessionOperationRoot {
    path: PathBuf,
}

impl SessionOperationRoot {
    /// Construct a `SessionOperationRoot` from an existing directory path.
    ///
    /// The path must be absolute. The directory is not required to exist yet.
    pub fn new(path: PathBuf) -> Result<Self, SessionStoreError> {
        if path.is_relative() {
            return Err(SessionStoreError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "session operation root must be an absolute path",
            )));
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `sessions/` directory.
    pub fn sessions_directory(&self) -> PathBuf {
        self.path.join("sessions")
    }

    /// `sessions/<session-id>/` directory.
    pub fn session_directory(&self, session: &SessionIdentity) -> PathBuf {
        self.sessions_directory().join(session.id())
    }

    /// `sessions/<session-id>/session.json`
    pub fn session_record_path(&self, session: &SessionIdentity) -> PathBuf {
        self.session_directory(session).join("session.json")
    }

    /// `sessions/<session-id>/session.lock`
    pub fn lease_path(&self, session: &SessionIdentity) -> PathBuf {
        self.session_directory(session).join("session.lock")
    }

    /// `session_status.json`
    pub fn status_path(&self) -> PathBuf {
        self.path.join("session_status.json")
    }
}

// ---------------------------------------------------------------------------
// PreparedSession / RunningSession
// ---------------------------------------------------------------------------

/// A session record in the `Prepared` state, returned after creation.
pub struct PreparedSession {
    record: SessionRecordV1,
}

impl PreparedSession {
    pub fn record(&self) -> &SessionRecordV1 {
        &self.record
    }

    pub fn into_record(self) -> SessionRecordV1 {
        self.record
    }
}

impl std::fmt::Debug for PreparedSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedSession")
            .field("session", self.record.session())
            .field("revision", &self.record.revision())
            .finish_non_exhaustive()
    }
}

/// A session record in the `Running` state, with an active exclusive lease.
///
/// The lease is retained for the lifetime of the running session.
/// Not `Clone`; not constructible by callers outside this crate.
pub struct RunningSession {
    record: SessionRecordV1,
    lease: SessionLease,
}

impl RunningSession {
    pub fn record(&self) -> &SessionRecordV1 {
        &self.record
    }

    pub fn into_record(self) -> SessionRecordV1 {
        self.record
        // lease dropped here → lock released
    }

    /// Crate-internal constructor used by runners that acquire the lease independently.
    pub(crate) fn from_parts(record: SessionRecordV1, lease: SessionLease) -> Self {
        Self { record, lease }
    }
}

impl std::fmt::Debug for RunningSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunningSession")
            .field("session", self.record.session())
            .field("revision", &self.record.revision())
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// SessionStore
// ---------------------------------------------------------------------------

pub struct SessionStore {
    operation_root: SessionOperationRoot,
    clock: Box<dyn SessionClock>,
}

impl SessionStore {
    pub fn open(operation_root: SessionOperationRoot) -> Result<Self, SessionStoreError> {
        Ok(Self {
            operation_root,
            clock: Box::new(SystemClock),
        })
    }

    #[cfg(test)]
    pub(crate) fn open_with_clock(
        operation_root: SessionOperationRoot,
        clock: Box<dyn SessionClock>,
    ) -> Result<Self, SessionStoreError> {
        Ok(Self { operation_root, clock })
    }

    // ------------------------------------------------------------------
    // Session creation
    // ------------------------------------------------------------------

    /// Create a new `Prepared` session record and write it to disk.
    ///
    /// Generates a new session identity and creates the session directory.
    pub fn create_prepared(
        &self,
        input: NewSessionRecord,
    ) -> Result<PreparedSession, SessionStoreError> {
        let session_id_str = generate_session_id();
        let session = crate::runtime::invocation::SessionInvocationIdentity::new(&session_id_str)
            .map_err(|e| SessionStoreError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("generated invalid session id: {e}"),
            )))?;

        let record = SessionRecordV1::new_prepared(
            NewSessionRecord { session, ..input },
            self.clock.as_ref(),
        );

        let session_dir = self.operation_root.session_directory(record.session());
        fs::create_dir_all(&session_dir).map_err(SessionStoreError::DirectoryCreation)?;

        let record_path = self.operation_root.session_record_path(record.session());
        let json = record.to_json().map_err(SessionStoreError::Encoding)?;
        write_atomic(&record_path, json.as_bytes())
            .map_err(SessionStoreError::AtomicPersistence)?;

        Ok(PreparedSession { record })
    }

    // ------------------------------------------------------------------
    // Load
    // ------------------------------------------------------------------

    pub fn load(
        &self,
        session: &SessionIdentity,
    ) -> Result<SessionRecordV1, SessionStoreError> {
        let path = self.operation_root.session_record_path(session);
        let json = fs::read_to_string(&path).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                SessionStoreError::MissingSession
            } else {
                SessionStoreError::Io(e)
            }
        })?;
        SessionRecordV1::from_json(&json).map_err(SessionStoreError::CorruptSession)
    }

    pub fn load_status(&self) -> Result<Option<SessionStatusV1>, SessionStoreError> {
        let path = self.operation_root.status_path();
        let json = match fs::read_to_string(&path) {
            Ok(j) => j,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(SessionStoreError::Io(e)),
        };
        let status = SessionStatusV1::from_json(&json).map_err(|e| {
            SessionStoreError::CorruptSession(e)
        })?;
        Ok(Some(status))
    }

    // ------------------------------------------------------------------
    // Lease
    // ------------------------------------------------------------------

    pub fn acquire_lease(
        &self,
        session: &SessionIdentity,
    ) -> Result<SessionLease, SessionLeaseError> {
        let path = self.operation_root.lease_path(session);
        SessionLease::acquire(path)
    }

    // ------------------------------------------------------------------
    // Transition
    // ------------------------------------------------------------------

    /// Apply a state transition to the session record using optimistic revision control.
    ///
    /// Reads the current record, validates the expected revision, validates the
    /// transition, applies it, persists the updated record, and then updates the
    /// root summary.
    ///
    /// If the summary update fails after the record succeeds, returns
    /// `SessionStoreError::PartialCommit`. Call `rebuild_status_from_record` to recover.
    pub fn transition(
        &self,
        session: &SessionIdentity,
        expected_revision: u64,
        transition: SessionTransition,
    ) -> Result<SessionRecordV1, SessionStoreError> {
        let current = self.load(session)?;

        if current.revision() != expected_revision {
            return Err(SessionStoreError::RevisionConflict {
                expected: expected_revision,
                actual: current.revision(),
            });
        }

        validate_transition(current.state(), &transition)
            .map_err(SessionStoreError::InvalidTransition)?;

        let updated = apply_transition(current, transition, self.clock.as_ref());

        let record_path = self.operation_root.session_record_path(session);
        let json = updated.to_json().map_err(SessionStoreError::Encoding)?;
        write_atomic(&record_path, json.as_bytes())
            .map_err(SessionStoreError::AtomicPersistence)?;

        // Update root summary
        let status = SessionStatusV1::from_record(&updated, self.clock.as_ref());
        match self.write_status(&status) {
            Ok(()) => {}
            Err(summary_err) => {
                return Err(SessionStoreError::PartialCommit {
                    record_error: None,
                    summary_error: Box::new(summary_err),
                });
            }
        }

        Ok(updated)
    }

    // ------------------------------------------------------------------
    // Promote Prepared → Running
    // ------------------------------------------------------------------

    /// Transition a `PreparedSession` to `Running`, consuming the prepared value
    /// and binding the provided lease to the new `RunningSession`.
    ///
    /// The lease must have been acquired by the caller before calling this method.
    pub fn promote_to_running(
        &self,
        prepared: PreparedSession,
        lease: SessionLease,
    ) -> Result<RunningSession, SessionStoreError> {
        let session_id = prepared.record.session().clone();
        let revision = prepared.record.revision();
        let new_record = self.transition(
            &session_id,
            revision,
            SessionTransition::ToRunning,
        )?;
        Ok(RunningSession { record: new_record, lease })
    }

    // ------------------------------------------------------------------
    // Terminal transitions from RunningSession
    // ------------------------------------------------------------------

    /// Persist `Succeeded` state and consume the `RunningSession` (releasing the lease).
    pub fn complete_succeeded(
        &self,
        running: RunningSession,
    ) -> Result<SessionRecordV1, SessionStoreError> {
        let session_id = running.record.session().clone();
        let revision = running.record.revision();
        // running (and its lease) will be dropped on return from this fn
        self.transition(&session_id, revision, SessionTransition::ToSucceeded)
    }

    /// Persist `Failed` state and consume the `RunningSession` (releasing the lease).
    pub fn complete_failed(
        &self,
        running: RunningSession,
        kind: crate::session::model::SessionFailureKind,
        summary: Option<String>,
    ) -> Result<SessionRecordV1, SessionStoreError> {
        let session_id = running.record.session().clone();
        let revision = running.record.revision();
        self.transition(&session_id, revision, SessionTransition::ToFailed { kind, summary })
    }

    // ------------------------------------------------------------------
    // Summary reconstruction
    // ------------------------------------------------------------------

    /// Rebuild the root `session_status.json` from the authoritative detailed record.
    ///
    /// Use this after a `PartialCommit` error to restore consistency.
    pub fn rebuild_status_from_record(
        &self,
        session: &SessionIdentity,
    ) -> Result<SessionStatusV1, SessionStoreError> {
        let record = self.load(session)?;
        let status = SessionStatusV1::from_record(&record, self.clock.as_ref());
        self.write_status(&status)?;
        Ok(status)
    }

    // ------------------------------------------------------------------
    // Stale ownership reconciliation
    // ------------------------------------------------------------------

    /// Check whether the current session is stale (process died leaving a non-terminal record).
    ///
    /// 1. Load the current detailed record.
    /// 2. Attempt non-blocking lease acquisition.
    /// 3. Lease contention → live owner → return `Ok(None)`.
    /// 4. Successful lease acquisition of a `Prepared` or `Running` record → stale.
    /// 5. Transition the stale record to `Failed` with `StaleOwnership`.
    /// 6. Update the root summary.
    /// 7. Release the temporary lease.
    pub fn reconcile_stale_current_session(
        &self,
        session: &SessionIdentity,
    ) -> Result<Option<SessionRecordV1>, SessionStoreError> {
        let record = match self.load(session) {
            Ok(r) => r,
            Err(SessionStoreError::MissingSession) => return Ok(None),
            Err(e) => return Err(e),
        };

        if matches!(record.state(), SessionState::Succeeded | SessionState::Abandoned) {
            return Ok(None);
        }

        if record.state() == SessionState::Failed {
            return Ok(None);
        }

        // Attempt non-blocking lease acquisition
        let lease = match self.acquire_lease(session) {
            Ok(lease) => lease,
            Err(SessionLeaseError::AlreadyOwned) => {
                // Live owner holds the lock
                return Ok(None);
            }
            Err(e) => return Err(SessionStoreError::LeaseRequired(e)),
        };

        // We acquired the lease → the previous owner is gone → stale
        let revision = record.revision();
        let result = self
            .transition(
                session,
                revision,
                SessionTransition::ToFailed {
                    kind: crate::session::model::SessionFailureKind::StaleOwnership,
                    summary: Some("stale session ownership: prior process terminated without completing".to_string()),
                },
            )
            .map_err(|e| {
                SessionStoreError::StaleOwnershipReconciliationFailed(Box::new(e))
            })?;

        drop(lease);

        Ok(Some(result))
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn write_status(&self, status: &SessionStatusV1) -> Result<(), SessionStoreError> {
        let path = self.operation_root.status_path();
        let json = status.to_json().map_err(SessionStoreError::Encoding)?;
        write_atomic(&path, json.as_bytes()).map_err(SessionStoreError::AtomicPersistence)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Transition application
// ---------------------------------------------------------------------------

fn apply_transition(
    mut record: SessionRecordV1,
    transition: SessionTransition,
    clock: &dyn SessionClock,
) -> SessionRecordV1 {
    use crate::session::model::{SessionFailureV1, SessionState};

    let now = clock.now();
    let new_state = transition.target_state();

    // Apply state-specific mutations
    match &transition {
        SessionTransition::ToRunning => {
            record.started_at = Some(now);
        }
        SessionTransition::ToSucceeded => {
            record.finished_at = Some(now);
        }
        SessionTransition::ToFailed { kind, summary } => {
            record.finished_at = Some(now);
            record.failure = Some(SessionFailureV1::new(*kind, summary.clone()));
        }
        SessionTransition::ToAbandoned => {
            record.finished_at = Some(now);
            record.failure = None;
        }
    }

    record.state = new_state;
    record.revision += 1;
    record.updated_at = now;

    record
}

// ---------------------------------------------------------------------------
// Atomic file write
// ---------------------------------------------------------------------------

fn write_atomic(dest: &Path, content: &[u8]) -> Result<(), io::Error> {
    let dir = dest.parent().unwrap_or(Path::new("/"));

    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(content)?;
    tmp.flush()?;
    tmp.as_file().sync_all()?;

    tmp.persist(dest).map_err(|e| e.error)?;

    // Best-effort directory sync
    if let Ok(dir_file) = fs::File::open(dir) {
        let _ = dir_file.sync_all();
    }

    Ok(())
}
