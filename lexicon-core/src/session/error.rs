use std::fmt;

// ---------------------------------------------------------------------------
// Session encoding / decoding
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum SessionEncodingError {
    Serialization(String),
}

impl fmt::Display for SessionEncodingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(msg) => write!(f, "session serialization error: {msg}"),
        }
    }
}

impl std::error::Error for SessionEncodingError {}

#[derive(Debug)]
pub enum SessionDecodingError {
    JsonSyntax(String),
    UnknownSchemaVersion(u32),
    UnknownField { field: &'static str, value: String },
    InvalidInvariant(String),
    IdentityMismatch { field: &'static str, expected: String, actual: String },
    InvalidTimestamp(String),
    InvalidRevision { message: String },
    StructuralDocument(String),
}

impl fmt::Display for SessionDecodingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsonSyntax(msg) => write!(f, "invalid JSON in session document: {msg}"),
            Self::UnknownSchemaVersion(v) => {
                write!(f, "unknown session schema version: {v}")
            }
            Self::UnknownField { field, value } => {
                write!(f, "unknown {field} identifier: {value}")
            }
            Self::InvalidInvariant(msg) => {
                write!(f, "invalid session document invariant: {msg}")
            }
            Self::IdentityMismatch { field, expected, actual } => {
                write!(
                    f,
                    "session identity mismatch for {field}: expected {expected}, actual {actual}"
                )
            }
            Self::InvalidTimestamp(msg) => write!(f, "invalid session timestamp: {msg}"),
            Self::InvalidRevision { message } => write!(f, "invalid session revision: {message}"),
            Self::StructuralDocument(msg) => {
                write!(f, "malformed session document: {msg}")
            }
        }
    }
}

impl std::error::Error for SessionDecodingError {}

// ---------------------------------------------------------------------------
// Transition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTransitionError {
    InvalidTransition {
        from: crate::session::model::SessionState,
        to: crate::session::model::SessionState,
    },
    ImmutableFieldMutation { field: &'static str },
    RevisionRollback { expected: u64, actual: u64 },
    TerminalStateReached { state: crate::session::model::SessionState },
}

impl fmt::Display for SessionTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition { from, to } => {
                write!(f, "invalid session state transition: {from:?} → {to:?}")
            }
            Self::ImmutableFieldMutation { field } => {
                write!(f, "attempted mutation of immutable session field: {field}")
            }
            Self::RevisionRollback { expected, actual } => {
                write!(
                    f,
                    "session revision rollback: expected revision ≥ {expected}, got {actual}"
                )
            }
            Self::TerminalStateReached { state } => {
                write!(f, "session is in terminal state: {state:?}")
            }
        }
    }
}

impl std::error::Error for SessionTransitionError {}

// ---------------------------------------------------------------------------
// Lease
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum SessionLeaseError {
    AlreadyOwned,
    Io(std::io::Error),
}

impl fmt::Display for SessionLeaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyOwned => f.write_str("session lease is already owned by another process"),
            Self::Io(err) => write!(f, "session lease I/O error: {err}"),
        }
    }
}

impl std::error::Error for SessionLeaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AlreadyOwned => None,
            Self::Io(err) => Some(err),
        }
    }
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum SessionStoreError {
    Encoding(SessionEncodingError),
    Decoding(SessionDecodingError),
    InvalidTransition(SessionTransitionError),
    Io(std::io::Error),
    DirectoryCreation(std::io::Error),
    AtomicPersistence(std::io::Error),
    RootSummaryUpdate(Box<SessionStoreError>),
    PartialCommit {
        record_error: Option<Box<SessionStoreError>>,
        summary_error: Box<SessionStoreError>,
    },
    MissingSession,
    CorruptSession(SessionDecodingError),
    RevisionConflict { expected: u64, actual: u64 },
    LeaseRequired(SessionLeaseError),
    StaleOwnershipReconciliationFailed(Box<SessionStoreError>),
}

impl fmt::Display for SessionStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encoding(err) => write!(f, "session encoding error: {err}"),
            Self::Decoding(err) => write!(f, "session decoding error: {err}"),
            Self::InvalidTransition(err) => write!(f, "invalid session transition: {err}"),
            Self::Io(err) => write!(f, "session store I/O error: {err}"),
            Self::DirectoryCreation(err) => {
                write!(f, "session directory creation error: {err}")
            }
            Self::AtomicPersistence(err) => {
                write!(f, "atomic session file persistence error: {err}")
            }
            Self::RootSummaryUpdate(err) => {
                write!(f, "root session summary update error: {err}")
            }
            Self::PartialCommit { .. } => {
                f.write_str("partial session commit: record updated but summary update failed; reconciliation required")
            }
            Self::MissingSession => f.write_str("session record not found"),
            Self::CorruptSession(err) => write!(f, "corrupt session record: {err}"),
            Self::RevisionConflict { expected, actual } => {
                write!(
                    f,
                    "session revision conflict: expected {expected}, found {actual}"
                )
            }
            Self::LeaseRequired(err) => write!(f, "session lease acquisition error: {err}"),
            Self::StaleOwnershipReconciliationFailed(err) => {
                write!(f, "stale session ownership reconciliation failed: {err}")
            }
        }
    }
}

impl std::error::Error for SessionStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encoding(err) => Some(err),
            Self::Decoding(err) => Some(err),
            Self::InvalidTransition(err) => Some(err),
            Self::Io(err) => Some(err),
            Self::DirectoryCreation(err) => Some(err),
            Self::AtomicPersistence(err) => Some(err),
            Self::RootSummaryUpdate(err) => Some(err.as_ref()),
            Self::PartialCommit { summary_error, .. } => Some(summary_error.as_ref()),
            Self::MissingSession => None,
            Self::CorruptSession(err) => Some(err),
            Self::RevisionConflict { .. } => None,
            Self::LeaseRequired(err) => Some(err),
            Self::StaleOwnershipReconciliationFailed(err) => Some(err.as_ref()),
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime context
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum RuntimeContextError {
    MissingEnvironmentVariable,
    InvalidUtf8,
    Decoding(SessionDecodingError),
    IdentityMismatch { field: &'static str, expected: String, actual: String },
    PathMismatch { field: &'static str, expected: String, actual: String },
    RelativeProjectRoot,
    PathTraversal { field: &'static str },
    OperationRootDisagreement,
    ProtocolRootDisagreement,
    SessionDirectoryDisagreement,
}

impl fmt::Display for RuntimeContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEnvironmentVariable => {
                write!(f, "missing runtime context environment variable")
            }
            Self::InvalidUtf8 => f.write_str("runtime context environment variable is not valid UTF-8"),
            Self::Decoding(err) => write!(f, "runtime context decoding error: {err}"),
            Self::IdentityMismatch { field, expected, actual } => {
                write!(
                    f,
                    "runtime context identity mismatch for {field}: expected {expected}, actual {actual}"
                )
            }
            Self::PathMismatch { field, expected, actual } => {
                write!(
                    f,
                    "runtime context path mismatch for {field}: expected {expected}, actual {actual}"
                )
            }
            Self::RelativeProjectRoot => {
                f.write_str("runtime context project root must be an absolute path")
            }
            Self::PathTraversal { field } => {
                write!(f, "runtime context {field} path contains traversal component")
            }
            Self::OperationRootDisagreement => {
                f.write_str("runtime context operation root does not match expected path")
            }
            Self::ProtocolRootDisagreement => {
                f.write_str("runtime context protocol root does not match expected path")
            }
            Self::SessionDirectoryDisagreement => {
                f.write_str("runtime context session directory does not match session identity")
            }
        }
    }
}

impl std::error::Error for RuntimeContextError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decoding(err) => Some(err),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Core runner session initialisation
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum CoreRunnerSessionError {
    ContextDecode(RuntimeContextError),
    StoreOpen(SessionStoreError),
    LeaseAcquisition(SessionLeaseError),
    TransitionToRunning(SessionStoreError),
    TerminalPersistence(SessionStoreError),
}

impl fmt::Display for CoreRunnerSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextDecode(err) => write!(f, "runtime context decoding failed: {err}"),
            Self::StoreOpen(err) => write!(f, "session store open failed: {err}"),
            Self::LeaseAcquisition(err) => write!(f, "session lease acquisition failed: {err}"),
            Self::TransitionToRunning(err) => {
                write!(f, "session transition to running failed: {err}")
            }
            Self::TerminalPersistence(err) => {
                write!(f, "terminal session state persistence failed: {err}")
            }
        }
    }
}

impl std::error::Error for CoreRunnerSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ContextDecode(err) => Some(err),
            Self::StoreOpen(err) => Some(err),
            Self::LeaseAcquisition(err) => Some(err),
            Self::TransitionToRunning(err) => Some(err),
            Self::TerminalPersistence(err) => Some(err),
        }
    }
}
