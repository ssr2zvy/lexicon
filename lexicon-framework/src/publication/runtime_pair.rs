use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use lexicon_core::runtime::{RuntimeIdentity, RuntimeOperation};

use crate::build::{
    ProcessingRuntimeBundleAdmissionError, RuntimeBundleAdmissionError, StagedHttpRuntimeBundle,
    StagedProcessingRuntimeBundle, admit_http_runtime_bundle, admit_processing_runtime_bundle,
};
use crate::publication::runtime_bundle_replacement::{
    RuntimeBundleReplacementError, prepare_runtime_bundle_replacement_for_staged_directory,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimePairCleanupWarning {
    AcquisitionBackup { path: PathBuf, error: String },
    ProcessingBackup { path: PathBuf, error: String },
    ParentSync { path: PathBuf, error: String },
}

impl fmt::Display for RuntimePairCleanupWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AcquisitionBackup { path, error } => write!(
                formatter,
                "failed to remove acquisition backup '{}' after publication: {error}",
                path.display()
            ),
            Self::ProcessingBackup { path, error } => write!(
                formatter,
                "failed to remove processing backup '{}' after publication: {error}",
                path.display()
            ),
            Self::ParentSync { path, error } => write!(
                formatter,
                "failed to synchronize parent directory '{}' after publication: {error}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for RuntimePairCleanupWarning {}

#[derive(Debug)]
pub enum RuntimePairPublicationError {
    InvalidDestinations,
    InvalidAcquisitionIdentity,
    InvalidProcessingIdentity,
    AcquisitionStagedAdmission {
        source: RuntimeBundleAdmissionError,
        destination: PathBuf,
        expected_identity: RuntimeIdentity,
    },
    ProcessingStagedAdmission {
        source: ProcessingRuntimeBundleAdmissionError,
        destination: PathBuf,
        expected_identity: RuntimeIdentity,
    },
    PrepareAcquisition {
        source: String,
        destination: PathBuf,
    },
    PrepareProcessing {
        source: String,
        destination: PathBuf,
    },
    AcquisitionDestinationAdmission {
        source: RuntimeBundleAdmissionError,
        destination: PathBuf,
        expected_identity: RuntimeIdentity,
    },
    ProcessingDestinationAdmission {
        source: ProcessingRuntimeBundleAdmissionError,
        destination: PathBuf,
        expected_identity: RuntimeIdentity,
    },
    Rollback {
        acquisition: Option<String>,
        processing: Option<String>,
        acquisition_destination: PathBuf,
        processing_destination: PathBuf,
    },
}

impl fmt::Display for RuntimePairPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDestinations => formatter.write_str(
                "publication destinations are invalid: they must be distinct, non-overlapping, and backed by valid directories",
            ),
            Self::InvalidAcquisitionIdentity => formatter.write_str(
                "expected acquisition identity does not declare the acquisition operation",
            ),
            Self::InvalidProcessingIdentity => formatter.write_str(
                "expected processing identity does not declare the processing operation",
            ),
            Self::AcquisitionStagedAdmission {
                source,
                destination,
                expected_identity,
            } => write!(
                formatter,
                "failed to admit staged acquisition bundle '{}' for identity {:?}: {source}",
                destination.display(),
                expected_identity
            ),
            Self::ProcessingStagedAdmission {
                source,
                destination,
                expected_identity,
            } => write!(
                formatter,
                "failed to admit staged processing bundle '{}' for identity {:?}: {source}",
                destination.display(),
                expected_identity
            ),
            Self::PrepareAcquisition { source, destination } => write!(
                formatter,
                "failed to prepare acquisition replacement at '{}': {source}",
                destination.display()
            ),
            Self::PrepareProcessing { source, destination } => write!(
                formatter,
                "failed to prepare processing replacement at '{}': {source}",
                destination.display()
            ),
            Self::AcquisitionDestinationAdmission {
                source,
                destination,
                expected_identity,
            } => write!(
                formatter,
                "failed to admit installed acquisition destination '{}' for identity {:?}: {source}",
                destination.display(),
                expected_identity
            ),
            Self::ProcessingDestinationAdmission {
                source,
                destination,
                expected_identity,
            } => write!(
                formatter,
                "failed to admit installed processing destination '{}' for identity {:?}: {source}",
                destination.display(),
                expected_identity
            ),
            Self::Rollback {
                acquisition,
                processing,
                acquisition_destination,
                processing_destination,
            } => {
                let mut message = format!(
                    "publication failed after install; rollback attempted for acquisition '{}' and processing '{}'",
                    acquisition_destination.display(),
                    processing_destination.display()
                );
                if let Some(acquisition_error) = acquisition {
                    message.push_str(&format!("; acquisition rollback: {acquisition_error}"));
                }
                if let Some(processing_error) = processing {
                    message.push_str(&format!("; processing rollback: {processing_error}"));
                }
                formatter.write_str(&message)
            }
        }
    }
}

impl std::error::Error for RuntimePairPublicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AcquisitionStagedAdmission { source, .. } => Some(source),
            Self::ProcessingStagedAdmission { source, .. } => Some(source),
            Self::AcquisitionDestinationAdmission { source, .. } => Some(source),
            Self::ProcessingDestinationAdmission { source, .. } => Some(source),
            Self::PrepareAcquisition { .. }
            | Self::PrepareProcessing { .. }
            | Self::InvalidDestinations
            | Self::InvalidAcquisitionIdentity
            | Self::InvalidProcessingIdentity
            | Self::Rollback { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct PublishedRuntimePair {
    acquisition_directory: PathBuf,
    processing_directory: PathBuf,
    cleanup_warnings: Vec<RuntimePairCleanupWarning>,
}

impl PublishedRuntimePair {
    pub fn acquisition_directory(&self) -> &Path {
        self.acquisition_directory.as_path()
    }

    pub fn processing_directory(&self) -> &Path {
        self.processing_directory.as_path()
    }

    pub fn cleanup_warnings(&self) -> &[RuntimePairCleanupWarning] {
        self.cleanup_warnings.as_slice()
    }
}

pub fn publish_runtime_pair(
    acquisition: StagedHttpRuntimeBundle,
    processing: StagedProcessingRuntimeBundle,
    acquisition_destination: &Path,
    processing_destination: &Path,
    expected_acquisition_identity: RuntimeIdentity,
    expected_processing_identity: RuntimeIdentity,
) -> Result<PublishedRuntimePair, RuntimePairPublicationError> {
    if validate_destination_pair(acquisition_destination, processing_destination).is_err() {
        return Err(RuntimePairPublicationError::InvalidDestinations);
    }
    if expected_acquisition_identity.operation() != RuntimeOperation::Acquisition {
        return Err(RuntimePairPublicationError::InvalidAcquisitionIdentity);
    }
    if expected_processing_identity.operation() != RuntimeOperation::Processing {
        return Err(RuntimePairPublicationError::InvalidProcessingIdentity);
    }

    let acquisition_staged = acquisition;
    let processing_staged = processing;

    admit_http_runtime_bundle(
        acquisition_staged.directory(),
        expected_acquisition_identity,
    )
    .map_err(
        |source| RuntimePairPublicationError::AcquisitionStagedAdmission {
            source,
            destination: acquisition_destination.to_path_buf(),
            expected_identity: expected_acquisition_identity,
        },
    )?;
    admit_processing_runtime_bundle(processing_staged.directory(), expected_processing_identity)
        .map_err(
            |source| RuntimePairPublicationError::ProcessingStagedAdmission {
                source,
                destination: processing_destination.to_path_buf(),
                expected_identity: expected_processing_identity,
            },
        )?;

    let mut acquisition_prepared = prepare_runtime_bundle_replacement_for_staged_directory(
        acquisition_staged
            .into_owned_staged_runtime_directory()
            .map_err(
                |source| RuntimePairPublicationError::AcquisitionStagedAdmission {
                    source: RuntimeBundleAdmissionError::BundleMetadata {
                        path: acquisition_destination.to_path_buf(),
                        source: std::io::Error::new(std::io::ErrorKind::Other, source.to_string()),
                    },
                    destination: acquisition_destination.to_path_buf(),
                    expected_identity: expected_acquisition_identity,
                },
            )?,
        acquisition_destination,
    )
    .map_err(|source| RuntimePairPublicationError::PrepareAcquisition {
        source: source.to_string(),
        destination: acquisition_destination.to_path_buf(),
    })?;

    let mut processing_prepared = match prepare_runtime_bundle_replacement_for_staged_directory(
        processing_staged
            .into_owned_staged_runtime_directory()
            .map_err(
                |source| RuntimePairPublicationError::ProcessingStagedAdmission {
                    source: ProcessingRuntimeBundleAdmissionError::BundleMetadata {
                        path: processing_destination.to_path_buf(),
                        source: std::io::Error::new(std::io::ErrorKind::Other, source.to_string()),
                    },
                    destination: processing_destination.to_path_buf(),
                    expected_identity: expected_processing_identity,
                },
            )?,
        processing_destination,
    ) {
        Ok(prepared) => prepared,
        Err(source) => {
            let _ = acquisition_prepared.rollback();
            return Err(RuntimePairPublicationError::PrepareProcessing {
                source: source.to_string(),
                destination: processing_destination.to_path_buf(),
            });
        }
    };

    if let Err(_) =
        admit_http_runtime_bundle(acquisition_destination, expected_acquisition_identity)
    {
        let processing_rollback = processing_prepared
            .rollback()
            .err()
            .map(|error| error.to_string());
        let acquisition_rollback = acquisition_prepared
            .rollback()
            .err()
            .map(|error| error.to_string());
        return Err(RuntimePairPublicationError::Rollback {
            acquisition: acquisition_rollback,
            processing: processing_rollback,
            acquisition_destination: acquisition_destination.to_path_buf(),
            processing_destination: processing_destination.to_path_buf(),
        });
    }

    if let Err(_) =
        admit_processing_runtime_bundle(processing_destination, expected_processing_identity)
    {
        let processing_rollback = processing_prepared
            .rollback()
            .err()
            .map(|error| error.to_string());
        let acquisition_rollback = acquisition_prepared
            .rollback()
            .err()
            .map(|error| error.to_string());
        return Err(RuntimePairPublicationError::Rollback {
            acquisition: acquisition_rollback,
            processing: processing_rollback,
            acquisition_destination: acquisition_destination.to_path_buf(),
            processing_destination: processing_destination.to_path_buf(),
        });
    }

    acquisition_prepared.mark_committed();
    processing_prepared.mark_committed();

    let mut warnings = Vec::new();
    if let Err(error) = acquisition_prepared.cleanup_backup() {
        if let Some(backup_path) = acquisition_prepared.backup_path() {
            warnings.push(RuntimePairCleanupWarning::AcquisitionBackup {
                path: backup_path.to_path_buf(),
                error: error.to_string(),
            });
        } else {
            warnings.push(RuntimePairCleanupWarning::ParentSync {
                path: acquisition_prepared.parent_path().to_path_buf(),
                error: error.to_string(),
            });
        }
    }
    if let Err(error) = processing_prepared.cleanup_backup() {
        if let Some(backup_path) = processing_prepared.backup_path() {
            warnings.push(RuntimePairCleanupWarning::ProcessingBackup {
                path: backup_path.to_path_buf(),
                error: error.to_string(),
            });
        } else {
            warnings.push(RuntimePairCleanupWarning::ParentSync {
                path: processing_prepared.parent_path().to_path_buf(),
                error: error.to_string(),
            });
        }
    }

    Ok(PublishedRuntimePair {
        acquisition_directory: acquisition_destination.to_path_buf(),
        processing_directory: processing_destination.to_path_buf(),
        cleanup_warnings: warnings,
    })
}

fn validate_destination_pair(
    acquisition_destination: &Path,
    processing_destination: &Path,
) -> Result<(), ()> {
    if acquisition_destination == processing_destination
        || path_is_ancestor(acquisition_destination, processing_destination)
        || path_is_ancestor(processing_destination, acquisition_destination)
    {
        return Err(());
    }

    let acquisition_parent = acquisition_destination.parent().ok_or(())?;
    let processing_parent = processing_destination.parent().ok_or(())?;
    validate_parent_directory(acquisition_parent)?;
    validate_parent_directory(processing_parent)?;
    validate_existing_destination(acquisition_destination)?;
    validate_existing_destination(processing_destination)?;
    Ok(())
}

fn validate_parent_directory(parent: &Path) -> Result<(), ()> {
    let metadata = fs::symlink_metadata(parent).map_err(|_| ())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(());
    }
    Ok(())
}

fn validate_existing_destination(destination: &Path) -> Result<(), ()> {
    if !destination.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(destination).map_err(|_| ())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(());
    }
    Ok(())
}

fn path_is_ancestor(ancestor: &Path, candidate: &Path) -> bool {
    if ancestor == candidate {
        return false;
    }
    candidate.starts_with(ancestor)
}
