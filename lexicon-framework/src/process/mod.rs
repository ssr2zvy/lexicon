//! FOREGROUND-01 cross-platform process supervision boundary.
//!
//! The audit requires a `SupervisedChild` and `ProcessTreeLauncher` trait
//! pair so the foreground supervisor can:
//!
//! * Spawn the runtime in a new process group (Unix) or a Windows Job
//!   Object whose `KILL_ON_JOB_CLOSE` limit guarantees a tree reap.
//! * Probe for cancellation through a typed `CancellationSource`.
//! * Drive a graceful shutdown, escalate to forced termination on
//!   timeout, and wait until the OS confirms the tree is gone.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::Path;
use std::process::ExitStatus;
use std::time::Duration;

pub mod unix;
pub mod windows;
pub mod cancellation;

/// The class of cancellation the operator host received. The audit maps
/// shell signal conventions back to platform-flavoured variants so the
/// `CancellationSource` can surface them without leaking libc or win32
/// types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationKind {
    /// Equates to SIGINT on Unix and CTRL_C_EVENT on Windows.
    Interrupt,
    /// Equates to SIGTERM on Unix and CTRL_BREAK_EVENT on Windows.
    Terminate,
    /// Equates to SIGHUP/disconnect on Unix and console close on Windows.
    ConsoleClose,
}

impl CancellationKind {
    /// POSIX signal number this kind maps to, if the runtime cares. Unix
    /// callers may use this in their signal mask.
    pub fn unix_signal(&self) -> Option<u8> {
        match self {
            Self::Interrupt => Some(2),
            Self::Terminate => Some(15),
            Self::ConsoleClose => Some(1),
        }
    }

    /// Windows control-event identifier. `windows.rs` wires this into
    /// `GenerateConsoleCtrlEvent`.
    pub fn windows_control_event(&self) -> Option<u32> {
        match self {
            Self::Interrupt => Some(0), // CTRL_C_EVENT
            Self::Terminate => Some(1), // CTRL_BREAK_EVENT
            Self::ConsoleClose => Some(2), // close/logoff/shutdown
        }
    }
}

impl fmt::Display for CancellationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Interrupt => formatter.write_str("interrupt"),
            Self::Terminate => formatter.write_str("terminate"),
            Self::ConsoleClose => formatter.write_str("console-close"),
        }
    }
}

/// Source of cancellation requested by the operator host. Concrete
/// platforms provide atomic+destructively-checked variants; tests
/// substitute a deterministic variant.
pub trait CancellationSource: Send + Sync {
    /// Return the most recent cancellation kind requested, or `None` if
    /// the supervisor should continue polling. After returning `Some`,
    /// implementations should remain `None` until the next cancellation
    /// lands so a graceful shutdown is not re-triggered erroneously.
    fn requested(&self) -> Option<CancellationKind>;
}

/// Owned, platform-specific child handle. The supervisor stores one of
/// these inside `RunningForegroundExecution`; it never touches a bare
/// `std::process::Child` directly.
pub trait SupervisedChild {
    /// The OS process id of the direct child (not the group root).
    fn id(&self) -> u32;
    /// Non-blocking poll for the direct child to exit.
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>>;
    /// Begin a graceful stop. The OS-level semantics differ by platform
    /// (SIGINT to a process group vs `GenerateConsoleCtrlEvent`).
    fn request_graceful_shutdown(
        &mut self,
        kind: CancellationKind,
    ) -> std::io::Result<()>;
    /// Force-terminate the entire tree. POSIX `kill(-pgid, SIGKILL)`;
    /// Windows `TerminateJobObject`.
    fn force_terminate_tree(&mut self) -> std::io::Result<()>;
    /// Block until the direct child has been reaped (no zombies). The
    /// audit forbids dropping the child without reaping.
    fn wait_reaped(&mut self) -> std::io::Result<ExitStatus>;
}

/// Spawns runtime executables under the supervision model. `ProductionLauncher`
/// selects the platform implementation at compile time; tests inject a
/// fake launcher.
pub trait ProcessTreeLauncher: Send + Sync {
    fn spawn(
        &self,
        executable: &Path,
        arguments: &[OsString],
        context_environment: &OsStr,
        working_directory: &Path,
    ) -> std::io::Result<Box<dyn SupervisedChild>>;
}

/// Tunables used by the foreground supervisor's wait loop. The audit
/// mandates explicit, bounded values rather than implicit "wait a bit"
/// heuristics.
#[derive(Clone, Copy, Debug)]
pub struct CancellationPolicy {
    /// Time the graceful shutdown has to escalate to forced termination.
    pub graceful_timeout: Duration,
    /// Time between cancellation polls when nothing has changed. The audit
    /// prefers polling within a 50–250 ms window over async signals.
    pub poll_interval: Duration,
}

impl CancellationPolicy {
    pub const fn default_cli_policy() -> Self {
        Self {
            graceful_timeout: Duration::from_secs(15),
            poll_interval: Duration::from_millis(100),
        }
    }
}

/// Default production launcher selected at compile time. Tests bypass
/// this entirely.
pub struct ProductionLauncher;

impl ProcessTreeLauncher for ProductionLauncher {
    fn spawn(
        &self,
        executable: &Path,
        arguments: &[OsString],
        context_environment: &OsStr,
        working_directory: &Path,
    ) -> std::io::Result<Box<dyn SupervisedChild>> {
        #[cfg(unix)]
        {
            unix::launch(
                executable,
                arguments,
                context_environment,
                working_directory,
            )
        }
        #[cfg(windows)]
        {
            windows::launch(
                executable,
                arguments,
                context_environment,
                working_directory,
            )
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (executable, arguments, context_environment, working_directory);
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "lexicon foreground supervision has no platform implementation for this target",
            ))
        }
    }
}

/// Outcome of the foreground wait loop. The audit distinguishes
/// graceful cancellation, forced termination, completed-normal, and
/// uncertain outcomes so callers can keep clear accounting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisionOutcome {
    /// Direct child completed with `status` while no cancellation landed.
    Completed { status: Option<i32> },
    /// Operator requested cancellation; we sent a graceful signal and
    /// the process exited on its own before the timeout.
    CancelledGracefully {
        kind: CancellationKind,
        status: Option<i32>,
    },
    /// Operator requested cancellation; the graceful attempt timed out
    /// and we escalated to forced termination. The status code may still
    /// be absent if the OS reported the kill differently.
    CancelledForcefully {
        kind: CancellationKind,
        status: Option<i32>,
    },
    /// Operator requested cancellation; we are unsure whether the
    /// process is gone. This is the audit's "ownership-uncertain" case
    /// (FOREGROUND-02 explicitly reserves it).
    CancellationUncertain {
        kind: CancellationKind,
        status: Option<i32>,
    },
}

impl fmt::Display for SupervisionOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Completed { status } => {
                write!(formatter, "completed (status {status:?})")
            }
            Self::CancelledGracefully { kind, status } => {
                write!(formatter, "cancelled gracefully ({kind}, status {status:?})")
            }
            Self::CancelledForcefully { kind, status } => {
                write!(formatter, "cancelled forcefully ({kind}, status {status:?})")
            }
            Self::CancellationUncertain { kind, status } => {
                write!(formatter, "cancellation uncertain ({kind}, status {status:?})")
            }
        }
    }
}

/// Cancellation-aware wait loop. The audit's FOREGROUND-01 requires this
/// replacement for the `Child::wait` blocking call: we poll both the
/// cancellation source and the child, escalate on timeout, and report
/// `SupervisionOutcome` rather than a bare `ExitStatus`.
pub fn wait_with_cancellation(
    child: &mut dyn SupervisedChild,
    cancellation: &dyn CancellationSource,
    policy: CancellationPolicy,
) -> std::io::Result<SupervisionOutcome> {
    use std::time::Instant;
    let start = Instant::now();
    loop {
        if let Some(kind) = cancellation.requested() {
            let _ = child.request_graceful_shutdown(kind);
            let deadline = Instant::now() + policy.graceful_timeout;
            loop {
                if let Some(status) = child.try_wait()? {
                    let status = status.code();
                    return Ok(SupervisionOutcome::CancelledGracefully { kind, status });
                }
                if Instant::now() >= deadline {
                    child.force_terminate_tree()?;
                    let final_status = child.wait_reaped()?.code();
                    return Ok(SupervisionOutcome::CancelledForcefully {
                        kind,
                        status: final_status,
                    });
                }
                std::thread::sleep(policy.poll_interval);
            }
        }

        if let Some(status) = child.try_wait()? {
            return Ok(SupervisionOutcome::Completed {
                status: status.code(),
            });
        }

        if Instant::now() >= start + policy.graceful_timeout {
            // No cancellation observed but the OS-level wait budget
            // elapsed without an exit. We conservatively classify as
            // uncertain so the caller re-spawns rather than misreporting
            // success.
            return Ok(SupervisionOutcome::CancellationUncertain {
                kind: CancellationKind::Terminate,
                status: None,
            });
        }

        std::thread::sleep(policy.poll_interval);
    }
}
