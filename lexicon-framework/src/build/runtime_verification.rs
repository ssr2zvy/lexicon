use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use lexicon_core::processing::ProcessingRuntimeInformationV1;
use lexicon_core::runtime::{RuntimeIdentity, RuntimeInformationV1};

use super::runtime_probe::{
    AdmittedProcessingRuntimeInformation, AdmittedRuntimeInformation, ProcessingRuntimeProbeExecutionError,
    RuntimeProbeExecutionError, probe_http_runtime_information, probe_processing_runtime_information,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashedRuntimeArtifact {
    path: PathBuf,
    size: u64,
    sha256: String,
}

impl HashedRuntimeArtifact {
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

#[derive(Debug)]
pub enum RuntimeArtifactHashError {
    MissingCandidate { path: PathBuf },
    NotRegularFile { path: PathBuf },
    Read { path: PathBuf, source: std::io::Error },
}

impl fmt::Display for RuntimeArtifactHashError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCandidate { path } => {
                write!(formatter, "runtime candidate does not exist: {}", path.display())
            }
            Self::NotRegularFile { path } => {
                write!(formatter, "runtime candidate is not a regular file: {}", path.display())
            }
            Self::Read { path, source } => {
                write!(formatter, "failed to read runtime candidate '{}': {source}", path.display())
            }
        }
    }
}

impl std::error::Error for RuntimeArtifactHashError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::MissingCandidate { .. } | Self::NotRegularFile { .. } => None,
        }
    }
}

pub fn hash_runtime_executable(executable: &Path) -> Result<HashedRuntimeArtifact, RuntimeArtifactHashError> {
    let metadata = match fs::metadata(executable) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(RuntimeArtifactHashError::MissingCandidate {
                path: executable.to_path_buf(),
            });
        }
        Err(error) => {
            return Err(RuntimeArtifactHashError::Read {
                path: executable.to_path_buf(),
                source: error,
            });
        }
    };

    if !metadata.is_file() {
        return Err(RuntimeArtifactHashError::NotRegularFile {
            path: executable.to_path_buf(),
        });
    }

    let bytes = fs::read(executable).map_err(|source| RuntimeArtifactHashError::Read {
        path: executable.to_path_buf(),
        source,
    })?;

    let digest = Sha256::digest(&bytes);
    let sha256 = format!("{:x}", digest);

    Ok(HashedRuntimeArtifact {
        path: executable.to_path_buf(),
        size: bytes.len() as u64,
        sha256,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedHttpRuntime {
    artifact: HashedRuntimeArtifact,
    information: AdmittedRuntimeInformation,
}

impl VerifiedHttpRuntime {
    pub fn artifact(&self) -> &HashedRuntimeArtifact {
        &self.artifact
    }

    pub fn information(&self) -> &RuntimeInformationV1 {
        self.information.information()
    }

    pub fn admitted_information(&self) -> &AdmittedRuntimeInformation {
        &self.information
    }
}

#[derive(Debug)]
pub enum HttpRuntimeVerificationError {
    InitialHash(RuntimeArtifactHashError),
    Probe(RuntimeProbeExecutionError),
    FinalHash(RuntimeArtifactHashError),
    ArtifactChangedDuringProbe {
        before: HashedRuntimeArtifact,
        after: HashedRuntimeArtifact,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedProcessingRuntime {
    artifact: HashedRuntimeArtifact,
    information: AdmittedProcessingRuntimeInformation,
}

impl VerifiedProcessingRuntime {
    pub fn artifact(&self) -> &HashedRuntimeArtifact {
        &self.artifact
    }

    pub fn information(&self) -> &ProcessingRuntimeInformationV1 {
        self.information.information()
    }

    pub fn admitted_information(&self) -> &AdmittedProcessingRuntimeInformation {
        &self.information
    }
}

#[derive(Debug)]
pub enum ProcessingRuntimeVerificationError {
    InitialHash(RuntimeArtifactHashError),
    Probe(ProcessingRuntimeProbeExecutionError),
    FinalHash(RuntimeArtifactHashError),
    ArtifactChangedDuringProbe {
        before: HashedRuntimeArtifact,
        after: HashedRuntimeArtifact,
    },
}

impl fmt::Display for HttpRuntimeVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitialHash(error) => write!(formatter, "initial runtime hash failed: {error}"),
            Self::Probe(error) => write!(formatter, "runtime information probe failed: {error}"),
            Self::FinalHash(error) => write!(formatter, "final runtime hash failed: {error}"),
            Self::ArtifactChangedDuringProbe { before, after } => write!(
                formatter,
                "runtime candidate changed during verification: before={} size={} sha256={} after={} size={} sha256={}",
                before.path().display(),
                before.size(),
                before.sha256(),
                after.path().display(),
                after.size(),
                after.sha256(),
            ),
        }
    }
}

impl fmt::Display for ProcessingRuntimeVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitialHash(error) => write!(formatter, "initial processing runtime hash failed: {error}"),
            Self::Probe(error) => write!(formatter, "processing runtime information probe failed: {error}"),
            Self::FinalHash(error) => write!(formatter, "final processing runtime hash failed: {error}"),
            Self::ArtifactChangedDuringProbe { before, after } => write!(
                formatter,
                "processing runtime candidate changed during verification: before={} size={} sha256={} after={} size={} sha256={}",
                before.path().display(),
                before.size(),
                before.sha256(),
                after.path().display(),
                after.size(),
                after.sha256(),
            ),
        }
    }
}

impl std::error::Error for HttpRuntimeVerificationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InitialHash(error) | Self::FinalHash(error) => Some(error),
            Self::Probe(error) => Some(error),
            Self::ArtifactChangedDuringProbe { .. } => None,
        }
    }
}

impl std::error::Error for ProcessingRuntimeVerificationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InitialHash(error) | Self::FinalHash(error) => Some(error),
            Self::Probe(error) => Some(error),
            Self::ArtifactChangedDuringProbe { .. } => None,
        }
    }
}

fn verify_runtime_candidate_with<FHash, FProbe, TInfo, TResult, TProbeError, TError, FResult, FInitial, FProbeMap, FFinal, FChanged>(
    executable: &Path,
    expected_identity: RuntimeIdentity,
    hash_fn: FHash,
    probe_fn: FProbe,
    build_result: FResult,
    initial_error: FInitial,
    probe_error: FProbeMap,
    final_error: FFinal,
    changed_error: FChanged,
) -> Result<TResult, TError>
where
    FHash: Fn(&Path) -> Result<HashedRuntimeArtifact, RuntimeArtifactHashError>,
    FProbe: Fn(&Path, RuntimeIdentity) -> Result<TInfo, TProbeError>,
    FResult: FnOnce(HashedRuntimeArtifact, TInfo) -> TResult,
    FInitial: Fn(RuntimeArtifactHashError) -> TError,
    FProbeMap: Fn(TProbeError) -> TError,
    FFinal: Fn(RuntimeArtifactHashError) -> TError,
    FChanged: Fn(HashedRuntimeArtifact, HashedRuntimeArtifact) -> TError,
{
    let before = hash_fn(executable).map_err(initial_error)?;
    let information = probe_fn(executable, expected_identity).map_err(probe_error)?;
    let after = hash_fn(executable).map_err(final_error)?;

    if before.path != after.path || before.size != after.size || before.sha256 != after.sha256 {
        return Err(changed_error(before, after));
    }

    Ok(build_result(before, information))
}

fn verify_http_runtime_candidate_with<FHash, FProbe>(
    executable: &Path,
    expected_identity: RuntimeIdentity,
    hash_fn: FHash,
    probe_fn: FProbe,
) -> Result<VerifiedHttpRuntime, HttpRuntimeVerificationError>
where
    FHash: Fn(&Path) -> Result<HashedRuntimeArtifact, RuntimeArtifactHashError>,
    FProbe: Fn(&Path, RuntimeIdentity) -> Result<AdmittedRuntimeInformation, RuntimeProbeExecutionError>,
{
    verify_runtime_candidate_with(
        executable,
        expected_identity,
        hash_fn,
        probe_fn,
        |artifact, information| VerifiedHttpRuntime { artifact, information },
        HttpRuntimeVerificationError::InitialHash,
        HttpRuntimeVerificationError::Probe,
        HttpRuntimeVerificationError::FinalHash,
        |before, after| HttpRuntimeVerificationError::ArtifactChangedDuringProbe { before, after },
    )
}

pub fn verify_http_runtime_candidate(
    executable: &Path,
    expected_identity: RuntimeIdentity,
) -> Result<VerifiedHttpRuntime, HttpRuntimeVerificationError> {
    verify_http_runtime_candidate_with(
        executable,
        expected_identity,
        hash_runtime_executable,
        |path, identity| probe_http_runtime_information(path, identity),
    )
}

pub(crate) fn verify_processing_runtime_candidate_with<FHash, FProbe>(
    executable: &Path,
    expected_identity: RuntimeIdentity,
    hash_fn: FHash,
    probe_fn: FProbe,
) -> Result<VerifiedProcessingRuntime, ProcessingRuntimeVerificationError>
where
    FHash: Fn(&Path) -> Result<HashedRuntimeArtifact, RuntimeArtifactHashError>,
    FProbe: Fn(&Path, RuntimeIdentity) -> Result<AdmittedProcessingRuntimeInformation, ProcessingRuntimeProbeExecutionError>,
{
    verify_runtime_candidate_with(
        executable,
        expected_identity,
        hash_fn,
        probe_fn,
        |artifact, information| VerifiedProcessingRuntime { artifact, information },
        ProcessingRuntimeVerificationError::InitialHash,
        ProcessingRuntimeVerificationError::Probe,
        ProcessingRuntimeVerificationError::FinalHash,
        |before, after| ProcessingRuntimeVerificationError::ArtifactChangedDuringProbe { before, after },
    )
}

pub fn verify_processing_runtime_candidate(
    executable: &Path,
    expected_identity: RuntimeIdentity,
) -> Result<VerifiedProcessingRuntime, ProcessingRuntimeVerificationError> {
    verify_processing_runtime_candidate_with(
        executable,
        expected_identity,
        hash_runtime_executable,
        |path, identity| probe_processing_runtime_information(path, identity),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use sha2::Digest;

    use lexicon_core::processing::{ProcessingRuntimeInformationV1, ProcessingSourceContractV1};
    use lexicon_core::protocols::http::{HttpCapabilitySet, HttpSourceContractV1};
    use lexicon_core::runtime::{RuntimeIdentity, RuntimeInformationV1};

    use super::{
        HashedRuntimeArtifact, HttpRuntimeVerificationError, ProcessingRuntimeVerificationError,
        ProcessingRuntimeProbeExecutionError, RuntimeArtifactHashError,
        hash_runtime_executable, verify_http_runtime_candidate, verify_http_runtime_candidate_with,
        verify_processing_runtime_candidate, verify_processing_runtime_candidate_with,
    };

    fn fixture_runtime_info_json(identity: RuntimeIdentity) -> Vec<u8> {
        let source = HttpSourceContractV1::new(|_, _| Ok(()));
        RuntimeInformationV1::from_http_source(identity, &source, HttpCapabilitySet::empty())
            .to_json()
            .unwrap()
            .into_bytes()
    }

    fn make_executable_script(path: &std::path::Path, body: &str) -> std::path::PathBuf {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
        path.to_path_buf()
    }

    #[test]
    fn stable_valid_candidate_produces_verified_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("runtime-probe");
        let json = fixture_runtime_info_json(RuntimeIdentity::http_acquisition("example-source", 1));
        let shell_json = String::from_utf8(json.clone()).unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--lexicon-runtime-information-v1\" ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nexit 1\n",
            shell_json.replace('\\', "\\\\").replace('\'', "\\'")
        );
        make_executable_script(&candidate, &script);

        let verified = verify_http_runtime_candidate(&candidate, RuntimeIdentity::http_acquisition("example-source", 1)).unwrap();

        assert_eq!(verified.artifact().path(), candidate);
        assert_eq!(verified.artifact().size() as usize, fs::read(&candidate).unwrap().len());
        let bytes = fs::read(&candidate).unwrap();
        assert_eq!(verified.artifact().sha256(), format!("{:x}", sha2::Sha256::digest(&bytes)));
        assert_eq!(verified.information().identity(), RuntimeIdentity::http_acquisition("example-source", 1));
        assert_eq!(verified.information().required_capabilities(), HttpCapabilitySet::empty());
        assert_eq!(verified.information().available_capabilities(), HttpCapabilitySet::empty());
    }

    #[test]
    fn initial_hash_failure_prevents_probe_execution() {
        let temp = tempfile::tempdir().unwrap();
        let non_existent = temp.path().join("lexicon-does-not-exist-verify-runtime");
        let _ = fs::remove_file(&non_existent);

        let error = verify_http_runtime_candidate(
            &non_existent,
            RuntimeIdentity::http_acquisition("example-source", 1),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            HttpRuntimeVerificationError::InitialHash(RuntimeArtifactHashError::MissingCandidate { .. })
        ));
    }

    #[test]
    fn final_hash_failure_returns_final_hash() {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("runtime-final-hash");
        let script = "#!/bin/sh\nif [ \"$1\" = \"--lexicon-runtime-information-v1\" ]; then\n  printf '%s\\n' '{\"schema_version\":1,\"identity\":{\"source\":\"example-source\",\"version\":1,\"protocol\":\"http-acquisition\"},\"source\":{\"protocol\":\"http\",\"name\":\"example-source\",\"descriptor\":{\"contract_version\":1},\"acquire\":null,\"resume\":null},\"available_capabilities\":[]}'\n  exit 0\nfi\nexit 1\n";
        make_executable_script(&candidate, script);

        let before = hash_runtime_executable(&candidate).unwrap();
        let call_count = std::cell::Cell::new(0usize);
        let error = verify_http_runtime_candidate_with(
            &candidate,
            RuntimeIdentity::http_acquisition("example-source", 1),
            |_| {
                let count = call_count.get();
                call_count.set(count + 1);
                if count == 0 {
                    Ok(before.clone())
                } else {
                    Err(RuntimeArtifactHashError::Read {
                        path: candidate.clone(),
                        source: std::io::Error::new(std::io::ErrorKind::Other, "hash failure"),
                    })
                }
            },
            |_, _| {
                let source = HttpSourceContractV1::new(|_, _| Ok(()));
                let output = RuntimeInformationV1::from_http_source(
                    RuntimeIdentity::http_acquisition("example-source", 1),
                    &source,
                    HttpCapabilitySet::empty(),
                )
                .to_json()
                .unwrap();
                let mut bytes = output.into_bytes();
                bytes.push(b'\n');
                crate::build::runtime_probe::admit_http_runtime_information_probe(
                    RuntimeIdentity::http_acquisition("example-source", 1),
                    &bytes,
                )
                .map_err(crate::build::runtime_probe::RuntimeProbeExecutionError::Admission)
            },
        )
        .unwrap_err();

        assert!(matches!(error, HttpRuntimeVerificationError::FinalHash(_)));
    }

    #[test]
    fn artifact_changed_during_probe_returns_changed_error() {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("runtime-changed");
        let script = "#!/bin/sh\nif [ \"$1\" = \"--lexicon-runtime-information-v1\" ]; then\n  printf '%s\\n' '{\"schema_version\":1,\"identity\":{\"source\":\"example-source\",\"version\":1,\"protocol\":\"http-acquisition\"},\"source\":{\"protocol\":\"http\",\"name\":\"example-source\",\"descriptor\":{\"contract_version\":1},\"acquire\":null,\"resume\":null},\"available_capabilities\":[]}'\n  exit 0\nfi\nexit 1\n";
        make_executable_script(&candidate, script);

        let expected = RuntimeIdentity::http_acquisition("example-source", 1);
        let error = verify_http_runtime_candidate_with(
            &candidate,
            expected,
            |path| {
                let bytes = fs::read(path).unwrap();
                let digest = format!("{:x}", sha2::Sha256::digest(&bytes));
                Ok(HashedRuntimeArtifact {
                    path: path.to_path_buf(),
                    size: bytes.len() as u64,
                    sha256: digest,
                })
            },
            |path, identity| {
                let before = fs::read(path).unwrap();
                let after = format!("{}-mutated", String::from_utf8(before).unwrap());
                fs::write(path, after).unwrap();
                let source = HttpSourceContractV1::new(|_, _| Ok(()));
                let output = RuntimeInformationV1::from_http_source(identity, &source, HttpCapabilitySet::empty())
                    .to_json()
                    .unwrap();
                let mut bytes = output.into_bytes();
                bytes.push(b'\n');
                crate::build::runtime_probe::admit_http_runtime_information_probe(identity, &bytes)
                    .map_err(crate::build::runtime_probe::RuntimeProbeExecutionError::Admission)
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            HttpRuntimeVerificationError::ArtifactChangedDuringProbe { .. }
        ));
    }

    fn fixture_processing_runtime_info_json(identity: RuntimeIdentity) -> Vec<u8> {
        let source = ProcessingSourceContractV1::new(|_, _| Ok(()));
        ProcessingRuntimeInformationV1::from_processing_source(identity, &source)
            .unwrap()
            .to_json()
            .unwrap()
            .into_bytes()
    }

    #[test]
    fn stable_processing_candidate_produces_verified_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("processing-runtime-probe");
        let json = fixture_processing_runtime_info_json(RuntimeIdentity::http_processing("example-source", 1));
        let shell_json = String::from_utf8(json.clone()).unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--lexicon-runtime-information-v1\" ]; then\n  printf '%s\\n' '{0}'\n  exit 0\nfi\nexit 1\n",
            shell_json
        );
        make_executable_script(&candidate, &script);

        let verified = verify_processing_runtime_candidate(&candidate, RuntimeIdentity::http_processing("example-source", 1)).unwrap();

        assert_eq!(verified.artifact().path(), candidate);
        assert_eq!(verified.artifact().size() as usize, fs::read(&candidate).unwrap().len());
        let bytes = fs::read(&candidate).unwrap();
        assert_eq!(verified.artifact().sha256(), format!("{:x}", sha2::Sha256::digest(&bytes)));
        assert_eq!(verified.information().identity(), RuntimeIdentity::http_processing("example-source", 1));
    }

    #[test]
    fn processing_initial_hash_failure_prevents_probe_execution() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("lexicon-does-not-exist-processing-verify-runtime");
        let _ = fs::remove_file(&missing);

        let error = verify_processing_runtime_candidate(&missing, RuntimeIdentity::http_processing("example-source", 1)).unwrap_err();

        assert!(matches!(
            error,
            ProcessingRuntimeVerificationError::InitialHash(RuntimeArtifactHashError::MissingCandidate { .. })
        ));
    }

    #[test]
    fn processing_probe_failure_returns_probe() {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("processing-runtime-probe-failure");
        let output = fixture_processing_runtime_info_json(RuntimeIdentity::http_processing("example-source", 1));
        let shell_json = String::from_utf8(output).unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--lexicon-runtime-information-v1\" ]; then\n  printf '%s\\n' '{0}'\n  exit 0\nfi\nexit 1\n",
            shell_json
        );
        make_executable_script(&candidate, &script);

        let error = verify_processing_runtime_candidate_with(
            &candidate,
            RuntimeIdentity::http_processing("example-source", 1),
            hash_runtime_executable,
            |_, _| Err(ProcessingRuntimeProbeExecutionError::Spawn { source: std::io::Error::new(std::io::ErrorKind::Other, "spawn failed") }),
        )
        .unwrap_err();

        assert!(matches!(error, ProcessingRuntimeVerificationError::Probe(ProcessingRuntimeProbeExecutionError::Spawn { .. })));
    }

    #[test]
    fn processing_final_hash_failure_returns_final_hash() {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("processing-final-hash");
        let script = "#!/bin/sh\nif [ \"$1\" = \"--lexicon-runtime-information-v1\" ]; then\n  printf '%s\\n' '{\"schema_version\":1,\"identity\":{\"source\":\"example-source\",\"protocol\":\"http\",\"operation\":\"processing\",\"source_contract_version\":1},\"descriptor\":{\"contract_version\":1}}'\n  exit 0\nfi\nexit 1\n";
        make_executable_script(&candidate, script);

        let before = hash_runtime_executable(&candidate).unwrap();
        let call_count = std::cell::Cell::new(0usize);
        let error = verify_processing_runtime_candidate_with(
            &candidate,
            RuntimeIdentity::http_processing("example-source", 1),
            |_| {
                let count = call_count.get();
                call_count.set(count + 1);
                if count == 0 {
                    Ok(before.clone())
                } else {
                    Err(RuntimeArtifactHashError::Read {
                        path: candidate.clone(),
                        source: std::io::Error::new(std::io::ErrorKind::Other, "hash failure"),
                    })
                }
            },
            |_, _| {
                let source = ProcessingSourceContractV1::new(|_, _| Ok(()));
                let output = ProcessingRuntimeInformationV1::from_processing_source(
                    RuntimeIdentity::http_processing("example-source", 1),
                    &source,
                )
                .unwrap()
                .to_json()
                .unwrap();
                let mut bytes = output.into_bytes();
                bytes.push(b'\n');
                crate::build::runtime_probe::admit_processing_runtime_information_probe(
                    RuntimeIdentity::http_processing("example-source", 1),
                    &bytes,
                )
                .map_err(crate::build::runtime_probe::ProcessingRuntimeProbeExecutionError::Admission)
            },
        )
        .unwrap_err();

        assert!(matches!(error, ProcessingRuntimeVerificationError::FinalHash(_)));
    }

    #[test]
    fn processing_artifact_changed_during_probe_returns_changed_error() {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("processing-changed");
        let script = "#!/bin/sh\nif [ \"$1\" = \"--lexicon-runtime-information-v1\" ]; then\n  printf '%s\\n' '{\"schema_version\":1,\"identity\":{\"source\":\"example-source\",\"protocol\":\"http\",\"operation\":\"processing\",\"source_contract_version\":1},\"descriptor\":{\"contract_version\":1}}'\n  exit 0\nfi\nexit 1\n";
        make_executable_script(&candidate, script);

        let expected = RuntimeIdentity::http_processing("example-source", 1);
        let error = verify_processing_runtime_candidate_with(
            &candidate,
            expected,
            |path| {
                let bytes = fs::read(path).unwrap();
                let digest = format!("{:x}", sha2::Sha256::digest(&bytes));
                Ok(HashedRuntimeArtifact {
                    path: path.to_path_buf(),
                    size: bytes.len() as u64,
                    sha256: digest,
                })
            },
            |path, identity| {
                let before = fs::read(path).unwrap();
                let after = format!("{}-mutated", String::from_utf8(before).unwrap());
                fs::write(path, after).unwrap();
                let source = ProcessingSourceContractV1::new(|_, _| Ok(()));
                let output = ProcessingRuntimeInformationV1::from_processing_source(identity, &source)
                    .unwrap()
                    .to_json()
                    .unwrap();
                let mut bytes = output.into_bytes();
                bytes.push(b'\n');
                crate::build::runtime_probe::admit_processing_runtime_information_probe(identity, &bytes)
                    .map_err(crate::build::runtime_probe::ProcessingRuntimeProbeExecutionError::Admission)
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProcessingRuntimeVerificationError::ArtifactChangedDuringProbe { .. }
        ));
    }
}
