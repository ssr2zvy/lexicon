use std::path::Path;

use lexicon_core::protocols::http::HttpSourceContractV1;
use lexicon_core::processing::ProcessingSourceContractV1;
use lexicon_core::runtime::OwnedRuntimeIdentity;

use crate::build::{
    AdmittedHttpRuntimeBundle, AdmittedProcessingRuntimeBundle, HashedRuntimeArtifact,
    admit_http_runtime_bundle_owned,
    admit_processing_runtime_bundle_owned, hash_runtime_executable,
};
use crate::data::error::{ExecutableIntegrityError, ForegroundDataExecutionError};
use crate::data::project::RuntimeProjectLayout;
use crate::data::request::DataOperation;

// ---------------------------------------------------------------------------
// Bundle admission result
// ---------------------------------------------------------------------------

/// Admitted runtime bundle for the acquisition (HTTP) operation.
pub struct AdmittedAcquisitionBundle {
    pub bundle: AdmittedHttpRuntimeBundle,
    pub identity: OwnedRuntimeIdentity,
}

/// Admitted runtime bundle for the processing operation.
pub struct AdmittedProcessingBundle {
    pub bundle: AdmittedProcessingRuntimeBundle,
    pub identity: OwnedRuntimeIdentity,
}

/// Either acquisition or processing admitted bundle, with resolved executable path and identity.
pub enum AdmittedBundle {
    Acquisition(AdmittedAcquisitionBundle),
    Processing(AdmittedProcessingBundle),
}

impl AdmittedBundle {
    pub fn identity(&self) -> &OwnedRuntimeIdentity {
        match self {
            Self::Acquisition(b) => &b.identity,
            Self::Processing(b) => &b.identity,
        }
    }

    pub fn executable_path(&self) -> &Path {
        match self {
            Self::Acquisition(b) => b.bundle.executable_path(),
            Self::Processing(b) => b.bundle.executable_path(),
        }
    }

    pub fn admitted_artifact(&self) -> &HashedRuntimeArtifact {
        match self {
            Self::Acquisition(b) => b.bundle.artifact(),
            Self::Processing(b) => b.bundle.artifact(),
        }
    }

    /// Returns the `RuntimeIdentity` from the admitted bundle's manifest.
    ///
    /// This is the parsed identity that retains the 'static source name via the
    /// core library's internal Box::leak in `from_json`. Use this when constructing
    /// invocation envelopes that require `RuntimeIdentity`.
    pub fn information_identity(&self) -> lexicon_core::runtime::RuntimeIdentity {
        match self {
            Self::Acquisition(b) => b.bundle.runtime_information().identity(),
            Self::Processing(b) => b.bundle.runtime_information().identity(),
        }
    }
}

// ---------------------------------------------------------------------------
// Admission
// ---------------------------------------------------------------------------

/// Admit the runtime bundle appropriate for the given operation and verify that
/// the admitted bundle identity matches the expected owned identity.
pub fn admit_bundle(
    layout: &RuntimeProjectLayout,
    operation: DataOperation,
) -> Result<AdmittedBundle, ForegroundDataExecutionError> {
    match operation {
        DataOperation::Acquisition => admit_acquisition_bundle(layout),
        DataOperation::Processing => admit_processing_bundle(layout),
    }
}

fn admit_acquisition_bundle(
    layout: &RuntimeProjectLayout,
) -> Result<AdmittedBundle, ForegroundDataExecutionError> {
    let bundle_dir = layout.acquisition_bundle_directory();

    let expected_identity = OwnedRuntimeIdentity::http_acquisition(
        layout.source_name(),
        HttpSourceContractV1::CONTRACT_VERSION,
    );

    let bundle = admit_http_runtime_bundle_owned(&bundle_dir, &expected_identity)
        .map_err(ForegroundDataExecutionError::HttpBundleAdmission)?;

    let actual_identity = bundle
        .runtime_information()
        .identity()
        .into_owned_identity();

    if actual_identity != expected_identity {
        return Err(ForegroundDataExecutionError::RuntimeIdentityMismatch {
            expected: format!("{expected_identity:?}"),
            actual: format!("{actual_identity:?}"),
        });
    }

    Ok(AdmittedBundle::Acquisition(AdmittedAcquisitionBundle {
        bundle,
        identity: actual_identity,
    }))
}

fn admit_processing_bundle(
    layout: &RuntimeProjectLayout,
) -> Result<AdmittedBundle, ForegroundDataExecutionError> {
    let bundle_dir = layout.processing_bundle_directory();

    let expected_identity = OwnedRuntimeIdentity::http_processing(
        layout.source_name(),
        ProcessingSourceContractV1::CONTRACT_VERSION,
    );

    let bundle = admit_processing_runtime_bundle_owned(&bundle_dir, &expected_identity)
        .map_err(ForegroundDataExecutionError::ProcessingBundleAdmission)?;

    let actual_identity = bundle
        .runtime_information()
        .identity()
        .into_owned_identity();

    if actual_identity != expected_identity {
        return Err(ForegroundDataExecutionError::RuntimeIdentityMismatch {
            expected: format!("{expected_identity:?}"),
            actual: format!("{actual_identity:?}"),
        });
    }

    Ok(AdmittedBundle::Processing(AdmittedProcessingBundle {
        bundle,
        identity: actual_identity,
    }))
}

// ---------------------------------------------------------------------------
// Pre-launch integrity recheck
// ---------------------------------------------------------------------------

/// Immediately before spawning: verify the admitted executable still matches
/// the admitted artifact (size + SHA-256). Reject symlinks.
pub fn recheck_executable_integrity(
    admitted: &AdmittedBundle,
) -> Result<(), ExecutableIntegrityError> {
    let executable = admitted.executable_path();
    let original = admitted.admitted_artifact();

    // Reject symlinks.
    let symlink_meta = std::fs::symlink_metadata(executable).map_err(|e| {
        ExecutableIntegrityError::Inspection(crate::build::RuntimeArtifactHashError::Read {
            path: executable.to_path_buf(),
            source: e,
        })
    })?;
    if symlink_meta.file_type().is_symlink() {
        return Err(ExecutableIntegrityError::Changed {
            path: executable.to_path_buf(),
            expected: original.clone(),
            actual: original.clone(),
        });
    }

    // Re-hash.
    let current = hash_runtime_executable(executable)
        .map_err(ExecutableIntegrityError::Inspection)?;

    if current.size() != original.size() {
        return Err(ExecutableIntegrityError::Changed {
            path: executable.to_path_buf(),
            expected: original.clone(),
            actual: current.clone(),
        });
    }
    if current.sha256() != original.sha256() {
        return Err(ExecutableIntegrityError::Changed {
            path: executable.to_path_buf(),
            expected: original.clone(),
            actual: current,
        });
    }

    Ok(())
}

/// Returns true if the admitted HTTP bundle registers a resume handler.
pub fn acquisition_bundle_has_resume(bundle: &AdmittedAcquisitionBundle) -> bool {
    bundle.bundle.runtime_information().resume_handler_registered()
}
