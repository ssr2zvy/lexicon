use lexicon_core::runtime::RuntimeExecutionMode;
use lexicon_core::session::SessionIdentity;

use crate::data::request::DataOperation;

/// A successful foreground data execution outcome.
///
/// Represents:
/// - Child exited successfully (exit code 0).
/// - The detailed session record transitioned to `Succeeded`.
/// - The root session summary agrees.
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
