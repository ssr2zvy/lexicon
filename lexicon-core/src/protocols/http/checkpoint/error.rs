use std::fmt;

use crate::protocols::http::transaction::error::HttpManagedPathError;
use crate::protocols::http::transaction::metadata::HttpTransactionAdmissionError;
use crate::protocols::http::transaction::{
    HttpAttemptIdentityError, HttpLogicalRequestKeyError,
};
use crate::session::{SessionIdentity, SessionStoreError};

// ---------------------------------------------------------------------------
// HttpCheckpointKeyError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HttpCheckpointKeyError {
    InvalidKey(HttpLogicalRequestKeyError),
}

impl fmt::Display for HttpCheckpointKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey(_) => formatter.write_str("checkpoint logical key is invalid"),
        }
    }
}

impl std::error::Error for HttpCheckpointKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidKey(error) => Some(error),
        }
    }
}

// ---------------------------------------------------------------------------
// HttpCheckpointEncodingError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HttpCheckpointEncodingError {
    Serialization(serde_json::Error),
    OversizedDocument { size: usize, limit: usize },
}

impl fmt::Display for HttpCheckpointEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(_) => formatter.write_str("failed to serialize checkpoint document"),
            Self::OversizedDocument { .. } => {
                formatter.write_str("checkpoint document exceeds the maximum encoded size")
            }
        }
    }
}

impl std::error::Error for HttpCheckpointEncodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serialization(error) => Some(error),
            Self::OversizedDocument { .. } => None,
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
    InvalidProjectIdentity,
    InvalidRuntimeIdentity,
    InvalidSessionId,
    InvalidTransactionIdentity,
    InvalidAttemptIdentity(HttpAttemptIdentityError),
    InvalidTimestamp,
}

impl fmt::Display for HttpCheckpointDecodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OversizedDocument { .. } => {
                formatter.write_str("checkpoint document exceeds the maximum encoded size")
            }
            Self::Deserialization(_) => formatter.write_str("failed to deserialize checkpoint document"),
            Self::UnknownSchemaVersion { .. } => {
                formatter.write_str("checkpoint schema version is unsupported")
            }
            Self::InvalidKey(_) => formatter.write_str("checkpoint logical key is invalid"),
            Self::InvalidKeyHash => formatter.write_str("checkpoint key hash is invalid"),
            Self::KeyHashMismatch => {
                formatter.write_str("checkpoint key hash does not match the logical key")
            }
            Self::FilenameMismatch => {
                formatter.write_str("checkpoint filename does not match the logical key hash")
            }
            Self::InvalidProjectIdentity => formatter.write_str("checkpoint project identity is invalid"),
            Self::InvalidRuntimeIdentity => formatter.write_str("checkpoint runtime identity is invalid"),
            Self::InvalidSessionId => formatter.write_str("checkpoint session identity is invalid"),
            Self::InvalidTransactionIdentity => {
                formatter.write_str("checkpoint transaction identity is invalid")
            }
            Self::InvalidAttemptIdentity(_) => {
                formatter.write_str("checkpoint attempt identity is invalid")
            }
            Self::InvalidTimestamp => formatter.write_str("checkpoint timestamp is invalid"),
        }
    }
}

impl std::error::Error for HttpCheckpointDecodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Deserialization(error) => Some(error),
            Self::InvalidKey(error) => Some(error),
            Self::InvalidAttemptIdentity(error) => Some(error),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Session entry and transaction lookup helpers
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HttpCheckpointSessionEntryError {
    Metadata(std::io::Error),
    Symlink,
    NonUtf8Name,
    InvalidSessionIdentity,
    UnexpectedFile,
}

impl fmt::Display for HttpCheckpointSessionEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Metadata(_) => formatter.write_str("failed to inspect checkpoint session entry"),
            Self::Symlink => formatter.write_str("checkpoint session entry cannot be a symlink"),
            Self::NonUtf8Name => formatter.write_str("checkpoint session entry name is not valid UTF-8"),
            Self::InvalidSessionIdentity => {
                formatter.write_str("checkpoint session entry name is not a valid session identity")
            }
            Self::UnexpectedFile => formatter.write_str("checkpoint session entry has an invalid managed type"),
        }
    }
}

impl std::error::Error for HttpCheckpointSessionEntryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Metadata(error) => Some(error),
            Self::Symlink
            | Self::NonUtf8Name
            | Self::InvalidSessionIdentity
            | Self::UnexpectedFile => None,
        }
    }
}

#[derive(Debug)]
pub enum HttpCheckpointTransactionLookupError {
    RawRootEnumeration(std::io::Error),
    EntryMetadata(std::io::Error),
    EntrySymlink,
    EntryNameInvalid,
    MissingTransaction,
    AmbiguousTransaction,
    Admission(HttpTransactionAdmissionError),
}

impl fmt::Display for HttpCheckpointTransactionLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawRootEnumeration(_) => formatter.write_str("failed to enumerate raw transaction entries"),
            Self::EntryMetadata(_) => formatter.write_str("failed to inspect raw transaction entry"),
            Self::EntrySymlink => formatter.write_str("raw transaction entry cannot be a symlink"),
            Self::EntryNameInvalid => formatter.write_str("raw transaction entry name is invalid"),
            Self::MissingTransaction => formatter.write_str("checkpoint transaction could not be resolved"),
            Self::AmbiguousTransaction => {
                formatter.write_str("checkpoint transaction identity resolves to multiple entries")
            }
            Self::Admission(_) => formatter.write_str("checkpoint transaction admission failed"),
        }
    }
}

impl std::error::Error for HttpCheckpointTransactionLookupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RawRootEnumeration(error) => Some(error),
            Self::EntryMetadata(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::EntrySymlink
            | Self::EntryNameInvalid
            | Self::MissingTransaction
            | Self::AmbiguousTransaction => None,
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
    NotRegularFile,
    Read(std::io::Error),
    Decoding(HttpCheckpointDecodingError),
    ProjectMismatch,
    RuntimeMismatch,
    SessionMismatch,
    SessionStore(SessionStoreError),
    SessionProjectMismatch,
    SessionRuntimeMismatch,
    SessionNotRunning,
    TransactionLookup(HttpCheckpointTransactionLookupError),
    TransactionSessionMismatch,
    TransactionKeyMismatch,
    TransactionNotResponse,
    AttemptMismatch,
    TimestampBeforeTransaction,
    TimestampBeforeSessionStart,
    TimestampAfterSessionFinish,
}

impl fmt::Display for HttpCheckpointAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManagedPath(_) => formatter.write_str("checkpoint path is outside the trusted managed root"),
            Self::PathLayoutInvalid => formatter.write_str("checkpoint path layout is invalid"),
            Self::NotRegularFile => formatter.write_str("checkpoint path must be a regular file"),
            Self::Read(_) => formatter.write_str("failed to read checkpoint document"),
            Self::Decoding(_) => formatter.write_str("failed to decode checkpoint document"),
            Self::ProjectMismatch => formatter.write_str("checkpoint project identity does not match"),
            Self::RuntimeMismatch => formatter.write_str("checkpoint runtime identity does not match"),
            Self::SessionMismatch => formatter.write_str("checkpoint session identity does not match"),
            Self::SessionStore(_) => formatter.write_str("failed to load checkpoint session record"),
            Self::SessionProjectMismatch => {
                formatter.write_str("checkpoint session record project identity does not match")
            }
            Self::SessionRuntimeMismatch => {
                formatter.write_str("checkpoint session record runtime identity does not match")
            }
            Self::SessionNotRunning => {
                formatter.write_str("checkpoint session record never reached the running state")
            }
            Self::TransactionLookup(_) => formatter.write_str("failed to resolve checkpoint transaction"),
            Self::TransactionSessionMismatch => {
                formatter.write_str("checkpoint transaction belongs to a different session")
            }
            Self::TransactionKeyMismatch => {
                formatter.write_str("checkpoint transaction logical key does not match")
            }
            Self::TransactionNotResponse => {
                formatter.write_str("checkpoint transaction is not a completed response")
            }
            Self::AttemptMismatch => formatter.write_str("checkpoint attempt identity does not match"),
            Self::TimestampBeforeTransaction => {
                formatter.write_str("checkpoint timestamp precedes the recorded transaction completion")
            }
            Self::TimestampBeforeSessionStart => {
                formatter.write_str("checkpoint timestamp precedes the session start time")
            }
            Self::TimestampAfterSessionFinish => {
                formatter.write_str("checkpoint timestamp exceeds the terminal session finish time")
            }
        }
    }
}

impl std::error::Error for HttpCheckpointAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManagedPath(error) => Some(error),
            Self::Read(error) => Some(error),
            Self::Decoding(error) => Some(error),
            Self::SessionStore(error) => Some(error),
            Self::TransactionLookup(error) => Some(error),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Publication and partial commit errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HttpCheckpointPublicationError {
    Collision,
    Io(std::io::Error),
    UnsupportedPlatform,
    InvalidPathArgument,
}

impl fmt::Display for HttpCheckpointPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Collision => formatter.write_str("checkpoint destination already exists"),
            Self::Io(_) => formatter.write_str("failed to publish checkpoint"),
            Self::UnsupportedPlatform => {
                formatter.write_str("checkpoint publication is unsupported on this platform")
            }
            Self::InvalidPathArgument => formatter.write_str("checkpoint publication path is invalid"),
        }
    }
}

impl std::error::Error for HttpCheckpointPublicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Collision | Self::UnsupportedPlatform | Self::InvalidPathArgument => None,
        }
    }
}

#[derive(Debug)]
pub enum HttpCheckpointPostPublicationError {
    DirectorySync(std::io::Error),
    TemporaryCleanup(std::io::Error),
    CleanupDirectorySync(std::io::Error),
}

impl fmt::Display for HttpCheckpointPostPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectorySync(_) => {
                formatter.write_str("checkpoint published but directory sync failed")
            }
            Self::TemporaryCleanup(_) => {
                formatter.write_str("checkpoint published but temporary cleanup failed")
            }
            Self::CleanupDirectorySync(_) => formatter.write_str(
                "checkpoint published but cleanup durability sync failed",
            ),
        }
    }
}

impl std::error::Error for HttpCheckpointPostPublicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DirectorySync(error)
            | Self::TemporaryCleanup(error)
            | Self::CleanupDirectorySync(error) => Some(error),
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
    TransactionIdentityMismatch,
    TransactionKeyMismatch,
    TransactionSessionMismatch,
    TransactionNotResponse,
    ManagedPath(HttpManagedPathError),
    CheckpointDirectoryCreation(std::io::Error),
    Clock(crate::protocols::http::transaction::error::HttpClockError),
    TimestampInvariant,
    Encoding(HttpCheckpointEncodingError),
    TemporaryFileCreation(std::io::Error),
    TemporaryFileWrite(std::io::Error),
    TemporaryFileSync(std::io::Error),
    Publication(HttpCheckpointPublicationError),
    ExistingCorrupt(HttpCheckpointAdmissionError),
    ExistingIdentityMismatch,
    PartialCommit(Box<HttpCheckpointPartialCommit>),
}

impl fmt::Display for HttpCheckpointCommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmanagedContext => formatter.write_str("checkpoint commit is unavailable in unmanaged context"),
            Self::InvalidKey(_) => formatter.write_str("checkpoint logical key is invalid"),
            Self::SessionValidation(_) => formatter.write_str("checkpoint session validation failed"),
            Self::NoTransactionForKey => {
                formatter.write_str("no progress-published transaction exists for the checkpoint key")
            }
            Self::TransactionAdmission(_) => {
                formatter.write_str("checkpoint transaction admission failed")
            }
            Self::TransactionIdentityMismatch => {
                formatter.write_str("checkpoint registry transaction identity does not match disk state")
            }
            Self::TransactionKeyMismatch => {
                formatter.write_str("checkpoint transaction logical key does not match")
            }
            Self::TransactionSessionMismatch => {
                formatter.write_str("checkpoint transaction belongs to a different session")
            }
            Self::TransactionNotResponse => {
                formatter.write_str("checkpoint transaction is not a completed response")
            }
            Self::ManagedPath(_) => formatter.write_str("checkpoint managed path validation failed"),
            Self::CheckpointDirectoryCreation(_) => {
                formatter.write_str("failed to create checkpoint directory")
            }
            Self::Clock(_) => formatter.write_str("failed to acquire checkpoint timestamp"),
            Self::TimestampInvariant => {
                formatter.write_str("checkpoint timestamp does not satisfy session or transaction ordering")
            }
            Self::Encoding(_) => formatter.write_str("failed to encode checkpoint document"),
            Self::TemporaryFileCreation(_) => {
                formatter.write_str("failed to create checkpoint temporary file")
            }
            Self::TemporaryFileWrite(_) => {
                formatter.write_str("failed to write checkpoint temporary file")
            }
            Self::TemporaryFileSync(_) => {
                formatter.write_str("failed to sync checkpoint temporary file")
            }
            Self::Publication(_) => formatter.write_str("failed to publish checkpoint"),
            Self::ExistingCorrupt(_) => formatter.write_str("existing checkpoint is corrupt or incompatible"),
            Self::ExistingIdentityMismatch => {
                formatter.write_str("existing checkpoint identity does not match the requested commit")
            }
            Self::PartialCommit(_) => {
                formatter.write_str("checkpoint was published but post-publication cleanup failed")
            }
        }
    }
}

impl std::error::Error for HttpCheckpointCommitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidKey(error) => Some(error),
            Self::SessionValidation(error) => Some(error),
            Self::TransactionAdmission(error) => Some(error),
            Self::ManagedPath(error) => Some(error),
            Self::CheckpointDirectoryCreation(error) => Some(error),
            Self::Clock(error) => Some(error),
            Self::Encoding(error) => Some(error),
            Self::TemporaryFileCreation(error) => Some(error),
            Self::TemporaryFileWrite(error) => Some(error),
            Self::TemporaryFileSync(error) => Some(error),
            Self::Publication(error) => Some(error),
            Self::ExistingCorrupt(error) => Some(error),
            Self::PartialCommit(error) => Some(error.as_ref()),
            Self::UnmanagedContext
            | Self::NoTransactionForKey
            | Self::TransactionIdentityMismatch
            | Self::TransactionKeyMismatch
            | Self::TransactionSessionMismatch
            | Self::TransactionNotResponse
            | Self::TimestampInvariant
            | Self::ExistingIdentityMismatch => None,
        }
    }
}

#[derive(Debug)]
pub struct HttpCheckpointPartialCommit {
    checkpoint: super::model::CommittedHttpCheckpoint,
    source: HttpCheckpointPostPublicationError,
}

impl HttpCheckpointPartialCommit {
    pub(crate) fn new(
        checkpoint: super::model::CommittedHttpCheckpoint,
        source: HttpCheckpointPostPublicationError,
    ) -> Self {
        Self { checkpoint, source }
    }

    pub fn checkpoint(&self) -> &super::model::CommittedHttpCheckpoint {
        &self.checkpoint
    }

    pub fn checkpoint_path(&self) -> &std::path::Path {
        self.checkpoint.checkpoint_path()
    }

    pub fn key(&self) -> &crate::protocols::http::transaction::HttpLogicalRequestKey {
        self.checkpoint.key()
    }

    pub fn session(&self) -> &SessionIdentity {
        self.checkpoint.session()
    }

    pub fn transaction_identity(
        &self,
    ) -> &crate::protocols::http::transaction::HttpTransactionIdentity {
        self.checkpoint.transaction_identity()
    }

    pub fn attempt_identity(&self) -> &crate::protocols::http::transaction::HttpAttemptIdentity {
        self.checkpoint.attempt_identity()
    }
}

impl fmt::Display for HttpCheckpointPartialCommit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("checkpoint was published but post-publication cleanup failed")
    }
}

impl std::error::Error for HttpCheckpointPartialCommit {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

// ---------------------------------------------------------------------------
// HttpCheckpointLookupError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HttpCheckpointLookupError {
    UnmanagedContext,
    InvalidKey(HttpCheckpointKeyError),
    SessionValidation(crate::protocols::http::context::SessionValidationError),
    SessionEnumeration(std::io::Error),
    SessionEntry(HttpCheckpointSessionEntryError),
    CandidateAdmission {
        session: SessionIdentity,
        source: Box<HttpCheckpointAdmissionError>,
    },
}

impl fmt::Display for HttpCheckpointLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmanagedContext => formatter.write_str("checkpoint lookup is unavailable in unmanaged context"),
            Self::InvalidKey(_) => formatter.write_str("checkpoint lookup key is invalid"),
            Self::SessionValidation(_) => formatter.write_str("checkpoint lookup context validation failed"),
            Self::SessionEnumeration(_) => formatter.write_str("failed to enumerate session entries"),
            Self::SessionEntry(_) => formatter.write_str("checkpoint session entry is malformed"),
            Self::CandidateAdmission { .. } => {
                formatter.write_str("checkpoint candidate failed strict admission")
            }
        }
    }
}

impl std::error::Error for HttpCheckpointLookupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidKey(error) => Some(error),
            Self::SessionValidation(error) => Some(error),
            Self::SessionEnumeration(error) => Some(error),
            Self::SessionEntry(error) => Some(error),
            Self::CandidateAdmission { source, .. } => Some(source.as_ref()),
            Self::UnmanagedContext => None,
        }
    }
}

// ---------------------------------------------------------------------------
// HttpHistoricalLookupError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HttpHistoricalLookupError {
    UnmanagedContext,
    InvalidKey(HttpCheckpointKeyError),
    InvalidHeaderName,
    SessionValidation(crate::protocols::http::context::SessionValidationError),
    RawRootEnumeration(std::io::Error),
    ManagedEntryCorrupt(HttpCheckpointTransactionLookupError),
    TransactionAdmission(HttpTransactionAdmissionError),
    SessionStore(SessionStoreError),
    ProjectMismatch,
    RuntimeMismatch,
    SessionMismatch,
    TransactionKeyMismatch,
    TransactionNotResponse,
    NonUtf8HeaderValue,
    HeaderRedacted,
    DuplicateTransactionIdentity,
}

impl fmt::Display for HttpHistoricalLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmanagedContext => formatter.write_str("historical lookup is unavailable in unmanaged context"),
            Self::InvalidKey(_) => formatter.write_str("historical lookup key is invalid"),
            Self::InvalidHeaderName => formatter.write_str("historical lookup header name is invalid"),
            Self::SessionValidation(_) => {
                formatter.write_str("historical lookup context validation failed")
            }
            Self::RawRootEnumeration(_) => {
                formatter.write_str("failed to enumerate managed raw transaction entries")
            }
            Self::ManagedEntryCorrupt(_) => formatter.write_str("managed raw transaction entry is corrupt"),
            Self::TransactionAdmission(_) => formatter.write_str("managed raw transaction admission failed"),
            Self::SessionStore(_) => formatter.write_str("failed to load historical transaction session record"),
            Self::ProjectMismatch => formatter.write_str("historical transaction project identity does not match"),
            Self::RuntimeMismatch => formatter.write_str("historical transaction runtime identity does not match"),
            Self::SessionMismatch => formatter.write_str("historical transaction session identity does not match"),
            Self::TransactionKeyMismatch => formatter.write_str("historical transaction logical key does not match"),
            Self::TransactionNotResponse => formatter.write_str("historical transaction is not a response"),
            Self::NonUtf8HeaderValue => formatter.write_str("historical response header is not valid UTF-8"),
            Self::HeaderRedacted => formatter.write_str("historical response header is redacted"),
            Self::DuplicateTransactionIdentity => {
                formatter.write_str("historical lookup found duplicate managed transaction identities")
            }
        }
    }
}

impl std::error::Error for HttpHistoricalLookupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidKey(error) => Some(error),
            Self::SessionValidation(error) => Some(error),
            Self::RawRootEnumeration(error) => Some(error),
            Self::ManagedEntryCorrupt(error) => Some(error),
            Self::TransactionAdmission(error) => Some(error),
            Self::SessionStore(error) => Some(error),
            Self::UnmanagedContext
            | Self::InvalidHeaderName
            | Self::ProjectMismatch
            | Self::RuntimeMismatch
            | Self::SessionMismatch
            | Self::TransactionKeyMismatch
            | Self::TransactionNotResponse
            | Self::NonUtf8HeaderValue
            | Self::HeaderRedacted
            | Self::DuplicateTransactionIdentity => None,
        }
    }
}
