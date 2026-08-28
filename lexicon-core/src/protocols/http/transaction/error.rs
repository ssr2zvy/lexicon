use std::fmt;
use std::path::{Path, PathBuf};
use std::{fs, io};

use super::identity::HttpTransactionIdentityError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpManagedPathValidationMode {
    ExistingDirectory,
    ExistingRegularFile,
    CreatableDirectory,
    CreatableRegularFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpManagedPathTargetType {
    Directory,
    RegularFile,
}

#[derive(Debug)]
pub enum HttpManagedPathError {
    RelativePath { path: PathBuf },
    PathOutsideTrustedRoot {
        trusted_root: PathBuf,
        target_path: PathBuf,
    },
    ComponentInspection {
        path: PathBuf,
        source: io::Error,
    },
    Symlink { path: PathBuf },
    NonDirectoryAncestor { path: PathBuf },
    MissingTarget { path: PathBuf },
    WrongTargetType {
        path: PathBuf,
        expected: HttpManagedPathTargetType,
    },
    InvalidCreatableSuffixComponent { path: PathBuf },
}

impl fmt::Display for HttpManagedPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RelativePath { .. } => formatter.write_str("managed path must be absolute"),
            Self::PathOutsideTrustedRoot { .. } => {
                formatter.write_str("managed path is outside the trusted root")
            }
            Self::ComponentInspection { .. } => {
                formatter.write_str("failed to inspect managed path component")
            }
            Self::Symlink { .. } => formatter.write_str("managed path cannot contain symlinks"),
            Self::NonDirectoryAncestor { .. } => {
                formatter.write_str("managed path ancestor is not a directory")
            }
            Self::MissingTarget { .. } => formatter.write_str("managed path target is missing"),
            Self::WrongTargetType { expected, .. } => match expected {
                HttpManagedPathTargetType::Directory => {
                    formatter.write_str("managed path target must be a directory")
                }
                HttpManagedPathTargetType::RegularFile => {
                    formatter.write_str("managed path target must be a regular file")
                }
            },
            Self::InvalidCreatableSuffixComponent { .. } => {
                formatter.write_str("managed path creatable suffix is invalid")
            }
        }
    }
}

impl std::error::Error for HttpManagedPathError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ComponentInspection { source, .. } => Some(source),
            _ => None,
        }
    }
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
pub enum HttpMetadataPersistenceError {
    ManagedPath(HttpManagedPathError),
    TemporaryFile(io::Error),
    Write(io::Error),
    FileSync(io::Error),
    Persist(io::Error),
    DirectorySync(io::Error),
}

impl fmt::Display for HttpMetadataPersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("failed to persist HTTP metadata")
    }
}

impl std::error::Error for HttpMetadataPersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManagedPath(error) => Some(error),
            Self::TemporaryFile(error)
            | Self::Write(error)
            | Self::FileSync(error)
            | Self::Persist(error)
            | Self::DirectorySync(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum HttpIncompleteMarkerError {
    Clock(HttpClockError),
    MetadataEncoding(serde_json::Error),
    ManagedPath(HttpManagedPathError),
    TemporaryFile(io::Error),
    MetadataWrite(io::Error),
    MetadataFileSync(io::Error),
    AtomicMarkerPublication(io::Error),
    ResponseDirectorySync(io::Error),
}

impl fmt::Display for HttpIncompleteMarkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("failed to persist incomplete-response marker")
    }
}

impl std::error::Error for HttpIncompleteMarkerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Clock(error) => Some(error),
            Self::MetadataEncoding(error) => Some(error),
            Self::ManagedPath(error) => Some(error),
            Self::TemporaryFile(error)
            | Self::MetadataWrite(error)
            | Self::MetadataFileSync(error)
            | Self::AtomicMarkerPublication(error)
            | Self::ResponseDirectorySync(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub struct IncompleteHttpResponseFailure {
    pub stream_error: HttpBodyStreamingError,
    pub partial_body_sync_error: Option<io::Error>,
    pub marker_error: Option<HttpIncompleteMarkerError>,
    pub bytes_recorded: u64,
    pub partial_body_sha256: Option<String>,
}

impl fmt::Display for IncompleteHttpResponseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HTTP response body streaming failed")
    }
}

impl std::error::Error for IncompleteHttpResponseFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.stream_error)
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
    ManagedPath(HttpManagedPathError),
    DirectoryCreation(io::Error),
    ExclusiveStagingCreation(io::Error),
    Clock(HttpClockError),
    IdentityAllocation(HttpTransactionIdentityAllocationError),
    MetadataEncoding(serde_json::Error),
    MetadataPersistence(HttpMetadataPersistenceError),
    BodyPersistence(io::Error),
    BodyStreaming(HttpBodyStreamingError),
    DurableSync(io::Error),
    Publication(HttpTransactionPublicationError),
    PostRenameSyncFailed(PostRenameSyncFailure),
    IncompleteResponseStreamingFailed(IncompleteHttpResponseFailure),
}

impl fmt::Display for HttpRecorderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManagedPath(_) => formatter.write_str("invalid managed transaction path"),
            Self::DirectoryCreation(_) => {
                formatter.write_str("failed to create transaction staging directory")
            }
            Self::ExclusiveStagingCreation(_) => {
                formatter.write_str("failed to exclusively create transaction staging directory")
            }
            Self::Clock(_) => formatter.write_str("failed to acquire HTTP transaction timestamp"),
            Self::IdentityAllocation(_) => {
                formatter.write_str("failed to allocate HTTP transaction identity")
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
            Self::Publication(_) => formatter.write_str("failed to finalize HTTP transaction"),
            Self::PostRenameSyncFailed(_) => {
                formatter.write_str("HTTP transaction published but raw-data parent sync failed")
            }
            Self::IncompleteResponseStreamingFailed(_) => formatter.write_str(
                "HTTP response body streaming failed while recording an incomplete response",
            ),
        }
    }
}

impl std::error::Error for HttpRecorderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManagedPath(error) => Some(error),
            Self::DirectoryCreation(error) => Some(error),
            Self::ExclusiveStagingCreation(error) => Some(error),
            Self::Clock(error) => Some(error),
            Self::IdentityAllocation(error) => Some(error),
            Self::MetadataEncoding(error) => Some(error),
            Self::MetadataPersistence(error) => Some(error),
            Self::BodyPersistence(error) => Some(error),
            Self::BodyStreaming(error) => Some(error),
            Self::DurableSync(error) => Some(error),
            Self::Publication(error) => Some(error),
            Self::PostRenameSyncFailed(error) => Some(error),
            Self::IncompleteResponseStreamingFailed(error) => Some(error),
        }
    }
}

pub(crate) fn validate_managed_path(
    trusted_root: &Path,
    target_path: &Path,
    mode: HttpManagedPathValidationMode,
) -> Result<(), HttpManagedPathError> {
    if trusted_root.is_relative() {
        return Err(HttpManagedPathError::RelativePath {
            path: trusted_root.to_path_buf(),
        });
    }
    if target_path.is_relative() {
        return Err(HttpManagedPathError::RelativePath {
            path: target_path.to_path_buf(),
        });
    }

    validate_existing_component_chain(trusted_root, true)?;
    if !target_path.starts_with(trusted_root) {
        return Err(HttpManagedPathError::PathOutsideTrustedRoot {
            trusted_root: trusted_root.to_path_buf(),
            target_path: target_path.to_path_buf(),
        });
    }

    let relative_suffix = target_path
        .strip_prefix(trusted_root)
        .map_err(|_| HttpManagedPathError::PathOutsideTrustedRoot {
            trusted_root: trusted_root.to_path_buf(),
            target_path: target_path.to_path_buf(),
        })?;
    let suffix_components: Vec<_> = relative_suffix.components().collect();

    let creatable_mode = matches!(
        mode,
        HttpManagedPathValidationMode::CreatableDirectory
            | HttpManagedPathValidationMode::CreatableRegularFile
    );
    if creatable_mode {
        let mut invalid_component_path = trusted_root.to_path_buf();
        for component in &suffix_components {
            invalid_component_path.push(component.as_os_str());
            if !matches!(component, std::path::Component::Normal(_)) {
                return Err(HttpManagedPathError::InvalidCreatableSuffixComponent {
                    path: invalid_component_path,
                });
            }
        }
    }

    let mut current = trusted_root.to_path_buf();
    let mut missing_at: Option<usize> = None;
    for (index, component) in suffix_components.iter().enumerate() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(HttpManagedPathError::Symlink {
                        path: current.clone(),
                    });
                }
                let is_target = index + 1 == suffix_components.len();
                if !is_target && !metadata.is_dir() {
                    return Err(HttpManagedPathError::NonDirectoryAncestor {
                        path: current.clone(),
                    });
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing_at = Some(index);
                break;
            }
            Err(source) if source.kind() == io::ErrorKind::NotADirectory => {
                return Err(HttpManagedPathError::NonDirectoryAncestor {
                    path: current.clone(),
                })
            }
            Err(source) => {
                return Err(HttpManagedPathError::ComponentInspection {
                    path: current.clone(),
                    source,
                })
            }
        }
    }

    match mode {
        HttpManagedPathValidationMode::ExistingDirectory => {
            ensure_existing_target_type(target_path, HttpManagedPathTargetType::Directory)
        }
        HttpManagedPathValidationMode::ExistingRegularFile => {
            ensure_existing_target_type(target_path, HttpManagedPathTargetType::RegularFile)
        }
        HttpManagedPathValidationMode::CreatableDirectory => {
            if missing_at.is_none() {
                ensure_existing_target_type(target_path, HttpManagedPathTargetType::Directory)
            } else {
                Ok(())
            }
        }
        HttpManagedPathValidationMode::CreatableRegularFile => {
            if missing_at.is_none() {
                ensure_existing_target_type(target_path, HttpManagedPathTargetType::RegularFile)
            } else {
                Ok(())
            }
        }
    }
}

fn validate_existing_component_chain(
    path: &Path,
    expect_directory: bool,
) -> Result<(), HttpManagedPathError> {
    let mut current = PathBuf::new();
    let components: Vec<_> = path.components().collect();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(HttpManagedPathError::Symlink {
                        path: current.clone(),
                    });
                }
                let is_target = index + 1 == components.len();
                if (!is_target || expect_directory) && !metadata.is_dir() {
                    return Err(HttpManagedPathError::NonDirectoryAncestor {
                        path: current.clone(),
                    });
                }
            }
            Err(source) if source.kind() == io::ErrorKind::NotADirectory => {
                return Err(HttpManagedPathError::NonDirectoryAncestor {
                    path: current.clone(),
                })
            }
            Err(source) => {
                return Err(HttpManagedPathError::ComponentInspection {
                    path: current.clone(),
                    source,
                })
            }
        }
    }

    Ok(())
}

fn ensure_existing_target_type(
    path: &Path,
    expected: HttpManagedPathTargetType,
) -> Result<(), HttpManagedPathError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(HttpManagedPathError::MissingTarget {
                path: path.to_path_buf(),
            })
        }
        Err(source) => {
            return Err(HttpManagedPathError::ComponentInspection {
                path: path.to_path_buf(),
                source,
            })
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(HttpManagedPathError::Symlink {
            path: path.to_path_buf(),
        });
    }
    let ok = match expected {
        HttpManagedPathTargetType::Directory => metadata.is_dir(),
        HttpManagedPathTargetType::RegularFile => metadata.is_file(),
    };
    if !ok {
        return Err(HttpManagedPathError::WrongTargetType {
            path: path.to_path_buf(),
            expected,
        });
    }
    Ok(())
}
