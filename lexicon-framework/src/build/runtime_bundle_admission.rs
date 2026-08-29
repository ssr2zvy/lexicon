use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use lexicon_core::processing::{
    ProcessingRuntimeCompatibilityError, ProcessingRuntimeInformationV1,
};
use lexicon_core::runtime::{RuntimeCompatibilityError, RuntimeIdentity, RuntimeInformationV1};

use super::{
    ExecutableSha256, HashedRuntimeArtifact, ProcessingRuntimeManifestDecodingError,
    ProcessingRuntimeManifestV1, RuntimeArtifactHashError, RuntimeManifestV1,
    hash_runtime_executable,
};

pub const MAX_RUNTIME_MANIFEST_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedHttpRuntimeBundle {
    directory: PathBuf,
    executable_path: PathBuf,
    manifest_path: PathBuf,
    manifest: RuntimeManifestV1,
    artifact: HashedRuntimeArtifact,
}

impl AdmittedHttpRuntimeBundle {
    pub fn directory(&self) -> &Path {
        self.directory.as_path()
    }

    pub fn executable_path(&self) -> &Path {
        self.executable_path.as_path()
    }

    pub fn manifest_path(&self) -> &Path {
        self.manifest_path.as_path()
    }

    pub fn manifest(&self) -> &RuntimeManifestV1 {
        &self.manifest
    }

    pub fn artifact(&self) -> &HashedRuntimeArtifact {
        &self.artifact
    }

    pub fn runtime_information(&self) -> &RuntimeInformationV1 {
        self.manifest.runtime_information()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedProcessingRuntimeBundle {
    directory: PathBuf,
    executable_path: PathBuf,
    manifest_path: PathBuf,
    manifest: ProcessingRuntimeManifestV1,
    artifact: HashedRuntimeArtifact,
}

impl AdmittedProcessingRuntimeBundle {
    pub fn directory(&self) -> &Path {
        self.directory.as_path()
    }

    pub fn executable_path(&self) -> &Path {
        self.executable_path.as_path()
    }

    pub fn manifest_path(&self) -> &Path {
        self.manifest_path.as_path()
    }

    pub fn manifest(&self) -> &ProcessingRuntimeManifestV1 {
        &self.manifest
    }

    pub fn artifact(&self) -> &HashedRuntimeArtifact {
        &self.artifact
    }

    pub fn runtime_information(&self) -> &ProcessingRuntimeInformationV1 {
        self.manifest.runtime_information()
    }
}

#[derive(Debug)]
pub enum RuntimeBundleAdmissionError {
    BundleMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    BundleIsSymlink {
        path: PathBuf,
    },
    BundleNotDirectory {
        path: PathBuf,
    },
    ManifestMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    ManifestIsSymlink {
        path: PathBuf,
    },
    ManifestNotRegularFile {
        path: PathBuf,
    },
    ManifestTooLarge {
        maximum: usize,
        actual: u64,
    },
    ReadManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidManifestBoundary,
    DecodeManifest(super::RuntimeManifestDecodingError),
    Incompatible(RuntimeCompatibilityError),
    /// Compatibility check failed using an owned identity (dynamic source name).
    IncompatibleOwned(String),
    ReadDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    UnexpectedDirectoryEntry {
        path: PathBuf,
    },
    MissingExecutable {
        path: PathBuf,
    },
    ExecutableIsSymlink {
        path: PathBuf,
    },
    ExecutableNotRegularFile {
        path: PathBuf,
    },
    HashExecutable(RuntimeArtifactHashError),
    ArtifactMismatch {
        expected_size: u64,
        actual_size: u64,
        expected_sha256: ExecutableSha256,
        actual_sha256: ExecutableSha256,
    },
}

impl fmt::Display for RuntimeBundleAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BundleMetadata { path, source } => {
                write!(
                    formatter,
                    "failed to stat bundle '{}': {source}",
                    path.display()
                )
            }
            Self::BundleIsSymlink { path } => {
                write!(
                    formatter,
                    "bundle path '{}' must not be a symlink",
                    path.display()
                )
            }
            Self::BundleNotDirectory { path } => {
                write!(
                    formatter,
                    "bundle path '{}' is not a directory",
                    path.display()
                )
            }
            Self::ManifestMetadata { path, source } => {
                write!(
                    formatter,
                    "failed to stat manifest '{}': {source}",
                    path.display()
                )
            }
            Self::ManifestIsSymlink { path } => {
                write!(
                    formatter,
                    "manifest '{}' must not be a symlink",
                    path.display()
                )
            }
            Self::ManifestNotRegularFile { path } => {
                write!(
                    formatter,
                    "manifest '{}' is not a regular file",
                    path.display()
                )
            }
            Self::ManifestTooLarge { maximum, actual } => {
                write!(
                    formatter,
                    "manifest exceeds the {} byte limit (actual: {actual})",
                    maximum
                )
            }
            Self::ReadManifest { path, source } => {
                write!(
                    formatter,
                    "failed to read manifest '{}': {source}",
                    path.display()
                )
            }
            Self::InvalidManifestBoundary => formatter
                .write_str("manifest does not match the required exact final newline boundary"),
            Self::DecodeManifest(error) => {
                write!(formatter, "failed to decode runtime manifest: {error}")
            }
            Self::Incompatible(error) => {
                write!(
                    formatter,
                    "runtime compatibility validation failed: {error}"
                )
            }
            Self::IncompatibleOwned(msg) => {
                write!(formatter, "runtime compatibility validation failed: {msg}")
            }
            Self::ReadDirectory { path, source } => {
                write!(
                    formatter,
                    "failed to read bundle directory '{}': {source}",
                    path.display()
                )
            }
            Self::UnexpectedDirectoryEntry { path } => {
                write!(
                    formatter,
                    "unexpected entry in bundle directory: {}",
                    path.display()
                )
            }
            Self::MissingExecutable { path } => {
                write!(
                    formatter,
                    "manifest-declared executable is missing: {}",
                    path.display()
                )
            }
            Self::ExecutableIsSymlink { path } => {
                write!(
                    formatter,
                    "manifest-declared executable '{}' is a symlink",
                    path.display()
                )
            }
            Self::ExecutableNotRegularFile { path } => {
                write!(
                    formatter,
                    "manifest-declared executable '{}' is not a regular file",
                    path.display()
                )
            }
            Self::HashExecutable(error) => {
                write!(formatter, "failed to hash executable: {error}")
            }
            Self::ArtifactMismatch {
                expected_size,
                actual_size,
                expected_sha256,
                actual_sha256,
            } => {
                write!(
                    formatter,
                    "runtime executable digest mismatch: expected size={} sha256={} actual size={} sha256={}",
                    expected_size, expected_sha256, actual_size, actual_sha256
                )
            }
        }
    }
}

impl std::error::Error for RuntimeBundleAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BundleMetadata { source, .. }
            | Self::ManifestMetadata { source, .. }
            | Self::ReadManifest { source, .. }
            | Self::ReadDirectory { source, .. } => Some(source),
            Self::DecodeManifest(error) => Some(error),
            Self::Incompatible(error) => Some(error),
            Self::HashExecutable(error) => Some(error),
            Self::BundleIsSymlink { .. }
            | Self::BundleNotDirectory { .. }
            | Self::ManifestIsSymlink { .. }
            | Self::ManifestNotRegularFile { .. }
            | Self::ManifestTooLarge { .. }
            | Self::InvalidManifestBoundary
            | Self::UnexpectedDirectoryEntry { .. }
            | Self::MissingExecutable { .. }
            | Self::ExecutableIsSymlink { .. }
            | Self::ExecutableNotRegularFile { .. }
            | Self::ArtifactMismatch { .. }
            | Self::IncompatibleOwned(..) => None,
        }
    }
}

#[derive(Debug)]
pub enum ProcessingRuntimeBundleAdmissionError {
    BundleMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    BundleIsSymlink {
        path: PathBuf,
    },
    BundleNotDirectory {
        path: PathBuf,
    },
    ManifestMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    ManifestIsSymlink {
        path: PathBuf,
    },
    ManifestNotRegularFile {
        path: PathBuf,
    },
    ManifestTooLarge {
        maximum: usize,
        actual: u64,
    },
    ReadManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidManifestBoundary,
    DecodeManifest(ProcessingRuntimeManifestDecodingError),
    Incompatible(ProcessingRuntimeCompatibilityError),
    /// Compatibility check failed using an owned identity (dynamic source name).
    IncompatibleOwned(String),
    ReadDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    UnexpectedDirectoryEntry {
        path: PathBuf,
    },
    MissingExecutable {
        path: PathBuf,
    },
    ExecutableIsSymlink {
        path: PathBuf,
    },
    ExecutableNotRegularFile {
        path: PathBuf,
    },
    HashExecutable(RuntimeArtifactHashError),
    ArtifactMismatch {
        expected_size: u64,
        actual_size: u64,
        expected_sha256: ExecutableSha256,
        actual_sha256: ExecutableSha256,
    },
}

impl fmt::Display for ProcessingRuntimeBundleAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BundleMetadata { path, source } => {
                write!(
                    formatter,
                    "failed to stat bundle '{}': {source}",
                    path.display()
                )
            }
            Self::BundleIsSymlink { path } => {
                write!(
                    formatter,
                    "bundle path '{}' must not be a symlink",
                    path.display()
                )
            }
            Self::BundleNotDirectory { path } => {
                write!(
                    formatter,
                    "bundle path '{}' is not a directory",
                    path.display()
                )
            }
            Self::ManifestMetadata { path, source } => {
                write!(
                    formatter,
                    "failed to stat manifest '{}': {source}",
                    path.display()
                )
            }
            Self::ManifestIsSymlink { path } => {
                write!(
                    formatter,
                    "manifest '{}' must not be a symlink",
                    path.display()
                )
            }
            Self::ManifestNotRegularFile { path } => {
                write!(
                    formatter,
                    "manifest '{}' is not a regular file",
                    path.display()
                )
            }
            Self::ManifestTooLarge { maximum, actual } => {
                write!(
                    formatter,
                    "manifest exceeds the {} byte limit (actual: {actual})",
                    maximum
                )
            }
            Self::ReadManifest { path, source } => {
                write!(
                    formatter,
                    "failed to read manifest '{}': {source}",
                    path.display()
                )
            }
            Self::InvalidManifestBoundary => formatter
                .write_str("manifest does not match the required exact final newline boundary"),
            Self::DecodeManifest(error) => {
                write!(
                    formatter,
                    "failed to decode processing runtime manifest: {error}"
                )
            }
            Self::Incompatible(error) => {
                write!(
                    formatter,
                    "processing runtime compatibility validation failed: {error}"
                )
            }
            Self::IncompatibleOwned(msg) => {
                write!(
                    formatter,
                    "processing runtime compatibility validation failed: {msg}"
                )
            }
            Self::ReadDirectory { path, source } => {
                write!(
                    formatter,
                    "failed to read bundle directory '{}': {source}",
                    path.display()
                )
            }
            Self::UnexpectedDirectoryEntry { path } => {
                write!(
                    formatter,
                    "unexpected entry in bundle directory: {}",
                    path.display()
                )
            }
            Self::MissingExecutable { path } => {
                write!(
                    formatter,
                    "manifest-declared executable is missing: {}",
                    path.display()
                )
            }
            Self::ExecutableIsSymlink { path } => {
                write!(
                    formatter,
                    "manifest-declared executable '{}' is a symlink",
                    path.display()
                )
            }
            Self::ExecutableNotRegularFile { path } => {
                write!(
                    formatter,
                    "manifest-declared executable '{}' is not a regular file",
                    path.display()
                )
            }
            Self::HashExecutable(error) => {
                write!(formatter, "failed to hash executable: {error}")
            }
            Self::ArtifactMismatch {
                expected_size,
                actual_size,
                expected_sha256,
                actual_sha256,
            } => {
                write!(
                    formatter,
                    "processing runtime executable digest mismatch: expected size={} sha256={} actual size={} sha256={}",
                    expected_size, expected_sha256, actual_size, actual_sha256
                )
            }
        }
    }
}

impl std::error::Error for ProcessingRuntimeBundleAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BundleMetadata { source, .. }
            | Self::ManifestMetadata { source, .. }
            | Self::ReadManifest { source, .. }
            | Self::ReadDirectory { source, .. } => Some(source),
            Self::DecodeManifest(error) => Some(error),
            Self::Incompatible(error) => Some(error),
            Self::HashExecutable(error) => Some(error),
            Self::BundleIsSymlink { .. }
            | Self::BundleNotDirectory { .. }
            | Self::ManifestIsSymlink { .. }
            | Self::ManifestNotRegularFile { .. }
            | Self::ManifestTooLarge { .. }
            | Self::InvalidManifestBoundary
            | Self::IncompatibleOwned(_)
            | Self::UnexpectedDirectoryEntry { .. }
            | Self::MissingExecutable { .. }
            | Self::ExecutableIsSymlink { .. }
            | Self::ExecutableNotRegularFile { .. }
            | Self::ArtifactMismatch { .. } => None,
        }
    }
}

pub fn admit_http_runtime_bundle(
    bundle_directory: &Path,
    expected_identity: RuntimeIdentity,
) -> Result<AdmittedHttpRuntimeBundle, RuntimeBundleAdmissionError> {
    let bundle_metadata = fs::symlink_metadata(bundle_directory).map_err(|source| {
        RuntimeBundleAdmissionError::BundleMetadata {
            path: bundle_directory.to_path_buf(),
            source,
        }
    })?;
    if bundle_metadata.file_type().is_symlink() {
        return Err(RuntimeBundleAdmissionError::BundleIsSymlink {
            path: bundle_directory.to_path_buf(),
        });
    }
    if !bundle_metadata.is_dir() {
        return Err(RuntimeBundleAdmissionError::BundleNotDirectory {
            path: bundle_directory.to_path_buf(),
        });
    }

    let manifest_path = bundle_directory.join("runtime.json");
    let manifest_metadata = fs::symlink_metadata(&manifest_path).map_err(|source| {
        RuntimeBundleAdmissionError::ManifestMetadata {
            path: manifest_path.clone(),
            source,
        }
    })?;
    if manifest_metadata.file_type().is_symlink() {
        return Err(RuntimeBundleAdmissionError::ManifestIsSymlink {
            path: manifest_path.clone(),
        });
    }
    if !manifest_metadata.is_file() {
        return Err(RuntimeBundleAdmissionError::ManifestNotRegularFile {
            path: manifest_path.clone(),
        });
    }
    if manifest_metadata.len() > MAX_RUNTIME_MANIFEST_BYTES as u64 {
        return Err(RuntimeBundleAdmissionError::ManifestTooLarge {
            maximum: MAX_RUNTIME_MANIFEST_BYTES,
            actual: manifest_metadata.len(),
        });
    }

    let manifest_bytes =
        fs::read(&manifest_path).map_err(|source| RuntimeBundleAdmissionError::ReadManifest {
            path: manifest_path.clone(),
            source,
        })?;
    let manifest_text = validate_manifest_text(&manifest_bytes).map_err(|error| match error {
        ManifestBoundaryError::InvalidBoundary => {
            RuntimeBundleAdmissionError::InvalidManifestBoundary
        }
        ManifestBoundaryError::TooLarge { maximum, actual } => {
            RuntimeBundleAdmissionError::ManifestTooLarge { maximum, actual }
        }
    })?;
    let manifest = RuntimeManifestV1::from_json(manifest_text)
        .map_err(RuntimeBundleAdmissionError::DecodeManifest)?;

    manifest
        .runtime_information()
        .validate_compatibility(expected_identity)
        .map_err(RuntimeBundleAdmissionError::Incompatible)?;

    let executable_path = bundle_directory.join(manifest.executable_name());
    if executable_path.parent().map(Path::new) != Some(bundle_directory) {
        return Err(RuntimeBundleAdmissionError::UnexpectedDirectoryEntry {
            path: executable_path.clone(),
        });
    }

    let executable_metadata = match fs::symlink_metadata(&executable_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(RuntimeBundleAdmissionError::MissingExecutable {
                path: executable_path.clone(),
            });
        }
        Err(error) => {
            return Err(RuntimeBundleAdmissionError::ReadManifest {
                path: executable_path.clone(),
                source: error,
            });
        }
    };
    if executable_metadata.file_type().is_symlink() {
        return Err(RuntimeBundleAdmissionError::ExecutableIsSymlink {
            path: executable_path.clone(),
        });
    }
    if !executable_metadata.is_file() {
        return Err(RuntimeBundleAdmissionError::ExecutableNotRegularFile {
            path: executable_path.clone(),
        });
    }

    let entries = fs::read_dir(bundle_directory).map_err(|source| {
        RuntimeBundleAdmissionError::ReadDirectory {
            path: bundle_directory.to_path_buf(),
            source,
        }
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| RuntimeBundleAdmissionError::ReadDirectory {
            path: bundle_directory.to_path_buf(),
            source,
        })?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if name == "runtime.json" || name == manifest.executable_name() {
            continue;
        }
        return Err(RuntimeBundleAdmissionError::UnexpectedDirectoryEntry { path: entry.path() });
    }

    let artifact = hash_runtime_executable(&executable_path)
        .map_err(RuntimeBundleAdmissionError::HashExecutable)?;
    let expected_sha256 = manifest.executable_sha256();
    let actual_sha256 = ExecutableSha256::from_hex(artifact.sha256()).unwrap();
    if manifest.executable_size() != artifact.size() || expected_sha256 != actual_sha256 {
        return Err(RuntimeBundleAdmissionError::ArtifactMismatch {
            expected_size: manifest.executable_size(),
            actual_size: artifact.size(),
            expected_sha256,
            actual_sha256,
        });
    }

    Ok(AdmittedHttpRuntimeBundle {
        directory: bundle_directory.to_path_buf(),
        executable_path: executable_path.clone(),
        manifest_path: manifest_path.clone(),
        manifest,
        artifact,
    })
}

pub fn admit_processing_runtime_bundle(
    bundle_directory: &Path,
    expected_identity: RuntimeIdentity,
) -> Result<AdmittedProcessingRuntimeBundle, ProcessingRuntimeBundleAdmissionError> {
    let bundle_metadata = fs::symlink_metadata(bundle_directory).map_err(|source| {
        ProcessingRuntimeBundleAdmissionError::BundleMetadata {
            path: bundle_directory.to_path_buf(),
            source,
        }
    })?;
    if bundle_metadata.file_type().is_symlink() {
        return Err(ProcessingRuntimeBundleAdmissionError::BundleIsSymlink {
            path: bundle_directory.to_path_buf(),
        });
    }
    if !bundle_metadata.is_dir() {
        return Err(ProcessingRuntimeBundleAdmissionError::BundleNotDirectory {
            path: bundle_directory.to_path_buf(),
        });
    }

    let manifest_path = bundle_directory.join("runtime.json");
    let manifest_metadata = fs::symlink_metadata(&manifest_path).map_err(|source| {
        ProcessingRuntimeBundleAdmissionError::ManifestMetadata {
            path: manifest_path.clone(),
            source,
        }
    })?;
    if manifest_metadata.file_type().is_symlink() {
        return Err(ProcessingRuntimeBundleAdmissionError::ManifestIsSymlink {
            path: manifest_path.clone(),
        });
    }
    if !manifest_metadata.is_file() {
        return Err(
            ProcessingRuntimeBundleAdmissionError::ManifestNotRegularFile {
                path: manifest_path.clone(),
            },
        );
    }
    if manifest_metadata.len() > MAX_RUNTIME_MANIFEST_BYTES as u64 {
        return Err(ProcessingRuntimeBundleAdmissionError::ManifestTooLarge {
            maximum: MAX_RUNTIME_MANIFEST_BYTES,
            actual: manifest_metadata.len(),
        });
    }

    let manifest_bytes = fs::read(&manifest_path).map_err(|source| {
        ProcessingRuntimeBundleAdmissionError::ReadManifest {
            path: manifest_path.clone(),
            source,
        }
    })?;
    let manifest_text = validate_manifest_text(&manifest_bytes).map_err(|error| match error {
        ManifestBoundaryError::InvalidBoundary => {
            ProcessingRuntimeBundleAdmissionError::InvalidManifestBoundary
        }
        ManifestBoundaryError::TooLarge { maximum, actual } => {
            ProcessingRuntimeBundleAdmissionError::ManifestTooLarge { maximum, actual }
        }
    })?;
    let manifest = ProcessingRuntimeManifestV1::from_json(manifest_text)
        .map_err(ProcessingRuntimeBundleAdmissionError::DecodeManifest)?;

    manifest
        .runtime_information()
        .validate_compatibility(expected_identity)
        .map_err(ProcessingRuntimeBundleAdmissionError::Incompatible)?;

    let executable_path = bundle_directory.join(manifest.executable_name());
    if executable_path.parent().map(Path::new) != Some(bundle_directory) {
        return Err(
            ProcessingRuntimeBundleAdmissionError::UnexpectedDirectoryEntry {
                path: executable_path.clone(),
            },
        );
    }

    let executable_metadata = match fs::symlink_metadata(&executable_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ProcessingRuntimeBundleAdmissionError::MissingExecutable {
                path: executable_path.clone(),
            });
        }
        Err(error) => {
            return Err(ProcessingRuntimeBundleAdmissionError::ReadManifest {
                path: executable_path.clone(),
                source: error,
            });
        }
    };
    if executable_metadata.file_type().is_symlink() {
        return Err(ProcessingRuntimeBundleAdmissionError::ExecutableIsSymlink {
            path: executable_path.clone(),
        });
    }
    if !executable_metadata.is_file() {
        return Err(
            ProcessingRuntimeBundleAdmissionError::ExecutableNotRegularFile {
                path: executable_path.clone(),
            },
        );
    }

    let entries = fs::read_dir(bundle_directory).map_err(|source| {
        ProcessingRuntimeBundleAdmissionError::ReadDirectory {
            path: bundle_directory.to_path_buf(),
            source,
        }
    })?;
    for entry in entries {
        let entry =
            entry.map_err(
                |source| ProcessingRuntimeBundleAdmissionError::ReadDirectory {
                    path: bundle_directory.to_path_buf(),
                    source,
                },
            )?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if name == "runtime.json" || name == manifest.executable_name() {
            continue;
        }
        return Err(
            ProcessingRuntimeBundleAdmissionError::UnexpectedDirectoryEntry { path: entry.path() },
        );
    }

    let artifact = hash_runtime_executable(&executable_path)
        .map_err(ProcessingRuntimeBundleAdmissionError::HashExecutable)?;
    let expected_sha256 = manifest.executable_sha256();
    let actual_sha256 = ExecutableSha256::from_hex(artifact.sha256()).unwrap();
    if manifest.executable_size() != artifact.size() || expected_sha256 != actual_sha256 {
        return Err(ProcessingRuntimeBundleAdmissionError::ArtifactMismatch {
            expected_size: manifest.executable_size(),
            actual_size: artifact.size(),
            expected_sha256,
            actual_sha256,
        });
    }

    Ok(AdmittedProcessingRuntimeBundle {
        directory: bundle_directory.to_path_buf(),
        executable_path: executable_path.clone(),
        manifest_path: manifest_path.clone(),
        manifest,
        artifact,
    })
}

/// Variant of [`admit_http_runtime_bundle`] that accepts an `OwnedRuntimeIdentity`.
///
/// Reads, validates, and hashes the bundle using the same logic as the static-identity
/// variant, but performs the compatibility check against an owned identity so that
/// dynamic source names can be used without `Box::leak`.
pub fn admit_http_runtime_bundle_owned(
    bundle_directory: &Path,
    expected_identity: &lexicon_core::runtime::OwnedRuntimeIdentity,
) -> Result<AdmittedHttpRuntimeBundle, RuntimeBundleAdmissionError> {
    let bundle_metadata = fs::symlink_metadata(bundle_directory).map_err(|source| {
        RuntimeBundleAdmissionError::BundleMetadata {
            path: bundle_directory.to_path_buf(),
            source,
        }
    })?;
    if bundle_metadata.file_type().is_symlink() {
        return Err(RuntimeBundleAdmissionError::BundleIsSymlink {
            path: bundle_directory.to_path_buf(),
        });
    }
    if !bundle_metadata.is_dir() {
        return Err(RuntimeBundleAdmissionError::BundleNotDirectory {
            path: bundle_directory.to_path_buf(),
        });
    }

    let manifest_path = bundle_directory.join("runtime.json");
    let manifest_metadata = fs::symlink_metadata(&manifest_path).map_err(|source| {
        RuntimeBundleAdmissionError::ManifestMetadata {
            path: manifest_path.clone(),
            source,
        }
    })?;
    if manifest_metadata.file_type().is_symlink() {
        return Err(RuntimeBundleAdmissionError::ManifestIsSymlink {
            path: manifest_path.clone(),
        });
    }
    if !manifest_metadata.is_file() {
        return Err(RuntimeBundleAdmissionError::ManifestNotRegularFile {
            path: manifest_path.clone(),
        });
    }
    if manifest_metadata.len() > MAX_RUNTIME_MANIFEST_BYTES as u64 {
        return Err(RuntimeBundleAdmissionError::ManifestTooLarge {
            maximum: MAX_RUNTIME_MANIFEST_BYTES,
            actual: manifest_metadata.len(),
        });
    }

    let manifest_bytes =
        fs::read(&manifest_path).map_err(|source| RuntimeBundleAdmissionError::ReadManifest {
            path: manifest_path.clone(),
            source,
        })?;
    let manifest_text = validate_manifest_text(&manifest_bytes).map_err(|error| match error {
        ManifestBoundaryError::InvalidBoundary => {
            RuntimeBundleAdmissionError::InvalidManifestBoundary
        }
        ManifestBoundaryError::TooLarge { maximum, actual } => {
            RuntimeBundleAdmissionError::ManifestTooLarge { maximum, actual }
        }
    })?;
    let manifest = RuntimeManifestV1::from_json(manifest_text)
        .map_err(RuntimeBundleAdmissionError::DecodeManifest)?;

    manifest
        .runtime_information()
        .validate_compatibility_owned(expected_identity)
        .map_err(RuntimeBundleAdmissionError::IncompatibleOwned)?;

    let executable_path = bundle_directory.join(manifest.executable_name());
    if executable_path.parent().map(Path::new) != Some(bundle_directory) {
        return Err(RuntimeBundleAdmissionError::UnexpectedDirectoryEntry {
            path: executable_path.clone(),
        });
    }

    let executable_metadata = match fs::symlink_metadata(&executable_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(RuntimeBundleAdmissionError::MissingExecutable {
                path: executable_path.clone(),
            });
        }
        Err(error) => {
            return Err(RuntimeBundleAdmissionError::ReadManifest {
                path: executable_path.clone(),
                source: error,
            });
        }
    };
    if executable_metadata.file_type().is_symlink() {
        return Err(RuntimeBundleAdmissionError::ExecutableIsSymlink {
            path: executable_path.clone(),
        });
    }
    if !executable_metadata.is_file() {
        return Err(RuntimeBundleAdmissionError::ExecutableNotRegularFile {
            path: executable_path.clone(),
        });
    }

    let entries = fs::read_dir(bundle_directory).map_err(|source| {
        RuntimeBundleAdmissionError::ReadDirectory {
            path: bundle_directory.to_path_buf(),
            source,
        }
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| RuntimeBundleAdmissionError::ReadDirectory {
            path: bundle_directory.to_path_buf(),
            source,
        })?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if name == "runtime.json" || name == manifest.executable_name() {
            continue;
        }
        return Err(RuntimeBundleAdmissionError::UnexpectedDirectoryEntry { path: entry.path() });
    }

    let artifact = hash_runtime_executable(&executable_path)
        .map_err(RuntimeBundleAdmissionError::HashExecutable)?;
    let expected_sha256 = manifest.executable_sha256();
    let actual_sha256 = ExecutableSha256::from_hex(artifact.sha256()).unwrap();
    if manifest.executable_size() != artifact.size() || expected_sha256 != actual_sha256 {
        return Err(RuntimeBundleAdmissionError::ArtifactMismatch {
            expected_size: manifest.executable_size(),
            actual_size: artifact.size(),
            expected_sha256,
            actual_sha256,
        });
    }

    Ok(AdmittedHttpRuntimeBundle {
        directory: bundle_directory.to_path_buf(),
        executable_path: executable_path.clone(),
        manifest_path: manifest_path.clone(),
        manifest,
        artifact,
    })
}

/// Variant of [`admit_processing_runtime_bundle`] that accepts an `OwnedRuntimeIdentity`.
pub fn admit_processing_runtime_bundle_owned(
    bundle_directory: &Path,
    expected_identity: &lexicon_core::runtime::OwnedRuntimeIdentity,
) -> Result<AdmittedProcessingRuntimeBundle, ProcessingRuntimeBundleAdmissionError> {

    let bundle_metadata = fs::symlink_metadata(bundle_directory).map_err(|source| {
        ProcessingRuntimeBundleAdmissionError::BundleMetadata {
            path: bundle_directory.to_path_buf(),
            source,
        }
    })?;
    if bundle_metadata.file_type().is_symlink() {
        return Err(ProcessingRuntimeBundleAdmissionError::BundleIsSymlink {
            path: bundle_directory.to_path_buf(),
        });
    }
    if !bundle_metadata.is_dir() {
        return Err(ProcessingRuntimeBundleAdmissionError::BundleNotDirectory {
            path: bundle_directory.to_path_buf(),
        });
    }

    let manifest_path = bundle_directory.join("runtime.json");
    let manifest_metadata = fs::symlink_metadata(&manifest_path).map_err(|source| {
        ProcessingRuntimeBundleAdmissionError::ManifestMetadata {
            path: manifest_path.clone(),
            source,
        }
    })?;
    if manifest_metadata.file_type().is_symlink() {
        return Err(ProcessingRuntimeBundleAdmissionError::ManifestIsSymlink {
            path: manifest_path.clone(),
        });
    }
    if !manifest_metadata.is_file() {
        return Err(ProcessingRuntimeBundleAdmissionError::ManifestNotRegularFile {
            path: manifest_path.clone(),
        });
    }
    if manifest_metadata.len() > MAX_RUNTIME_MANIFEST_BYTES as u64 {
        return Err(ProcessingRuntimeBundleAdmissionError::ManifestTooLarge {
            maximum: MAX_RUNTIME_MANIFEST_BYTES,
            actual: manifest_metadata.len(),
        });
    }

    let manifest_bytes = fs::read(&manifest_path).map_err(|source| {
        ProcessingRuntimeBundleAdmissionError::ReadManifest {
            path: manifest_path.clone(),
            source,
        }
    })?;
    let manifest_text = validate_manifest_text(&manifest_bytes).map_err(|error| match error {
        ManifestBoundaryError::InvalidBoundary => {
            ProcessingRuntimeBundleAdmissionError::InvalidManifestBoundary
        }
        ManifestBoundaryError::TooLarge { maximum, actual } => {
            ProcessingRuntimeBundleAdmissionError::ManifestTooLarge { maximum, actual }
        }
    })?;

    let manifest = ProcessingRuntimeManifestV1::from_json(manifest_text)
        .map_err(ProcessingRuntimeBundleAdmissionError::DecodeManifest)?;

    manifest
        .runtime_information()
        .validate_compatibility_owned(expected_identity)
        .map_err(ProcessingRuntimeBundleAdmissionError::IncompatibleOwned)?;

    let executable_path = bundle_directory.join(manifest.executable_name());
    if executable_path.parent().map(Path::new) != Some(bundle_directory) {
        return Err(ProcessingRuntimeBundleAdmissionError::UnexpectedDirectoryEntry {
            path: executable_path.clone(),
        });
    }

    let executable_metadata = match fs::symlink_metadata(&executable_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ProcessingRuntimeBundleAdmissionError::MissingExecutable {
                path: executable_path.clone(),
            });
        }
        Err(error) => {
            return Err(ProcessingRuntimeBundleAdmissionError::ReadManifest {
                path: executable_path.clone(),
                source: error,
            });
        }
    };
    if executable_metadata.file_type().is_symlink() {
        return Err(ProcessingRuntimeBundleAdmissionError::ExecutableIsSymlink {
            path: executable_path.clone(),
        });
    }
    if !executable_metadata.is_file() {
        return Err(ProcessingRuntimeBundleAdmissionError::ExecutableNotRegularFile {
            path: executable_path.clone(),
        });
    }

    let entries = fs::read_dir(bundle_directory).map_err(|source| {
        ProcessingRuntimeBundleAdmissionError::ReadDirectory {
            path: bundle_directory.to_path_buf(),
            source,
        }
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| ProcessingRuntimeBundleAdmissionError::ReadDirectory {
            path: bundle_directory.to_path_buf(),
            source,
        })?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if name == "runtime.json" || name == manifest.executable_name() {
            continue;
        }
        return Err(ProcessingRuntimeBundleAdmissionError::UnexpectedDirectoryEntry {
            path: entry.path(),
        });
    }

    let artifact = hash_runtime_executable(&executable_path)
        .map_err(ProcessingRuntimeBundleAdmissionError::HashExecutable)?;
    let expected_sha256 = manifest.executable_sha256();
    let actual_sha256 = ExecutableSha256::from_hex(artifact.sha256()).unwrap();
    if manifest.executable_size() != artifact.size() || expected_sha256 != actual_sha256 {
        return Err(ProcessingRuntimeBundleAdmissionError::ArtifactMismatch {
            expected_size: manifest.executable_size(),
            actual_size: artifact.size(),
            expected_sha256,
            actual_sha256,
        });
    }

    Ok(AdmittedProcessingRuntimeBundle {
        directory: bundle_directory.to_path_buf(),
        executable_path: executable_path.clone(),
        manifest_path: manifest_path.clone(),
        manifest,
        artifact,
    })
}

#[derive(Debug)]
enum ManifestBoundaryError {
    TooLarge { maximum: usize, actual: u64 },
    InvalidBoundary,
}

fn validate_manifest_text<'a>(manifest_bytes: &'a [u8]) -> Result<&'a str, ManifestBoundaryError> {
    if manifest_bytes.is_empty() {
        return Err(ManifestBoundaryError::InvalidBoundary);
    }
    if manifest_bytes.iter().any(|byte| *byte == 0) {
        return Err(ManifestBoundaryError::InvalidBoundary);
    }
    if manifest_bytes.len() > MAX_RUNTIME_MANIFEST_BYTES {
        return Err(ManifestBoundaryError::TooLarge {
            maximum: MAX_RUNTIME_MANIFEST_BYTES,
            actual: manifest_bytes.len() as u64,
        });
    }
    let newline_index = manifest_bytes.len() - 1;
    if manifest_bytes[newline_index] != b'\n' {
        return Err(ManifestBoundaryError::InvalidBoundary);
    }
    if manifest_bytes.len() >= 2 && manifest_bytes[newline_index - 1] == b'\n' {
        return Err(ManifestBoundaryError::InvalidBoundary);
    }
    if manifest_bytes.iter().any(|byte| *byte == b'\r') {
        return Err(ManifestBoundaryError::InvalidBoundary);
    }

    let without_final_newline = &manifest_bytes[..newline_index];
    if without_final_newline.is_empty() {
        return Err(ManifestBoundaryError::InvalidBoundary);
    }
    if without_final_newline
        .first()
        .is_some_and(|byte| byte.is_ascii_whitespace())
        || without_final_newline
            .last()
            .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        return Err(ManifestBoundaryError::InvalidBoundary);
    }

    let text = std::str::from_utf8(without_final_newline)
        .map_err(|_| ManifestBoundaryError::InvalidBoundary)?;
    if text.starts_with(char::is_whitespace) || text.ends_with(char::is_whitespace) {
        return Err(ManifestBoundaryError::InvalidBoundary);
    }

    Ok(text)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use lexicon_core::protocols::http::{HttpCapabilitySet, HttpSourceContractV1};
    use lexicon_core::runtime::{RuntimeIdentity, RuntimeInformationV1};

    use super::{
        MAX_RUNTIME_MANIFEST_BYTES, ProcessingRuntimeBundleAdmissionError,
        RuntimeBundleAdmissionError, admit_http_runtime_bundle, admit_processing_runtime_bundle,
    };
    use crate::build::{
        stage_verified_http_runtime_bundle, stage_verified_processing_runtime_bundle,
        verify_http_runtime_candidate, verify_processing_runtime_candidate,
    };

    fn fixture_verified_runtime(
        identity: RuntimeIdentity,
    ) -> (crate::build::VerifiedHttpRuntime, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("runtime-admission-candidate");
        let source = HttpSourceContractV1::new(|_, _| Ok(()));
        let info =
            RuntimeInformationV1::from_http_source(identity, &source, HttpCapabilitySet::empty());
        let json = info.to_json().unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--lexicon-runtime-info\" ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nexit 1\n",
            json.replace('\\', "\\\\").replace('\'', "\\'")
        );
        fs::write(&candidate, script).unwrap();
        let mut permissions = fs::metadata(&candidate).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&candidate, permissions).unwrap();

        (
            verify_http_runtime_candidate(&candidate, identity).unwrap(),
            dir,
        )
    }

    fn fixture_verified_processing_runtime(
        identity: RuntimeIdentity,
    ) -> (crate::build::VerifiedProcessingRuntime, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("processing-runtime-admission-candidate");
        let source = lexicon_core::processing::ProcessingSourceContractV1::new(|_, _| Ok(()));
        let info =
            lexicon_core::processing::ProcessingRuntimeInformationV1::from_processing_source(
                identity, &source,
            )
            .unwrap();
        let json = info.to_json().unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--lexicon-runtime-info\" ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nexit 1\n",
            json.replace('\\', "\\\\").replace('\'', "\\'")
        );
        fs::write(&candidate, script).unwrap();
        let mut permissions = fs::metadata(&candidate).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&candidate, permissions).unwrap();

        (
            verify_processing_runtime_candidate(&candidate, identity).unwrap(),
            dir,
        )
    }

    #[test]
    fn admitted_bundle_matches_the_staged_directory() {
        let dir = tempfile::tempdir().unwrap();
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let (verified, _fixture_dir) = fixture_verified_runtime(identity);
        let staged = stage_verified_http_runtime_bundle(dir.path(), "runtime", &verified).unwrap();

        let admitted = admit_http_runtime_bundle(staged.directory(), identity).unwrap();

        assert_eq!(admitted.directory(), staged.directory());
        assert_eq!(admitted.manifest_path(), staged.manifest_path());
        assert_eq!(admitted.executable_path(), staged.executable_path());
        assert_eq!(
            admitted.manifest().executable_name(),
            staged.manifest().executable_name()
        );
        assert_eq!(admitted.runtime_information().identity(), identity);
        assert_eq!(admitted.artifact().size(), verified.artifact().size());
        assert_eq!(admitted.artifact().sha256(), verified.artifact().sha256());
    }

    #[test]
    fn missing_bundle_path_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("lexicon-missing-bundle-admission");
        let _ = fs::remove_dir_all(&missing);
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);

        let error = admit_http_runtime_bundle(&missing, identity).unwrap_err();
        assert!(matches!(
            error,
            RuntimeBundleAdmissionError::BundleMetadata { .. }
        ));
    }

    #[test]
    fn manifest_too_large_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let (verified, _fixture_dir) = fixture_verified_runtime(identity);
        let bundle = stage_verified_http_runtime_bundle(dir.path(), "runtime", &verified).unwrap();

        let manifest_path = bundle.directory().join("runtime.json");
        let mut data = std::iter::repeat(b'a')
            .take(MAX_RUNTIME_MANIFEST_BYTES + 1)
            .collect::<Vec<_>>();
        data.extend(b"\n");
        fs::write(&manifest_path, data).unwrap();

        let error = admit_http_runtime_bundle(bundle.directory(), identity).unwrap_err();
        assert!(matches!(
            error,
            RuntimeBundleAdmissionError::ManifestTooLarge { .. }
        ));
    }

    #[test]
    fn malformed_manifest_boundary_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let (verified, _fixture_dir) = fixture_verified_runtime(identity);
        let bundle = stage_verified_http_runtime_bundle(dir.path(), "runtime", &verified).unwrap();

        let manifest_path = bundle.directory().join("runtime.json");
        fs::write(&manifest_path, b"{\"schema_version\":1}\n\n").unwrap();

        let error = admit_http_runtime_bundle(bundle.directory(), identity).unwrap_err();
        assert!(matches!(
            error,
            RuntimeBundleAdmissionError::InvalidManifestBoundary
        ));
    }

    #[test]
    fn same_size_artifact_mismatch_is_detected_by_sha256() {
        let dir = tempfile::tempdir().unwrap();
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let (verified, _fixture_dir) = fixture_verified_runtime(identity);
        let bundle = stage_verified_http_runtime_bundle(dir.path(), "runtime", &verified).unwrap();
        let executable_path = bundle.directory().join(bundle.manifest().executable_name());

        let bytes = fs::read(&executable_path).unwrap();
        let mut modified = bytes.clone();
        modified[0] = modified[0].wrapping_add(1);
        fs::write(&executable_path, &modified).unwrap();

        let error = admit_http_runtime_bundle(bundle.directory(), identity).unwrap_err();
        assert!(matches!(
            error,
            RuntimeBundleAdmissionError::ArtifactMismatch { .. }
        ));
    }

    #[test]
    fn processing_bundle_admission_matches_the_staged_directory() {
        let dir = tempfile::tempdir().unwrap();
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let (verified, _fixture_dir) = fixture_verified_processing_runtime(identity);
        let staged =
            stage_verified_processing_runtime_bundle(dir.path(), "processing-runtime", &verified)
                .unwrap();

        let admitted = admit_processing_runtime_bundle(staged.directory(), identity).unwrap();

        assert_eq!(admitted.directory(), staged.directory());
        assert_eq!(admitted.manifest_path(), staged.manifest_path());
        assert_eq!(admitted.executable_path(), staged.executable_path());
        assert_eq!(
            admitted.manifest().executable_name(),
            staged.manifest().executable_name()
        );
        assert_eq!(admitted.runtime_information().identity(), identity);
        assert_eq!(admitted.artifact().size(), verified.artifact().size());
        assert_eq!(admitted.artifact().sha256(), verified.artifact().sha256());
    }

    #[test]
    fn malformed_processing_manifest_boundary_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let (verified, _fixture_dir) = fixture_verified_processing_runtime(identity);
        let bundle =
            stage_verified_processing_runtime_bundle(dir.path(), "processing-runtime", &verified)
                .unwrap();

        let manifest_path = bundle.directory().join("runtime.json");
        fs::write(&manifest_path, b"{\"schema_version\":1}\n\n").unwrap();

        let error = admit_processing_runtime_bundle(bundle.directory(), identity).unwrap_err();
        assert!(matches!(
            error,
            ProcessingRuntimeBundleAdmissionError::InvalidManifestBoundary
        ));
    }
}
