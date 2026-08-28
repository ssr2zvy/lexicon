use std::fmt;
use std::path::PathBuf;

use crate::protocols::http::transaction::error::HttpManagedPathError;
use crate::runtime::OwnedRuntimeIdentity;
use crate::session::{ProjectIdentity, SessionIdentity, SessionStoreError};

pub type ProcessingResult<T> = Result<T, ProcessingError>;

#[derive(Debug)]
pub struct ProcessingError;

impl fmt::Display for ProcessingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("processing failed")
    }
}

impl std::error::Error for ProcessingError {}

#[derive(Debug)]
pub enum ProcessingContextConstructionError {
    RuntimeProtocolMismatch,
    RuntimeOperationMismatch,
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
            Self::ManagedPath(_) => formatter.write_str("processing database managed path validation failed"),
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

#[derive(Debug)]
pub enum ProcessingDatabaseOpenError {
    Path(ProcessingDatabasePathError),
    ConnectionOpen(rusqlite::Error),
    BusyTimeoutConfiguration(rusqlite::Error),
    BaselineConfiguration(rusqlite::Error),
}

impl fmt::Display for ProcessingDatabaseOpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(_) => formatter.write_str("processing database path initialization failed"),
            Self::ConnectionOpen(_) => formatter.write_str("processing database connection open failed"),
            Self::BusyTimeoutConfiguration(_) => {
                formatter.write_str("processing database busy-timeout configuration failed")
            }
            Self::BaselineConfiguration(_) => {
                formatter.write_str("processing database baseline configuration failed")
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
        }
    }
}

#[derive(Debug)]
pub enum ProcessingDatabaseTransactionError {
    Commit(rusqlite::Error),
    Rollback(rusqlite::Error),
}

impl fmt::Display for ProcessingDatabaseTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Commit(_) => formatter.write_str("processing database commit failed"),
            Self::Rollback(_) => formatter.write_str("processing database rollback failed"),
        }
    }
}

impl std::error::Error for ProcessingDatabaseTransactionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Commit(error) => Some(error),
            Self::Rollback(error) => Some(error),
        }
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

    pub fn project(&self) -> &ProjectIdentity { &self.project }
    pub fn runtime(&self) -> &OwnedRuntimeIdentity { &self.runtime }
    pub fn session(&self) -> &SessionIdentity { &self.session }
    pub fn database_path(&self) -> &std::path::Path { &self.database_path }
    pub fn persistence_error(&self) -> &SessionStoreError { &self.persistence_error }
}

impl fmt::Display for ProcessingDatabasePartialCommit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("processing database commit succeeded but session completion persistence failed")
    }
}

impl std::error::Error for ProcessingDatabasePartialCommit {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.persistence_error)
    }
}
