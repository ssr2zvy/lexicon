use std::fmt;

use lexicon_core::runtime::invocation::{
    RuntimeInvocationConstructionError, RuntimeInvocationValueError,
};
use lexicon_core::runtime::invocation_transport::RuntimeInvocationTransportEncodingError;
use lexicon_core::session::{
    SessionFailureCode, SessionFailureKind, SessionState, SessionStoreError,
};

use crate::build::{
    ProcessingRuntimeBundleAdmissionError, RuntimeArtifactHashError, RuntimeBundleAdmissionError,
};
use crate::data::outcome::ObservedChildTermination;
use crate::session::SessionCoordinationError;

// ---------------------------------------------------------------------------
// Typed sub-errors for project discovery and layout validation
// ---------------------------------------------------------------------------

/// Error during project root discovery.
#[derive(Debug)]
pub enum ProjectDiscoveryError {
    /// Failed to read the current working directory.
    CurrentDirectory(std::io::Error),
    /// `find_project_root` failed: no `lexicon.toml` found, or nested project.
    FindRoot(String),
}

impl fmt::Display for ProjectDiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentDirectory(e) => write!(f, "failed to determine current directory: {e}"),
            Self::FindRoot(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ProjectDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentDirectory(e) => Some(e),
            Self::FindRoot(_) => None,
        }
    }
}

/// Error parsing or validating `lexicon.toml`.
#[derive(Debug)]
pub enum ProjectConfigurationError {
    /// Failed to read `lexicon.toml`.
    Read(std::io::Error),
    /// `lexicon.toml` could not be parsed as TOML.
    TomlDecode(String),
    /// The parsed project schema is invalid.
    Schema(String),
    /// The resolved project identity is invalid.
    Identity(String),
    /// Catch-all for other configuration errors.
    Other(String),
}

impl fmt::Display for ProjectConfigurationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(e) => write!(f, "failed to read lexicon.toml: {e}"),
            Self::TomlDecode(msg) => write!(f, "failed to parse lexicon.toml: {msg}"),
            Self::Schema(msg) => write!(f, "invalid project schema: {msg}"),
            Self::Identity(msg) => write!(f, "invalid project identity: {msg}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ProjectConfigurationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(e) => Some(e),
            _ => None,
        }
    }
}

/// Error validating the runtime project layout (source directories, protocol root, etc.).
#[derive(Debug)]
pub enum RuntimeProjectLayoutError {
    /// The configured sources root is not a directory or is outside the project root.
    SourcesRoot(SourcesRootValidationError),
    /// The source identity failed validation.
    SourceIdentity(RuntimeInvocationValueError),
    /// A required path exists but is not a directory (may be a file or symlink).
    NotADirectory { path: std::path::PathBuf, kind: PathKind },
    /// A required path does not exist.
    MissingPath { path: std::path::PathBuf, kind: PathKind },
    /// A symlink was found where the path policy prohibits symlinks.
    SymlinkNotPermitted { path: std::path::PathBuf },
    /// Failed to read filesystem metadata for a path.
    MetadataIo { path: std::path::PathBuf, source: std::io::Error },
    /// Lexical path containment check failed.
    PathContainment(PathContainmentError),
}

impl fmt::Display for RuntimeProjectLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourcesRoot(e) => write!(f, "sources root validation failed: {e}"),
            Self::SourceIdentity(e) => write!(f, "invalid source name: {e}"),
            Self::NotADirectory { path, kind } => {
                write!(f, "{kind} at {} is not a directory", path.display())
            }
            Self::MissingPath { path, kind } => {
                write!(f, "{kind} not found at {}", path.display())
            }
            Self::SymlinkNotPermitted { path } => {
                write!(f, "symlink found where a real directory is required: {}", path.display())
            }
            Self::MetadataIo { path, source } => {
                write!(f, "failed to read metadata for {}: {source}", path.display())
            }
            Self::PathContainment(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RuntimeProjectLayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourcesRoot(e) => Some(e),
            Self::SourceIdentity(e) => Some(e),
            Self::MetadataIo { source, .. } => Some(source),
            Self::PathContainment(e) => Some(e),
            _ => None,
        }
    }
}

/// A human-readable label for a required filesystem path.
#[derive(Debug, Clone, Copy)]
pub enum PathKind {
    SourcesRoot,
    SourceDirectory,
    ProtocolRoot,
    OperationRoot,
    RawDataDirectory,
    ProcessedDataDirectory,
    RuntimeBundleDirectory,
}

impl fmt::Display for PathKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SourcesRoot => "configured sources root",
            Self::SourceDirectory => "source directory",
            Self::ProtocolRoot => "HTTP protocol root",
            Self::OperationRoot => "operation workspace root",
            Self::RawDataDirectory => "data/raw directory",
            Self::ProcessedDataDirectory => "data/processed directory",
            Self::RuntimeBundleDirectory => "runtime bundle directory",
        })
    }
}

/// Error when the configured sources root fails validation.
#[derive(Debug)]
pub enum SourcesRootValidationError {
    /// Not contained within the project root.
    OutsideProjectRoot {
        sources_root: std::path::PathBuf,
        project_root: std::path::PathBuf,
    },
    /// Exists but is not a directory.
    NotADirectory(std::path::PathBuf),
    /// Failed to read filesystem metadata.
    MetadataIo { path: std::path::PathBuf, source: std::io::Error },
}

impl fmt::Display for SourcesRootValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideProjectRoot { sources_root, project_root } => write!(
                f,
                "sources root {} is not contained within project root {}",
                sources_root.display(),
                project_root.display()
            ),
            Self::NotADirectory(path) => {
                write!(f, "sources root {} exists but is not a directory", path.display())
            }
            Self::MetadataIo { path, source } => {
                write!(f, "failed to read metadata for sources root {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for SourcesRootValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MetadataIo { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Error when a trusted path containment invariant is violated.
#[derive(Debug)]
pub enum PathContainmentError {
    /// Sources root is not contained within the project root (lexical check).
    SourcesRootOutsideProject {
        sources_root: std::path::PathBuf,
        project_root: std::path::PathBuf,
    },
    /// Source name contains path traversal components.
    SourceNameTraversal(String),
    /// Protocol root does not match the expected canonical path.
    ProtocolRootMismatch {
        actual: std::path::PathBuf,
        expected: std::path::PathBuf,
    },
}

impl fmt::Display for PathContainmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourcesRootOutsideProject { sources_root, project_root } => write!(
                f,
                "sources root {} is not contained within project root {}",
                sources_root.display(),
                project_root.display()
            ),
            Self::SourceNameTraversal(name) => {
                write!(f, "source name '{}' contains path traversal", name)
            }
            Self::ProtocolRootMismatch { actual, expected } => write!(
                f,
                "protocol root {} does not equal expected {}",
                actual.display(),
                expected.display()
            ),
        }
    }
}

impl std::error::Error for PathContainmentError {}

// ---------------------------------------------------------------------------
// Typed sub-errors for invocation construction
// ---------------------------------------------------------------------------

/// Error building the `RuntimeInvocationEnvelopeV1` before launch.
#[derive(Debug)]
pub enum ForegroundInvocationConstructionError {
    /// The project identity extracted from the project name is invalid.
    InvalidProjectIdentity(RuntimeInvocationValueError),
    /// The session identity is invalid.
    InvalidSessionIdentity(RuntimeInvocationValueError),
    /// The envelope itself failed construction (contract version, execution mode, etc.).
    EnvelopeConstruction(RuntimeInvocationConstructionError),
}

impl fmt::Display for ForegroundInvocationConstructionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProjectIdentity(e) => {
                write!(f, "invalid project identity for invocation: {e}")
            }
            Self::InvalidSessionIdentity(e) => {
                write!(f, "invalid session identity for invocation: {e}")
            }
            Self::EnvelopeConstruction(e) => {
                write!(f, "invocation envelope construction failed: {e}")
            }
        }
    }
}

impl std::error::Error for ForegroundInvocationConstructionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidProjectIdentity(e) => Some(e),
            Self::InvalidSessionIdentity(e) => Some(e),
            Self::EnvelopeConstruction(e) => Some(e),
        }
    }
}

// ---------------------------------------------------------------------------
// Typed error for any pre-launch preparation failure
// ---------------------------------------------------------------------------

/// Any error that occurs after `PreparedSessionLaunch` exists but before a
/// successful spawn. Used as the `preparation` field of
/// [`ForegroundDataExecutionError::PreparationFailureAndPersistenceFailure`].
#[derive(Debug)]
pub enum ForegroundPreparationError {
    InvocationConstruction(ForegroundInvocationConstructionError),
    InvocationEncoding(RuntimeInvocationTransportEncodingError),
    ExecutableIntegrityChanged {
        path: std::path::PathBuf,
        detail: String,
    },
    ExecutableIntegrityCheck(RuntimeArtifactHashError),
    ProcessSpawn(std::io::Error),
}

impl fmt::Display for ForegroundPreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvocationConstruction(e) => write!(f, "{e}"),
            Self::InvocationEncoding(e) => write!(f, "invocation transport encoding failed: {e}"),
            Self::ExecutableIntegrityChanged { path, detail } => write!(
                f,
                "runtime executable changed after admission at {}: {detail}",
                path.display()
            ),
            Self::ExecutableIntegrityCheck(e) => {
                write!(f, "pre-launch executable integrity check failed: {e}")
            }
            Self::ProcessSpawn(e) => write!(f, "failed to launch runtime process: {e}"),
        }
    }
}

impl std::error::Error for ForegroundPreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvocationConstruction(e) => Some(e),
            Self::InvocationEncoding(e) => Some(e),
            Self::ExecutableIntegrityCheck(e) => Some(e),
            Self::ProcessSpawn(e) => Some(e),
            Self::ExecutableIntegrityChanged { .. } => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Wait-recovery failure
// ---------------------------------------------------------------------------

/// Collected errors from a failed wait-recovery path.
#[derive(Debug)]
pub struct WaitRecoveryFailure {
    /// The original error from `Child::wait()`.
    pub wait_error: std::io::Error,
    /// Error from `Child::kill()`, if kill was attempted.
    pub kill_error: Option<std::io::Error>,
    /// Error from the reap `wait()` after kill, if attempted.
    pub reap_error: Option<std::io::Error>,
    /// Error from session reconciliation during recovery, if attempted.
    pub reconciliation_error: Option<SessionCoordinationError>,
}

impl fmt::Display for WaitRecoveryFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "wait failed: {}", self.wait_error)?;
        if let Some(e) = &self.kill_error {
            write!(f, "; kill failed: {e}")?;
        }
        if let Some(e) = &self.reap_error {
            write!(f, "; reap failed: {e}")?;
        }
        if let Some(e) = &self.reconciliation_error {
            write!(f, "; session reconciliation failed: {e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for WaitRecoveryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.wait_error)
    }
}

// ---------------------------------------------------------------------------
// Top-level execution error
// ---------------------------------------------------------------------------

/// Top-level typed error hierarchy for `execute_foreground_data`.
#[derive(Debug)]
pub enum ForegroundDataExecutionError {
    /// `--bg` was supplied; background execution is not implemented in this milestone.
    BackgroundModeUnsupported,
    /// Project root discovery failed (no `lexicon.toml` found or nested project).
    ProjectDiscovery(ProjectDiscoveryError),
    /// Project configuration (`lexicon.toml`) is invalid or malformed.
    ProjectConfiguration(ProjectConfigurationError),
    /// The runtime project layout (sources root, source dir, protocol root, etc.) is invalid.
    ProjectLayout(RuntimeProjectLayoutError),
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
    /// Invocation envelope construction failed, and the session was transitioned to Failed.
    InvocationConstruction(ForegroundInvocationConstructionError),
    /// Invocation transport encoding failed, and the session was transitioned to Failed.
    InvocationEncoding(RuntimeInvocationTransportEncodingError),
    /// Pre-launch executable integrity recheck detected a change after admission.
    ExecutableIntegrityChanged { path: std::path::PathBuf, detail: String },
    /// Pre-launch executable integrity recheck encountered an I/O or hash error.
    ExecutableIntegrityCheck(RuntimeArtifactHashError),
    /// Process spawn failed; the session was transitioned to Failed.
    ProcessSpawn {
        source: std::io::Error,
        /// If session failure persistence also failed, the cause is included here.
        persistence_failure: Option<SessionCoordinationError>,
    },
    /// A preparation-phase failure occurred and persistence of the session-failed transition
    /// also failed. Both errors are preserved.
    PreparationFailureAndPersistenceFailure {
        preparation: ForegroundPreparationError,
        persistence: SessionCoordinationError,
    },
    /// Waiting for the child process failed and the recovery path also encountered errors.
    ProcessWaitRecovery(Box<WaitRecoveryFailure>),
    /// The child exited with zero but the session record was not in a terminal state, and
    /// transitioning it to Failed also failed.
    AbnormalTerminationPersistence {
        termination: ObservedChildTermination,
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
    /// The detailed session record's identity fields do not match the prepared invocation.
    SessionIdentityDisagreement {
        field: &'static str,
        expected: String,
        actual: String,
    },
    /// The root session summary disagrees with the detailed record after successful termination,
    /// and rebuilding the summary also failed.
    RootSummaryReconciliationFailed {
        detail: String,
        rebuild_error: Option<SessionStoreError>,
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
        failure_kind: SessionFailureKind,
        failure_code: SessionFailureCode,
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
    ExitSessionDisagreement {
        termination: ObservedChildTermination,
        durable_state: SessionState,
    },
}

impl fmt::Display for ForegroundDataExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackgroundModeUnsupported => {
                f.write_str("background execution (--bg) is not supported in this release")
            }
            Self::ProjectDiscovery(e) => write!(f, "project discovery failed: {e}"),
            Self::ProjectConfiguration(e) => write!(f, "project configuration error: {e}"),
            Self::ProjectLayout(e) => write!(f, "project layout error: {e}"),
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
            Self::StaleSessionReconciliation(err) => {
                write!(f, "stale session reconciliation failed: {err}")
            }
            Self::SessionSelection(err) => write!(f, "session selection failed: {err}"),
            Self::ResumeHandlerUnavailable => {
                f.write_str("the previous acquisition failed and resume is not available for this source; use --abandon-past-fail to abandon the failed session and start a fresh run")
            }
            Self::Abandonment(err) => write!(f, "session abandonment failed: {err}"),
            Self::SessionPreparation(err) => write!(f, "session preparation failed: {err}"),
            Self::InvocationConstruction(e) => {
                write!(f, "invocation envelope construction failed: {e}")
            }
            Self::InvocationEncoding(e) => {
                write!(f, "invocation transport encoding failed: {e}")
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
            Self::PreparationFailureAndPersistenceFailure { preparation, persistence } => write!(
                f,
                "preparation failed: {preparation}; additionally, session failure persistence failed: {persistence}"
            ),
            Self::ProcessWaitRecovery(e) => {
                write!(f, "runtime process wait failed with incomplete recovery: {e}")
            }
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
            Self::SessionIdentityDisagreement { field, expected, actual } => write!(
                f,
                "session identity disagreement after termination: field '{field}' expected '{expected}', found '{actual}'"
            ),
            Self::RootSummaryReconciliationFailed { detail, rebuild_error: None } => {
                write!(f, "root session summary disagreement: {detail}")
            }
            Self::RootSummaryReconciliationFailed { detail, rebuild_error: Some(e) } => {
                write!(f, "root session summary disagreement: {detail}; rebuild also failed: {e}")
            }
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
                "{operation} failed for source '{source}' (session {session}): kind={}, code={}, exit={exit_code}",
                failure_kind.identifier(),
                failure_code.identifier(),
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
            Self::ExitSessionDisagreement { termination, durable_state } => {
                write!(
                    f,
                    "child exit status and session record disagree: termination={termination:?}, durable_state={durable_state:?}"
                )
            }
        }
    }
}

impl std::error::Error for ForegroundDataExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ProjectDiscovery(e) => Some(e),
            Self::ProjectConfiguration(e) => Some(e),
            Self::ProjectLayout(e) => Some(e),
            Self::HttpBundleAdmission(err) => Some(err),
            Self::ProcessingBundleAdmission(err) => Some(err),
            Self::StaleSessionReconciliation(err) => Some(err),
            Self::SessionSelection(err) => Some(err),
            Self::Abandonment(err) => Some(err),
            Self::SessionPreparation(err) => Some(err),
            Self::InvocationConstruction(e) => Some(e),
            Self::InvocationEncoding(e) => Some(e),
            Self::ExecutableIntegrityCheck(err) => Some(err),
            Self::ProcessSpawn { source, .. } => Some(source),
            Self::PreparationFailureAndPersistenceFailure { preparation, .. } => Some(preparation),
            Self::ProcessWaitRecovery(e) => Some(e.as_ref()),
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
