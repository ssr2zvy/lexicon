use std::fmt;
use std::path::{Path, PathBuf};

use crate::protocols::http::transaction::error::HttpManagedPathError;
use crate::runtime::OwnedRuntimeIdentity;
use crate::session::{ProjectIdentity, SessionIdentity, SessionStoreError};

pub type ProcessingResult<T> = Result<T, ProcessingError>;

// ---------------------------------------------------------------------------
// ProcessingError (source boundary)
// ---------------------------------------------------------------------------

/// Error returned by a processing source handler.
///
/// The source may retain a typed nested cause through [`ProcessingError::source`]
/// or return a Core-safe static message through [`ProcessingError::source_message`].
///
/// `Display` is deliberately bounded: it renders only static text authored at
/// compile time. It never renders SQL, row data, request or response bodies,
/// headers, URLs, or source arguments. Nested causes remain reachable through
/// [`std::error::Error::source`] for programmatic inspection, and the runner never
/// persists source-authored text into session records.
#[derive(Debug)]
pub enum ProcessingError {
    /// A named source operation failed with a retained typed cause.
    Source {
        operation: &'static str,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    /// The source reported a failure using a static Core-safe message.
    SourceMessage { message: &'static str },
}

impl ProcessingError {
    /// Construct a source failure that retains a typed nested cause.
    pub fn source(
        operation: &'static str,
        error: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Source {
            operation,
            source: Box::new(error),
        }
    }

    /// Construct a source failure from a static Core-safe message.
    pub fn source_message(message: &'static str) -> Self {
        Self::SourceMessage { message }
    }

    /// The static operation label, when this failure names one.
    pub fn operation(&self) -> Option<&'static str> {
        match self {
            Self::Source { operation, .. } => Some(operation),
            Self::SourceMessage { .. } => None,
        }
    }

    /// The static message, when this failure carries one.
    pub fn message(&self) -> Option<&'static str> {
        match self {
            Self::Source { .. } => None,
            Self::SourceMessage { message } => Some(message),
        }
    }
}

impl fmt::Display for ProcessingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source { operation, .. } => {
                write!(formatter, "processing source operation failed: {operation}")
            }
            Self::SourceMessage { message } => {
                write!(formatter, "processing source reported a failure: {message}")
            }
        }
    }
}

impl std::error::Error for ProcessingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source { source, .. } => Some(source.as_ref()),
            Self::SourceMessage { .. } => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Context construction
// ---------------------------------------------------------------------------

/// A managed path category, used instead of raw paths in bounded diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingManagedPathCategory {
    OperationRoot,
    SessionDirectory,
    RawDataDirectory,
    ProcessedDataDirectory,
    DatabaseFile,
}

impl ProcessingManagedPathCategory {
    /// Stable snake_case identifier for this managed path category.
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::OperationRoot => "operation_root",
            Self::SessionDirectory => "session_directory",
            Self::RawDataDirectory => "raw_data_directory",
            Self::ProcessedDataDirectory => "processed_data_directory",
            Self::DatabaseFile => "database_file",
        }
    }
}

impl fmt::Display for ProcessingManagedPathCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.identifier())
    }
}

#[derive(Debug)]
pub enum ProcessingContextConstructionError {
    RuntimeProtocolMismatch,
    RuntimeOperationMismatch,
    /// A managed path did not equal the exact path required by the layout.
    ManagedPathDisagreement {
        category: ProcessingManagedPathCategory,
        expected: PathBuf,
        actual: PathBuf,
    },
    /// A catalog entry does not belong to the processing project.
    CatalogProjectMismatch { catalog_index: usize },
    /// A catalog entry does not use the HTTP protocol.
    CatalogProtocolMismatch { catalog_index: usize },
    /// A catalog entry does not belong to the processing runtime source.
    CatalogSourceMismatch { catalog_index: usize },
}

impl ProcessingContextConstructionError {
    /// The managed path category involved, when this failure names one.
    pub fn managed_path_category(&self) -> Option<ProcessingManagedPathCategory> {
        match self {
            Self::ManagedPathDisagreement { category, .. } => Some(*category),
            _ => None,
        }
    }

    /// The exact path the layout required, when this failure names one.
    pub fn expected_path(&self) -> Option<&Path> {
        match self {
            Self::ManagedPathDisagreement { expected, .. } => Some(expected.as_path()),
            _ => None,
        }
    }

    /// The path that was supplied, when this failure names one.
    pub fn actual_path(&self) -> Option<&Path> {
        match self {
            Self::ManagedPathDisagreement { actual, .. } => Some(actual.as_path()),
            _ => None,
        }
    }

    /// The offending catalog index, when this failure names one.
    pub fn catalog_index(&self) -> Option<usize> {
        match self {
            Self::CatalogProjectMismatch { catalog_index }
            | Self::CatalogProtocolMismatch { catalog_index }
            | Self::CatalogSourceMismatch { catalog_index } => Some(*catalog_index),
            _ => None,
        }
    }
}

impl fmt::Display for ProcessingContextConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeProtocolMismatch => {
                formatter.write_str("processing context requires HTTP runtime protocol")
            }
            Self::RuntimeOperationMismatch => {
                formatter.write_str("processing context requires processing runtime operation")
            }
            Self::ManagedPathDisagreement { category, .. } => write!(
                formatter,
                "processing context managed path disagreement: {category}"
            ),
            Self::CatalogProjectMismatch { .. } => formatter.write_str(
                "processing transaction catalog entry does not match processing project",
            ),
            Self::CatalogProtocolMismatch { .. } => formatter
                .write_str("processing transaction catalog entry does not use the HTTP protocol"),
            Self::CatalogSourceMismatch { .. } => formatter.write_str(
                "processing transaction catalog entry does not match processing runtime source",
            ),
        }
    }
}

impl std::error::Error for ProcessingContextConstructionError {}

// ---------------------------------------------------------------------------
// SQLite sidecar policy
// ---------------------------------------------------------------------------

/// The SQLite sidecar file kinds recognized by the processing path policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingDatabaseSidecarKind {
    /// The transient DELETE-mode rollback journal, `<database>-journal`.
    RollbackJournal,
    /// A write-ahead log file, `<database>-wal`. Never permitted.
    WriteAheadLog,
    /// A shared-memory file, `<database>-shm`. Never permitted.
    SharedMemory,
}

impl ProcessingDatabaseSidecarKind {
    /// Stable snake_case identifier for this sidecar kind.
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::RollbackJournal => "rollback_journal",
            Self::WriteAheadLog => "write_ahead_log",
            Self::SharedMemory => "shared_memory",
        }
    }

    /// Filename suffix appended to the canonical database filename.
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::RollbackJournal => "-journal",
            Self::WriteAheadLog => "-wal",
            Self::SharedMemory => "-shm",
        }
    }
}

impl fmt::Display for ProcessingDatabaseSidecarKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.identifier())
    }
}

#[derive(Debug)]
pub enum ProcessingDatabaseSidecarError {
    /// The sidecar path could not be inspected.
    Inspection {
        kind: ProcessingDatabaseSidecarKind,
        path: PathBuf,
        source: std::io::Error,
    },
    /// A symlink exists at a sidecar path.
    Symlink {
        kind: ProcessingDatabaseSidecarKind,
        path: PathBuf,
    },
    /// A non-regular file exists at a sidecar path.
    WrongFileType {
        kind: ProcessingDatabaseSidecarKind,
        path: PathBuf,
    },
    /// A sidecar kind that is never permitted was found on disk.
    ForbiddenSidecarPresent {
        kind: ProcessingDatabaseSidecarKind,
        path: PathBuf,
    },
    /// The transient rollback journal survived transaction completion.
    RollbackJournalNotCleanedUp { path: PathBuf },
}

impl ProcessingDatabaseSidecarError {
    /// The sidecar kind involved in this failure.
    pub fn kind(&self) -> ProcessingDatabaseSidecarKind {
        match self {
            Self::Inspection { kind, .. }
            | Self::Symlink { kind, .. }
            | Self::WrongFileType { kind, .. }
            | Self::ForbiddenSidecarPresent { kind, .. } => *kind,
            Self::RollbackJournalNotCleanedUp { .. } => {
                ProcessingDatabaseSidecarKind::RollbackJournal
            }
        }
    }

    /// The sidecar path involved, retained for programmatic recovery.
    pub fn path(&self) -> &Path {
        match self {
            Self::Inspection { path, .. }
            | Self::Symlink { path, .. }
            | Self::WrongFileType { path, .. }
            | Self::ForbiddenSidecarPresent { path, .. }
            | Self::RollbackJournalNotCleanedUp { path } => path.as_path(),
        }
    }
}

impl fmt::Display for ProcessingDatabaseSidecarError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inspection { kind, .. } => {
                write!(formatter, "failed to inspect SQLite sidecar: {kind}")
            }
            Self::Symlink { kind, .. } => {
                write!(formatter, "SQLite sidecar path is a symlink: {kind}")
            }
            Self::WrongFileType { kind, .. } => {
                write!(
                    formatter,
                    "SQLite sidecar path is not a regular file: {kind}"
                )
            }
            Self::ForbiddenSidecarPresent { kind, .. } => {
                write!(formatter, "SQLite sidecar is not permitted: {kind}")
            }
            Self::RollbackJournalNotCleanedUp { .. } => {
                formatter.write_str("SQLite rollback journal was not cleaned up")
            }
        }
    }
}

impl std::error::Error for ProcessingDatabaseSidecarError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inspection { source, .. } => Some(source),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Database path admission
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ProcessingDatabasePathError {
    ManagedPath(HttpManagedPathError),
    /// The processed-data root did not equal `protocol_root/data/processed`.
    ProcessedRootDisagreement { expected: PathBuf, actual: PathBuf },
    /// The database filename did not match the runtime source name.
    DatabaseNameDisagreement { expected: PathBuf, actual: PathBuf },
    /// A SQLite sidecar path violated the allowed sidecar policy.
    Sidecar(ProcessingDatabaseSidecarError),
}

impl ProcessingDatabasePathError {
    /// The managed-path failure, when present.
    pub fn managed_path_error(&self) -> Option<&HttpManagedPathError> {
        match self {
            Self::ManagedPath(error) => Some(error),
            _ => None,
        }
    }

    /// The sidecar policy failure, when present.
    pub fn sidecar_error(&self) -> Option<&ProcessingDatabaseSidecarError> {
        match self {
            Self::Sidecar(error) => Some(error),
            _ => None,
        }
    }

    /// The exact path the layout required, when this failure names one.
    pub fn expected_path(&self) -> Option<&Path> {
        match self {
            Self::ProcessedRootDisagreement { expected, .. }
            | Self::DatabaseNameDisagreement { expected, .. } => Some(expected.as_path()),
            _ => None,
        }
    }

    /// The path that was supplied, when this failure names one.
    pub fn actual_path(&self) -> Option<&Path> {
        match self {
            Self::ProcessedRootDisagreement { actual, .. }
            | Self::DatabaseNameDisagreement { actual, .. } => Some(actual.as_path()),
            _ => None,
        }
    }
}

impl fmt::Display for ProcessingDatabasePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManagedPath(_) => {
                formatter.write_str("processing database managed path validation failed")
            }
            Self::ProcessedRootDisagreement { .. } => formatter
                .write_str("processing processed-data root does not match the protocol layout"),
            Self::DatabaseNameDisagreement { .. } => formatter
                .write_str("processing database filename does not match the runtime source"),
            Self::Sidecar(_) => {
                formatter.write_str("processing database sidecar policy validation failed")
            }
        }
    }
}

impl std::error::Error for ProcessingDatabasePathError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManagedPath(error) => Some(error),
            Self::Sidecar(error) => Some(error),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Baseline configuration
// ---------------------------------------------------------------------------

/// The baseline SQLite settings the supported processing route requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingDatabaseSetting {
    ForeignKeys,
    JournalMode,
    TransactionActive,
}

impl ProcessingDatabaseSetting {
    /// Stable snake_case identifier for this setting.
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::ForeignKeys => "foreign_keys",
            Self::JournalMode => "journal_mode",
            Self::TransactionActive => "transaction_active",
        }
    }
}

impl fmt::Display for ProcessingDatabaseSetting {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.identifier())
    }
}

#[derive(Debug)]
pub enum ProcessingDatabaseConfigurationError {
    /// The effective setting could not be read back.
    Readback {
        setting: ProcessingDatabaseSetting,
        source: rusqlite::Error,
    },
    /// The effective setting did not match the required value.
    Disagreement {
        setting: ProcessingDatabaseSetting,
        expected: &'static str,
        actual: String,
    },
}

impl ProcessingDatabaseConfigurationError {
    /// The setting involved in this failure.
    pub fn setting(&self) -> ProcessingDatabaseSetting {
        match self {
            Self::Readback { setting, .. } | Self::Disagreement { setting, .. } => *setting,
        }
    }

    /// The required value, when this failure is a disagreement.
    pub fn expected(&self) -> Option<&'static str> {
        match self {
            Self::Disagreement { expected, .. } => Some(expected),
            Self::Readback { .. } => None,
        }
    }

    /// The effective value, when this failure is a disagreement.
    pub fn actual(&self) -> Option<&str> {
        match self {
            Self::Disagreement { actual, .. } => Some(actual.as_str()),
            Self::Readback { .. } => None,
        }
    }

    /// The underlying SQLite error, when present.
    pub fn sqlite_error(&self) -> Option<&rusqlite::Error> {
        match self {
            Self::Readback { source, .. } => Some(source),
            Self::Disagreement { .. } => None,
        }
    }
}

impl fmt::Display for ProcessingDatabaseConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Readback { setting, .. } => write!(
                formatter,
                "failed to read back processing database setting: {setting}"
            ),
            Self::Disagreement { setting, .. } => write!(
                formatter,
                "processing database setting does not match the required baseline: {setting}"
            ),
        }
    }
}

impl std::error::Error for ProcessingDatabaseConfigurationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Readback { source, .. } => Some(source),
            Self::Disagreement { .. } => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Durability
// ---------------------------------------------------------------------------

/// The durability operation that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingDurabilityPhase {
    /// Re-validating the database file after creation.
    DatabaseCreationValidation,
    /// Syncing the database file itself.
    DatabaseFileSync,
    /// Syncing the processed-data directory.
    ProcessedDirectorySync,
}

impl ProcessingDurabilityPhase {
    /// Stable snake_case identifier for this phase.
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::DatabaseCreationValidation => "database_creation_validation",
            Self::DatabaseFileSync => "database_file_sync",
            Self::ProcessedDirectorySync => "processed_directory_sync",
        }
    }
}

impl fmt::Display for ProcessingDurabilityPhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.identifier())
    }
}

#[derive(Debug)]
pub enum ProcessingDatabaseDurabilityError {
    /// A managed filesystem sync failed.
    Sync {
        phase: ProcessingDurabilityPhase,
        path: PathBuf,
        source: std::io::Error,
    },
    /// The database file failed re-validation after creation.
    Validation {
        phase: ProcessingDurabilityPhase,
        source: ProcessingDatabasePathError,
    },
}

impl ProcessingDatabaseDurabilityError {
    /// The durability phase involved in this failure.
    pub fn phase(&self) -> ProcessingDurabilityPhase {
        match self {
            Self::Sync { phase, .. } | Self::Validation { phase, .. } => *phase,
        }
    }

    /// The managed path involved, when this failure names one.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Sync { path, .. } => Some(path.as_path()),
            Self::Validation { .. } => None,
        }
    }

    /// The underlying I/O failure, when present.
    pub fn io_error(&self) -> Option<&std::io::Error> {
        match self {
            Self::Sync { source, .. } => Some(source),
            Self::Validation { .. } => None,
        }
    }

    /// The underlying path admission failure, when present.
    pub fn path_error(&self) -> Option<&ProcessingDatabasePathError> {
        match self {
            Self::Validation { source, .. } => Some(source),
            Self::Sync { .. } => None,
        }
    }
}

impl fmt::Display for ProcessingDatabaseDurabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sync { phase, .. } => {
                write!(
                    formatter,
                    "processing database durability sync failed: {phase}"
                )
            }
            Self::Validation { phase, .. } => write!(
                formatter,
                "processing database durability validation failed: {phase}"
            ),
        }
    }
}

impl std::error::Error for ProcessingDatabaseDurabilityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sync { source, .. } => Some(source),
            Self::Validation { source, .. } => Some(source),
        }
    }
}

// ---------------------------------------------------------------------------
// Database open
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ProcessingDatabaseOpenError {
    Path(ProcessingDatabasePathError),
    ConnectionOpen(rusqlite::Error),
    BusyTimeoutConfiguration(rusqlite::Error),
    BaselineConfiguration(rusqlite::Error),
    Configuration(ProcessingDatabaseConfigurationError),
    Durability(ProcessingDatabaseDurabilityError),
}

impl ProcessingDatabaseOpenError {
    /// The path admission failure, when present.
    pub fn path_error(&self) -> Option<&ProcessingDatabasePathError> {
        match self {
            Self::Path(error) => Some(error),
            _ => None,
        }
    }

    /// The baseline-configuration disagreement, when present.
    pub fn configuration_error(&self) -> Option<&ProcessingDatabaseConfigurationError> {
        match self {
            Self::Configuration(error) => Some(error),
            _ => None,
        }
    }

    /// The durability failure, when present.
    pub fn durability_error(&self) -> Option<&ProcessingDatabaseDurabilityError> {
        match self {
            Self::Durability(error) => Some(error),
            _ => None,
        }
    }

    /// The underlying SQLite error, when present.
    pub fn sqlite_error(&self) -> Option<&rusqlite::Error> {
        match self {
            Self::ConnectionOpen(error)
            | Self::BusyTimeoutConfiguration(error)
            | Self::BaselineConfiguration(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for ProcessingDatabaseOpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(_) => formatter.write_str("processing database path initialization failed"),
            Self::ConnectionOpen(_) => {
                formatter.write_str("processing database connection open failed")
            }
            Self::BusyTimeoutConfiguration(_) => {
                formatter.write_str("processing database busy-timeout configuration failed")
            }
            Self::BaselineConfiguration(_) => {
                formatter.write_str("processing database baseline configuration failed")
            }
            Self::Configuration(_) => {
                formatter.write_str("processing database baseline configuration disagreed")
            }
            Self::Durability(_) => {
                formatter.write_str("processing database creation durability failed")
            }
        }
    }
}

impl std::error::Error for ProcessingDatabaseOpenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Path(error) => Some(error),
            Self::ConnectionOpen(error) => Some(error),
            Self::BusyTimeoutConfiguration(error) => Some(error),
            Self::BaselineConfiguration(error) => Some(error),
            Self::Configuration(error) => Some(error),
            Self::Durability(error) => Some(error),
        }
    }
}

// ---------------------------------------------------------------------------
// Database transaction state
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ProcessingDatabaseTransactionError {
    Commit(rusqlite::Error),
    Rollback(rusqlite::Error),
    /// A commit or rollback was requested after the transaction was committed.
    AlreadyCommitted,
    /// A commit or rollback was requested after the transaction was rolled back.
    AlreadyRolledBack,
    /// The SQLite connection reports no active transaction when one is required.
    TransactionNotActive,
}

impl ProcessingDatabaseTransactionError {
    /// The underlying SQLite error, when present.
    pub fn sqlite_error(&self) -> Option<&rusqlite::Error> {
        match self {
            Self::Commit(error) | Self::Rollback(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for ProcessingDatabaseTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Commit(_) => formatter.write_str("processing database commit failed"),
            Self::Rollback(_) => formatter.write_str("processing database rollback failed"),
            Self::AlreadyCommitted => {
                formatter.write_str("processing database transaction was already committed")
            }
            Self::AlreadyRolledBack => {
                formatter.write_str("processing database transaction was already rolled back")
            }
            Self::TransactionNotActive => {
                formatter.write_str("processing database transaction is not active")
            }
        }
    }
}

impl std::error::Error for ProcessingDatabaseTransactionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Commit(error) | Self::Rollback(error) => Some(error),
            _ => None,
        }
    }
}

/// The point at which a Core-owned transaction boundary was found to be lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingTransactionBoundaryPhase {
    /// Detected before the source handler was invoked.
    BeforeHandler,
    /// Detected after the source handler returned.
    AfterHandler,
}

impl ProcessingTransactionBoundaryPhase {
    /// Stable snake_case identifier for this phase.
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::BeforeHandler => "before_handler",
            Self::AfterHandler => "after_handler",
        }
    }
}

impl fmt::Display for ProcessingTransactionBoundaryPhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.identifier())
    }
}

/// The Core-owned SQLite transaction boundary was lost around the source handler.
///
/// This enforces the supported Core route. It is not hostile-code confinement:
/// trusted native source code can always end a transaction deliberately. Core
/// detects the accidental case and refuses to report processing success.
#[derive(Debug)]
pub struct ProcessingTransactionBoundaryViolation {
    phase: ProcessingTransactionBoundaryPhase,
    possible_database_partial_commit: bool,
}

impl ProcessingTransactionBoundaryViolation {
    pub(crate) fn new(
        phase: ProcessingTransactionBoundaryPhase,
        possible_database_partial_commit: bool,
    ) -> Self {
        Self {
            phase,
            possible_database_partial_commit,
        }
    }

    pub fn phase(&self) -> ProcessingTransactionBoundaryPhase {
        self.phase
    }

    /// Whether source-committed changes may already be durable.
    ///
    /// When true, Core does not claim that the database was rolled back.
    pub fn possible_database_partial_commit(&self) -> bool {
        self.possible_database_partial_commit
    }
}

impl fmt::Display for ProcessingTransactionBoundaryViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "processing source ended the Core-owned database transaction: {}",
            self.phase
        )
    }
}

impl std::error::Error for ProcessingTransactionBoundaryViolation {}

// ---------------------------------------------------------------------------
// Committed-but-incomplete outcomes
// ---------------------------------------------------------------------------

/// Why a committed processing database could not be reported as a clean success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingDatabasePartialCommitPhase {
    /// SQLite committed, but post-commit durability failed.
    PostCommitDurability,
    /// SQLite committed and durability succeeded, but sidecar validation failed.
    PostCommitSidecarValidation,
    /// SQLite committed and the database is durable, but session persistence failed.
    SessionCompletionPersistence,
    /// The source ended the transaction and changes may already be durable.
    SourceTransactionBoundaryLoss,
}

impl ProcessingDatabasePartialCommitPhase {
    /// Stable snake_case identifier for this phase.
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::PostCommitDurability => "post_commit_durability",
            Self::PostCommitSidecarValidation => "post_commit_sidecar_validation",
            Self::SessionCompletionPersistence => "session_completion_persistence",
            Self::SourceTransactionBoundaryLoss => "source_transaction_boundary_loss",
        }
    }
}

impl fmt::Display for ProcessingDatabasePartialCommitPhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.identifier())
    }
}

/// The typed cause retained alongside a database partial commit.
#[derive(Debug)]
pub enum ProcessingDatabasePartialCommitCause {
    Durability(ProcessingDatabaseDurabilityError),
    Sidecar(ProcessingDatabaseSidecarError),
    SessionPersistence(SessionStoreError),
    TransactionBoundary(ProcessingTransactionBoundaryViolation),
}

impl ProcessingDatabasePartialCommitCause {
    fn as_error(&self) -> &(dyn std::error::Error + 'static) {
        match self {
            Self::Durability(error) => error,
            Self::Sidecar(error) => error,
            Self::SessionPersistence(error) => error,
            Self::TransactionBoundary(error) => error,
        }
    }
}

/// The SQLite transaction committed, but the processing session cannot be reported
/// as `Succeeded`.
///
/// Core never claims the database was rolled back after SQLite already committed it.
#[derive(Debug)]
pub struct ProcessingDatabasePartialCommit {
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
    database_path: PathBuf,
    phase: ProcessingDatabasePartialCommitPhase,
    cause: ProcessingDatabasePartialCommitCause,
    session_persistence_error: Option<SessionStoreError>,
}

impl ProcessingDatabasePartialCommit {
    pub(crate) fn new(
        project: ProjectIdentity,
        runtime: OwnedRuntimeIdentity,
        session: SessionIdentity,
        database_path: PathBuf,
        phase: ProcessingDatabasePartialCommitPhase,
        cause: ProcessingDatabasePartialCommitCause,
    ) -> Self {
        Self {
            project,
            runtime,
            session,
            database_path,
            phase,
            cause,
            session_persistence_error: None,
        }
    }

    /// Attach a session-persistence failure that occurred while recording this outcome.
    pub(crate) fn with_session_persistence_error(mut self, error: SessionStoreError) -> Self {
        self.session_persistence_error = Some(error);
        self
    }

    pub fn project(&self) -> &ProjectIdentity {
        &self.project
    }

    pub fn runtime(&self) -> &OwnedRuntimeIdentity {
        &self.runtime
    }

    pub fn session(&self) -> &SessionIdentity {
        &self.session
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn phase(&self) -> ProcessingDatabasePartialCommitPhase {
        self.phase
    }

    /// The typed cause that prevented a clean success.
    pub fn cause(&self) -> &ProcessingDatabasePartialCommitCause {
        &self.cause
    }

    /// The durability failure, when that is the retained cause.
    pub fn durability_error(&self) -> Option<&ProcessingDatabaseDurabilityError> {
        match &self.cause {
            ProcessingDatabasePartialCommitCause::Durability(error) => Some(error),
            _ => None,
        }
    }

    /// The sidecar failure, when that is the retained cause.
    pub fn sidecar_error(&self) -> Option<&ProcessingDatabaseSidecarError> {
        match &self.cause {
            ProcessingDatabasePartialCommitCause::Sidecar(error) => Some(error),
            _ => None,
        }
    }

    /// The transaction-boundary violation, when that is the retained cause.
    pub fn transaction_boundary_violation(
        &self,
    ) -> Option<&ProcessingTransactionBoundaryViolation> {
        match &self.cause {
            ProcessingDatabasePartialCommitCause::TransactionBoundary(error) => Some(error),
            _ => None,
        }
    }

    /// Any session-persistence failure retained with this outcome.
    pub fn session_persistence_error(&self) -> Option<&SessionStoreError> {
        match (&self.cause, self.session_persistence_error.as_ref()) {
            (ProcessingDatabasePartialCommitCause::SessionPersistence(error), _) => Some(error),
            (_, retained) => retained,
        }
    }
}

impl fmt::Display for ProcessingDatabasePartialCommit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "processing database commit succeeded but the session cannot be reported successful: {}",
            self.phase
        )
    }
}

impl std::error::Error for ProcessingDatabasePartialCommit {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_error())
    }
}

/// A SQLite `COMMIT` failed in a way that does not prove the absence of durable changes.
///
/// Core reports the uncertainty honestly rather than claiming a rollback guarantee
/// SQLite cannot provide. The processing session never becomes `Succeeded`.
#[derive(Debug)]
pub struct ProcessingDatabaseCommitOutcomeUncertain {
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
    database_path: PathBuf,
    commit_error: rusqlite::Error,
    session_persistence_error: Option<SessionStoreError>,
}

impl ProcessingDatabaseCommitOutcomeUncertain {
    pub(crate) fn new(
        project: ProjectIdentity,
        runtime: OwnedRuntimeIdentity,
        session: SessionIdentity,
        database_path: PathBuf,
        commit_error: rusqlite::Error,
    ) -> Self {
        Self {
            project,
            runtime,
            session,
            database_path,
            commit_error,
            session_persistence_error: None,
        }
    }

    pub(crate) fn with_session_persistence_error(mut self, error: SessionStoreError) -> Self {
        self.session_persistence_error = Some(error);
        self
    }

    pub fn project(&self) -> &ProjectIdentity {
        &self.project
    }

    pub fn runtime(&self) -> &OwnedRuntimeIdentity {
        &self.runtime
    }

    pub fn session(&self) -> &SessionIdentity {
        &self.session
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn commit_error(&self) -> &rusqlite::Error {
        &self.commit_error
    }

    pub fn session_persistence_error(&self) -> Option<&SessionStoreError> {
        self.session_persistence_error.as_ref()
    }
}

impl fmt::Display for ProcessingDatabaseCommitOutcomeUncertain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "processing database commit outcome is uncertain; durable changes cannot be ruled out",
        )
    }
}

impl std::error::Error for ProcessingDatabaseCommitOutcomeUncertain {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.commit_error)
    }
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ProcessingLifecycleError {
    /// The running lifecycle owner was unexpectedly unavailable.
    RunningSessionUnavailable,
}

impl fmt::Display for ProcessingLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RunningSessionUnavailable => {
                formatter.write_str("processing running session lifecycle owner is unavailable")
            }
        }
    }
}

impl std::error::Error for ProcessingLifecycleError {}

// ---------------------------------------------------------------------------
// Setup failures
// ---------------------------------------------------------------------------

/// A failure raised between entering `Running` and invoking the source handler.
#[derive(Debug)]
pub enum ProcessingSetupError {
    TransactionDiscovery(crate::processing::ProcessingTransactionDiscoveryError),
    DatabasePath(ProcessingDatabasePathError),
    DatabaseOpen(ProcessingDatabaseOpenError),
    ContextConstruction(ProcessingContextConstructionError),
    TransactionBoundary(ProcessingTransactionBoundaryViolation),
}

impl ProcessingSetupError {
    /// The stable session failure code for this setup phase.
    pub fn failure_code(&self) -> crate::session::SessionFailureCode {
        use crate::session::SessionFailureCode;
        match self {
            Self::TransactionDiscovery(error) => {
                if error.is_provenance_failure() {
                    SessionFailureCode::ProcessingTransactionProvenanceFailed
                } else {
                    SessionFailureCode::ProcessingTransactionDiscoveryFailed
                }
            }
            Self::DatabasePath(_) => SessionFailureCode::ProcessingDatabasePathInvalid,
            Self::DatabaseOpen(_) => SessionFailureCode::ProcessingDatabaseOpenFailed,
            Self::ContextConstruction(_) => SessionFailureCode::ProcessingContextConstructionFailed,
            Self::TransactionBoundary(_) => {
                SessionFailureCode::ProcessingDatabaseTransactionFailed
            }
        }
    }

    /// A bounded Core-authored diagnostic for this setup phase.
    ///
    /// Never contains URLs, headers, bodies, SQL, source arguments, or source text.
    pub fn diagnostic(&self) -> &'static str {
        match self {
            Self::TransactionDiscovery(_) => "processing raw transaction discovery failed",
            Self::DatabasePath(_) => "processing database path admission failed",
            Self::DatabaseOpen(_) => "processing database open or configuration failed",
            Self::ContextConstruction(_) => "processing context construction failed",
            Self::TransactionBoundary(_) => "processing database transaction was not active",
        }
    }

    /// The retained discovery failure, when present.
    pub fn transaction_discovery_error(
        &self,
    ) -> Option<&crate::processing::ProcessingTransactionDiscoveryError> {
        match self {
            Self::TransactionDiscovery(error) => Some(error),
            _ => None,
        }
    }

    /// The retained database path failure, when present.
    pub fn database_path_error(&self) -> Option<&ProcessingDatabasePathError> {
        match self {
            Self::DatabasePath(error) => Some(error),
            _ => None,
        }
    }

    /// The retained database open failure, when present.
    pub fn database_open_error(&self) -> Option<&ProcessingDatabaseOpenError> {
        match self {
            Self::DatabaseOpen(error) => Some(error),
            _ => None,
        }
    }

    /// The retained context construction failure, when present.
    pub fn context_construction_error(&self) -> Option<&ProcessingContextConstructionError> {
        match self {
            Self::ContextConstruction(error) => Some(error),
            _ => None,
        }
    }

    /// The retained transaction-boundary violation, when present.
    pub fn transaction_boundary_violation(
        &self,
    ) -> Option<&ProcessingTransactionBoundaryViolation> {
        match self {
            Self::TransactionBoundary(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for ProcessingSetupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.diagnostic())
    }
}

impl std::error::Error for ProcessingSetupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TransactionDiscovery(error) => Some(error),
            Self::DatabasePath(error) => Some(error),
            Self::DatabaseOpen(error) => Some(error),
            Self::ContextConstruction(error) => Some(error),
            Self::TransactionBoundary(error) => Some(error),
        }
    }
}

/// A setup failure whose terminal session persistence also failed.
///
/// Both typed failures are retained; neither is reduced to a string.
#[derive(Debug)]
pub struct ProcessingSetupAndPersistenceFailure {
    setup_error: ProcessingSetupError,
    persistence_error: SessionStoreError,
}

impl ProcessingSetupAndPersistenceFailure {
    pub(crate) fn new(
        setup_error: ProcessingSetupError,
        persistence_error: SessionStoreError,
    ) -> Self {
        Self {
            setup_error,
            persistence_error,
        }
    }

    /// The original setup failure.
    pub fn setup_error(&self) -> &ProcessingSetupError {
        &self.setup_error
    }

    /// The terminal-persistence failure that occurred while recording it.
    pub fn persistence_error(&self) -> &SessionStoreError {
        &self.persistence_error
    }
}

impl fmt::Display for ProcessingSetupAndPersistenceFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("processing setup failed and terminal session persistence also failed")
    }
}

impl std::error::Error for ProcessingSetupAndPersistenceFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.setup_error)
    }
}
