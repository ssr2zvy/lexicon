use std::fmt;
use std::path::{Path, PathBuf};

use super::transactions::ProcessingTransactionDiscoveryError;
use crate::protocols::http::transaction::error::HttpManagedPathError;
use crate::runtime::OwnedRuntimeIdentity;
use crate::session::{ProjectIdentity, SessionFailureCode, SessionIdentity, SessionStoreError};

pub type ProcessingResult<T> = Result<T, ProcessingError>;

/// Error surfaced by a processing source handler.
///
/// The `Display` surface is intentionally bounded: it never renders the arbitrary
/// text of a wrapped source error, only a stable operation label or an explicit
/// message. The wrapped error remains reachable through [`std::error::Error::source`].
#[derive(Debug)]
pub enum ProcessingError {
    Source {
        operation: &'static str,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    SourceMessage(String),
}

impl ProcessingError {
    pub fn source(
        operation: &'static str,
        error: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Source {
            operation,
            source: Box::new(error),
        }
    }

    pub fn source_message(message: impl Into<String>) -> Self {
        Self::SourceMessage(message.into())
    }
}

impl fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source { operation, .. } => {
                write!(f, "source processing failed at operation: {operation}")
            }
            Self::SourceMessage(msg) => write!(f, "source processing failed: {msg}"),
        }
    }
}

impl std::error::Error for ProcessingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source { source, .. } => Some(source.as_ref()),
            Self::SourceMessage(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum ProcessingContextConstructionError {
    RuntimeProtocolMismatch,
    RuntimeOperationMismatch,
    DatabasePathMismatch,
    OperationRootMismatch,
    RawDataDirectoryMismatch,
    ProcessedDataDirectoryMismatch,
    SessionDirectoryMismatch,
    CatalogProjectMismatch,
    CatalogProtocolMismatch,
    CatalogSourceMismatch,
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
            Self::DatabasePathMismatch => {
                formatter.write_str("processing context database path does not match derived location")
            }
            Self::OperationRootMismatch => {
                formatter.write_str("processing context operation root does not match protocol layout")
            }
            Self::RawDataDirectoryMismatch => {
                formatter.write_str("processing context raw data directory does not match protocol layout")
            }
            Self::ProcessedDataDirectoryMismatch => {
                formatter.write_str("processing context processed data directory does not match protocol layout")
            }
            Self::SessionDirectoryMismatch => {
                formatter.write_str("processing context session directory does not end in session identity")
            }
            Self::CatalogProjectMismatch => {
                formatter.write_str("processing context catalog entry project does not match processing project")
            }
            Self::CatalogProtocolMismatch => {
                formatter.write_str("processing context catalog entry runtime protocol is not HTTP")
            }
            Self::CatalogSourceMismatch => {
                formatter.write_str("processing context catalog entry runtime source does not match processing runtime source")
            }
        }
    }
}

impl std::error::Error for ProcessingContextConstructionError {}

#[derive(Debug)]
pub enum ProcessingDatabasePathError {
    ManagedPath(HttpManagedPathError),
}

impl fmt::Display for ProcessingDatabasePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManagedPath(_) => {
                formatter.write_str("processing database managed path validation failed")
            }
        }
    }
}

impl std::error::Error for ProcessingDatabasePathError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManagedPath(error) => Some(error),
        }
    }
}

/// Verification of the effective baseline SQLite configuration after opening.
#[derive(Debug)]
pub enum ProcessingDatabaseConfigurationError {
    PragmaQuery(rusqlite::Error),
    ForeignKeysNotEnabled,
    UnexpectedJournalMode { actual: String },
    TransactionNotStarted,
}

impl fmt::Display for ProcessingDatabaseConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PragmaQuery(_) => {
                formatter.write_str("processing database pragma verification query failed")
            }
            Self::ForeignKeysNotEnabled => {
                formatter.write_str("processing database foreign_keys pragma is not enabled")
            }
            Self::UnexpectedJournalMode { .. } => {
                formatter.write_str("processing database journal_mode is not the expected DELETE mode")
            }
            Self::TransactionNotStarted => {
                formatter.write_str("processing database transaction was not started")
            }
        }
    }
}

impl std::error::Error for ProcessingDatabaseConfigurationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PragmaQuery(error) => Some(error),
            _ => None,
        }
    }
}

/// Durability failure while syncing database file or its parent directory.
#[derive(Debug)]
pub enum ProcessingDatabaseDurabilityError {
    FileSyncFailed(std::io::Error),
    DirectorySyncFailed(std::io::Error),
}

impl fmt::Display for ProcessingDatabaseDurabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileSyncFailed(_) => {
                formatter.write_str("processing database file durability sync failed")
            }
            Self::DirectorySyncFailed(_) => {
                formatter.write_str("processing database directory durability sync failed")
            }
        }
    }
}

impl std::error::Error for ProcessingDatabaseDurabilityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FileSyncFailed(error) => Some(error),
            Self::DirectorySyncFailed(error) => Some(error),
        }
    }
}

/// Violation of the SQLite sidecar-file policy around the database file.
#[derive(Debug)]
pub enum ProcessingDatabaseSidecarError {
    Inspection { path: PathBuf, source: std::io::Error },
    UnexpectedWalFile { path: PathBuf },
    UnexpectedShmFile { path: PathBuf },
    JournalSymlink { path: PathBuf },
    JournalWrongType { path: PathBuf },
    JournalNotCleanedUp { path: PathBuf },
}

impl fmt::Display for ProcessingDatabaseSidecarError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inspection { .. } => {
                formatter.write_str("processing database sidecar inspection failed")
            }
            Self::UnexpectedWalFile { .. } => {
                formatter.write_str("processing database rejected an unexpected WAL sidecar file")
            }
            Self::UnexpectedShmFile { .. } => {
                formatter.write_str("processing database rejected an unexpected SHM sidecar file")
            }
            Self::JournalSymlink { .. } => {
                formatter.write_str("processing database rejected a symlinked journal sidecar")
            }
            Self::JournalWrongType { .. } => {
                formatter.write_str("processing database rejected a journal sidecar of the wrong type")
            }
            Self::JournalNotCleanedUp { .. } => {
                formatter.write_str("processing database journal sidecar was not cleaned up after the transaction")
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

#[derive(Debug)]
pub enum ProcessingDatabaseOpenError {
    Path(ProcessingDatabasePathError),
    ConnectionOpen(rusqlite::Error),
    BusyTimeoutConfiguration(rusqlite::Error),
    BaselineConfiguration(rusqlite::Error),
    BaselineVerification(ProcessingDatabaseConfigurationError),
    Sidecar(ProcessingDatabaseSidecarError),
    Durability(ProcessingDatabaseDurabilityError),
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
            Self::BaselineVerification(_) => {
                formatter.write_str("processing database baseline verification failed")
            }
            Self::Sidecar(_) => {
                formatter.write_str("processing database sidecar policy validation failed")
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
            Self::BaselineVerification(error) => Some(error),
            Self::Sidecar(error) => Some(error),
            Self::Durability(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum ProcessingDatabaseTransactionError {
    Commit(rusqlite::Error),
    Rollback(rusqlite::Error),
    AlreadyCommitted,
    AlreadyRolledBack,
    TransactionNotActive,
    BoundaryViolation,
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
            Self::BoundaryViolation => {
                formatter.write_str("processing source violated the database transaction boundary")
            }
        }
    }
}

impl std::error::Error for ProcessingDatabaseTransactionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Commit(error) => Some(error),
            Self::Rollback(error) => Some(error),
            _ => None,
        }
    }
}

/// Aggregated setup-phase error prior to running the source handler.
#[derive(Debug)]
pub enum ProcessingSetupError {
    TransactionDiscovery(ProcessingTransactionDiscoveryError),
    DatabasePath(ProcessingDatabasePathError),
    DatabaseOpen(ProcessingDatabaseOpenError),
    ContextConstruction(ProcessingContextConstructionError),
}

impl ProcessingSetupError {
    /// The session failure code that corresponds to this setup error.
    pub fn failure_code(&self) -> SessionFailureCode {
        match self {
            Self::TransactionDiscovery(error) => match error {
                ProcessingTransactionDiscoveryError::Provenance(_) => {
                    SessionFailureCode::ProcessingTransactionProvenanceFailed
                }
                _ => SessionFailureCode::ProcessingTransactionDiscoveryFailed,
            },
            Self::DatabasePath(_) => SessionFailureCode::ProcessingDatabasePathInvalid,
            Self::DatabaseOpen(_) => SessionFailureCode::ProcessingDatabaseOpenFailed,
            Self::ContextConstruction(_) => SessionFailureCode::ProcessingContextConstructionFailed,
        }
    }

    /// A stable diagnostic label for this setup error.
    pub fn diagnostic(&self) -> String {
        self.failure_code().identifier().to_string()
    }
}

impl fmt::Display for ProcessingSetupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TransactionDiscovery(_) => {
                formatter.write_str("processing setup transaction discovery failed")
            }
            Self::DatabasePath(_) => {
                formatter.write_str("processing setup database path derivation failed")
            }
            Self::DatabaseOpen(_) => formatter.write_str("processing setup database open failed"),
            Self::ContextConstruction(_) => {
                formatter.write_str("processing setup context construction failed")
            }
        }
    }
}

impl std::error::Error for ProcessingSetupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TransactionDiscovery(error) => Some(error),
            Self::DatabasePath(error) => Some(error),
            Self::DatabaseOpen(error) => Some(error),
            Self::ContextConstruction(error) => Some(error),
        }
    }
}

/// A setup failure whose terminal session persistence also failed.
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

    pub fn setup_error(&self) -> &ProcessingSetupError {
        &self.setup_error
    }

    pub fn persistence_error(&self) -> &SessionStoreError {
        &self.persistence_error
    }
}

impl fmt::Display for ProcessingSetupAndPersistenceFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "processing runtime setup failed and terminal session failure persistence also failed",
        )
    }
}

impl std::error::Error for ProcessingSetupAndPersistenceFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.setup_error)
    }
}

/// The database commit statement returned an error, leaving the durable commit
/// outcome uncertain (it may or may not have taken effect).
#[derive(Debug)]
pub struct ProcessingDatabaseCommitOutcomeUncertain {
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
    database_path: PathBuf,
    commit_error: rusqlite::Error,
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
        }
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
}

impl fmt::Display for ProcessingDatabaseCommitOutcomeUncertain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("processing database commit failed with uncertain durable outcome")
    }
}

impl std::error::Error for ProcessingDatabaseCommitOutcomeUncertain {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.commit_error)
    }
}

#[derive(Debug)]
pub struct ProcessingDatabasePartialCommit {
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
    database_path: PathBuf,
    persistence_error: SessionStoreError,
}

impl ProcessingDatabasePartialCommit {
    pub(crate) fn new(
        project: ProjectIdentity,
        runtime: OwnedRuntimeIdentity,
        session: SessionIdentity,
        database_path: PathBuf,
        persistence_error: SessionStoreError,
    ) -> Self {
        Self {
            project,
            runtime,
            session,
            database_path,
            persistence_error,
        }
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
    pub fn database_path(&self) -> &std::path::Path {
        &self.database_path
    }
    pub fn persistence_error(&self) -> &SessionStoreError {
        &self.persistence_error
    }
}

impl fmt::Display for ProcessingDatabasePartialCommit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "processing database commit succeeded but session completion persistence failed",
        )
    }
}

impl std::error::Error for ProcessingDatabasePartialCommit {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.persistence_error)
    }
}

/// The database commit succeeded but the post-commit durability sync failed.
#[derive(Debug)]
pub struct ProcessingDatabaseCommitDurabilityFailure {
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
    database_path: PathBuf,
    durability_error: ProcessingDatabaseDurabilityError,
}

impl ProcessingDatabaseCommitDurabilityFailure {
    pub(crate) fn new(
        project: ProjectIdentity,
        runtime: OwnedRuntimeIdentity,
        session: SessionIdentity,
        database_path: PathBuf,
        durability_error: ProcessingDatabaseDurabilityError,
    ) -> Self {
        Self {
            project,
            runtime,
            session,
            database_path,
            durability_error,
        }
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
    pub fn durability_error(&self) -> &ProcessingDatabaseDurabilityError {
        &self.durability_error
    }
}

impl fmt::Display for ProcessingDatabaseCommitDurabilityFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "processing database commit succeeded but post-commit durability sync failed",
        )
    }
}

impl std::error::Error for ProcessingDatabaseCommitDurabilityFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.durability_error)
    }
}
