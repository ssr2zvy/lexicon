use lexicon_core::session::{
    SessionOperation, SessionRecordV1, SessionState, SessionStore, SessionStoreError,
    SessionStatusV1,
};

use super::error::SessionCoordinationError;

/// Result of checking a current session's liveness status.
pub(super) enum CurrentSessionStatus {
    /// No current session exists.
    None,
    /// The current session completed successfully.
    Succeeded,
    /// The current session was abandoned.
    Abandoned,
    /// A live lease is held by another process; the session is active.
    Live(SessionRecordV1),
    /// The current session failed and has not been abandoned.
    Failed(SessionRecordV1),
    /// The current session was stale and has been reconciled to Failed.
    StaleReconciled(SessionRecordV1),
}

/// Check the current session's status, reconciling stale ownership as needed.
pub(super) fn assess_current_session(
    store: &SessionStore,
    status: &Option<SessionStatusV1>,
) -> Result<CurrentSessionStatus, SessionCoordinationError> {
    let Some(status) = status else {
        return Ok(CurrentSessionStatus::None);
    };

    let Some(session_id) = status.current_session() else {
        return Ok(CurrentSessionStatus::None);
    };

    let record = match store.load(session_id) {
        Ok(r) => r,
        Err(SessionStoreError::MissingSession) => return Ok(CurrentSessionStatus::None),
        Err(e) => return Err(SessionCoordinationError::Store(e)),
    };

    match record.state() {
        SessionState::Succeeded => return Ok(CurrentSessionStatus::Succeeded),
        SessionState::Abandoned => return Ok(CurrentSessionStatus::Abandoned),
        SessionState::Failed => return Ok(CurrentSessionStatus::Failed(record)),
        SessionState::Prepared | SessionState::Running => {}
    }

    // Try to reconcile potentially stale ownership.
    match store.reconcile_stale_current_session(session_id) {
        Ok(Some(reconciled)) => Ok(CurrentSessionStatus::StaleReconciled(reconciled)),
        Ok(None) => {
            // Either a live owner holds the lease, or the record was already terminal.
            let updated = store.load(session_id).map_err(SessionCoordinationError::Store)?;
            match updated.state() {
                SessionState::Succeeded => Ok(CurrentSessionStatus::Succeeded),
                SessionState::Abandoned => Ok(CurrentSessionStatus::Abandoned),
                SessionState::Failed => Ok(CurrentSessionStatus::Failed(updated)),
                SessionState::Prepared | SessionState::Running => {
                    Ok(CurrentSessionStatus::Live(updated))
                }
            }
        }
        Err(e) => Err(SessionCoordinationError::Store(e)),
    }
}

/// Determine whether a new run session can be started given the current status.
///
/// - Rejects an actively owned `Prepared` or `Running` current session.
/// - Rejects an unresolved `Failed` current session unless abandonment was applied.
/// - Allows a new run after `Succeeded`, `Abandoned`, `None`, or a stale reconciled session.
pub(super) fn validate_run_selection(
    current: &CurrentSessionStatus,
) -> Result<(), SessionCoordinationError> {
    match current {
        CurrentSessionStatus::None
        | CurrentSessionStatus::Succeeded
        | CurrentSessionStatus::Abandoned
        | CurrentSessionStatus::StaleReconciled(_) => Ok(()),
        CurrentSessionStatus::Live(_) => Err(SessionCoordinationError::LiveSessionAlreadyActive),
        CurrentSessionStatus::Failed(_) => Err(SessionCoordinationError::UnresolvedFailure),
    }
}

/// Determine whether a resume session can be started given the current status.
///
/// Requires acquisition operation. Processing resume is not supported.
/// Requires a prior resumable (reconciled or failed) session.
pub(super) fn validate_resume_selection(
    operation: SessionOperation,
    current: &CurrentSessionStatus,
) -> Result<(), SessionCoordinationError> {
    if operation != SessionOperation::Acquisition {
        return Err(SessionCoordinationError::ResumeNotSupportedForOperation);
    }

    match current {
        CurrentSessionStatus::Failed(_) | CurrentSessionStatus::StaleReconciled(_) => Ok(()),
        CurrentSessionStatus::Live(_) => Err(SessionCoordinationError::LiveSessionAlreadyActive),
        CurrentSessionStatus::None
        | CurrentSessionStatus::Succeeded
        | CurrentSessionStatus::Abandoned => Err(SessionCoordinationError::ResumeUnavailable),
    }
}
