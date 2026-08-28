use std::path::{Path, PathBuf};

use super::error::{
    ProcessingContextConstructionError, ProcessingDatabaseTransactionError,
    ProcessingManagedPathCategory, ProcessingTransactionBoundaryPhase,
    ProcessingTransactionBoundaryViolation,
};
use super::transactions::ProcessingHttpTransactionCatalog;
use crate::runtime::{OwnedRuntimeIdentity, RuntimeOperation, RuntimeProtocol};
use crate::session::{ProjectIdentity, SessionDataPaths, SessionIdentity};

/// Directory name of the processing operation root beneath the protocol root.
pub(crate) const PROCESSING_OPERATION_DIRECTORY: &str = "process-data";

/// Filename extension of the source-specific processing database.
pub(crate) const PROCESSING_DATABASE_EXTENSION: &str = "sqlite3";

/// Derive the canonical processing database filename for a runtime source.
pub(crate) fn processing_database_file_name(runtime: &OwnedRuntimeIdentity) -> String {
    format!(
        "{}.{PROCESSING_DATABASE_EXTENSION}",
        runtime.source_name()
    )
}

/// Derive the canonical processing database path beneath the processed-data directory.
pub(crate) fn processing_database_path(
    processed_data_directory: &Path,
    runtime: &OwnedRuntimeIdentity,
) -> PathBuf {
    processed_data_directory.join(processing_database_file_name(runtime))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessingDatabaseState {
    OpenTransaction,
    Committed,
    RolledBack,
    /// The transaction ended outside Core's control, or SQLite left its outcome
    /// uncertain. Core issues no further transaction statements in this state.
    EndedOutsideCore,
}

/// Bound processing context provided to processing source handlers.
///
/// Construction proves every identity and path relationship the admitted type
/// claims. A context that combines separately valid but mutually inconsistent
/// components cannot be created.
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
    /// Construct a fully checked processing context.
    ///
    /// Required relationships:
    /// ```text
    /// operation_root           = protocol_root/process-data
    /// session_directory        = operation_root/sessions/<session-id>
    /// raw_data_directory       = protocol_root/data/raw
    /// processed_data_directory = protocol_root/data/processed
    /// database_path            = processed_data_directory/<runtime-source>.sqlite3
    /// ```
    /// Every catalog entry must additionally agree with the processing project, the
    /// HTTP protocol, and the processing runtime source.
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

        let protocol_root = paths.protocol_root();

        require_exact_path(
            ProcessingManagedPathCategory::OperationRoot,
            protocol_root.join(PROCESSING_OPERATION_DIRECTORY),
            paths.operation_root(),
        )?;
        require_exact_path(
            ProcessingManagedPathCategory::RawDataDirectory,
            protocol_root.join("data").join("raw"),
            paths.raw_data_directory(),
        )?;
        require_exact_path(
            ProcessingManagedPathCategory::ProcessedDataDirectory,
            protocol_root.join("data").join("processed"),
            paths.processed_data_directory(),
        )?;
        require_exact_path(
            ProcessingManagedPathCategory::SessionDirectory,
            paths.operation_root().join("sessions").join(session.id()),
            paths.session_directory(),
        )?;
        require_exact_path(
            ProcessingManagedPathCategory::DatabaseFile,
            processing_database_path(paths.processed_data_directory(), &runtime),
            &database_path,
        )?;

        validate_catalog_agreement(&transactions, &project, &runtime)?;

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

    pub fn protocol_root(&self) -> &Path {
        self.paths.protocol_root()
    }
    pub fn operation_root(&self) -> &Path {
        self.paths.operation_root()
    }
    pub fn session_directory(&self) -> &Path {
        self.paths.session_directory()
    }
    pub fn raw_data_directory(&self) -> &Path {
        self.paths.raw_data_directory()
    }
    pub fn processed_data_directory(&self) -> &Path {
        self.paths.processed_data_directory()
    }

    pub fn project(&self) -> &ProjectIdentity {
        &self.project
    }
    pub fn runtime(&self) -> &OwnedRuntimeIdentity {
        &self.runtime
    }
    pub fn session_identity(&self) -> &SessionIdentity {
        &self.session
    }
    pub fn transactions(&self) -> &ProcessingHttpTransactionCatalog {
        &self.transactions
    }
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }
    pub fn database(&mut self) -> &mut rusqlite::Connection {
        &mut self.database
    }

    /// Borrow the admitted transaction catalog and the database simultaneously.
    ///
    /// The two borrows are disjoint, so a source can iterate admitted transactions
    /// while writing to SQLite without cloning the catalog:
    ///
    /// ```ignore
    /// let (transactions, database) = context.resources();
    /// for transaction in transactions.iter() {
    ///     database.execute(/* source-owned SQL */)?;
    /// }
    /// ```
    pub fn resources(
        &mut self,
    ) -> (
        &ProcessingHttpTransactionCatalog,
        &mut rusqlite::Connection,
    ) {
        (&self.transactions, &mut self.database)
    }

    /// Require that the Core-owned SQLite transaction is still active.
    ///
    /// Enforces the supported Core route around the source handler. Trusted native
    /// source code can still end a transaction deliberately; this detects the
    /// accidental case so Core never reports success after boundary loss.
    pub(crate) fn require_transaction_active(
        &self,
        phase: ProcessingTransactionBoundaryPhase,
    ) -> Result<(), ProcessingTransactionBoundaryViolation> {
        if self.database.is_autocommit() {
            // A source `COMMIT`/`END` may have made changes durable; a source
            // `ROLLBACK` may not have. Core cannot distinguish the two here, so it
            // conservatively reports that a partial commit is possible after the
            // handler ran and refuses to claim a rollback guarantee.
            let possible_database_partial_commit =
                matches!(phase, ProcessingTransactionBoundaryPhase::AfterHandler);
            return Err(ProcessingTransactionBoundaryViolation::new(
                phase,
                possible_database_partial_commit,
            ));
        }
        Ok(())
    }

    /// Commit the Core-owned SQLite transaction.
    ///
    /// Legal only from the open state. A second commit, or a commit after rollback,
    /// is a typed error rather than a silent success.
    pub(crate) fn commit_database(&mut self) -> Result<(), ProcessingDatabaseTransactionError> {
        self.require_open_transaction()?;
        self.database
            .execute_batch("COMMIT;")
            .map_err(ProcessingDatabaseTransactionError::Commit)?;
        self.database_state = ProcessingDatabaseState::Committed;
        Ok(())
    }

    /// Roll back the Core-owned SQLite transaction.
    ///
    /// Legal only from the open state. A second rollback, or a rollback after commit,
    /// is a typed error rather than a silent success.
    pub(crate) fn rollback_database(&mut self) -> Result<(), ProcessingDatabaseTransactionError> {
        self.require_open_transaction()?;
        self.database
            .execute_batch("ROLLBACK;")
            .map_err(ProcessingDatabaseTransactionError::Rollback)?;
        self.database_state = ProcessingDatabaseState::RolledBack;
        Ok(())
    }

    /// Whether SQLite still reports an active transaction on this connection.
    pub(crate) fn database_transaction_active(&self) -> bool {
        !self.database.is_autocommit()
    }

    /// Record that the transaction ended outside Core's control or with an uncertain
    /// outcome, so Core issues no further transaction statements on this connection.
    pub(crate) fn mark_transaction_ended_outside_core(&mut self) {
        self.database_state = ProcessingDatabaseState::EndedOutsideCore;
    }

    /// Reject every database state transition other than `Open → Committed` and
    /// `Open → RolledBack`.
    fn require_open_transaction(&self) -> Result<(), ProcessingDatabaseTransactionError> {
        match self.database_state {
            ProcessingDatabaseState::OpenTransaction => Ok(()),
            ProcessingDatabaseState::Committed => {
                Err(ProcessingDatabaseTransactionError::AlreadyCommitted)
            }
            ProcessingDatabaseState::RolledBack => {
                Err(ProcessingDatabaseTransactionError::AlreadyRolledBack)
            }
            ProcessingDatabaseState::EndedOutsideCore => {
                Err(ProcessingDatabaseTransactionError::TransactionNotActive)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests() -> Self {
        let protocol_root =
            std::path::PathBuf::from("/test/project/sources/test-source/http");
        let operation_root = protocol_root.join(PROCESSING_OPERATION_DIRECTORY);
        let session_directory = operation_root.join("sessions").join("test-session");
        let raw_data_directory = protocol_root.join("data").join("raw");
        let processed_data_directory = protocol_root.join("data").join("processed");

        let paths = SessionDataPaths::from_legacy_parts(
            protocol_root,
            operation_root,
            session_directory,
            raw_data_directory,
            processed_data_directory.clone(),
        );

        let project = ProjectIdentity::new("test-project").expect("valid project id");
        let runtime = OwnedRuntimeIdentity::http_processing("test-source", 1);
        let session = SessionIdentity::new("test-session").expect("valid session id");
        let database_path = processing_database_path(&processed_data_directory, &runtime);
        let database = rusqlite::Connection::open_in_memory().expect("in-memory sqlite connection");
        database
            .execute_batch("BEGIN IMMEDIATE;")
            .expect("begin transaction");

        Self::new(
            paths,
            project,
            runtime,
            session,
            ProcessingHttpTransactionCatalog::new(Vec::new()),
            database_path,
            database,
        )
        .expect("valid processing context")
    }
}

/// Require that a supplied managed path equals the exact path the layout demands.
fn require_exact_path(
    category: ProcessingManagedPathCategory,
    expected: PathBuf,
    actual: &Path,
) -> Result<(), ProcessingContextConstructionError> {
    if actual != expected {
        return Err(
            ProcessingContextConstructionError::ManagedPathDisagreement {
                category,
                expected,
                actual: actual.to_path_buf(),
            },
        );
    }
    Ok(())
}

/// Require that every catalog entry belongs to this project, protocol, and source.
fn validate_catalog_agreement(
    transactions: &ProcessingHttpTransactionCatalog,
    project: &ProjectIdentity,
    runtime: &OwnedRuntimeIdentity,
) -> Result<(), ProcessingContextConstructionError> {
    for (catalog_index, entry) in transactions.iter().enumerate() {
        if entry.project() != project {
            return Err(ProcessingContextConstructionError::CatalogProjectMismatch {
                catalog_index,
            });
        }
        if entry.acquisition_runtime().protocol() != RuntimeProtocol::Http {
            return Err(ProcessingContextConstructionError::CatalogProtocolMismatch {
                catalog_index,
            });
        }
        if entry.acquisition_runtime().source_name() != runtime.source_name() {
            return Err(ProcessingContextConstructionError::CatalogSourceMismatch {
                catalog_index,
            });
        }
    }
    Ok(())
}

impl std::fmt::Debug for ProcessingContext {
    /// Bounded debug output.
    ///
    /// Renders identities and managed path categories only. The database path and
    /// other managed filesystem paths are deliberately omitted.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessingContext")
            .field("project", &self.project.name())
            .field("runtime_source", &self.runtime.source_name())
            .field("session", &self.session.id())
            .field(
                "database",
                &ProcessingManagedPathCategory::DatabaseFile.identifier(),
            )
            .field("transactions", &self.transactions.len())
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
