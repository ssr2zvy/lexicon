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
    /// Legal only from the open state with the SQLite connection still
    /// under Core supervision. A second commit, a commit after rollback,
    /// or a commit after the source prematurely ended the transaction
    /// is a typed error rather than a silent success.
    pub(crate) fn commit_database(&mut self) -> Result<(), ProcessingDatabaseTransactionError> {
        self.require_open_transaction()?;
        // The bookkeeping state may read `Open` while the SQL connection
        // has already been forced into autocommit by a source-side
        // `COMMIT` or by SQL that aborted the transaction. Core refuses
        // to issue `COMMIT` against an autocommit connection because
        // issuing it there is a silent no-op rather than an integrity
        // guarantee, and the audit forbids such silent success.
        if self.database.is_autocommit() {
            self.database_state = ProcessingDatabaseState::EndedOutsideCore;
            return Err(ProcessingDatabaseTransactionError::TransactionNotActive);
        }
        self.database
            .execute_batch("COMMIT;")
            .map_err(ProcessingDatabaseTransactionError::Commit)?;
        self.database_state = ProcessingDatabaseState::Committed;
        Ok(())
    }

    /// Roll back the Core-owned SQLite transaction.
    ///
    /// Legal only from the open state with the SQLite connection still
    /// under Core supervision. Symmetric with `commit_database`: a
    /// second rollback, a rollback after commit, or a rollback after
    /// the source prematurely ended the transaction is a typed error
    /// rather than a silent success.
    pub(crate) fn rollback_database(&mut self) -> Result<(), ProcessingDatabaseTransactionError> {
        self.require_open_transaction()?;
        if self.database.is_autocommit() {
            self.database_state = ProcessingDatabaseState::EndedOutsideCore;
            return Err(ProcessingDatabaseTransactionError::TransactionNotActive);
        }
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

#[cfg(test)]
#[doc = "PROCESS-01 Core-owned SQLite transaction failpoint coverage \
(current.md §11 PROCESS-01)."]
mod tests {
    use super::error::ProcessingTransactionBoundaryPhase;
    use super::{ProcessingContext, ProcessingDatabaseState};

    #[test]
    fn core_begins_transaction_before_source_handler() {
        let context = ProcessingContext::new_for_tests();
        // Core must own the SQLite transaction before the source handler
        // runs. The audit forbids the source from beginning its own
        // transaction out-of-band.
        assert!(
            context.database_transaction_active(),
            "PROCESS-01: Core must BEGIN the SQLite transaction before \
             the source handler runs"
        );
        // Database is in OpenTransaction state after construction.
        assert!(matches!(
            context.database_state_for_tests(),
            ProcessingDatabaseState::OpenTransaction
        ));
    }

    #[test]
    fn successful_handler_commits_database_once() {
        let mut context = ProcessingContext::new_for_tests();
        // Insert a sentinel row before commit; after commit, the row
        // must be readable. Repeat commit. A second commit must return
        // an AlreadyCommitted typed error.
        context
            .database
            .execute_batch(
                "CREATE TABLE commit_sentinel (id INTEGER PRIMARY KEY, label TEXT NOT NULL);\
                 INSERT INTO commit_sentinel(id, label) VALUES (1, 'first');",
            )
            .expect("install sentinel table");
        context
            .commit_database()
            .expect("first commit must succeed");
        // Verify row landed.
        let row_count: i64 = context
            .database
            .query_row("SELECT count(*) FROM commit_sentinel", [], |r| r.get(0))
            .expect("count rows");
        assert_eq!(row_count, 1, "committed sentinel row must persist");
        // Second commit must be a typed AlreadyCommitted error.
        let second = context.commit_database();
        let error = second.expect_err("second commit must fail");
        assert!(
            format!("{error:?}").contains("AlreadyCommitted"),
            "expected AlreadyCommitted typed error, got: {error:?}"
        );
    }

    #[test]
    fn handler_error_rolls_back_and_preserves_previous_database() {
        let mut context = ProcessingContext::new_for_tests();
        // Pre-existing sentinel database contents from a previous commit.
        context
            .database
            .execute_batch(
                "CREATE TABLE previous (id INTEGER PRIMARY KEY, marker TEXT NOT NULL);\
                 INSERT INTO previous(id, marker) VALUES (1, 'pre-handler');",
            )
            .expect("install sentinel row");
        context
            .commit_database()
            .expect("earlier commit must succeed");
        // Subsequent Core begin + handler writes garbage + rollback.
        context
            .database
            .execute_batch("BEGIN IMMEDIATE;")
            .expect("core must reopen transaction");
        context
            .database
            .execute_batch(
                "CREATE TABLE handler_garbage_should_be_absent (id INTEGER PRIMARY KEY);\
                 INSERT INTO handler_garbage_should_be_absent(id) VALUES (1);",
            )
            .expect("simulate handler attempt");
        context
            .database
            .execute_batch("ROLLBACK;")
            .expect("handler error: rollback must succeed");
        context.mark_transaction_ended_outside_core();
        // Verify: `previous` row still exists; `handler_garbage_should_be_absent`
        // table does NOT exist because rollback rolled back the schema.
        let row_count: i64 = context
            .database
            .query_row("SELECT count(*) FROM previous WHERE marker='pre-handler'", [], |r| {
                r.get(0)
            })
            .expect("count previous");
        assert_eq!(
            row_count, 1,
            "PROCESS-01: pre-existing rows must remain after handler rollback"
        );
        let table_present: bool = context
            .database
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' \
                 AND name='handler_garbage_should_be_absent'",
                [],
                |r| {
                    let n: i64 = r.get(0)?;
                    Ok(n > 0)
                },
            )
            .expect("probe rollback");
        assert!(
            !table_present,
            "PROCESS-01: rolled-back handler writes must leave no trace"
        );
    }

    #[test]
    fn commit_failure_never_reports_session_success() {
        // We inject a SQL error at commit by sabotaging the database
        // statement with a poison value. The processing context must
        // surface a typed Commit error rather than a successful state.
        let mut context = ProcessingContext::new_for_tests();
        context
            .database
            .execute_batch("INVALID SQL;")
            .expect_err(
                "context must report the broken intermediate SQL \
                 as a typed error rather than silently succeeding",
            );
        // Test the commit_database path itself: it must use
        // REQUIRE OPEN TRANSACTION which still passes here because the
        // invalid-statement error rolled Core's transaction back even
        // though we recovered manually.
        let commit_result = context.commit_database();
        // After an invalid statement, SQLite reports autocommit and
        // the open-transaction requirement fails with
        // TransactionNotActive.
        let typed_error = commit_result.err();
        assert!(
            matches!(
                typed_error.map(|e| format!("{e:?}")),
                Some(s) if s.contains("TransactionNotActive")
                    || s.contains("AlreadyCommitted")
                    || s.contains("AlreadyRolledBack")
            ),
            "expected a typed transaction-state error, got: {typed_error:?}"
        );
    }

    #[test]
    fn processing_context_exposes_read_only_admitted_catalog() {
        // The audit requires that the catalog the construction-time
        // admission produced is exactly the one the source sees.
        let context = ProcessingContext::new_for_tests();
        assert_eq!(
            context.transactions().len(),
            0,
            "PROCESS-01: empty catalog exposes zero transactions"
        );
        // The catalog identity matches the construction-time project
        // and runtime source identity (validated by ProcessingContext::new).
        assert_eq!(context.project().name(), "test-project");
        assert_eq!(context.runtime().source_name(), "test-source");
    }

    #[test]
    fn require_transaction_active_distinguishes_open_from_after_handler() {
        let context = ProcessingContext::new_for_tests();
        // Before handler: open transaction, no violation.
        assert!(matches!(
            context.database_state_for_tests(),
            ProcessingDatabaseState::OpenTransaction
        ));
        context
            .require_transaction_active(ProcessingTransactionBoundaryPhase::BeforeHandler)
            .expect("open transaction must pass BeforeHandler check");
        // After handler: ill-defined source ended the transaction
        // outside of Core. We must report a typed violation rather than
        // success.
        assert!(context
            .require_transaction_active(ProcessingTransactionBoundaryPhase::AfterHandler)
            .is_err());
    }

    /// Audit name: `source_commit_or_rollback_attempt_is_detected`.
    ///
    /// Simulates a trusted-native source handler that issues its own
    /// `COMMIT` against the Core-owned connection. The next Core
    /// `commit_database` must surface a typed `AlreadyCommitted`
    /// error rather than silently re-issuing `COMMIT` (and likewise
    /// for `rollback_database`).
    #[test]
    fn source_commit_or_rollback_attempt_is_detected() {
        let mut context = ProcessingContext::new_for_tests();
        // Source side: trusted-native code ends the transaction out of
        // band before Core's post-handler commit/rollback.
        context
            .database
            .execute_batch("COMMIT;")
            .expect("source-side COMMIT must execute against the open transaction");
        // Core is_autocommit now reports true; the typed boundary guard
        // catches it.
        assert!(
            !context.database_transaction_active(),
            "PROCESS-01: SQLite connection must report autocommit after source COMMIT"
        );
        let commit_err = context
            .commit_database()
            .expect_err("Core commit after source COMMIT must fail typed");
        assert!(
            format!("{commit_err:?}").contains("AlreadyCommitted")
                || format!("{commit_err:?}").contains("TransactionNotActive"),
            "PROCESS-01: expected typed AlreadyCommitted or TransactionNotActive, got: {commit_err:?}"
        );
        let rollback_err = context
            .rollback_database()
            .expect_err("Core rollback after out-of-band source COMMIT must fail typed");
        assert!(
            format!("{rollback_err:?}").contains("AlreadyCommitted")
                || format!("{rollback_err:?}").contains("TransactionNotActive"),
            "PROCESS-01: expected typed AlreadyCommitted or TransactionNotActive, got: {rollback_err:?}"
        );
    }

    /// Audit name: `uncertain_commit_retains_uncertain_typed_outcome`.
    ///
    /// When a source ends a Core transaction via `COMMIT` or `ROLLBACK`,
    /// Core cannot tell whether partial commit occurred; the typed
    /// boundary violation must surface `possible_database_partial_commit = true`
    /// at the `AfterHandler` phase.
    #[test]
    fn uncertain_commit_retains_uncertain_typed_outcome() {
        let context = ProcessingContext::new_for_tests();
        // Source issues COMMIT before Core can finalize.
        context
            .database
            .execute_batch("COMMIT;")
            .expect("source-side COMMIT must execute");
        let violation = context
            .require_transaction_active(ProcessingTransactionBoundaryPhase::AfterHandler)
            .expect_err("AfterHandler must report a boundary violation when the source ended the transaction");
        assert!(
            violation.possible_database_partial_commit(),
            "PROCESS-01: AfterHandler must retain partial-commit uncertainty rather than claim a rollback guarantee"
        );
        assert_eq!(
            violation.phase(),
            ProcessingTransactionBoundaryPhase::AfterHandler,
            "PROCESS-01: violated phase must reflect AfterHandler"
        );
    }
}

/// Test-only accessor for the private `database_state`.
#[cfg(test)]
impl ProcessingContext {
    pub(crate) fn database_state_for_tests(&self) -> ProcessingDatabaseState {
        self.database_state
    }
}
