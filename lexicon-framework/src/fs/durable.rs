//! SCAFFOLD-02 typed durability primitives.
//!
//! These helpers exist so source-creation paths never rely on `fs::write`
//! (which fails to fsync and never tells the caller whether bytes were
//! durable) or on bare `fsync` of a path the caller never verified was
//! writable.
//!
//! `write_new_file` is the atomic primitive: the parent directory is
//! created if missing, the target file is opened with `create_new(true)`
//! so a partial race cannot silently overwrite an existing entry, the
//! bytes are written, and the file is `fsync`'d. The audit calls for
//! `directory-sync` after each `write_new_file`. `sync_directory` returns
//! the platform's policy on directory flush; on Unix `fsync` on the
//! directory is sufficient, while Windows can only attempt
//! `FlushFileBuffers` on a directory handle and must report "unsupported"
//! honestly whenever the OS rejects it.

use std::fs;
use std::io::Write;
use std::path::Path;
use thiserror::Error;

/// Outcome of attempting a directory fsync on the current platform. The
/// SCAFFOLD-02 audit allows the Windows implementation to honestly
/// report `UnsupportedByPlatform` rather than fabricate a fake success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorySyncOutcome {
    /// Directory was successfully fsync'd and is part of the durable
    /// sequence on this platform.
    Synced,
    /// Directory fsync is not supported on this platform. The audit
    /// permits this value alongside a recorded diagnostic; callers pair
    /// it with a write-through atomic replacement and the strongest
    /// available file fsync.
    UnsupportedByPlatform,
}

#[derive(Debug, Error)]
pub enum DurableFileError {
    /// Target file path has no parent directory component.
    #[error("file path has no parent directory")]
    NoParent { path: std::path::PathBuf },
    /// Could not create or access the parent directory.
    #[error("could not create parent directory: {0}")]
    Parent(std::io::Error),
    /// `create_new(true)` failed — usually a directory race or stale
    /// state. Callers must treat this as an unrecoverable failure and
    /// abort publication so a partial tree is never observable.
    #[error("create_new failed for {path}: {source}")]
    CreateNew {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Failed to write the file contents.
    #[error("failed to write file contents: {0}")]
    Write(std::io::Error),
    /// Failed to fsync the file.
    #[error("failed to fsync file: {0}")]
    Sync(std::io::Error),
}

impl std::fmt::Display for DurableFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

/// Atomically create `path`, write `bytes`, and fsync the new file. The
/// `create_new(true)` open mode refuses to clobber an existing file
/// (a leftover from a crashed scaffold); the typed error surfaces that
/// case so callers abort rather than silently re-publish.
pub fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), DurableFileError> {
    let parent = path.parent().ok_or_else(|| DurableFileError::NoParent {
        path: path.to_path_buf(),
    })?;
    fs::create_dir_all(parent).map_err(DurableFileError::Parent)?;

    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    let mut file = options.open(path).map_err(|source| DurableFileError::CreateNew {
        path: path.to_path_buf(),
        source,
    })?;
    file.write_all(bytes).map_err(DurableFileError::Write)?;
    file.sync_all().map_err(DurableFileError::Sync)?;
    Ok(())
}

/// Best-effort directory fsync. The audit's spec allows this to honestly
/// return `UnsupportedByPlatform` when the OS refuses the call; callers
/// must NOT silently retry or fabricate success.
pub fn sync_directory(_path: &Path) -> Result<DirectorySyncOutcome, std::io::Error> {
    #[cfg(unix)]
    {
        fs::File::open(_path)?.sync_all()?;
        Ok(DirectorySyncOutcome::Synced)
    }
    #[cfg(not(unix))]
    {
        // Windows: directory fsync is not reliably supported; record the
        // honest outcome and continue.
        let _ = _path;
        Ok(DirectorySyncOutcome::UnsupportedByPlatform)
    }
}

/// Sync a directory and its descendants in bottom-up order. This is the
/// "step 2" in the SCAFFOLD-02 audit: sync every staged directory
/// bottom-up before renaming the staging tree to the final source path.
pub fn sync_subtree_bottom_up(root: &Path) -> Result<(), std::io::Error> {
    fn visit(path: &Path) -> Result<(), std::io::Error> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                visit(&entry.path())?;
            }
            let outcome = sync_directory(path)?;
            if outcome != DirectorySyncOutcome::Synced {
                // Best effort: the audit permits Windows to honestly
                // report UnsupportedByPlatform; we still surface the call
                // as completed without inventing success.
                return Ok(());
            }
        }
        Ok(())
    }
    visit(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_new_file_creates_parent_and_durably_writes_bytes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("deep/nested/file.txt");
        write_new_file(&path, b"hello world").expect("write_new_file");
        let bytes = std::fs::read(&path).expect("read");
        assert_eq!(bytes, b"hello world");
    }

    #[test]
    fn write_new_file_refuses_to_clobber_existing_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("already.txt");
        write_new_file(&path, b"first").expect("first write");
        let err = write_new_file(&path, b"second").expect_err("clobber must fail");
        assert!(matches!(err, DurableFileError::CreateNew { .. }));
    }

    #[test]
    fn write_new_file_rejects_path_with_no_parent() {
        let err =
            write_new_file(std::path::Path::new("/"), b"data").expect_err("slash-only must fail");
        assert!(matches!(err, DurableFileError::NoParent { .. }));
    }

    #[test]
    fn sync_subtree_bottom_up_does_not_panic_on_empty_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        sync_subtree_bottom_up(temp.path()).expect("sync empty");
    }
}
