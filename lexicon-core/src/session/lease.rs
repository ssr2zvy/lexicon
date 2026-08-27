use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

use crate::session::error::SessionLeaseError;

/// An exclusive cross-process session lease backed by an operating-system file lock.
///
/// The lease is held as long as this value exists. Dropping the value releases the lock.
///
/// The lock file's mere existence is not treated as proof of active ownership;
/// the OS advisory lock is the ownership primitive.
pub struct SessionLease {
    file: File,
    path: PathBuf,
}

impl std::fmt::Debug for SessionLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionLease")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl SessionLease {
    /// Attempt to acquire an exclusive non-blocking lease at `path`.
    ///
    /// Returns `Err(SessionLeaseError::AlreadyOwned)` if another process holds the lock.
    pub(crate) fn acquire(path: PathBuf) -> Result<Self, SessionLeaseError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(SessionLeaseError::Io)?;

        try_lock_exclusive_nonblocking(&file).map_err(|e| match e {
            LockError::Contended => SessionLeaseError::AlreadyOwned,
            LockError::Io(io) => SessionLeaseError::Io(io),
        })?;

        // Record PID for diagnostics only; PID reuse must not be the ownership primitive.
        write_pid_diagnostic(&file);

        Ok(Self { file, path })
    }

    /// Path to the lock file.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for SessionLease {
    fn drop(&mut self) {
        // The OS lock is released when the file descriptor is closed,
        // but we unlock explicitly for clarity.
        unlock(&self.file);
    }
}

// ---------------------------------------------------------------------------
// Platform-specific locking implementation
// ---------------------------------------------------------------------------

enum LockError {
    Contended,
    Io(std::io::Error),
}

#[cfg(unix)]
fn try_lock_exclusive_nonblocking(file: &File) -> Result<(), LockError> {
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();
    let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
        return Err(LockError::Contended);
    }
    Err(LockError::Io(err))
}

#[cfg(windows)]
fn try_lock_exclusive_nonblocking(file: &File) -> Result<(), LockError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        LockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;

    let handle = file.as_raw_handle() as HANDLE;
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        LockFileEx(
            handle,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            u32::MAX,
            u32::MAX,
            &mut overlapped,
        )
    };
    if ok != 0 {
        Ok(())
    } else {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(33) {
            // ERROR_LOCK_VIOLATION
            Err(LockError::Contended)
        } else {
            Err(LockError::Io(err))
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn try_lock_exclusive_nonblocking(_file: &File) -> Result<(), LockError> {
    Err(LockError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "file locking is not supported on this platform",
    )))
}

#[cfg(unix)]
fn unlock(file: &File) {
    use std::os::unix::io::AsRawFd;
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
}

#[cfg(windows)]
fn unlock(file: &File) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::UnlockFileEx;
    use windows_sys::Win32::System::IO::OVERLAPPED;

    let handle = file.as_raw_handle() as HANDLE;
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    unsafe { UnlockFileEx(handle, 0, u32::MAX, u32::MAX, &mut overlapped) };
}

#[cfg(not(any(unix, windows)))]
fn unlock(_file: &File) {}

// ---------------------------------------------------------------------------
// PID diagnostic write
// ---------------------------------------------------------------------------

fn write_pid_diagnostic(mut file: &File) {
    let pid = std::process::id();
    let _ = file.seek(SeekFrom::Start(0));
    let _ = file.write_all(format!("{pid}\n").as_bytes());
    let _ = file.set_len(format!("{pid}\n").len() as u64);
}
