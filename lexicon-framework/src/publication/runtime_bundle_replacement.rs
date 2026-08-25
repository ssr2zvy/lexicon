use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::build::runtime_staging::RuntimeBundleStagingTransferError;
use crate::build::{RuntimeBundleAdmissionError, StagedHttpRuntimeBundle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplacementState {
    Prepared,
    Committed,
    RolledBack,
}

#[derive(Debug)]
pub(crate) enum RuntimeBundleReplacementError {
    DestinationParentMissing { path: PathBuf },
    DestinationParentNotDirectory { path: PathBuf },
    DestinationParentIsSymlink { path: PathBuf },
    DestinationIsSymlink { path: PathBuf },
    DestinationNotDirectory { path: PathBuf },
    BackupPathExists { path: PathBuf },
    StagingTransfer { source: RuntimeBundleStagingTransferError },
    StagedBundleAdmission { source: RuntimeBundleAdmissionError },
    CreateBackupPath { path: PathBuf, source: io::Error },
    MoveDestinationToBackup {
        destination: PathBuf,
        backup: PathBuf,
        source: io::Error,
    },
    MoveStagedBundleToDestination {
        staged: PathBuf,
        destination: PathBuf,
        source: io::Error,
    },
    RestoreDestinationFromBackup {
        destination: PathBuf,
        backup: PathBuf,
        source: io::Error,
    },
    PrepareFailed {
        destination: PathBuf,
        backup: Option<PathBuf>,
        primary: io::Error,
        restore: Option<io::Error>,
    },
    DestinationParentSync {
        path: PathBuf,
        source: io::Error,
    },
    RemoveBackup { path: PathBuf, source: io::Error },
    RemoveDestination { path: PathBuf, source: io::Error },
    RestoreBackup { destination: PathBuf, backup: PathBuf, source: io::Error },
    InvalidTransition { expected: &'static str, actual: &'static str },
}

impl fmt::Display for RuntimeBundleReplacementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DestinationParentMissing { path } => {
                write!(formatter, "destination parent '{}' does not exist", path.display())
            }
            Self::DestinationParentNotDirectory { path } => {
                write!(formatter, "destination parent '{}' is not a directory", path.display())
            }
            Self::DestinationParentIsSymlink { path } => {
                write!(formatter, "destination parent '{}' must not be a symlink", path.display())
            }
            Self::DestinationIsSymlink { path } => {
                write!(formatter, "destination bundle '{}' must not be a symlink", path.display())
            }
            Self::DestinationNotDirectory { path } => {
                write!(formatter, "destination bundle '{}' is not a directory", path.display())
            }
            Self::BackupPathExists { path } => {
                write!(formatter, "backup path '{}' already exists", path.display())
            }
            Self::StagingTransfer { source } => {
                write!(formatter, "failed to transfer staged bundle into publication ownership: {source}")
            }
            Self::StagedBundleAdmission { source } => {
                write!(formatter, "staged bundle admission failed before publication: {source}")
            }
            Self::CreateBackupPath { path, source } => {
                write!(formatter, "failed to construct a backup path under '{}': {source}", path.display())
            }
            Self::MoveDestinationToBackup { destination, backup, source } => {
                write!(
                    formatter,
                    "failed to move existing destination '{}' to backup '{}': {source}",
                    destination.display(),
                    backup.display()
                )
            }
            Self::MoveStagedBundleToDestination { staged, destination, source } => {
                write!(
                    formatter,
                    "failed to move staged bundle '{}' into destination '{}': {source}",
                    staged.display(),
                    destination.display()
                )
            }
            Self::RestoreDestinationFromBackup { destination, backup, source } => {
                write!(
                    formatter,
                    "failed to restore backup '{}' back to destination '{}': {source}",
                    backup.display(),
                    destination.display()
                )
            }
            Self::PrepareFailed {
                destination,
                backup,
                primary,
                restore,
            } => {
                if let Some(backup_path) = backup {
                    write!(
                        formatter,
                        "prepare failed for destination '{}': {}. backup restoration {}.",
                        destination.display(),
                        primary,
                        match restore {
                            Some(_) => "failed",
                            None => "succeeded",
                        }
                    )?;
                    if let Some(restore_error) = restore {
                        write!(formatter, " Backup path: {}. Restore error: {restore_error}", backup_path.display())?;
                    }
                    Ok(())
                } else {
                    write!(
                        formatter,
                        "prepare failed for destination '{}': {}",
                        destination.display(),
                        primary
                    )
                }
            }
            Self::DestinationParentSync { path, source } => {
                write!(
                    formatter,
                    "failed to synchronize destination parent '{}': {source}",
                    path.display()
                )
            }
            Self::RemoveBackup { path, source } => {
                write!(formatter, "failed to remove backup '{}': {source}", path.display())
            }
            Self::RemoveDestination { path, source } => {
                write!(formatter, "failed to remove destination '{}': {source}", path.display())
            }
            Self::RestoreBackup {
                destination,
                backup,
                source,
            } => {
                write!(
                    formatter,
                    "failed to restore backup '{}' to destination '{}': {source}",
                    backup.display(),
                    destination.display()
                )
            }
            Self::InvalidTransition { expected, actual } => {
                write!(
                    formatter,
                    "invalid replacement transition: expected state {expected}, found {actual}"
                )
            }
        }
    }
}

impl std::error::Error for RuntimeBundleReplacementError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::StagingTransfer { source } => Some(source),
            Self::StagedBundleAdmission { source } => Some(source),
            Self::CreateBackupPath { source, .. }
            | Self::MoveDestinationToBackup { source, .. }
            | Self::MoveStagedBundleToDestination { source, .. }
            | Self::RestoreDestinationFromBackup { source, .. }
            | Self::DestinationParentSync { source, .. }
            | Self::RemoveBackup { source, .. }
            | Self::RemoveDestination { source, .. }
            | Self::RestoreBackup { source, .. } => Some(source),
            Self::DestinationParentMissing { .. }
            | Self::DestinationParentNotDirectory { .. }
            | Self::DestinationParentIsSymlink { .. }
            | Self::DestinationIsSymlink { .. }
            | Self::DestinationNotDirectory { .. }
            | Self::BackupPathExists { .. }
            | Self::PrepareFailed { .. }
            | Self::InvalidTransition { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublishedRuntimeBundle {
    path: PathBuf,
}

impl PublishedRuntimeBundle {
    pub(crate) fn path(&self) -> &Path {
        self.path.as_path()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedRuntimeBundleReplacement {
    destination: PathBuf,
    backup: Option<PathBuf>,
    parent: PathBuf,
    state: ReplacementState,
}

impl PreparedRuntimeBundleReplacement {
    pub(crate) fn commit(mut self) -> Result<PublishedRuntimeBundle, RuntimeBundleReplacementError> {
        let destination = self.destination.clone();
        let backup = self.backup.clone();
        let parent = self.parent.clone();
        let state = self.state;
        if !matches!(state, ReplacementState::Prepared) {
            let actual = match state {
                ReplacementState::Prepared => "Prepared",
                ReplacementState::Committed => "Committed",
                ReplacementState::RolledBack => "RolledBack",
            };
            return Err(RuntimeBundleReplacementError::InvalidTransition {
                expected: "Prepared",
                actual,
            });
        }

        if let Some(backup_path) = backup.as_ref() {
            fs::remove_dir_all(backup_path).map_err(|source| RuntimeBundleReplacementError::RemoveBackup {
                path: backup_path.to_path_buf(),
                source,
            })?;
        }

        sync_parent_if_supported(&parent).map_err(|source| RuntimeBundleReplacementError::DestinationParentSync {
            path: parent.clone(),
            source,
        })?;

        self.state = ReplacementState::Committed;
        Ok(PublishedRuntimeBundle { path: destination })
    }

    pub(crate) fn rollback(mut self) -> Result<(), RuntimeBundleReplacementError> {
        let destination = self.destination.clone();
        let backup = self.backup.clone();
        let parent = self.parent.clone();
        let state = self.state;
        if !matches!(state, ReplacementState::Prepared) {
            let actual = match state {
                ReplacementState::Prepared => "Prepared",
                ReplacementState::Committed => "Committed",
                ReplacementState::RolledBack => "RolledBack",
            };
            return Err(RuntimeBundleReplacementError::InvalidTransition {
                expected: "Prepared",
                actual,
            });
        }

        let result = rollback_published_bundle(&destination, backup.as_deref(), &parent);
        self.state = ReplacementState::RolledBack;
        result
    }
}

impl Drop for PreparedRuntimeBundleReplacement {
    fn drop(&mut self) {
        if !matches!(self.state, ReplacementState::Prepared) {
            return;
        }

        let destination = self.destination.clone();
        let backup = self.backup.clone();
        let parent = self.parent.clone();
        let _ = rollback_published_bundle(&destination, backup.as_deref(), &parent);
        self.state = ReplacementState::RolledBack;
    }
}

pub(crate) fn prepare_runtime_bundle_replacement(
    staged: StagedHttpRuntimeBundle,
    published_bundle_path: &Path,
) -> Result<PreparedRuntimeBundleReplacement, RuntimeBundleReplacementError> {
    let destination = published_bundle_path.to_path_buf();
    let destination_parent = published_bundle_path
        .parent()
        .ok_or_else(|| RuntimeBundleReplacementError::DestinationParentMissing {
            path: destination.clone(),
        })?;

    let destination_parent_metadata = fs::symlink_metadata(destination_parent).map_err(|_| {
        RuntimeBundleReplacementError::DestinationParentMissing {
            path: destination_parent.to_path_buf(),
        }
    })?;
    if destination_parent_metadata.file_type().is_symlink() {
        return Err(RuntimeBundleReplacementError::DestinationParentIsSymlink {
            path: destination_parent.to_path_buf(),
        });
    }
    if !destination_parent_metadata.is_dir() {
        return Err(RuntimeBundleReplacementError::DestinationParentNotDirectory {
            path: destination_parent.to_path_buf(),
        });
    }

    let expected_identity = staged.manifest().runtime_information().identity();
    let staging_directory = staged
        .into_staging_directory()
        .map_err(|source| RuntimeBundleReplacementError::StagingTransfer { source })?;
    if let Err(source) = crate::build::admit_http_runtime_bundle(&staging_directory, expected_identity) {
        let _ = fs::remove_dir_all(&staging_directory);
        return Err(RuntimeBundleReplacementError::StagedBundleAdmission { source });
    }

    let mut backup = None;
    if destination.exists() {
        let backup_path = backup_path_for_destination(destination_parent, &destination)
            .map_err(|source| RuntimeBundleReplacementError::CreateBackupPath {
                path: destination_parent.to_path_buf(),
                source,
            })?;
        if backup_path.exists() {
            return Err(RuntimeBundleReplacementError::BackupPathExists {
                path: backup_path.clone(),
            });
        }

        let destination_metadata = fs::symlink_metadata(&destination).map_err(|source| {
            RuntimeBundleReplacementError::MoveDestinationToBackup {
                destination: destination.clone(),
                backup: backup_path.clone(),
                source,
            }
        })?;
        if destination_metadata.file_type().is_symlink() {
            return Err(RuntimeBundleReplacementError::DestinationIsSymlink {
                path: destination.clone(),
            });
        }
        if !destination_metadata.is_dir() {
            return Err(RuntimeBundleReplacementError::DestinationNotDirectory {
                path: destination.clone(),
            });
        }

        fs::rename(&destination, &backup_path).map_err(|source| RuntimeBundleReplacementError::MoveDestinationToBackup {
            destination: destination.clone(),
            backup: backup_path.clone(),
            source,
        })?;
        backup = Some(backup_path);
    }

    if let Err(primary) = fs::rename(&staging_directory, &destination) {
        if let Some(backup_path) = backup.as_ref() {
            let restore = fs::rename(backup_path, &destination).map_err(|source| {
                RuntimeBundleReplacementError::RestoreDestinationFromBackup {
                    destination: destination.clone(),
                    backup: backup_path.clone(),
                    source,
                }
            });
            match restore {
                Ok(()) => {
                    let _ = fs::remove_dir_all(&staging_directory);
                    return Err(RuntimeBundleReplacementError::PrepareFailed {
                        destination: destination.clone(),
                        backup: Some(backup_path.clone()),
                        primary,
                        restore: None,
                    });
                }
                Err(error) => {
                    let _ = fs::remove_dir_all(&staging_directory);
                    return Err(error);
                }
            }
        }

        let _ = fs::remove_dir_all(&staging_directory);
        return Err(RuntimeBundleReplacementError::PrepareFailed {
            destination: destination.clone(),
            backup: backup.clone(),
            primary,
            restore: None,
        });
    }

    match sync_parent_if_supported(destination_parent) {
        Ok(()) => {}
        Err(source) => {
            return Err(RuntimeBundleReplacementError::DestinationParentSync {
                path: destination_parent.to_path_buf(),
                source,
            });
        }
    }

    Ok(PreparedRuntimeBundleReplacement {
        destination,
        backup,
        parent: destination_parent.to_path_buf(),
        state: ReplacementState::Prepared,
    })
}

fn backup_path_for_destination(parent: &Path, destination: &Path) -> Result<PathBuf, io::Error> {
    let destination_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("bundle");
    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut candidate = parent.join(format!(
        ".{}.lexicon-backup-{}-{}",
        destination_name, std::process::id(), now_nanos,
    ));
    let mut index = 1;
    while candidate.exists() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        candidate = parent.join(format!(
            ".{}.lexicon-backup-{}-{}-{}",
            destination_name,
            std::process::id(),
            now_nanos,
            index,
        ));
        index += 1;
    }
    Ok(candidate)
}

fn sync_parent_if_supported(path: &Path) -> Result<(), io::Error> {
    let file = fs::File::open(path)?;
    match file.sync_all() {
        Ok(()) => Ok(()),
        Err(source)
            if matches!(source.kind(), io::ErrorKind::Unsupported | io::ErrorKind::InvalidInput) =>
        {
            Ok(())
        }
        Err(source) => Err(source),
    }
}

fn rollback_published_bundle(
    destination: &Path,
    backup: Option<&Path>,
    parent: &Path,
) -> Result<(), RuntimeBundleReplacementError> {
    let _ = fs::remove_dir_all(destination);
    if let Some(backup_path) = backup {
        fs::rename(backup_path, destination).map_err(|source| RuntimeBundleReplacementError::RestoreBackup {
            destination: destination.to_path_buf(),
            backup: backup_path.to_path_buf(),
            source,
        })?;
    }
    sync_parent_if_supported(parent).map_err(|source| RuntimeBundleReplacementError::DestinationParentSync {
        path: parent.to_path_buf(),
        source,
    })?;
    Ok(())
}
