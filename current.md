Implementation report: processing correctness, durability, and error-preservation closure

Milestone status

Complete. All twenty-one repository-grounded defects from the previous `current.md` are corrected. No background supervision, `__operator-host`, lexicon build, or automatic build-before-run work was started.

Files changed

* `lexicon-core/src/processing/error.rs` — rewritten typed error hierarchy.
* `lexicon-core/src/processing/transactions.rs` — rewritten discovery and provenance.
* `lexicon-core/src/processing/context.rs` — rewritten context invariants and database state machine.
* `lexicon-core/src/processing/runner.rs` — rewritten execution ownership, SQLite policy, durability, and terminal outcomes.
* `lexicon-core/src/processing/mod.rs` — tightened public/internal API boundary.
* `lexicon-core/src/session/model.rs` — added stable processing session failure codes.
* `lexicon-core/src/protocols/http/transaction/recorder.rs` — authoritative staging-name grammar plus shared durability boundary.
* `lexicon-core/src/protocols/http/transaction/metadata.rs` — authoritative finalized-name parser shared internally.
* `lexicon-core/src/protocols/http/context.rs` — removed a duplicate directory-name parser by delegating to the authoritative one.
* `lexicon-framework/src/lib.rs` — generated processing scaffold now fails explicitly.
* `instructions.md` — workflow routes through `current.md`; all Cargo work goes through the test container.

Per-transaction provenance correction

Discovery now separates two concerns:

1. session-record admission, performed at most once per typed session identity;
2. transaction-to-session provenance validation, performed for every transaction.

`validate_session_record` proves the session-level invariants that depend only on the durable record: project agreement, HTTP protocol, acquisition operation, runtime source agreement, non-`Prepared` state, and presence of an execution start timestamp.

`validate_transaction_against_session` runs for every admitted transaction and proves session identity agreement, transaction creation timestamp, transaction completion timestamp, transaction ordering, the session start bound, and the terminal session finish bound when present.

Cache presence can no longer bypass transaction validation: the cached record is only a source of durable session facts, never a substitute for validating a transaction.

Typed provenance-cache behavior

The cache is `HashMap<SessionIdentity, SessionRecordV1>`. `SessionInvocationIdentity` already derived `Hash`, so no new derive was required and no typed identity is converted to a string for keying.

Removal of production processing expect and unwrap

Every ordinary-path `expect`, `unwrap`, and assertion is gone from processing discovery and execution:

* the `.expect("session provenance cache must contain loaded record")` lookup is replaced by a `let ... else` that returns `ProcessingTransactionDiscoveryError::ProvenanceCacheInvariant { acquisition_session }`;
* all four `running.take().expect("running lifecycle must exist")` calls are gone because the running session is no longer optional.

`ProcessingLifecycleError::RunningSessionUnavailable` exists for the case where absence is representable; the corrected ownership model makes it unreachable on the ordinary path. Test-only `expect` calls remain in fixture construction only.

Final running-session ownership representation

```rust
struct RunningProcessingExecution<'store> {
    running: crate::session::RunningRuntimeSession<'store>,
    project: crate::session::ProjectIdentity,
    runtime: crate::runtime::OwnedRuntimeIdentity,
    session: crate::session::SessionIdentity,
}
```

The owner is created immediately after `enter_running` and provides consuming operations for setup failure (`fail_setup`), source failure (`fail_source`), runtime failure (`fail_runtime`), successful completion (`complete`), and committed-database partial completion (through `fail_runtime` combined with a retained `ProcessingDatabasePartialCommit`). It is consumed exactly once on every path. `Option` no longer models a mandatory owner.

Setup plus persistence error preservation

```rust
pub enum ProcessingSetupError {
    TransactionDiscovery(ProcessingTransactionDiscoveryError),
    DatabasePath(ProcessingDatabasePathError),
    DatabaseOpen(ProcessingDatabaseOpenError),
    ContextConstruction(ProcessingContextConstructionError),
    TransactionBoundary(ProcessingTransactionBoundaryViolation),
}

pub struct ProcessingSetupAndPersistenceFailure {
    setup_error: ProcessingSetupError,
    persistence_error: SessionStoreError,
}
```

`fail_setup` persists the terminal failure state and returns `Setup(..)` on success or `SetupAndPersistence(..)` when persistence also fails. Both errors are retained as typed values with read-only accessors (`setup_error()`, `persistence_error()`); neither is reduced to `String`. `source()` returns the primary setup error.

Stable processing failure codes

`SessionFailureCode` gained six additive variants with stable snake_case identifiers:

* `ProcessingTransactionDiscoveryFailed` → `processing_transaction_discovery_failed`
* `ProcessingTransactionProvenanceFailed` → `processing_transaction_provenance_failed`
* `ProcessingDatabasePathInvalid` → `processing_database_path_invalid`
* `ProcessingDatabaseOpenFailed` → `processing_database_open_failed`
* `ProcessingDatabaseTransactionFailed` → `processing_database_transaction_failed`
* `ProcessingContextConstructionFailed` → `processing_context_construction_failed`

`identifier()` is updated. Strict session decoding follows automatically from the existing serde `rename_all = "snake_case"` derive; the change is purely additive to the session schema, which is the only session-schema change permitted by the milestone.

`ProcessingSetupError::failure_code()` selects the code per phase, distinguishing raw discovery from transaction provenance through `ProcessingTransactionDiscoveryError::is_provenance_failure()` rather than by inspecting `Display`. `ProcessingSetupError::diagnostic()` supplies bounded Core-authored `&'static str` diagnostics only. No URLs, headers, bodies, SQL, source arguments, or source error text are persisted.

Exact raw, acquisition, and processed root validation

* Discovery requires `raw_root == protocol_root/data/raw` exactly and returns `RawRootDisagreement { expected, actual }` otherwise. It establishes this invariant itself instead of trusting the validated runtime-context path supplied by the caller.
* Discovery derives `protocol_root/get-raw-data` itself, validates it as a managed existing directory, and returns `AcquisitionRootInvalid { acquisition_root, source }` otherwise, before opening the acquisition session store. Mere descendants of the protocol root are not accepted.
* `derive_processing_database_path` requires `processed_root == protocol_root/data/processed` exactly and returns the typed `ProcessedRootDisagreement`.

Exact partial-directory classification

The recorder now owns the staging-name grammar and exposes it internally:

* `PARTIAL_TRANSACTION_DIRECTORY_PREFIX`
* `staging_transaction_directory_name` / `finalized_transaction_directory_name` (used by the recorder itself)
* `classify_staging_transaction_directory_name` returning `NotStaging`, `Valid { timestamp, transaction_id }`, or `Malformed`

The grammar is exactly `.partial-<timestamp>-<transaction-id>` where the timestamp is a nonzero ASCII-decimal `u64` and the transaction id is a valid `HttpTransactionIdentity`. Processing classification now yields:

* valid Core partial transaction directory → ignored;
* malformed partial-looking directory → `RawEntryMalformedPartialDirectory`;
* finalized candidate → strictly admitted through `admit_transaction_from_disk`;
* unrelated directory → `RawEntryUnrecognizedDirectory`;
* non-UTF-8 name → `RawEntryNameInvalid`.

Arbitrary `.partial-*` names are no longer accepted as valid staging. Partial directories are never deleted. The finalized grammar is the single authoritative `parse_transaction_directory_name` in transaction admission; the duplicate copy in the acquisition HTTP context now delegates to it, preserving that path's existing acceptance behavior exactly.

Final processing-context invariants

`ProcessingContext::new` proves, in addition to HTTP protocol and processing operation:

```text
operation_root           = protocol_root/process-data
session_directory        = operation_root/sessions/<session-id>
raw_data_directory       = protocol_root/data/raw
processed_data_directory = protocol_root/data/processed
database_path            = processed_data_directory/<runtime-source>.sqlite3
```

Failures return `ProcessingContextConstructionError::ManagedPathDisagreement { category, expected, actual }` where `category` is a stable `ProcessingManagedPathCategory`. A context that combines separately valid but mutually inconsistent components cannot be constructed.

Catalog and context identity agreement

Every catalog entry is validated against the processing project, the HTTP protocol, and the processing runtime source, yielding `CatalogProjectMismatch`, `CatalogProtocolMismatch`, or `CatalogSourceMismatch` with the offending `catalog_index`.

Database state-transition behavior

Only two transitions succeed:

```text
Open → Committed
Open → RolledBack
```

Everything else is rejected with `AlreadyCommitted`, `AlreadyRolledBack`, or `TransactionNotActive`. A fourth internal state, `EndedOutsideCore`, records a transaction that ended outside Core's control or with an uncertain outcome; from it, commit and rollback both return `TransactionNotActive` and `Drop` issues no further statements. `Drop` still performs a best-effort rollback only while the transaction is genuinely open.

Source transaction-boundary detection

`ProcessingContext::require_transaction_active(phase)` requires `!connection.is_autocommit()` immediately before invoking the handler and again immediately after it returns. Loss of the boundary produces `ProcessingTransactionBoundaryViolation { phase, possible_database_partial_commit }`.

* Before the handler, the violation is a setup failure.
* After the handler, `possible_database_partial_commit` is true, because Core cannot distinguish a source `COMMIT`/`END` from a source `ROLLBACK`. The runner therefore returns `DatabasePartialCommit` with phase `SourceTransactionBoundaryLoss` and never claims the database was rolled back.

Processing success is never returned after boundary loss. This is documented in code as enforcement of the supported Core route, not hostile-code confinement.

Simultaneous catalog and database borrowing API

```rust
pub fn resources(
    &mut self,
) -> (&ProcessingHttpTransactionCatalog, &mut rusqlite::Connection);
```

The borrows are disjoint, so a source can iterate admitted transactions while writing rows without cloning the catalog. The individual `transactions()` and `database()` accessors are retained.

Final ProcessingError representation

```rust
pub enum ProcessingError {
    Source {
        operation: &'static str,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    SourceMessage { message: &'static str },
}
```

Constructors are `ProcessingError::source(operation, error)` and `ProcessingError::source_message(message)`, with `operation()` and `message()` accessors. `Display` renders only compile-time static text, so it can never contain SQL, row data, bodies, headers, URLs, or source arguments. `source()` returns the typed nested error where present. The handler signature is unchanged. Arbitrary source failure text is never persisted into session records: the runner persists `SafeSessionFailure::source_failure()` for ordinary source failures and Core-authored `&'static str` diagnostics elsewhere.

SQLite open flags

```rust
let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
    | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
rusqlite::Connection::open_with_flags(database_path, flags)
```

`SQLITE_OPEN_URI` is deliberately absent, so URI filename interpretation stays disabled and an alternate filename cannot be embedded in the managed path. The database is never opened read-only.

SQLite pragma verification

`PRAGMA foreign_keys = ON` and `PRAGMA journal_mode = DELETE` are applied before the transaction begins, because SQLite refuses journal-mode changes inside an active transaction. The effective configuration is then read back and verified:

* `PRAGMA foreign_keys` must be `1`;
* `PRAGMA journal_mode` must be `delete` (case-insensitive), so WAL stays disabled;
* after `BEGIN IMMEDIATE`, the connection must not be in autocommit.

Disagreements return `ProcessingDatabaseConfigurationError::Disagreement { setting, expected, actual }` and readback failures return `Readback { setting, source }`, both keyed by a stable `ProcessingDatabaseSetting`. Core never silently continues with an unexpected journal mode.

Database creation durability

When the invocation created the database, the file is re-validated as a managed existing regular file, then the database file and the processed-data directory are synchronized through the shared cross-platform Core durability boundary (`sync_regular_file` and `sync_directory` in the HTTP transaction recorder, now `pub(crate)` instead of duplicated). Failures return `ProcessingDatabaseOpenError::Durability(..)`.

Post-commit durability

After a successful SQLite `COMMIT`, the connection is closed, then the database file and processed-data directory are synchronized, then sidecar cleanup is validated, and only then is the session marked `Succeeded`. The session is never marked successful while required database durability remains unresolved.

If the commit succeeded but durability failed, the runner returns `ProcessingDatabasePartialCommit` with phase `PostCommitDurability`, retaining project, runtime, session, database path, failure phase, and the typed `ProcessingDatabaseDurabilityError`. Core never claims the database was rolled back after SQLite committed.

Rollback-journal policy

The allowed sidecar policy is explicit and centralized in `validate_database_sidecars`:

* only the transient rollback journal `<database>-journal` associated with the canonical database is permitted, and only before the database is opened (a journal left by an interrupted writer is legitimate and SQLite recovers it);
* a rollback journal that survives transaction completion returns `RollbackJournalNotCleanedUp`;
* pre-existing symlinks at any sidecar path return `Symlink`;
* pre-existing wrong file types return `WrongFileType`;
* inspection failures return `Inspection { kind, path, source }`.

Cleanup is validated after the transaction finishes on both the success and source-failure paths. Nothing is ever deleted: an unexpected user file whose name resembles a SQLite sidecar is reported, not removed. `data/processed` is not recursively accepted; only the three canonical sidecar paths are examined.

WAL/SHM rejection

`<database>-wal` and `<database>-shm` are never permitted. Their presence returns `ForbiddenSidecarPresent { kind, path }` both before open and after the transaction.

Commit-outcome uncertainty behavior

A failed `COMMIT` is classified conservatively using the connection's transaction state:

* transaction still active → definitely not committed → `DatabaseTransaction` or, when terminal persistence also failed, `DatabaseCommitAndPersistenceFailure`;
* transaction gone → outcome uncertain → `ProcessingDatabaseCommitOutcomeUncertain` retaining project, runtime, session, database path, and the SQLite error.

Core makes no rollback or no-change guarantee that SQLite cannot prove. The processing session never becomes `Succeeded` when the commit outcome is uncertain; a stable runtime failure code is persisted instead.

Combined typed-error accessors

Every combined failure exposes read-only typed accessors for all retained errors:

* setup plus terminal persistence — `ProcessingSetupAndPersistenceFailure::setup_error()` / `persistence_error()`;
* handler plus rollback — `handler_error()` / `database_transaction_error()` / `session_persistence_error()`;
* handler plus terminal persistence — `handler_error()` / `session_persistence_error()`;
* commit plus failure persistence — `database_transaction_error()` / `session_persistence_error()`;
* committed database plus success-persistence failure — `ProcessingDatabasePartialCommit::cause()` / `session_persistence_error()`;
* commit durability partial failure — `durability_error()`;
* sidecar partial failure — `sidecar_error()`;
* uncertain commit result — `commit_error()` / `session_persistence_error()`.

`ProcessingRuntimeInvocationExecutionError` adds `handler_error()`, `setup_error()`, `database_transaction_error()`, `sidecar_error()`, `database_partial_commit()`, `database_commit_outcome_uncertain()`, and `session_persistence_error()`. `source()` returns the primary error; secondary errors are inspectable without parsing `Display`.

Generated processing placeholder behavior

`format_processing_implementation_library` now emits an implementation that compiles while making incompleteness explicit:

```rust
Err(ProcessingError::source_message(
    "processing implementation is not configured",
))
```

It carries commented guidance showing `let (transactions, database) = context.resources();` with a transaction loop and a source-owned SQL call. An untouched generated source can no longer mark a real processing session `Succeeded` with an empty database. No processing mechanics were added to the managed runner `main.rs`; the runner template is unchanged.

Sensitive Debug and Display behavior

* `ProcessingContext::Debug` no longer prints `database_path`. It prints the project name, runtime source, session id, the stable managed path category `database_file`, and the catalog length, and remains non-exhaustive.
* New `Display` implementations render stable categories, phases, settings, and sidecar kinds rather than raw paths. They do not reveal URLs, headers, bodies, SQL, row data, source arguments, envelope JSON, runtime-context JSON, or environment values.
* Typed error fields still retain paths for programmatic recovery, reachable through accessors such as `ProcessingDatabaseSidecarError::path()` and `ProcessingDatabasePartialCommit::database_path()`.

Final processing runner sequence

```text
parse invocation
→ admit processing invocation
→ decode managed context
→ open processing SessionStore
→ bind processing session
→ enter Running with non-optional owner
→ validate exact raw/acquisition/processed roots
→ enumerate raw entries
→ strictly admit finalized transactions
→ load typed acquisition-session cache
→ validate every transaction against its session
→ build deterministic catalog
→ derive exact database path
→ validate main and sidecar paths
→ open SQLite with explicit flags
→ configure and verify baseline pragmas
→ BEGIN IMMEDIATE
→ construct fully checked ProcessingContext
→ verify transaction is active
→ invoke source handler
→ verify transaction remains active
→ success:
     SQLite COMMIT
     close connection
     database/file/directory durability
     sidecar validation
     processing session Succeeded
→ source failure:
     SQLite ROLLBACK
     sidecar validation
     processing session Failed
→ setup/runtime failure:
     preserve primary typed error
     persist stable processing failure code
→ partial or uncertain commit:
     preserve database provenance
     do not report success
```

Public and internal API boundary

Exposed through `lexicon_core::processing`: `ProcessingContext`, `ProcessingHttpTransaction`, `ProcessingHttpTransactionCatalog`, `ProcessingError`, `ProcessingResult`, a compatible `rusqlite`, the existing descriptor, admission, probe, and runner APIs, and the errors that genuinely cross the source or supervisor boundary, including the discovery, provenance, context construction, database path, open, configuration, transaction, durability, sidecar, partial commit, uncertain commit, lifecycle, and setup error types with their stable identifier enums.

Kept internal: the `context`, `contract`, `error`, `invocation`, and `transactions` modules are now private modules re-exported by name, so raw-directory classifiers, transaction admission helpers, the provenance cache, `RunningProcessingExecution`, database-state transitions, `validate_database_sidecars`, the commit and durability helpers, `require_transaction_active`, `mark_transaction_ended_outside_core`, and every unchecked constructor stay crate-internal. `processing::runner` remains public because generated managed runners import it by path.

Acquisition raw-data immutability confirmation

Processing performs no writes, renames, or deletions under `data/raw`. Discovery only reads directory entries and admits transactions from disk. Partial directories are ignored, never removed. Acquisition checkpoints, progress files, and acquisition sessions are only read: the acquisition session store is opened and `load` is called; no transition, write, or lease operation is performed against it.

Foreground supervision confirmation

Foreground supervision and session ownership are unchanged. `bind_runtime_session`, `enter_running`, lease inspection, supervisor lease ownership, foreground launching, and foreground reconciliation were not modified. Session transitions still go through `SessionStore::transition` and the existing `validate_transition` rules.

Background supervision and lexicon build confirmation

No background operator host, background handoff, signal forwarding, cancellation, processing checkpoints, incremental-processing policy, fixed source schemas, ORM behavior, decoded response readers, new HTTP capabilities, client certificates, proxies, lexicon build, automatic build-before-run, source migration, cross-compilation, MZA change, or installer change was added. `HttpCapabilitySet::empty()` is retained and `ClientCertificateV1` is not advertised.

Preserved behavior

Unchanged: the processing handler signature, acquisition and resume handler signatures, invocation-envelope JSON, argv transport, source argument preservation, acquisition admission, processing admission, runtime-information probes, the session schema apart from the additive failure-code variants, supervisor lease ownership, foreground launching, foreground reconciliation, HTTP transport, retries, redirects, raw transaction formats, raw-byte fidelity, header redaction, acquisition progress, checkpoints, managed runner entrypoints, source build, runtime verification, bundle staging, paired publication, CLI syntax, MZA, Protocol 1, and installer behavior.

Test source adjustments

Existing test source was adjusted only where the production API changed:

* two `Err(ProcessingError)` construction sites in the runner execution tests now use `ProcessingError::source_message("test source failure")`;
* `ProcessingContext::new_for_tests` derives its paths from a single protocol root so the fixture satisfies the stricter context invariants and the canonical database filename.

No test was weakened or removed.

Command-execution confirmation

No `cargo test`, `cargo check`, `cargo build`, `cargo fmt`, `cargo clippy`, `cargo metadata`, or `rustc` invocation was run. No lexicon CLI command, generated runner, processing runtime, SQLite tool, HTTP server, real or test HTTP request, workspace validation, or bundle/install automation was executed. No CLI command was attempted merely to confirm installation. Full validation remains deferred to the final project-wide validation milestone, and `instructions.md` now requires every future Cargo invocation to run inside the `lexicon-local-test` container.

Next step

Processing correctness closure is complete. Background supervision may now be considered as a separate milestone.
