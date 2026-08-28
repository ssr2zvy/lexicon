use std::fmt;
use std::path::{Path, PathBuf};
use std::{fs, io};

use super::identity::HttpTransactionIdentityError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedPathKind {
    Directory,
    RegularFileIfPresent,
}

#[derive(Debug)]
pub enum HttpClockError {
    BeforeEpoch,
    OutOfRange,
}

impl fmt::Display for HttpClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeforeEpoch => formatter.write_str("system clock is before the Unix epoch"),
            Self::OutOfRange => formatter.write_str("system clock is outside the supported range"),
        }
    }
}

impl std::error::Error for HttpClockError {}

#[derive(Debug)]
pub enum HttpBodyStreamingError {
    Io(io::Error),
    LengthOverflow,
}

impl HttpBodyStreamingError {
    pub(crate) fn stable_class(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::LengthOverflow => "length_overflow",
        }
    }
}

impl fmt::Display for HttpBodyStreamingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("failed to stream HTTP response body"),
            Self::LengthOverflow => formatter.write_str("HTTP response body length overflowed"),
        }
    }
}

impl std::error::Error for HttpBodyStreamingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::LengthOverflow => None,
        }
    }
}

#[derive(Debug)]
pub enum HttpTransactionIdentityAllocationError {
    Clock(HttpClockError),
    Identity(HttpTransactionIdentityError),
    Exhausted,
}

impl fmt::Display for HttpTransactionIdentityAllocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clock(_) => formatter.write_str("failed to allocate HTTP transaction timestamp"),
            Self::Identity(_) => formatter.write_str("failed to allocate HTTP transaction identity"),
            Self::Exhausted => formatter.write_str("HTTP transaction identity allocation exhausted"),
        }
    }
}

impl std::error::Error for HttpTransactionIdentityAllocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Clock(error) => Some(error),
            Self::Identity(error) => Some(error),
            Self::Exhausted => None,
        }
    }
}

#[derive(Debug)]
pub enum HttpTransactionPublicationError {
    Io(io::Error),
    Collision,
    UnsupportedPlatform,
}

impl fmt::Display for HttpTransactionPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("failed to publish HTTP transaction"),
            Self::Collision => formatter.write_str("final HTTP transaction path already exists"),
            Self::UnsupportedPlatform => {
                formatter.write_str("HTTP transaction publication is unsupported on this platform")
            }
        }
    }
}

impl std::error::Error for HttpTransactionPublicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Collision | Self::UnsupportedPlatform => None,
        }
    }
}

#[derive(Debug)]
pub struct PostRenameSyncFailure {
    pub transaction_id: String,
    pub final_path: PathBuf,
    pub cause: io::Error,
}

impl fmt::Display for PostRenameSyncFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "HTTP transaction directory was renamed but raw-data parent sync failed (partial commit)",
        )
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
    ManagedPathInspection(io::Error),
    ManagedPathTypeRejected { path: PathBuf, expected: ManagedPathKind },
    DirectoryCreation(io::Error),
    ExclusiveStagingCreation(io::Error),
    Clock(HttpClockError),
    IdentityInvalid(HttpTransactionIdentityError),
    MetadataEncoding(serde_json::Error),
    MetadataPersistence(io::Error),
    BodyPersistence(io::Error),
    BodyStreaming(HttpBodyStreamingError),
    DurableSync(io::Error),
    AtomicFinalize(io::Error),
    FinalPublicationCollision,
    UnsupportedPlatformPublication,
    IdentityAllocationExhausted,
    PostRenameSyncFailed(PostRenameSyncFailure),
    IncompleteResponseMarkerFailed {
        stream_cause: HttpBodyStreamingError,
        marker_cause: io::Error,
    },
}

impl fmt::Display for HttpRecorderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManagedRoot => formatter.write_str("invalid managed transaction root"),
            Self::SymlinkRejected { .. } => {
                formatter.write_str("managed transaction path cannot be a symlink")
            }
            Self::ManagedPathInspection(_) => {
                formatter.write_str("failed to inspect managed transaction path")
            }
            Self::ManagedPathTypeRejected { expected, .. } => match expected {
                ManagedPathKind::Directory => {
                    formatter.write_str("managed transaction path must be a directory")
                }
                ManagedPathKind::RegularFileIfPresent => {
                    formatter.write_str("managed transaction path must be a regular file")
                }
            },
            Self::DirectoryCreation(_) => {
                formatter.write_str("failed to create transaction staging directory")
            }
            Self::ExclusiveStagingCreation(_) => {
                formatter.write_str("failed to exclusively create transaction staging directory")
            }
            Self::Clock(_) => formatter.write_str("failed to acquire HTTP transaction timestamp"),
            Self::IdentityInvalid(_) => {
                formatter.write_str("failed to validate generated HTTP transaction identity")
            }
            Self::MetadataEncoding(_) => {
                formatter.write_str("failed to encode HTTP transaction metadata")
            }
            Self::MetadataPersistence(_) => {
                formatter.write_str("failed to persist HTTP transaction metadata")
            }
            Self::BodyPersistence(_) => formatter.write_str("failed to persist HTTP body data"),
            Self::BodyStreaming(_) => formatter.write_str("failed to stream HTTP response body"),
            Self::DurableSync(_) => formatter.write_str("failed to durably sync HTTP transaction"),
            Self::AtomicFinalize(_) => {
                formatter.write_str("failed to finalize HTTP transaction atomically")
            }
            Self::FinalPublicationCollision => {
                formatter.write_str("final HTTP transaction path already exists")
            }
            Self::UnsupportedPlatformPublication => formatter.write_str(
                "HTTP transaction publication is unsupported on this platform",
            ),
            Self::IdentityAllocationExhausted => {
                formatter.write_str("HTTP transaction identity allocation exhausted")
            }
            Self::PostRenameSyncFailed(_) => {
                formatter.write_str("HTTP transaction published but raw-data parent sync failed")
            }
            Self::IncompleteResponseMarkerFailed { .. } => formatter.write_str(
                "HTTP response body streaming failed and incomplete-response marker write also failed",
            ),
        }
    }
}

impl std::error::Error for HttpRecorderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DirectoryCreation(error) => Some(error),
            Self::ExclusiveStagingCreation(error) => Some(error),
            Self::ManagedPathInspection(error) => Some(error),
            Self::Clock(error) => Some(error),
            Self::IdentityInvalid(error) => Some(error),
            Self::MetadataEncoding(error) => Some(error),
            Self::MetadataPersistence(error) => Some(error),
            Self::BodyPersistence(error) => Some(error),
            Self::BodyStreaming(error) => Some(error),
            Self::DurableSync(error) => Some(error),
            Self::AtomicFinalize(error) => Some(error),
            Self::PostRenameSyncFailed(error) => Some(error),
            Self::IncompleteResponseMarkerFailed { stream_cause, .. } => Some(stream_cause),
            Self::InvalidManagedRoot
            | Self::SymlinkRejected { .. }
            | Self::FinalPublicationCollision
            | Self::UnsupportedPlatformPublication
            | Self::IdentityAllocationExhausted
            | Self::ManagedPathTypeRejected { .. } => None,
        }
    }
}

pub(crate) fn validate_managed_path(
    path: &Path,
    expected_type: ManagedPathKind,
) -> Result<(), HttpRecorderError> {
    if path.is_relative() {
        return Err(HttpRecorderError::InvalidManagedRoot);
    }

    let mut current = PathBuf::new();
    let components: Vec<_> = path.components().collect();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(HttpRecorderError::SymlinkRejected {
                        path: current.clone(),
                    });
                }
                let is_target = index + 1 == components.len();
                if is_target {
                    match expected_type {
                        ManagedPathKind::Directory if !metadata.is_dir() => {
                            return Err(HttpRecorderError::ManagedPathTypeRejected {
                                path: current,
                                expected: expected_type,
                            });
                        }
                        ManagedPathKind::RegularFileIfPresent if !metadata.is_file() => {
                            return Err(HttpRecorderError::ManagedPathTypeRejected {
                                path: current,
                                expected: expected_type,
                            });
                        }
                        _ => {}
                    }
                } else if !metadata.is_dir() {
                    return Err(HttpRecorderError::ManagedPathTypeRejected {
                        path: current,
                        expected: ManagedPathKind::Directory,
                    });
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if index + 1 != components.len() {
                    return Ok(());
                }
            }
            Err(error) => return Err(HttpRecorderError::ManagedPathInspection(error)),
        }
    }

    Ok(())
}
