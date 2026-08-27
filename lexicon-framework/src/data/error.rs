use std::fmt;

use lexicon_core::session::SessionStoreError;

use crate::build::{
    ProcessingRuntimeBundleAdmissionError, RuntimeArtifactHashError, RuntimeBundleAdmissionError,
};
use crate::session::SessionCoordinationError;

/// Top-level typed error hierarchy for `execute_foreground_data`.
#[derive(Debug)]
pub enum ForegroundDataExecutionError {
    /// `--bg` was supplied; background execution is not implemented in this milestone.
    BackgroundModeUnsupported,
    /// Project root discovery failed (no `lexicon.toml` found or nested project).
    ProjectDiscovery(String),
    /// Project configuration (`lexicon.toml`) is invalid or malformed.
    ProjectConfiguration(String),
    /// The configured sources root failed validation.
    ConfiguredSourcesRoot(String),
    /// The requested source name failed safe-identifier validation.
    InvalidSourceIdentity(String),
    /// The source directory does not exist inside the sources root.
    MissingSource { source_name: String, path: std::path::PathBuf },
    /// The expected protocol layout directory does not exist.
    MissingProtocolLayout { source_name: String, path: std::path::PathBuf },
    /// The expected operation workspace directory does not exist.
    MissingOperationLayout { operation: String, path: std::path::PathBuf },
    /// The runtime bundle directory does not exist; the user must run `lexicon source build`.
    MissingRuntimeBundle { operation: String, path: std::path::PathBuf },
    /// HTTP bundle admission failed.
    HttpBundleAdmission(RuntimeBundleAdmissionError),
    /// Processing bundle admission failed.
    ProcessingBundleAdmission(ProcessingRuntimeBundleAdmissionError),
    /// The admitted bundle identity does not match the expected runtime identity.
    RuntimeIdentityMismatch { expected: String, actual: String },
    /// Trusted runtime-path construction failed.
    TrustedPathConstruction(String),
    /// Stale-session reconciliation failed.
    StaleSessionReconciliation(SessionStoreError),
    /// Session selection policy rejected the current state.
    SessionSelection(SessionCoordinationError),
    /// The HTTP resume handler is not registered; explicit `--abandon-past-fail` is required.
    ResumeHandlerUnavailable,
    /// Abandonment of the prior failed session failed.
    Abandonment(SessionCoordinationError),
    /// Session preparation failed.
    SessionPreparation(SessionCoordinationError),
    /// Invocation envelope construction failed.
    InvocationConstruction(String),
    /// Invocation transport encoding failed.
    InvocationEncoding(String),
    /// Pre-launch executable integrity recheck failed: the executable changed after admission.
    ExecutableIntegrityChanged { path: std::path::PathBuf, detail: String },
    /// Pre-launch executable integrity recheck encountered an I/O or hash error.
    ExecutableIntegrityCheck(RuntimeArtifactHashError),
    /// Process spawn failed.
    ProcessSpawn {
        source: std::io::Error,
        /// If session failure persistence also failed, the cause is included here.
        persistence_failure: Option<SessionCoordinationError>,
    },
    /// Waiting for the child process to exit failed.
    ProcessWait(std::io::Error),
    /// The child exited with zero but the session record was not in a terminal state, and
    /// transitioning it to Failed also failed.
    AbnormalTerminationPersistence {
        termination: crate::data::outcome::ObservedChildTermination,
        persistence_failure: SessionCoordinationError,
    },
    /// The child exited with nonzero and the session remained Prepared/Running; transitioning
    /// it to Failed also failed.
    AbnormalExitPersistence {
        exit_code: i32,
        persistence_failure: SessionCoordinationError,
    },
    /// The durable session record is missing after child termination.
    MissingTerminalSession(SessionStoreError),
    /// The durable session record is present but cannot be decoded.
    CorruptTerminalSession(SessionStoreError),
    /// The root session status disagrees with the detailed record after successful termination.
    RootSummaryDisagreement(String),
    /// Combined execution and reconciliation failure.
    CombinedExecutionReconciliationFailure {
        execution_detail: String,
        reconciliation_detail: String,
    },
    /// The child exited zero but the session remained in a non-terminal state (Prepared or
    /// Running), which was subsequently transitioned to Failed.
    ZeroExitSessionIncomplete {
        session: String,
        operation: String,
    },
    /// The child exited with a nonzero code.
    ChildFailed {
        operation: String,
        source: String,
        session: String,
        failure_kind: String,
        failure_code: String,
        exit_code: i32,
    },
    /// The child terminated abnormally (signal, abort, etc.).
    AbnormalTermination {
        operation: String,
        source: String,
        session: String,
        signal: Option<i32>,
    },
    /// The child's exit status and durable session state disagree.
    ExitSessionDisagreement { detail: String },
}

impl fmt::Display for ForegroundDataExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackgroundModeUnsupported => {
                f.write_str("background execution (--bg) is not supported in this release")
            }
            Self::ProjectDiscovery(msg) => write!(f, "project discovery failed: {msg}"),
            Self::ProjectConfiguration(msg) => {
                write!(f, "project configuration error: {msg}")
            }
            Self::ConfiguredSourcesRoot(msg) => {
                write!(f, "configured sources root is invalid: {msg}")
            }
            Self::InvalidSourceIdentity(msg) => {
                write!(f, "invalid source name: {msg}")
            }
            Self::MissingSource { source_name, path } => write!(
                f,
                "source '{}' not found at {}",
                source_name,
                path.display()
            ),
            Self::MissingProtocolLayout { source_name, path } => write!(
                f,
                "HTTP protocol layout for source '{}' not found at {}",
                source_name,
                path.display()
            ),
            Self::MissingOperationLayout { operation, path } => write!(
                f,
                "{} operation workspace not found at {}",
                operation,
                path.display()
            ),
            Self::MissingRuntimeBundle { operation, path } => write!(
                f,
                "{} runtime bundle not found at {}; run `lexicon source build` to build the source",
                operation,
                path.display()
            ),
            Self::HttpBundleAdmission(err) => {
                write!(f, "HTTP runtime bundle admission failed: {err}")
            }
            Self::ProcessingBundleAdmission(err) => {
                write!(f, "processing runtime bundle admission failed: {err}")
            }
            Self::RuntimeIdentityMismatch { expected, actual } => write!(
                f,
                "runtime identity mismatch: expected '{expected}', found '{actual}'"
            ),
            Self::TrustedPathConstruction(msg) => {
                write!(f, "trusted runtime path construction failed: {msg}")
            }
            Self::StaleSessionReconciliation(err) => {
                write!(f, "stale session reconciliation failed: {err}")
            }
            Self::SessionSelection(err) => write!(f, "session selection failed: {err}"),
            Self::ResumeHandlerUnavailable => {
                f.write_str("the previous acquisition failed and resume is not available for this source; use --abandon-past-fail to abandon the failed session and start a fresh run")
            }
            Self::Abandonment(err) => write!(f, "session abandonment failed: {err}"),
            Self::SessionPreparation(err) => write!(f, "session preparation failed: {err}"),
            Self::InvocationConstruction(msg) => {
                write!(f, "invocation envelope construction failed: {msg}")
            }
            Self::InvocationEncoding(msg) => {
                write!(f, "invocation transport encoding failed: {msg}")
            }
            Self::ExecutableIntegrityChanged { path, detail } => write!(
                f,
                "runtime executable changed after admission at {}: {detail}",
                path.display()
            ),
            Self::ExecutableIntegrityCheck(err) => {
                write!(f, "pre-launch executable integrity check failed: {err}")
            }
            Self::ProcessSpawn { source, persistence_failure: None } => {
                write!(f, "failed to launch runtime process: {source}")
            }
            Self::ProcessSpawn { source, persistence_failure: Some(pf) } => {
                write!(
                    f,
                    "failed to launch runtime process: {source}; additionally, session failure persistence failed: {pf}"
                )
            }
            Self::ProcessWait(err) => write!(f, "failed to wait for runtime process: {err}"),
            Self::AbnormalTerminationPersistence { termination, persistence_failure } => write!(
                f,
                "runtime terminated abnormally ({termination:?}) and session failure persistence also failed: {persistence_failure}"
            ),
            Self::AbnormalExitPersistence { exit_code, persistence_failure } => write!(
                f,
                "runtime exited with code {exit_code} and session failure persistence also failed: {persistence_failure}"
            ),
            Self::MissingTerminalSession(err) => {
                write!(f, "session record missing after child termination: {err}")
            }
            Self::CorruptTerminalSession(err) => {
                write!(f, "session record corrupt after child termination: {err}")
            }
            Self::RootSummaryDisagreement(msg) => {
                write!(f, "root session summary disagrees after completion: {msg}")
            }
            Self::CombinedExecutionReconciliationFailure {
                execution_detail,
                reconciliation_detail,
            } => write!(
                f,
                "execution failed ({execution_detail}) and reconciliation also failed ({reconciliation_detail})"
            ),
            Self::ZeroExitSessionIncomplete { session, operation } => write!(
                f,
                "{operation} session {session} exited zero but did not reach a terminal state; treated as abnormal"
            ),
            Self::ChildFailed {
                operation,
                source,
                session,
                failure_kind,
                failure_code,
                exit_code,
            } => write!(
                f,
                "{operation} failed for source '{source}' (session {session}): kind={failure_kind}, code={failure_code}, exit={exit_code}"
            ),
            Self::AbnormalTermination { operation, source, session, signal } => {
                if let Some(sig) = signal {
                    write!(
                        f,
                        "{operation} for source '{source}' (session {session}) terminated by signal {sig}"
                    )
                } else {
                    write!(
                        f,
                        "{operation} for source '{source}' (session {session}) terminated abnormally"
                    )
                }
            }
            Self::ExitSessionDisagreement { detail } => {
                write!(f, "child exit status and session record disagree: {detail}")
            }
        }
    }
}

impl std::error::Error for ForegroundDataExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HttpBundleAdmission(err) => Some(err),
            Self::ProcessingBundleAdmission(err) => Some(err),
            Self::StaleSessionReconciliation(err) => Some(err),
            Self::SessionSelection(err) => Some(err),
            Self::Abandonment(err) => Some(err),
            Self::SessionPreparation(err) => Some(err),
            Self::ExecutableIntegrityCheck(err) => Some(err),
            Self::ProcessSpawn { source, .. } => Some(source),
            Self::ProcessWait(err) => Some(err),
            Self::AbnormalTerminationPersistence { persistence_failure, .. } => {
                Some(persistence_failure)
            }
            Self::AbnormalExitPersistence { persistence_failure, .. } => {
                Some(persistence_failure)
            }
            Self::MissingTerminalSession(err) => Some(err),
            Self::CorruptTerminalSession(err) => Some(err),
            _ => None,
        }
    }
}
