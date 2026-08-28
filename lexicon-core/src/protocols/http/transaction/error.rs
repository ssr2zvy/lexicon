use std::fmt;
use std::path::PathBuf;

/// The transaction identity and final path preserved when a post-rename sync fails.
#[derive(Debug)]
pub struct PostRenameSyncFailure {
    pub transaction_id: String,
    pub final_path: PathBuf,
    pub cause: std::io::Error,
}

impl fmt::Display for PostRenameSyncFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HTTP transaction directory was renamed but raw-data parent sync failed (partial commit)")
    }
}

impl std::error::Error for PostRenameSyncFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

#[derive(Debug)]
pub enum HttpRecorderError {
    InvalidManagedRoot,
    SymlinkRejected { path: PathBuf },
    DirectoryCreation(std::io::Error),
    ExclusiveStagingCreation(std::io::Error),
    MetadataEncoding(serde_json::Error),
    MetadataPersistence(std::io::Error),
    BodyPersistence(std::io::Error),
    BodyStreaming(std::io::Error),
    DurableSync(std::io::Error),
    AtomicFinalize(std::io::Error),
    FinalPublicationCollision,
    /// The staging directory was successfully renamed to the final directory, but the raw-data
    /// parent directory could not be synced. The transaction is a partial commit.
    PostRenameSyncFailed(PostRenameSyncFailure),
    /// An incomplete-response marker could not be persisted. Both the streaming error and this
    /// marker-write error are retained.
    IncompleteResponseMarkerFailed {
        stream_cause: std::io::Error,
        marker_cause: std::io::Error,
    },
}

impl fmt::Display for HttpRecorderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManagedRoot => formatter.write_str("invalid managed transaction root"),
            Self::SymlinkRejected { .. } => formatter.write_str("managed transaction path cannot be a symlink"),
            Self::DirectoryCreation(_) => formatter.write_str("failed to create transaction staging directory"),
            Self::ExclusiveStagingCreation(_) => formatter.write_str("failed to exclusively create transaction staging directory"),
            Self::MetadataEncoding(_) => formatter.write_str("failed to encode HTTP transaction metadata"),
            Self::MetadataPersistence(_) => formatter.write_str("failed to persist HTTP transaction metadata"),
            Self::BodyPersistence(_) => formatter.write_str("failed to persist HTTP body data"),
            Self::BodyStreaming(_) => formatter.write_str("failed to stream HTTP response body"),
            Self::DurableSync(_) => formatter.write_str("failed to durably sync HTTP transaction"),
            Self::AtomicFinalize(_) => formatter.write_str("failed to finalize HTTP transaction atomically"),
            Self::FinalPublicationCollision => formatter.write_str("final HTTP transaction path already exists"),
            Self::PostRenameSyncFailed(_) => formatter.write_str("HTTP transaction published but raw-data parent sync failed"),
            Self::IncompleteResponseMarkerFailed { .. } => formatter.write_str("HTTP response body streaming failed and incomplete-response marker write also failed"),
        }
    }
}

impl std::error::Error for HttpRecorderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DirectoryCreation(e) => Some(e),
            Self::ExclusiveStagingCreation(e) => Some(e),
            Self::MetadataEncoding(e) => Some(e),
            Self::MetadataPersistence(e) => Some(e),
            Self::BodyPersistence(e) => Some(e),
            Self::BodyStreaming(e) => Some(e),
            Self::DurableSync(e) => Some(e),
            Self::AtomicFinalize(e) => Some(e),
            Self::PostRenameSyncFailed(f) => Some(f),
            Self::IncompleteResponseMarkerFailed { stream_cause, .. } => Some(stream_cause),
            Self::InvalidManagedRoot
            | Self::SymlinkRejected { .. }
            | Self::FinalPublicationCollision => None,
        }
    }
}
