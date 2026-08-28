use lexicon_core::runtime::RuntimeExecutionMode;
use lexicon_core::session::SessionIdentity;

use crate::data::request::DataOperation;

/// A successful foreground data execution outcome.
///
/// This value may be returned only when **all** of the following are true:
///
/// 1. The exact admitted executable was launched (no substitution after bundle admission).
/// 2. The child process exited with code zero.
/// 3. The detailed session record is in `Succeeded` state.
/// 4. The detailed record's identity fields (project, runtime, session, operation,
///    execution mode, supervision mode) match the prepared invocation.
/// 5. The root `session_status.json` identifies the same current session.
/// 6. The root summary state and revision agree with the detailed record.
/// 7. No reconciliation error remains unresolved.
/// 8. The supervisor retained its session lease throughout steps 1–7.
#[derive(Debug)]
pub struct ForegroundDataOutcome {
    pub project: String,
    pub source: String,
    pub operation: DataOperation,
    pub session: SessionIdentity,
    pub execution_mode: RuntimeExecutionMode,
}

/// Observed child process termination, without collapsing to a Boolean.
#[derive(Debug, Clone)]
pub enum ObservedChildTermination {
    ExitCode(i32),
    Signaled { signal: Option<i32> },
    UnknownAbnormalTermination,
}

/// Outcome of successfully handing a prepared session off to the background
/// operator host.
///
/// This is returned once the operator-host process has acquired the session
/// lease (durable ownership confirmed); it does not imply the operation
/// itself has finished. The operator host continues running and supervising
/// the runtime independently of the initiating process from this point on.
#[derive(Debug)]
pub struct BackgroundHandoffOutcome {
    pub project: String,
    pub source: String,
    pub operation: DataOperation,
    pub session: SessionIdentity,
}
