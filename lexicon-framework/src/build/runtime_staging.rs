use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::runtime_verification::{VerifiedHttpRuntime, VerifiedProcessingRuntime};
use super::{
    ExecutableSha256, ProcessingRuntimeManifestConstructionError,
    ProcessingRuntimeManifestEncodingError, ProcessingRuntimeManifestV1, RuntimeArtifactHashError,
    RuntimeManifestConstructionError, RuntimeManifestEncodingError, RuntimeManifestV1,
    hash_runtime_executable,
};

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
                write!(
                    formatter,
                    "failed to construct a staged runtime manifest: {error}"
                )
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
                write!(
                    formatter,
                    "failed to hash staged runtime executable: {error}"
                )
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
                    expected_size, expected_sha256, actual_size, actual_sha256
                )
            }
            Self::EncodeManifest(error) => {
                write!(formatter, "failed to encode runtime manifest: {error}")
            }
            Self::CreateManifest { path, source } => {
                write!(
                    formatter,
                    "failed to create manifest '{}': {source}",
                    path.display()
                )
            }
            Self::WriteManifest { path, source } => {
                write!(
                    formatter,
                    "failed to write manifest '{}': {source}",
                    path.display()
                )
            }
            Self::SyncExecutable { path, source } => {
                write!(
                    formatter,
                    "failed to synchronize executable '{}': {source}",
                    path.display()
                )
            }
            Self::SyncManifest { path, source } => {
                write!(
                    formatter,
                    "failed to synchronize manifest '{}': {source}",
                    path.display()
                )
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

#[derive(Debug)]
pub enum ProcessingRuntimeBundleStagingError {
    ManifestConstruction(ProcessingRuntimeManifestConstructionError),
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
    EncodeManifest(ProcessingRuntimeManifestEncodingError),
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

impl fmt::Display for ProcessingRuntimeBundleStagingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestConstruction(error) => {
                write!(
                    formatter,
                    "failed to construct a staged processing runtime manifest: {error}"
                )
            }
            Self::InvalidStagingParent { path } => {
                write!(
                    formatter,
                    "invalid processing runtime staging parent '{}': it must exist and be a directory",
                    path.display()
                )
            }
            Self::CreateStagingDirectory { path, source } => {
                write!(
                    formatter,
                    "failed to create a unique processing runtime staging directory under '{}': {source}",
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
                    "failed to copy processing runtime executable '{}' to '{}': {source}",
                    source_path.display(),
                    destination_path.display()
                )
            }
            Self::HashStagedExecutable(error) => {
                write!(
                    formatter,
                    "failed to hash staged processing runtime executable: {error}"
                )
            }
            Self::CopiedArtifactMismatch {
                expected_size,
                actual_size,
                expected_sha256,
                actual_sha256,
            } => {
                write!(
                    formatter,
                    "staged processing runtime artifact mismatch: expected size={} sha256={} actual size={} sha256={}",
                    expected_size, expected_sha256, actual_size, actual_sha256
                )
            }
            Self::EncodeManifest(error) => {
                write!(
                    formatter,
                    "failed to encode processing runtime manifest: {error}"
                )
            }
            Self::CreateManifest { path, source } => {
                write!(
                    formatter,
                    "failed to create manifest '{}': {source}",
                    path.display()
                )
            }
            Self::WriteManifest { path, source } => {
                write!(
                    formatter,
                    "failed to write manifest '{}': {source}",
                    path.display()
                )
            }
            Self::SyncExecutable { path, source } => {
                write!(
                    formatter,
                    "failed to synchronize processing runtime executable '{}': {source}",
                    path.display()
                )
            }
            Self::SyncManifest { path, source } => {
                write!(
                    formatter,
                    "failed to synchronize manifest '{}': {source}",
                    path.display()
                )
            }
            Self::SyncDirectory { path, source } => {
                write!(
                    formatter,
                    "failed to synchronize processing runtime staging directory '{}': {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ProcessingRuntimeBundleStagingError {
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

#[derive(Debug)]
pub(crate) struct OwnedStagedRuntimeDirectory {
    path: PathBuf,
}

impl OwnedStagedRuntimeDirectory {
    pub(crate) fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub(crate) fn into_path(self) -> PathBuf {
        self.path
    }
}

#[derive(Debug)]
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

    pub(crate) fn into_staging_directory(
        self,
    ) -> Result<PathBuf, RuntimeBundleStagingTransferError> {
        Ok(self.directory.keep())
    }

    pub(crate) fn into_owned_staged_runtime_directory(
        self,
    ) -> Result<OwnedStagedRuntimeDirectory, RuntimeBundleStagingTransferError> {
        Ok(OwnedStagedRuntimeDirectory {
            path: self.directory.keep(),
        })
    }
}

#[derive(Debug)]
pub struct StagedProcessingRuntimeBundle {
    directory: tempfile::TempDir,
    executable_path: PathBuf,
    manifest_path: PathBuf,
    manifest: ProcessingRuntimeManifestV1,
}

impl StagedProcessingRuntimeBundle {
    pub fn directory(&self) -> &Path {
        self.directory.path()
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

    pub(crate) fn into_staging_directory(
        self,
    ) -> Result<PathBuf, RuntimeBundleStagingTransferError> {
        Ok(self.directory.keep())
    }

    pub(crate) fn into_owned_staged_runtime_directory(
        self,
    ) -> Result<OwnedStagedRuntimeDirectory, RuntimeBundleStagingTransferError> {
        Ok(OwnedStagedRuntimeDirectory {
            path: self.directory.keep(),
        })
    }
}

pub fn stage_verified_http_runtime_bundle(
    staging_parent: &Path,
    executable_name: &str,
    verified: &VerifiedHttpRuntime,
) -> Result<StagedHttpRuntimeBundle, RuntimeBundleStagingError> {
    let manifest = RuntimeManifestV1::from_verified_http_runtime(executable_name, verified)
        .map_err(RuntimeBundleStagingError::ManifestConstruction)?;

    let metadata = fs::metadata(staging_parent).map_err(|_| {
        RuntimeBundleStagingError::InvalidStagingParent {
            path: staging_parent.to_path_buf(),
        }
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

    fs::copy(source_path, &executable_path).map_err(|source| {
        RuntimeBundleStagingError::CopyExecutable {
            source_path: source_path.to_path_buf(),
            destination_path: executable_path.clone(),
            source,
        }
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

    let manifest_json = manifest
        .to_json()
        .map_err(RuntimeBundleStagingError::EncodeManifest)?;
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

    let executable_file = File::open(&executable_path).map_err(|source| {
        RuntimeBundleStagingError::SyncExecutable {
            path: executable_path.clone(),
            source,
        }
    })?;
    executable_file
        .sync_all()
        .map_err(|source| RuntimeBundleStagingError::SyncExecutable {
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

    match fs::File::open(&stage_directory_path).and_then(|file| file.sync_all()) {
        Ok(()) => {}
        Err(source)
            if matches!(
                source.kind(),
                io::ErrorKind::Unsupported | io::ErrorKind::InvalidInput
            ) =>
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

pub fn stage_verified_processing_runtime_bundle(
    staging_parent: &Path,
    executable_name: &str,
    verified: &VerifiedProcessingRuntime,
) -> Result<StagedProcessingRuntimeBundle, ProcessingRuntimeBundleStagingError> {
    let manifest =
        ProcessingRuntimeManifestV1::from_verified_processing_runtime(executable_name, verified)
            .map_err(ProcessingRuntimeBundleStagingError::ManifestConstruction)?;

    let metadata = fs::metadata(staging_parent).map_err(|_| {
        ProcessingRuntimeBundleStagingError::InvalidStagingParent {
            path: staging_parent.to_path_buf(),
        }
    })?;
    if !metadata.is_dir() {
        return Err(ProcessingRuntimeBundleStagingError::InvalidStagingParent {
            path: staging_parent.to_path_buf(),
        });
    }

    let stage_directory = tempfile::Builder::new()
        .prefix(".lexicon-processing-runtime-stage-")
        .tempdir_in(staging_parent)
        .map_err(
            |source| ProcessingRuntimeBundleStagingError::CreateStagingDirectory {
                path: staging_parent.to_path_buf(),
                source,
            },
        )?;
    let stage_directory_path = stage_directory.path().to_path_buf();
    let executable_path = stage_directory_path.join(manifest.executable_name());
    let source_path = verified.artifact().path();

    fs::copy(source_path, &executable_path).map_err(|source| {
        ProcessingRuntimeBundleStagingError::CopyExecutable {
            source_path: source_path.to_path_buf(),
            destination_path: executable_path.clone(),
            source,
        }
    })?;

    if let Ok(permissions) = fs::metadata(source_path).map(|metadata| metadata.permissions()) {
        let _ = fs::set_permissions(&executable_path, permissions);
    }

    let staged_hash = hash_runtime_executable(&executable_path)
        .map_err(ProcessingRuntimeBundleStagingError::HashStagedExecutable)?;

    let expected_size = verified.artifact().size();
    let actual_size = staged_hash.size();
    let expected_sha256 = ExecutableSha256::from_hex(verified.artifact().sha256()).unwrap();
    let actual_sha256 = ExecutableSha256::from_hex(staged_hash.sha256()).unwrap();

    if expected_size != actual_size || expected_sha256 != actual_sha256 {
        return Err(
            ProcessingRuntimeBundleStagingError::CopiedArtifactMismatch {
                expected_size,
                actual_size,
                expected_sha256,
                actual_sha256,
            },
        );
    }

    let manifest_json = manifest
        .to_json()
        .map_err(ProcessingRuntimeBundleStagingError::EncodeManifest)?;
    let manifest_bytes = format!("{manifest_json}\n");
    let manifest_path = stage_directory_path.join("runtime.json");

    let mut manifest_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manifest_path)
        .map_err(
            |source| ProcessingRuntimeBundleStagingError::CreateManifest {
                path: manifest_path.clone(),
                source,
            },
        )?;

    manifest_file
        .write_all(manifest_bytes.as_bytes())
        .map_err(
            |source| ProcessingRuntimeBundleStagingError::WriteManifest {
                path: manifest_path.clone(),
                source,
            },
        )?;

    let executable_file = File::open(&executable_path).map_err(|source| {
        ProcessingRuntimeBundleStagingError::SyncExecutable {
            path: executable_path.clone(),
            source,
        }
    })?;
    executable_file.sync_all().map_err(|source| {
        ProcessingRuntimeBundleStagingError::SyncExecutable {
            path: executable_path.clone(),
            source,
        }
    })?;

    manifest_file.flush().map_err(
        |source| ProcessingRuntimeBundleStagingError::WriteManifest {
            path: manifest_path.clone(),
            source,
        },
    )?;
    manifest_file.sync_all().map_err(|source| {
        ProcessingRuntimeBundleStagingError::SyncManifest {
            path: manifest_path.clone(),
            source,
        }
    })?;

    match fs::File::open(&stage_directory_path).and_then(|file| file.sync_all()) {
        Ok(()) => {}
        Err(source)
            if matches!(
                source.kind(),
                io::ErrorKind::Unsupported | io::ErrorKind::InvalidInput
            ) =>
        {
            // Some targets do not support directory fsync; the executable and manifest are still
            // synchronized individually before returning success.
        }
        Err(source) => {
            return Err(ProcessingRuntimeBundleStagingError::SyncDirectory {
                path: stage_directory_path.clone(),
                source,
            });
        }
    }

    Ok(StagedProcessingRuntimeBundle {
        directory: stage_directory,
        executable_path,
        manifest_path,
        manifest,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use lexicon_core::processing::{ProcessingRuntimeInformationV1, ProcessingSourceContractV1};
    use lexicon_core::runtime::RuntimeIdentity;

    use super::{
        ProcessingRuntimeBundleStagingError, ProcessingRuntimeManifestV1,
        stage_verified_processing_runtime_bundle,
    };
    use crate::build::verify_processing_runtime_candidate;

    fn make_executable_script(path: &std::path::Path, body: &str) -> std::path::PathBuf {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
        path.to_path_buf()
    }

    fn fixture_verified_processing_runtime()
    -> Result<
        (crate::build::VerifiedProcessingRuntime, tempfile::TempDir),
        crate::build::ProcessingRuntimeVerificationError,
    > {
        let source_dir = tempfile::tempdir().unwrap();
        let candidate = source_dir.path().join("processing-runtime");
        let source = ProcessingSourceContractV1::new(|_, _| Ok(()));
        let json = ProcessingRuntimeInformationV1::from_processing_source(
            RuntimeIdentity::http_processing("example-source", 1),
            &source,
        )
        .unwrap()
        .to_json()
        .unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--lexicon-runtime-info\" ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nexit 1\n",
            json
        );
        make_executable_script(&candidate, &script);

        verify_processing_runtime_candidate(
            &candidate,
            RuntimeIdentity::http_processing("example-source", 1),
        )
        .map(|verified| (verified, source_dir))
    }

    /// `ETXTBSY` ("text file busy") is a known transient race on overlay filesystems
    /// (the default container storage driver): the kernel can briefly still consider a
    /// freshly written-and-chmod'd file open for writing at the moment it is exec'd,
    /// even though the writer has already closed its file handle. It is unrelated to
    /// this module's staging logic.
    fn is_verification_spawn_busy(error: &crate::build::ProcessingRuntimeVerificationError) -> bool {
        matches!(
            error,
            crate::build::ProcessingRuntimeVerificationError::Probe(probe_error)
                if matches!(
                    probe_error,
                    crate::build::ProcessingRuntimeProbeExecutionError::Spawn { source }
                        if source.kind() == std::io::ErrorKind::ExecutableFileBusy
                )
        )
    }

    /// Retries `fixture_verified_processing_runtime` when it fails with
    /// `ExecutableFileBusy` verifying the fixture candidate, up to a small fixed
    /// attempt bound. Exhausting the retry budget panics with the original error
    /// instead of skipping; an environment that cannot execute the fixture still
    /// fails the test rather than reporting a false success.
    fn fixture_verified_processing_runtime_with_retry()
    -> (crate::build::VerifiedProcessingRuntime, tempfile::TempDir) {
        const MAX_ATTEMPTS: u32 = 3;
        let mut attempt = 0;
        loop {
            attempt += 1;
            match fixture_verified_processing_runtime() {
                Ok(pair) => return pair,
                Err(error) if attempt < MAX_ATTEMPTS && is_verification_spawn_busy(&error) => {
                    continue;
                }
                Err(error) => panic!("fixture setup failed: {error:?}"),
            }
        }
    }

    #[test]
    fn verified_processing_runtime_stages_successfully() {
        let (verified, _source_dir) = fixture_verified_processing_runtime_with_retry();
        let parent = tempfile::tempdir().unwrap();

        let bundle = stage_verified_processing_runtime_bundle(
            parent.path(),
            "processing-runtime",
            &verified,
        )
        .unwrap();

        assert_eq!(bundle.directory().parent().unwrap(), parent.path());

        let entries: Vec<_> = fs::read_dir(bundle.directory())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|name| name == "processing-runtime"));
        assert!(entries.iter().any(|name| name == "runtime.json"));

        assert_eq!(
            bundle.executable_path().file_name().unwrap(),
            "processing-runtime"
        );
        assert_eq!(bundle.manifest_path().file_name().unwrap(), "runtime.json");

        let staged_bytes = fs::read(bundle.executable_path()).unwrap();
        let source_bytes = fs::read(verified.artifact().path()).unwrap();
        assert_eq!(staged_bytes, source_bytes);
        assert_eq!(bundle.manifest().executable_name(), "processing-runtime");
        assert_eq!(
            bundle.manifest().executable_size(),
            staged_bytes.len() as u64
        );
        assert_eq!(
            bundle.manifest().executable_sha256().to_string(),
            verified.artifact().sha256()
        );

        let manifest_text = fs::read_to_string(bundle.manifest_path()).unwrap();
        let manifest_decoded = ProcessingRuntimeManifestV1::from_json(&manifest_text).unwrap();
        assert_eq!(manifest_decoded, *bundle.manifest());
        assert_eq!(
            manifest_decoded.runtime_information().identity(),
            RuntimeIdentity::http_processing("example-source", 1)
        );
        assert_eq!(manifest_text.chars().filter(|&ch| ch == '\n').count(), 1);
        assert!(manifest_text.ends_with('\n'));
        assert!(!manifest_text.contains("candidate"));
        assert!(!manifest_text.contains(verified.artifact().path().to_string_lossy().as_ref()));
    }

    #[test]
    fn invalid_executable_name_fails_before_directory_creation() {
        let (verified, _source_dir) = fixture_verified_processing_runtime_with_retry();
        let parent = tempfile::tempdir().unwrap();
        let read_before = fs::read_dir(parent.path()).unwrap().count();

        let error =
            stage_verified_processing_runtime_bundle(parent.path(), "../invalid", &verified)
                .unwrap_err();

        assert!(matches!(
            error,
            ProcessingRuntimeBundleStagingError::ManifestConstruction(_)
        ));
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), read_before);
    }
}
