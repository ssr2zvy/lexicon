use std::fmt;

use crate::protocols::http::transaction::error::{
    HttpManagedPathError, HttpTransactionAdmissionError,
};
use crate::protocols::http::transaction::HttpLogicalRequestKeyError;
use crate::session::error::SessionDecodingError;

// ---------------------------------------------------------------------------
// HttpCheckpointKeyError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HttpCheckpointKeyError {
    InvalidKey(HttpLogicalRequestKeyError),
}

impl fmt::Display for HttpCheckpointKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey(_) => f.write_str("checkpoint logical key is invalid"),
        }
    }
}

impl std::error::Error for HttpCheckpointKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidKey(e) => Some(e),
        }
    }
}

// ---------------------------------------------------------------------------
// HttpCheckpointEncodingError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HttpCheckpointEncodingError {
    Serialization(serde_json::Error),
}

impl fmt::Display for HttpCheckpointEncodingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(_) => f.write_str("failed to serialize checkpoint document"),
        }
    }
}

impl std::error::Error for HttpCheckpointEncodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serialization(e) => Some(e),
        }
    }
}

// ---------------------------------------------------------------------------
// HttpCheckpointDecodingError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HttpCheckpointDecodingError {
    OversizedDocument { size: usize, limit: usize },
    Deserialization(serde_json::Error),
    UnknownSchemaVersion { found: u32 },
    InvalidKey(HttpLogicalRequestKeyError),
    InvalidKeyHash,
    KeyHashMismatch,
    FilenameMismatch,
    InvalidSessionId,
    InvalidTransactionIdentity,
    InvalidAttemptIdentity,
    InvalidTimestamp,
    InvalidRuntimeProtocol,
    InvalidRuntimeOperation,
}

impl fmt::Display for HttpCheckpointDecodingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OversizedDocument { size, limit } => {
                write!(f, "checkpoint document too large: {size} bytes exceeds {limit} byte limit")
            }
            Self::Deserialization(_) => f.write_str("failed to deserialize checkpoint document"),
            Self::UnknownSchemaVersion { found } => {
                write!(f, "unknown checkpoint schema version: {found}")
            }
            Self::InvalidKey(_) => f.write_str("checkpoint logical key is invalid"),
            Self::InvalidKeyHash => f.write_str("checkpoint key hash is not valid lowercase hex SHA-256"),
            Self::KeyHashMismatch => f.write_str("checkpoint key hash does not match the logical key"),
            Self::FilenameMismatch => f.write_str("checkpoint filename does not match key hash"),
            Self::InvalidSessionId => f.write_str("checkpoint session identity is invalid"),
            Self::InvalidTransactionIdentity => {
                f.write_str("checkpoint transaction identity is invalid")
            }
            Self::InvalidAttemptIdentity => f.write_str("checkpoint attempt identity is invalid"),
            Self::InvalidTimestamp => f.write_str("checkpoint commit timestamp is invalid"),
            Self::InvalidRuntimeProtocol => f.write_str("checkpoint runtime protocol is unknown"),
            Self::InvalidRuntimeOperation => f.write_str("checkpoint runtime operation is unknown"),
        }
    }
}

impl std::error::Error for HttpCheckpointDecodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Deserialization(e) => Some(e),
            Self::InvalidKey(e) => Some(e),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// HttpCheckpointAdmissionError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HttpCheckpointAdmissionError {
    ManagedPath(HttpManagedPathError),
    PathLayoutInvalid,
    SymlinkRejected,
    NotRegularFile,
    Read(std::io::Error),
    Decoding(HttpCheckpointDecodingError),
    ProjectMismatch,
    RuntimeProtocolMismatch,
    RuntimeOperationMismatch,
    SessionMismatch,
    TransactionMismatch,
    AttemptMismatch,
    KeyMismatch,
    KeyHashMismatch,
    SessionRecord(SessionDecodingError),
    SessionProjectMismatch,
    SessionRuntimeMismatch,
    SessionNotStarted,
    ReferencedTransaction(HttpTransactionAdmissionError),
    TransactionSessionMismatch,
    TransactionKeyMismatch,
    TransactionNotResponse,
}

impl fmt::Display for HttpCheckpointAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManagedPath(_) => f.write_str("checkpoint path is outside the trusted root"),
            Self::PathLayoutInvalid => {
                f.write_str("checkpoint path does not match expected sessions/<id>/checkpoints/<hash>.json layout")
            }
            Self::SymlinkRejected => f.write_str("checkpoint path cannot contain symlinks"),
            Self::NotRegularFile => f.write_str("checkpoint path must be a regular file"),
            Self::Read(_) => f.write_str("failed to read checkpoint file"),
            Self::Decoding(_) => f.write_str("failed to decode checkpoint document"),
            Self::ProjectMismatch => f.write_str("checkpoint project identity does not match"),
            Self::RuntimeProtocolMismatch => {
                f.write_str("checkpoint runtime protocol does not match")
            }
            Self::RuntimeOperationMismatch => {
                f.write_str("checkpoint runtime operation does not match")
            }
            Self::SessionMismatch => f.write_str("checkpoint session identity does not match"),
            Self::TransactionMismatch => {
                f.write_str("checkpoint transaction identity does not match")
            }
            Self::AttemptMismatch => f.write_str("checkpoint attempt identity does not match"),
            Self::KeyMismatch => f.write_str("checkpoint logical key does not match"),
            Self::KeyHashMismatch => f.write_str("checkpoint key hash does not match"),
            Self::SessionRecord(_) => f.write_str("failed to decode checkpoint session record"),
            Self::SessionProjectMismatch => {
                f.write_str("checkpoint session record project does not match")
            }
            Self::SessionRuntimeMismatch => {
                f.write_str("checkpoint session record runtime does not match")
            }
            Self::SessionNotStarted => {
                f.write_str("checkpoint session record never reached the Running state")
            }
            Self::ReferencedTransaction(_) => {
                f.write_str("checkpoint referenced transaction admission failed")
            }
            Self::TransactionSessionMismatch => {
                f.write_str("checkpoint referenced transaction session does not match")
            }
            Self::TransactionKeyMismatch => {
                f.write_str("checkpoint referenced transaction logical key does not match")
            }
            Self::TransactionNotResponse => {
                f.write_str("checkpoint referenced transaction is not an HTTP response")
            }
        }
    }
}

impl std::error::Error for HttpCheckpointAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManagedPath(e) => Some(e),
            Self::Read(e) => Some(e),
            Self::Decoding(e) => Some(e),
            Self::SessionRecord(e) => Some(e),
            Self::ReferencedTransaction(e) => Some(e),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// HttpCheckpointCommitError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HttpCheckpointCommitError {
    UnmanagedContext,
    InvalidKey(HttpCheckpointKeyError),
    SessionValidation(crate::protocols::http::context::SessionValidationError),
    NoTransactionForKey,
    TransactionAdmission(HttpTransactionAdmissionError),
    TransactionSessionMismatch,
    TransactionKeyMismatch,
    TransactionNotResponse,
    ManagedPath(HttpManagedPathError),
    CheckpointDirectoryCreation(std::io::Error),
    Clock(crate::protocols::http::transaction::error::HttpClockError),
    Encoding(HttpCheckpointEncodingError),
    TempFileCreation(std::io::Error),
    Write(std::io::Error),
    FileSync(std::io::Error),
    AtomicPublication(std::io::Error),
    DirectorySync(std::io::Error),
    ExistingCorrupt(HttpCheckpointAdmissionError),
    ExistingIdentityMismatch,
    PartialCommit(Box<HttpCheckpointPartialCommitError>),
}

impl fmt::Display for HttpCheckpointCommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmanagedContext => f.write_str("checkpoint commit unavailable in unmanaged context"),
            Self::InvalidKey(_) => f.write_str("checkpoint key is invalid"),
            Self::SessionValidation(_) => f.write_str("session validation failed before checkpoint commit"),
            Self::NoTransactionForKey => f.write_str("no progress-published transaction found for this checkpoint key"),
            Self::TransactionAdmission(_) => f.write_str("checkpoint referenced transaction failed admission"),
            Self::TransactionSessionMismatch => {
                f.write_str("checkpoint referenced transaction belongs to a different session")
            }
            Self::TransactionKeyMismatch => {
                f.write_str("checkpoint referenced transaction has a different logical key")
            }
            Self::TransactionNotResponse => {
                f.write_str("checkpoint referenced transaction is not an HTTP response")
            }
            Self::ManagedPath(_) => f.write_str("checkpoint path validation failed"),
            Self::CheckpointDirectoryCreation(_) => {
                f.write_str("failed to create checkpoint directory")
            }
            Self::Clock(_) => f.write_str("failed to get timestamp for checkpoint commit"),
            Self::Encoding(_) => f.write_str("failed to encode checkpoint document"),
            Self::TempFileCreation(_) => f.write_str("failed to create checkpoint temporary file"),
            Self::Write(_) => f.write_str("failed to write checkpoint temporary file"),
            Self::FileSync(_) => f.write_str("failed to sync checkpoint file"),
            Self::AtomicPublication(_) => f.write_str("failed to atomically publish checkpoint file"),
            Self::DirectorySync(_) => f.write_str("checkpoint published but directory sync failed (partial commit)"),
            Self::ExistingCorrupt(_) => f.write_str("existing checkpoint at path is corrupt or incompatible"),
            Self::ExistingIdentityMismatch => {
                f.write_str("existing checkpoint at path has mismatched identity fields")
            }
            Self::PartialCommit(_) => {
                f.write_str("checkpoint was published but directory sync failed (partial commit)")
            }
        }
    }
}

impl std::error::Error for HttpCheckpointCommitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidKey(e) => Some(e),
            Self::SessionValidation(e) => Some(e),
            Self::TransactionAdmission(e) => Some(e),
            Self::ManagedPath(e) => Some(e),
            Self::CheckpointDirectoryCreation(e) => Some(e),
            Self::Clock(e) => Some(e),
            Self::Encoding(e) => Some(e),
            Self::TempFileCreation(e) => Some(e),
            Self::Write(e) => Some(e),
            Self::FileSync(e) => Some(e),
            Self::AtomicPublication(e) => Some(e),
            Self::DirectorySync(e) => Some(e),
            Self::ExistingCorrupt(e) => Some(e),
            Self::PartialCommit(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// HttpCheckpointPartialCommitError
// ---------------------------------------------------------------------------

/// Returned when the checkpoint file was published successfully but the
/// directory durability step failed.  The checkpoint may be discoverable
/// by a subsequent `has_checkpoint` call if the file survived.
#[derive(Debug)]
pub struct HttpCheckpointPartialCommitError {
    pub directory_sync_error: std::io::Error,
}

impl fmt::Display for HttpCheckpointPartialCommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("checkpoint published but checkpoint directory sync failed")
    }
}

impl std::error::Error for HttpCheckpointPartialCommitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.directory_sync_error)
    }
}

// ---------------------------------------------------------------------------
// HttpCheckpointLookupError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HttpCheckpointLookupError {
    UnmanagedContext,
    InvalidKey(HttpCheckpointKeyError),
    SessionEnumeration(std::io::Error),
    CorruptCandidate { session_id: String, source: Box<HttpCheckpointAdmissionError> },
}

impl fmt::Display for HttpCheckpointLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmanagedContext => {
                f.write_str("checkpoint lookup unavailable in unmanaged context")
            }
            Self::InvalidKey(_) => f.write_str("checkpoint lookup key is invalid"),
            Self::SessionEnumeration(_) => f.write_str("failed to enumerate session directories"),
            Self::CorruptCandidate { session_id, .. } => {
                write!(f, "corrupt or invalid checkpoint candidate in session '{session_id}'")
            }
        }
    }
}

impl std::error::Error for HttpCheckpointLookupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidKey(e) => Some(e),
            Self::SessionEnumeration(e) => Some(e),
            Self::CorruptCandidate { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
