use std::path::{Component, Path};

fn is_safe_executable_name(name: &str) -> bool {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('\0')
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || Path::new(name).is_absolute()
    {
        return false;
    }

    let mut components = Path::new(name).components();
    if matches!(
        components.next(),
        Some(Component::CurDir | Component::ParentDir)
    ) {
        return false;
    }

    if let Some(Component::Normal(part)) = components.next() {
        if part == std::ffi::OsStr::new(".") || part == std::ffi::OsStr::new("..") {
            return false;
        }
    }

    !name.is_empty() && !name.starts_with("/") && !name.starts_with("\\")
}

pub mod processing_runtime_manifest;
pub mod runtime_bundle_admission;
pub mod runtime_manifest;
pub mod runtime_probe;
pub mod runtime_staging;
pub mod runtime_verification;

pub use processing_runtime_manifest::{
    ProcessingRuntimeManifestConstructionError, ProcessingRuntimeManifestDecodingError,
    ProcessingRuntimeManifestEncodingError, ProcessingRuntimeManifestV1,
};
pub use runtime_bundle_admission::{
    AdmittedHttpRuntimeBundle, AdmittedProcessingRuntimeBundle, MAX_RUNTIME_MANIFEST_BYTES,
    ProcessingRuntimeBundleAdmissionError, RuntimeBundleAdmissionError, admit_http_runtime_bundle,
    admit_processing_runtime_bundle,
};
pub use runtime_manifest::{
    ExecutableSha256, ExecutableSha256ParseError, RUNTIME_MANIFEST_SCHEMA_VERSION,
    RuntimeManifestConstructionError, RuntimeManifestDecodingError, RuntimeManifestEncodingError,
    RuntimeManifestV1,
};
pub use runtime_probe::{
    AdmittedProcessingRuntimeInformation, AdmittedRuntimeInformation,
    MAX_RUNTIME_INFORMATION_PROBE_BYTES, MAX_RUNTIME_INFORMATION_PROBE_STDERR_BYTES,
    ProcessingRuntimeProbeAdmissionError, ProcessingRuntimeProbeExecutionError,
    RUNTIME_INFORMATION_PROBE_TIMEOUT, RuntimeProbeAdmissionError, RuntimeProbeExecutionError,
    admit_http_runtime_information_probe, admit_http_runtime_information_probe_owned,
    admit_processing_runtime_information_probe, admit_processing_runtime_information_probe_owned,
    probe_http_runtime_information, probe_http_runtime_information_owned,
    probe_processing_runtime_information, probe_processing_runtime_information_owned,
};
pub use runtime_staging::{
    ProcessingRuntimeBundleStagingError, RuntimeBundleStagingError, StagedHttpRuntimeBundle,
    StagedProcessingRuntimeBundle, stage_verified_http_runtime_bundle,
    stage_verified_processing_runtime_bundle,
};
pub use runtime_verification::{
    HashedRuntimeArtifact, HttpRuntimeVerificationError, ProcessingRuntimeVerificationError,
    RuntimeArtifactHashError, VerifiedHttpRuntime, VerifiedProcessingRuntime,
    hash_runtime_executable, verify_http_runtime_candidate, verify_http_runtime_candidate_owned,
    verify_processing_runtime_candidate, verify_processing_runtime_candidate_owned,
};
