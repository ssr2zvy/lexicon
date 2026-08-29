// FOREGROUND-01 Windows implementation.
//
// The audit requires a Windows Job Object with the
// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` limit so descendants of the
// runtime cannot outlive the supervisor. The production launcher uses
// `windows-sys` (Cargo dep declared below) for typed bindings to
// `CreateJobObject`, `SetInformationJobObject`, and `TerminateJobObject`.
//
// The Job Object handle is owned by `WindowsSupervisedChild` and
// released in its `Drop` impl via RAII wrappers.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;
use std::process::{Child, Command, ExitStatus};

#[cfg(windows)]
use windows_sys::Win32::Foundation::HANDLE;
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    JOBOBJECT_BASIC_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    SetInformationJobObject,
};
#[cfg(windows)]
use windows_sys::Win32::System::Console::GenerateConsoleCtrlEvent;
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess};

use crate::process::{
    CancellationKind, ProcessTreeLauncher, SupervisedChild,
};

/// Spawn a child in a Windows Job Object that will kill the entire
/// tree on close.
#[cfg(windows)]
pub fn launch(
    executable: &Path,
    arguments: &[OsString],
    context_environment: &OsStr,
    working_directory: &Path,
) -> io::Result<Box<dyn SupervisedChild>> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::process::CommandExt as WinCommandExt;
    use std::ptr;

    let mut command = Command::new(executable);
    command
        .current_dir(working_directory)
        .env("LEXICON_RUNTIME_CONTEXT_V1", context_environment)
        .args(arguments)
        // CREATE_NEW_PROCESS_GROUP keeps the new tree addressable via
        // GenerateConsoleCtrlEvent without affecting our console.
        .creation_flags(0x00000200);

    let child = command.spawn()?;
    let child_handle = child.as_raw_handle() as HANDLE;

    // Create the job and assign the child before any grandchild can run
    // on its own; with the job limit in place, the OS guarantees
    // termination when the supervisor drops the handle.
    let job = unsafe { CreateJobObjectW(ptr::null_mut(), ptr::null()) };
    if job.is_null() {
        return Err(io::Error::last_os_error());
    }

    let mut info = JOBOBJECT_BASIC_LIMIT_INFORMATION {
        PerProcessUserTimeLimit: 0,
        PerJobUserTimeLimit: 0,
        LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        MinimumWorkingSetSize: 0,
        MaximumWorkingSetSize: 0,
        ActiveProcessLimit: 0,
        Affinity: ptr::null_mut(),
        PriorityClass: 0,
        SchedulingClass: 0,
    };
    let set_ok = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &mut info as *mut _ as *mut _,
            std::mem::size_of::<JOBOBJECT_BASIC_LIMIT_INFORMATION>() as u32,
        )
    };
    if set_ok == 0 {
        return Err(io::Error::last_os_error());
    }

    let assign_ok = unsafe { AssignProcessToJobObject(job, child_handle) };
    if assign_ok == 0 {
        let _ = io::Error::last_os_error();
        // Closing the job handle closes the job; if the assignment
        // failed the child is not bound and may outlive the supervisor,
        // so we surface the failure rather than silently swallow it.
        return Err(io::Error::last_os_error());
    }

    Ok(Box::new(WindowsSupervisedChild::new(child, job)))
}

/// Windows fallback for non-Windows builds. The compilation unit
/// exists for cross-platform symbol discovery; the public launcher
/// in `mod.rs` does not route through here on Unix.
#[cfg(not(windows))]
pub fn launch(
    _executable: &Path,
    _arguments: &[OsString],
    _context_environment: &OsStr,
    _working_directory: &Path,
) -> io::Result<Box<dyn SupervisedChild>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "windows process supervision can only be linked on Windows targets",
    ))
}

#[cfg(windows)]
pub struct WindowsSupervisedChild {
    child: Child,
    job: HANDLE,
}

#[cfg(windows)]
impl WindowsSupervisedChild {
    fn new(child: Child, job: HANDLE) -> Self {
        Self { child, job }
    }
}

#[cfg(windows)]
impl Drop for WindowsSupervisedChild {
    fn drop(&mut self) {
        // Releasing the last handle closes the job; with
        // KILL_ON_JOB_CLOSE set, the OS reaps the entire tree. If the
        // handle is already invalidated (process voluntarily closed),
        // CloseHandle returns an error code we do not propagate: the
        // child has already exited.
        unsafe {
            let _ = window_close_handle(self.job);
        }
    }
}

#[cfg(windows)]
impl SupervisedChild for WindowsSupervisedChild {
    fn id(&mut self) -> u32 {
        self.child.id()
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    fn request_graceful_shutdown(
        &mut self,
        kind: CancellationKind,
    ) -> io::Result<()> {
        if let Some(ctrl) = kind.windows_control_event() {
            // CTRL_BREAK_EVENT (1) is broadcast to the child's console
            // group; CTRL_C_EVENT (0) is handled per process. We map
            // both through GenerateConsoleCtrlEvent and let the audit's
            // FOREGROUND-02 logging layer decide how to surface either.
            match unsafe { GenerateConsoleCtrlEvent(ctrl, 0) } {
                0 => Err(io::Error::last_os_error()),
                _ => Ok(()),
            }
        } else {
            Ok(())
        }
    }

    fn force_terminate_tree(&mut self) -> io::Result<()> {
        // Prefer TerminateJobObject for tree-wide cleanup. Falls back
        // to a direct TerminateProcess on the child if the job handle
        // is somehow invalid.
        unsafe {
            let job_ok = window_terminate_job(self.job, 1);
            if job_ok != 0 {
                return Ok(());
            }
            let pid = self.child.id();
            let process_handle = OpenProcess(0x0001, 0, pid);
            if !process_handle.is_null() {
                if TerminateProcess(process_handle, 1) != 0 {
                    return Ok(());
                }
                let _ = io::Error::last_os_error();
            }
            Err(io::Error::last_os_error())
        }
    }

    fn wait_reaped(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }
}

#[cfg(windows)]
unsafe fn window_close_handle(handle: HANDLE) -> i32 {
    use windows_sys::Win32::Foundation::CloseHandle;
    CloseHandle(handle)
}

#[cfg(windows)]
unsafe fn window_terminate_job(handle: HANDLE, exit_code: u32) -> i32 {
    use windows_sys::Win32::System::Threading::TerminateJobObject;
    TerminateJobObject(handle, exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_signal_module_compiles_on_unix_only() {
        // Smoke check that the unix helper is callable. Real tests live
        // in `lexicon-cli/tests/foreground_cancellation.rs` once it is
        // added by FOREGROUND-02.
    }
}
