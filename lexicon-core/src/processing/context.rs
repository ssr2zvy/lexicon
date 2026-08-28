use std::path::{Path, PathBuf};

use super::error::{ProcessingContextConstructionError, ProcessingDatabaseTransactionError};
use super::transactions::ProcessingHttpTransactionCatalog;
use crate::runtime::{OwnedRuntimeIdentity, RuntimeOperation, RuntimeProtocol};
use crate::session::{ProjectIdentity, SessionDataPaths, SessionIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessingDatabaseState {
    OpenTransaction,
    Committed,
    RolledBack,
}

/// Bound processing context provided to processing source handlers.
pub struct ProcessingContext {
    paths: SessionDataPaths,
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
    transactions: ProcessingHttpTransactionCatalog,
    database_path: PathBuf,
    database: rusqlite::Connection,
    database_state: ProcessingDatabaseState,
}

impl ProcessingContext {
    pub(crate) fn new(
        paths: SessionDataPaths,
        project: ProjectIdentity,
        runtime: OwnedRuntimeIdentity,
        session: SessionIdentity,
        transactions: ProcessingHttpTransactionCatalog,
        database_path: PathBuf,
        database: rusqlite::Connection,
    ) -> Result<Self, ProcessingContextConstructionError> {
        if runtime.protocol() != RuntimeProtocol::Http {
            return Err(ProcessingContextConstructionError::RuntimeProtocolMismatch);
        }

        if runtime.operation() != RuntimeOperation::Processing {
            return Err(ProcessingContextConstructionError::RuntimeOperationMismatch);
        }

        Ok(Self {
            paths,
            project,
            runtime,
            session,
            transactions,
            database_path,
            database,
            database_state: ProcessingDatabaseState::OpenTransaction,
        })
    }

    pub fn protocol_root(&self) -> &Path { self.paths.protocol_root() }
    pub fn operation_root(&self) -> &Path { self.paths.operation_root() }
    pub fn session_directory(&self) -> &Path { self.paths.session_directory() }
    pub fn raw_data_directory(&self) -> &Path { self.paths.raw_data_directory() }
    pub fn processed_data_directory(&self) -> &Path { self.paths.processed_data_directory() }

    pub fn project(&self) -> &ProjectIdentity { &self.project }
    pub fn runtime(&self) -> &OwnedRuntimeIdentity { &self.runtime }
    pub fn session_identity(&self) -> &SessionIdentity { &self.session }
    pub fn transactions(&self) -> &ProcessingHttpTransactionCatalog { &self.transactions }
    pub fn database_path(&self) -> &Path { &self.database_path }
    pub fn database(&mut self) -> &mut rusqlite::Connection { &mut self.database }

    pub(crate) fn commit_database(&mut self) -> Result<(), ProcessingDatabaseTransactionError> {
        if self.database_state != ProcessingDatabaseState::OpenTransaction {
            return Ok(());
        }
        self.database
            .execute_batch("COMMIT;")
            .map_err(ProcessingDatabaseTransactionError::Commit)?;
        self.database_state = ProcessingDatabaseState::Committed;
        Ok(())
    }

    pub(crate) fn rollback_database(&mut self) -> Result<(), ProcessingDatabaseTransactionError> {
        if self.database_state != ProcessingDatabaseState::OpenTransaction {
            return Ok(());
        }
        self.database
            .execute_batch("ROLLBACK;")
            .map_err(ProcessingDatabaseTransactionError::Rollback)?;
        self.database_state = ProcessingDatabaseState::RolledBack;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests() -> Self {
        let paths = SessionDataPaths::from_legacy_parts(
            std::path::PathBuf::from("/test/project/sources/test-source/http"),
            std::path::PathBuf::from("/test/project/sources/test-source/http/process-data"),
            std::path::PathBuf::from("/test/project/sources/test-source/http/process-data/sessions/test-session"),
            std::path::PathBuf::from("/test/project/sources/test-source/http/data/raw"),
            std::path::PathBuf::from("/test/project/sources/test-source/http/data/processed"),
        );

        let project = ProjectIdentity::new("test-project").expect("valid project id");
        let runtime = OwnedRuntimeIdentity::http_processing("test-source", 1);
        let session = SessionIdentity::new("test-session").expect("valid session id");
        let database = rusqlite::Connection::open_in_memory().expect("in-memory sqlite connection");
        database.execute_batch("BEGIN IMMEDIATE;").expect("begin transaction");

        Self::new(
            paths,
            project,
            runtime,
            session,
            ProcessingHttpTransactionCatalog::new(Vec::new()),
            std::path::PathBuf::from("/test/project/sources/test-source/http/data/processed/test-source.sqlite3"),
            database,
        )
        .expect("valid processing context")
    }
}

impl std::fmt::Debug for ProcessingContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessingContext")
            .field("project", &self.project.name())
            .field("runtime_source", &self.runtime.source_name())
            .field("session", &self.session.id())
            .field("database_path", &self.database_path)
            .finish_non_exhaustive()
    }
}

impl Drop for ProcessingContext {
    fn drop(&mut self) {
        if self.database_state == ProcessingDatabaseState::OpenTransaction {
            let _ = self.database.execute_batch("ROLLBACK;");
            self.database_state = ProcessingDatabaseState::RolledBack;
        }
    }
}
