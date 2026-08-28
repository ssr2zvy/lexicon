use std::fmt;

use lexicon_core::runtime::{OwnedRuntimeIdentity, RuntimeExecutionMode, RuntimeSupervisionMode};
use lexicon_core::runtime::invocation::{
    RuntimeInvocationConstructionError, RuntimeInvocationValueError,
};
use lexicon_core::runtime::invocation_transport::RuntimeInvocationTransportEncodingError;
use lexicon_core::session::{
    ProjectIdentity, SessionFailureCode, SessionFailureKind, SessionIdentity, SessionLeaseError,
    SessionOperation, SessionState, SessionStoreError,
};

use crate::build::{
    ProcessingRuntimeBundleAdmissionError, RuntimeArtifactHashError, RuntimeBundleAdmissionError,
};
use crate::data::outcome::ObservedChildTermination;
use crate::data::request::DataOperation;
use crate::session::SessionCoordinationError;
use crate::supervision::{OperatorHostInvocationDecodingError, OperatorHostInvocationEncodingError};
use crate::{ProjectConfigLoadError, ProjectRootDiscoveryError};

// ---------------------------------------------------------------------------
// Typed sub-errors for project discovery and layout validation
// ---------------------------------------------------------------------------

/// Error during project root discovery.
#[derive(Debug)]
pub enum ProjectDiscoveryError {
    /// Failed to read the current working directory.
    CurrentDirectory(std::io::Error),
    /// `find_project_root` failed: no `lexicon.toml` found, or nested project.
    FindRoot(ProjectRootDiscoveryError),
}

impl fmt::Display for ProjectDiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentDirectory(e) => write!(f, "failed to determine current directory: {e}"),
            Self::FindRoot(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ProjectDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentDirectory(e) => Some(e),
            Self::FindRoot(err) => Some(err),
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
    /// Typed project-config loading error.
    Load(ProjectConfigLoadError),
}

impl fmt::Display for ProjectConfigurationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(e) => write!(f, "failed to read lexicon.toml: {e}"),
            Self::TomlDecode(msg) => write!(f, "failed to parse lexicon.toml: {msg}"),
            Self::Schema(msg) => write!(f, "invalid project schema: {msg}"),
            Self::Identity(msg) => write!(f, "invalid project identity: {msg}"),
            Self::Load(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ProjectConfigurationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(e) => Some(e),
            Self::Load(err) => Some(err),
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
    ExecutableIntegrity(ExecutableIntegrityError),
    ProcessSpawn(std::io::Error),
}

impl fmt::Display for ForegroundPreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvocationConstruction(e) => write!(f, "{e}"),
            Self::InvocationEncoding(e) => write!(f, "invocation transport encoding failed: {e}"),
            Self::ExecutableIntegrity(e) => write!(f, "{e}"),
            Self::ProcessSpawn(e) => write!(f, "failed to launch runtime process: {e}"),
        }
    }
}

impl std::error::Error for ForegroundPreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvocationConstruction(e) => Some(e),
            Self::InvocationEncoding(e) => Some(e),
            Self::ExecutableIntegrity(e) => Some(e),
            Self::ProcessSpawn(e) => Some(e),
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
    /// Error from `Child::try_wait()`, if a probe failed.
    pub try_wait_error: Option<std::io::Error>,
    /// Error from the reap `wait()` after kill, if attempted.
    pub reap_error: Option<std::io::Error>,
    /// Error while loading the durable session record during recovery, if attempted.
    pub session_load_error: Option<Box<ForegroundDataExecutionError>>,
    /// Error from session reconciliation during recovery, if attempted.
    pub session_reconciliation_error: Option<Box<ForegroundDataExecutionError>>,
    /// Final durable state when recovery reached reconciliation.
    pub final_state: Option<SessionState>,
}

impl fmt::Display for WaitRecoveryFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "wait failed: {}", self.wait_error)?;
        if let Some(e) = &self.kill_error {
            write!(f, "; kill failed: {e}")?;
        }
        if let Some(e) = &self.try_wait_error {
            write!(f, "; try_wait failed: {e}")?;
        }
        if let Some(e) = &self.reap_error {
            write!(f, "; reap failed: {e}")?;
        }
        if let Some(e) = &self.session_load_error {
            write!(f, "; session load failed: {e}")?;
        }
        if let Some(e) = &self.session_reconciliation_error {
            write!(f, "; session reconciliation failed: {e}")?;
        }
        if let Some(state) = self.final_state {
            write!(f, "; final durable state: {state:?}")?;
        }
        Ok(())
    }
}

impl std::error::Error for WaitRecoveryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.wait_error)
    }
}

#[derive(Debug)]
pub enum ExecutableIntegrityError {
    Changed {
        path: std::path::PathBuf,
        expected: crate::build::HashedRuntimeArtifact,
        actual: crate::build::HashedRuntimeArtifact,
    },
    Inspection(RuntimeArtifactHashError),
}

impl fmt::Display for ExecutableIntegrityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Changed { path, expected, actual } => write!(
                f,
                "runtime executable changed after admission at {}: expected size/hash ({}, {}), found ({}, {})",
                path.display(),
                expected.size(),
                expected.sha256(),
                actual.size(),
                actual.sha256()
            ),
            Self::Inspection(err) => write!(f, "pre-launch executable integrity check failed: {err}"),
        }
    }
}

impl std::error::Error for ExecutableIntegrityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inspection(err) => Some(err),
            Self::Changed { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum TerminalSessionIdentityMismatch {
    Project {
        expected: ProjectIdentity,
        actual: ProjectIdentity,
    },
    Runtime {
        expected: OwnedRuntimeIdentity,
        actual: OwnedRuntimeIdentity,
    },
    Session {
        expected: SessionIdentity,
        actual: SessionIdentity,
    },
    Operation {
        expected: SessionOperation,
        actual: SessionOperation,
    },
    ExecutionMode {
        expected: RuntimeExecutionMode,
        actual: RuntimeExecutionMode,
    },
    SupervisionMode {
        expected: RuntimeSupervisionMode,
        actual: RuntimeSupervisionMode,
    },
}

#[derive(Debug)]
pub enum RootSummaryValidationError {
    Missing,
    Load(SessionStoreError),
    SchemaVersionMismatch { expected: u32, actual: u32 },
    ProjectMismatch,
    RuntimeMismatch,
    OperationMismatch,
    MissingCurrentSession,
    SessionMismatch,
    MissingCurrentState,
    StateMismatch { expected: SessionState, actual: SessionState },
    RevisionMismatch { expected: u64, actual: u64 },
}

impl fmt::Display for RootSummaryValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("root session_status.json is missing"),
            Self::Load(err) => write!(f, "failed to load root session_status.json: {err}"),
            Self::SchemaVersionMismatch { expected, actual } => {
                write!(f, "summary schema_version mismatch: expected {expected}, found {actual}")
            }
            Self::ProjectMismatch => f.write_str("summary project does not match detailed record"),
            Self::RuntimeMismatch => f.write_str("summary runtime does not match detailed record"),
            Self::OperationMismatch => f.write_str("summary operation does not match detailed record"),
            Self::MissingCurrentSession => f.write_str("summary current_session is missing"),
            Self::SessionMismatch => f.write_str("summary current_session does not match record"),
            Self::MissingCurrentState => f.write_str("summary current_state is missing"),
            Self::StateMismatch { expected, actual } => {
                write!(f, "summary current_state mismatch: expected {expected:?}, found {actual:?}")
            }
            Self::RevisionMismatch { expected, actual } => {
                write!(f, "summary revision mismatch: expected {expected}, found {actual}")
            }
        }
    }
}

impl std::error::Error for RootSummaryValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Load(err) => Some(err),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum RootSummaryReconciliationError {
    Validation(RootSummaryValidationError),
    Rebuild(SessionStoreError),
    ValidationAfterRebuild(RootSummaryValidationError),
}

impl fmt::Display for RootSummaryReconciliationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(err) => write!(f, "root summary validation failed: {err}"),
            Self::Rebuild(err) => write!(f, "failed to rebuild root summary: {err}"),
            Self::ValidationAfterRebuild(err) => {
                write!(f, "root summary still invalid after rebuild: {err}")
            }
        }
    }
}

impl std::error::Error for RootSummaryReconciliationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(err) => Some(err),
            Self::Rebuild(err) => Some(err),
            Self::ValidationAfterRebuild(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub struct ChildOwnershipUncertainError {
    pub wait_error: std::io::Error,
    pub try_wait_error: Option<std::io::Error>,
    pub kill_error: Option<std::io::Error>,
    pub reap_error: Option<std::io::Error>,
    pub session_load_error: Option<Box<ForegroundDataExecutionError>>,
    pub session_reconciliation_error: Option<Box<ForegroundDataExecutionError>>,
}

impl fmt::Display for ChildOwnershipUncertainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot prove child termination after wait failure: {}", self.wait_error)?;
        if let Some(e) = &self.try_wait_error {
            write!(f, "; try_wait failed: {e}")?;
        }
        if let Some(e) = &self.kill_error {
            write!(f, "; kill failed: {e}")?;
        }
        if let Some(e) = &self.reap_error {
            write!(f, "; reap failed: {e}")?;
        }
        if let Some(e) = &self.session_load_error {
            write!(f, "; session load failed: {e}")?;
        }
        if let Some(e) = &self.session_reconciliation_error {
            write!(f, "; session reconciliation failed: {e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ChildOwnershipUncertainError {
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
    /// Defensive misuse guard: a request with `background: true` reached
    /// `execute_foreground_data`, which only ever runs the foreground path.
    /// The CLI must route `--bg` requests through `execute_background_data` instead.
    BackgroundModeUnsupported,
    /// Failed to encode the operator-host invocation reference.
    OperatorHostEncoding(OperatorHostInvocationEncodingError),
    /// Failed to decode the operator-host invocation reference (operator-host side only).
    OperatorHostDecoding(OperatorHostInvocationDecodingError),
    /// Failed to re-execute the current binary in the `__operator-host` role.
    OperatorHostReExec(std::io::Error),
    /// Polling the session lease while waiting for operator-host ownership failed.
    OperatorHostOwnershipCheckFailed(SessionLeaseError),
    /// The operator-host process exited before it acquired the session lease.
    OperatorHostExitedBeforeOwnership { exit_code: Option<i32> },
    /// Timed out waiting for the operator-host process to acquire the session lease.
    OperatorHostOwnershipTimeout,
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
    /// Pre-launch executable integrity recheck failed.
    ExecutableIntegrity(ExecutableIntegrityError),
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
    /// Waiting/recovery failed and process ownership could not be proven.
    ChildOwnershipUncertain(Box<ChildOwnershipUncertainError>),
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
    SessionIdentityDisagreement(TerminalSessionIdentityMismatch),
    /// Root-summary validation/rebuild failed.
    RootSummaryReconciliationFailed(RootSummaryReconciliationError),
    /// The child exited zero but the session remained in a non-terminal state (Prepared or
    /// Running), which was subsequently transitioned to Failed.
    ZeroExitSessionIncomplete {
        session: SessionIdentity,
        operation: DataOperation,
    },
    /// The child exited with a nonzero code.
    ChildFailed {
        operation: DataOperation,
        source: String,
        session: SessionIdentity,
        failure_kind: SessionFailureKind,
        failure_code: SessionFailureCode,
        exit_code: Option<i32>,
    },
    /// The child terminated abnormally (signal, abort, etc.).
    AbnormalTermination {
        operation: DataOperation,
        source: String,
        session: SessionIdentity,
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
                f.write_str("internal error: a background-execution request reached the foreground execution path")
            }
            Self::OperatorHostEncoding(e) => {
                write!(f, "failed to encode operator-host invocation reference: {e}")
            }
            Self::OperatorHostDecoding(e) => {
                write!(f, "failed to decode operator-host invocation reference: {e}")
            }
            Self::OperatorHostReExec(e) => {
                write!(f, "failed to re-execute the operator-host process: {e}")
            }
            Self::OperatorHostOwnershipCheckFailed(e) => {
                write!(f, "failed to inspect operator-host session lease ownership: {e}")
            }
            Self::OperatorHostExitedBeforeOwnership { exit_code: Some(code) } => write!(
                f,
                "operator-host process exited (code {code}) before it acquired session ownership"
            ),
            Self::OperatorHostExitedBeforeOwnership { exit_code: None } => f.write_str(
                "operator-host process exited abnormally before it acquired session ownership",
            ),
            Self::OperatorHostOwnershipTimeout => f.write_str(
                "timed out waiting for the operator-host process to acquire session ownership",
            ),
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
            Self::ExecutableIntegrity(err) => write!(f, "{err}"),
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
            Self::ChildOwnershipUncertain(e) => {
                write!(f, "fatal supervision failure: child ownership uncertain: {e}")
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
            Self::SessionIdentityDisagreement(mismatch) => match mismatch {
                TerminalSessionIdentityMismatch::Project { expected, actual } => write!(
                    f,
                    "session identity disagreement after termination: project expected '{}', found '{}'",
                    expected.name(),
                    actual.name()
                ),
                TerminalSessionIdentityMismatch::Runtime { expected, actual } => write!(
                    f,
                    "session identity disagreement after termination: runtime expected '{}:{}:{}@{}', found '{}:{}:{}@{}'",
                    expected.source_name(),
                    expected.protocol().identifier(),
                    expected.operation().identifier(),
                    expected.source_contract_version(),
                    actual.source_name(),
                    actual.protocol().identifier(),
                    actual.operation().identifier(),
                    actual.source_contract_version()
                ),
                TerminalSessionIdentityMismatch::Session { expected, actual } => write!(
                    f,
                    "session identity disagreement after termination: session expected '{}', found '{}'",
                    expected.id(),
                    actual.id()
                ),
                TerminalSessionIdentityMismatch::Operation { expected, actual } => write!(
                    f,
                    "session identity disagreement after termination: operation expected '{}', found '{}'",
                    expected.identifier(),
                    actual.identifier()
                ),
                TerminalSessionIdentityMismatch::ExecutionMode { expected, actual } => write!(
                    f,
                    "session identity disagreement after termination: execution mode expected '{}', found '{}'",
                    expected.identifier(),
                    actual.identifier()
                ),
                TerminalSessionIdentityMismatch::SupervisionMode { expected, actual } => write!(
                    f,
                    "session identity disagreement after termination: supervision mode expected '{}', found '{}'",
                    expected.identifier(),
                    actual.identifier()
                ),
            },
            Self::RootSummaryReconciliationFailed(err) => {
                write!(f, "{err}")
            }
            Self::ZeroExitSessionIncomplete { session, operation } => write!(
                f,
                "{} session {} exited zero but did not reach a terminal state; treated as abnormal",
                operation.display_name(),
                session.id()
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
                "{} failed for source '{source}' (session {}): kind={}, code={}{}",
                operation.display_name(),
                session.id(),
                failure_kind.identifier(),
                failure_code.identifier(),
                exit_code.map(|code| format!(", exit={code}")).unwrap_or_default(),
            ),
            Self::AbnormalTermination { operation, source, session, signal } => {
                if let Some(sig) = signal {
                    write!(
                        f,
                        "{} for source '{source}' (session {}) terminated by signal {sig}",
                        operation.display_name(),
                        session.id()
                    )
                } else {
                    write!(
                        f,
                        "{} for source '{source}' (session {}) terminated abnormally",
                        operation.display_name(),
                        session.id()
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
            Self::ExecutableIntegrity(err) => Some(err),
            Self::ProcessSpawn { source, .. } => Some(source),
            Self::PreparationFailureAndPersistenceFailure { preparation, .. } => Some(preparation),
            Self::ProcessWaitRecovery(e) => Some(e.as_ref()),
            Self::ChildOwnershipUncertain(e) => Some(e.as_ref()),
            Self::AbnormalTerminationPersistence { persistence_failure, .. } => {
                Some(persistence_failure)
            }
            Self::AbnormalExitPersistence { persistence_failure, .. } => {
                Some(persistence_failure)
            }
            Self::MissingTerminalSession(err) => Some(err),
            Self::CorruptTerminalSession(err) => Some(err),
            Self::RootSummaryReconciliationFailed(err) => Some(err),
            Self::OperatorHostEncoding(err) => Some(err),
            Self::OperatorHostDecoding(err) => Some(err),
            Self::OperatorHostReExec(err) => Some(err),
            Self::OperatorHostOwnershipCheckFailed(err) => Some(err),
            _ => None,
        }
    }
}
