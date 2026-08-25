use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::{
    ExecutableSha256, RuntimeArtifactHashError, RuntimeManifestConstructionError,
    RuntimeManifestEncodingError, RuntimeManifestV1, hash_runtime_executable,
};
use super::runtime_verification::VerifiedHttpRuntime;

#[derive(Debug)]
pub enum RuntimeBundleStagingError {
    ManifestConstruction(RuntimeManifestConstructionError),
    InvalidStagingParent {
        path: PathBuf,
    },
    CreateStagingDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    CopyExecutable {
        source_path: PathBuf,
        destination_path: PathBuf,
        source: std::io::Error,
    },
    HashStagedExecutable(RuntimeArtifactHashError),
    CopiedArtifactMismatch {
        expected_size: u64,
        actual_size: u64,
        expected_sha256: ExecutableSha256,
        actual_sha256: ExecutableSha256,
    },
    EncodeManifest(RuntimeManifestEncodingError),
    CreateManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    WriteManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    SyncExecutable {
        path: PathBuf,
        source: std::io::Error,
    },
    SyncManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    SyncDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for RuntimeBundleStagingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestConstruction(error) => {
                write!(formatter, "failed to construct a staged runtime manifest: {error}")
            }
            Self::InvalidStagingParent { path } => {
                write!(
                    formatter,
                    "invalid runtime staging parent '{}': it must exist and be a directory",
                    path.display()
                )
            }
            Self::CreateStagingDirectory { path, source } => {
                write!(
                    formatter,
                    "failed to create a unique runtime staging directory under '{}': {source}",
                    path.display()
                )
            }
            Self::CopyExecutable {
                source_path,
                destination_path,
                source,
            } => {
                write!(
                    formatter,
                    "failed to copy runtime executable '{}' to '{}': {source}",
                    source_path.display(),
                    destination_path.display()
                )
            }
            Self::HashStagedExecutable(error) => {
                write!(formatter, "failed to hash staged runtime executable: {error}")
            }
            Self::CopiedArtifactMismatch {
                expected_size,
                actual_size,
                expected_sha256,
                actual_sha256,
            } => {
                write!(
                    formatter,
                    "staged runtime artifact mismatch: expected size={} sha256={} actual size={} sha256={}",
                    expected_size,
                    expected_sha256,
                    actual_size,
                    actual_sha256
                )
            }
            Self::EncodeManifest(error) => {
                write!(formatter, "failed to encode runtime manifest: {error}")
            }
            Self::CreateManifest { path, source } => {
                write!(formatter, "failed to create manifest '{}': {source}", path.display())
            }
            Self::WriteManifest { path, source } => {
                write!(formatter, "failed to write manifest '{}': {source}", path.display())
            }
            Self::SyncExecutable { path, source } => {
                write!(formatter, "failed to synchronize executable '{}': {source}", path.display())
            }
            Self::SyncManifest { path, source } => {
                write!(formatter, "failed to synchronize manifest '{}': {source}", path.display())
            }
            Self::SyncDirectory { path, source } => {
                write!(
                    formatter,
                    "failed to synchronize staging directory '{}': {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for RuntimeBundleStagingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManifestConstruction(error) => Some(error),
            Self::CreateStagingDirectory { source, .. }
            | Self::CopyExecutable { source, .. }
            | Self::CreateManifest { source, .. }
            | Self::WriteManifest { source, .. }
            | Self::SyncExecutable { source, .. }
            | Self::SyncManifest { source, .. }
            | Self::SyncDirectory { source, .. } => Some(source),
            Self::HashStagedExecutable(error) => Some(error),
            Self::EncodeManifest(error) => Some(error),
            Self::InvalidStagingParent { .. } | Self::CopiedArtifactMismatch { .. } => None,
        }
    }
}

pub struct StagedHttpRuntimeBundle {
    directory: tempfile::TempDir,
    executable_path: PathBuf,
    manifest_path: PathBuf,
    manifest: RuntimeManifestV1,
}

#[derive(Debug)]
pub(crate) struct RuntimeBundleStagingTransferError {
    path: PathBuf,
    source: io::Error,
}

impl fmt::Display for RuntimeBundleStagingTransferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to transfer staging directory '{}' into publication ownership: {}",
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for RuntimeBundleStagingTransferError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl StagedHttpRuntimeBundle {
    pub fn directory(&self) -> &Path {
        self.directory.path()
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

    pub(crate) fn into_staging_directory(self) -> Result<PathBuf, RuntimeBundleStagingTransferError> {
        Ok(self.directory.keep())
    }
}

pub fn stage_verified_http_runtime_bundle(
    staging_parent: &Path,
    executable_name: &str,
    verified: &VerifiedHttpRuntime,
) -> Result<StagedHttpRuntimeBundle, RuntimeBundleStagingError> {
    let manifest = RuntimeManifestV1::from_verified_http_runtime(executable_name, verified)
        .map_err(RuntimeBundleStagingError::ManifestConstruction)?;

    let metadata = fs::metadata(staging_parent).map_err(|_| RuntimeBundleStagingError::InvalidStagingParent {
        path: staging_parent.to_path_buf(),
    })?;
    if !metadata.is_dir() {
        return Err(RuntimeBundleStagingError::InvalidStagingParent {
            path: staging_parent.to_path_buf(),
        });
    }

    let stage_directory = tempfile::Builder::new()
        .prefix(".lexicon-http-runtime-stage-")
        .tempdir_in(staging_parent)
        .map_err(|source| RuntimeBundleStagingError::CreateStagingDirectory {
            path: staging_parent.to_path_buf(),
            source,
        })?;
    let stage_directory_path = stage_directory.path().to_path_buf();
    let executable_path = stage_directory_path.join(manifest.executable_name());
    let source_path = verified.artifact().path();

    fs::copy(source_path, &executable_path).map_err(|source| RuntimeBundleStagingError::CopyExecutable {
        source_path: source_path.to_path_buf(),
        destination_path: executable_path.clone(),
        source,
    })?;

    if let Ok(permissions) = fs::metadata(source_path).map(|metadata| metadata.permissions()) {
        let _ = fs::set_permissions(&executable_path, permissions);
    }

    let staged_hash = hash_runtime_executable(&executable_path)
        .map_err(RuntimeBundleStagingError::HashStagedExecutable)?;

    let expected_size = verified.artifact().size();
    let actual_size = staged_hash.size();
    let expected_sha256 = ExecutableSha256::from_hex(verified.artifact().sha256()).unwrap();
    let actual_sha256 = ExecutableSha256::from_hex(staged_hash.sha256()).unwrap();

    if expected_size != actual_size || expected_sha256 != actual_sha256 {
        return Err(RuntimeBundleStagingError::CopiedArtifactMismatch {
            expected_size,
            actual_size,
            expected_sha256,
            actual_sha256,
        });
    }

    let manifest_json = manifest.to_json().map_err(RuntimeBundleStagingError::EncodeManifest)?;
    let manifest_bytes = format!("{manifest_json}\n");
    let manifest_path = stage_directory_path.join("runtime.json");

    let mut manifest_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manifest_path)
        .map_err(|source| RuntimeBundleStagingError::CreateManifest {
            path: manifest_path.clone(),
            source,
        })?;

    manifest_file
        .write_all(manifest_bytes.as_bytes())
        .map_err(|source| RuntimeBundleStagingError::WriteManifest {
            path: manifest_path.clone(),
            source,
        })?;

    let executable_file = File::open(&executable_path).map_err(|source| RuntimeBundleStagingError::SyncExecutable {
        path: executable_path.clone(),
        source,
    })?;
    executable_file.sync_all().map_err(|source| RuntimeBundleStagingError::SyncExecutable {
        path: executable_path.clone(),
        source,
    })?;

    manifest_file
        .flush()
        .map_err(|source| RuntimeBundleStagingError::WriteManifest {
            path: manifest_path.clone(),
            source,
        })?;
    manifest_file
        .sync_all()
        .map_err(|source| RuntimeBundleStagingError::SyncManifest {
            path: manifest_path.clone(),
            source,
        })?;

    match fs::File::open(&stage_directory_path)
        .and_then(|file| file.sync_all())
    {
        Ok(()) => {}
        Err(source)
            if matches!(source.kind(), io::ErrorKind::Unsupported | io::ErrorKind::InvalidInput) =>
        {
            // Some targets do not support directory fsync; the executable and manifest are still
            // synchronized individually before returning success.
        }
        Err(source) => {
            return Err(RuntimeBundleStagingError::SyncDirectory {
                path: stage_directory_path.clone(),
                source,
            });
        }
    }

    Ok(StagedHttpRuntimeBundle {
        directory: stage_directory,
        executable_path,
        manifest_path,
        manifest,
    })
}

