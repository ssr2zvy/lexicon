use std::fmt;

use lexicon_core::session::{
    RuntimeContextError, SessionLeaseError, SessionStoreError,
};

/// Errors arising from session coordination operations.
#[derive(Debug)]
pub enum SessionCoordinationError {
    /// Session store operation failed.
    Store(SessionStoreError),
    /// Session lease operation failed.
    Lease(SessionLeaseError),
    /// A live session is already in progress; cannot start a new one.
    LiveSessionAlreadyActive,
    /// The current session is in a failed state that has not been abandoned.
    UnresolvedFailure,
    /// Resume is not available because there is no prior resumable session.
    ResumeUnavailable,
    /// Resume is not available for this operation type.
    ResumeNotSupportedForOperation,
    /// The session targeted for abandonment is in a state that cannot be abandoned.
    AbandonmentUnavailable { reason: &'static str },
    /// Failed to encode the runtime context document.
    ContextEncoding(RuntimeContextError),
    /// Session operation root could not be derived from the given paths.
    InvalidOperationRoot(RuntimeContextError),
}

impl fmt::Display for SessionCoordinationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(err) => write!(f, "session store error: {err}"),
            Self::Lease(err) => write!(f, "session lease error: {err}"),
            Self::LiveSessionAlreadyActive => {
                f.write_str("a live session is already active; cannot start a new session")
            }
            Self::UnresolvedFailure => {
                f.write_str("current session is in a failed state; use --abandon-past-fail to abandon it before retrying")
            }
            Self::ResumeUnavailable => {
                f.write_str("no prior resumable session is available")
            }
            Self::ResumeNotSupportedForOperation => {
                f.write_str("resume is only supported for acquisition sessions")
            }
            Self::AbandonmentUnavailable { reason } => {
                write!(f, "session abandonment unavailable: {reason}")
            }
            Self::ContextEncoding(err) => {
                write!(f, "runtime context encoding error: {err}")
            }
            Self::InvalidOperationRoot(err) => {
                write!(f, "invalid session operation root: {err}")
            }
        }
    }
}

impl std::error::Error for SessionCoordinationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(err) => Some(err),
            Self::Lease(err) => Some(err),
            Self::LiveSessionAlreadyActive
            | Self::UnresolvedFailure
            | Self::ResumeUnavailable
            | Self::ResumeNotSupportedForOperation
            | Self::AbandonmentUnavailable { .. }
            Self::ContextEncoding(err) => Some(err),
            Self::InvalidOperationRoot(err) => Some(err),
        }
    }
}
