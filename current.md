# Processing raw-transaction discovery and SQLite milestone report

## Files created and changed
- Created `/home/runner/work/lexicon/lexicon/lexicon-core/src/processing/transactions.rs`
- Updated `/home/runner/work/lexicon/lexicon/lexicon-core/src/processing/context.rs`
- Updated `/home/runner/work/lexicon/lexicon/lexicon-core/src/processing/error.rs`
- Updated `/home/runner/work/lexicon/lexicon/lexicon-core/src/processing/runner.rs`
- Updated `/home/runner/work/lexicon/lexicon/lexicon-core/src/processing/mod.rs`
- Updated `/home/runner/work/lexicon/lexicon/lexicon-core/Cargo.toml`
- Updated `/home/runner/work/lexicon/lexicon/lexicon-framework/src/lib.rs`
- Updated `/home/runner/work/lexicon/lexicon/current.md`

## Final processing module structure
- Added `processing::transactions` for raw-root discovery, strict transaction admission reuse, provenance validation, and deterministic catalog creation.
- `processing::context` now owns admitted identities, transaction catalog, canonical database path, SQLite connection, and transaction state.
- `processing::runner` now performs discovery + provenance + managed database setup + single SQLite transaction lifecycle around one handler call.
- `processing::error` now contains typed context/database error surfaces and partial-commit representation.
- `processing::mod` exports source-facing processing types and re-exports `rusqlite`.

## Final `ProcessingContext` representation
- `ProcessingContext` now contains private fields for:
  - validated session paths
  - admitted project identity
  - admitted runtime identity
  - processing session identity
  - `ProcessingHttpTransactionCatalog`
  - canonical database path
  - `rusqlite::Connection`
  - internal database state enum

## Typed processing identity accessors
- Added source-useful accessors:
  - `project()`
  - `runtime()`
  - `session_identity()`
  - `transactions()`
  - `database_path()`
  - `database()`
- Existing validated path accessors remain available.

## Raw-root discovery behavior
- Discovery scans only immediate children of `<protocol-root>/data/raw`.
- Symlink entries are typed failures.
- Regular files are typed failures.
- Unsupported file types are typed failures.
- Recognized partial directories (`.partial-*`) are ignored.
- Finalized directory candidates are admitted strictly via existing `admit_transaction_from_disk(...)`.
- Unrecognized directories are typed failures.
- Malformed finalized candidates fail discovery and are not reclassified as partial.

## Native entry-name handling
- Entry names are classified using native `OsStr` plus lossless `to_str()` checks only.
- No `to_string_lossy()` classification path is used in processing discovery.

## Partial transaction handling
- Recognized partial transaction directories are preserved and ignored.

## Malformed finalized transaction handling
- Any finalized candidate that fails strict admission produces a typed `TransactionAdmission` discovery failure.

## Strict transaction-admission reuse
- Processing discovery directly reuses existing internal HTTP transaction admission:
  - `crate::protocols::http::transaction::metadata::admit_transaction_from_disk(...)`

## Acquisition-session-store derivation
- Provenance store is derived exactly from `<protocol-root>/get-raw-data`.
- Processing does not accept independent raw strings for provenance store location.

## Transaction provenance validation
- For each distinct acquisition session referenced by admitted transactions, processing loads durable record once and validates:
  - project match
  - session identity agreement
  - runtime protocol is HTTP
  - runtime operation is acquisition
  - runtime source matches processing runtime source
  - state is not `Prepared`
  - execution start timestamp exists
  - transaction timestamps lie within durable temporal bounds

## Project/source/runtime/session filtering
- Transactions with mismatched project/source/runtime/session provenance are rejected with typed provenance errors.

## Acquisition-session state behavior
- `Prepared` provenance is rejected.
- Finalized transactions tied to `Running`, `Succeeded`, `Failed`, or `Abandoned` are allowed subject to timestamp validation.

## Timestamp validation
- Enforced checks include:
  - completion >= creation
  - creation >= session record creation
  - creation >= session started_at
  - if session has finished_at, completion <= finished_at

## Processing-visible transaction representation
- Added opaque `ProcessingHttpTransaction` with read-only accessors for:
  - processing project identity
  - admitted acquisition runtime identity
  - acquisition session identity
  - acquisition session state
  - authoritative `RecordedTransaction`

## Deterministic catalog ordering
- Catalog is deterministically sorted by:
  1. transaction creation timestamp
  2. transaction identity string

## Duplicate transaction behavior
- Duplicate transaction identities are rejected via typed `DuplicateTransactionIdentity` failure.

## Redirect, retry, and transport-failure visibility
- Discovery exposes each admitted finalized physical transaction independently; no redirect/retry/transport-failure collapsing is applied.

## Raw-body immutability behavior
- Processing discovery and runner read metadata/paths only; no raw-body rewrites, deletes, moves, or repairs were introduced.

## Header and redaction behavior
- Header parsing/redaction behavior remains authoritative in existing strict admission path; processing reuses admitted `RecordedTransaction` without lossy remapping.

## Canonical SQLite database path
- Runner derives canonical database path as:
  - `<protocol-root>/data/processed/<source-name>.sqlite3`
- Path comes from admitted processing runtime source identity.

## SQLite dependency and exposure boundary
- Added pinned dependency in `lexicon-core`:
  - `rusqlite = { version = "0.32.1", features = ["bundled"] }`
- Exposed compatibility boundary via:
  - `pub use rusqlite;` in `lexicon_core::processing`

## Managed database-path validation
- Runner validates protocol root and processed root with managed-path validator.
- Enforces processed root equals exactly `<protocol-root>/data/processed`.
- Validates database target as existing regular file or creatable regular file under managed root.
- Revalidates database path after opening and before handler invocation.

## SQLite connection configuration
- Runner opens database read-write/create.
- Applies baseline:
  - `PRAGMA foreign_keys = ON;`
  - `PRAGMA journal_mode = DELETE;`
  - `BEGIN IMMEDIATE;`
- Applies bounded busy timeout (`5s`).
- WAL was not enabled.

## Handler transaction lifecycle
- One processing handler invocation runs inside one SQLite transaction.
- Commit and rollback methods are runner-internal via `ProcessingContext` internal API.

## Handler-success sequence
- Handler `Ok(())` -> SQLite `COMMIT` -> processing session `Succeeded` persistence.
- If commit succeeds but session success persistence fails, runner returns typed partial-commit error.

## Handler-failure sequence
- Handler `Err` -> SQLite `ROLLBACK` -> processing session `Failed` persistence.
- Rollback failures preserve handler failure + rollback failure + optional terminal persistence failure.

## Panic/drop behavior
- `ProcessingContext::Drop` never commits.
- Dropping with open transaction performs best-effort rollback.

## Discovery and setup failure behavior
- On discovery/setup/path/open/context errors, handler is not invoked.
- Runner attempts Core-authored runtime failure persistence before returning typed setup error (or typed terminal persistence error if persistence fails).

## SQLite commit failure behavior
- Commit failures do not produce `Succeeded` state.
- Runner attempts runtime failure persistence; returns combined typed error if both commit and persistence fail.

## SQLite/session partial-commit representation
- Added `ProcessingDatabasePartialCommit` with retained:
  - processing project identity
  - processing runtime identity
  - processing session identity
  - canonical database path
  - typed session persistence failure

## Final processing runner sequence
- Updated runtime invocation path to:
  - parse invocation
  - admit processing invocation
  - decode runtime context
  - open processing session store
  - bind session
  - enter `Running`
  - derive typed paths/identities
  - discover and admit raw transactions
  - validate acquisition provenance
  - derive/validate canonical database path
  - open database + baseline config + `BEGIN IMMEDIATE`
  - construct `ProcessingContext`
  - invoke admitted source handler
  - commit or rollback SQLite
  - persist terminal session state

## Generated processing scaffold changes
- Updated generated processing implementation template to demonstrate:
  - iterating `context.transactions().iter()`
  - obtaining `context.database()`
  - source ownership of schema/SQL
- Removed mandatory `todo!` scaffold failure for processing template.

## Stale processing test-source alignment
- Removed obsolete test-side `ProcessingContext::default()` runner call arguments in processing runner tests.
- Did not reintroduce `ProcessingContext::default()`.

## Public/internal API boundary
- Public processing exports now include:
  - `ProcessingContext`
  - `ProcessingHttpTransaction`
  - `ProcessingHttpTransactionCatalog`
  - typed processing discovery/provenance/context/database/partial-commit errors
  - existing processing descriptor/admission/probe/runner APIs
  - compatible SQLite API re-export (`rusqlite`)
- Internalized behaviors remain in non-public helpers/modules (discovery internals, path derivation helpers, commit/rollback internals).

## Typed error hierarchy
- Added or extended typed processing errors for:
  - transaction discovery
  - provenance rejection
  - context construction
  - database path
  - database open
  - database commit/rollback
  - database/session partial commit
- `ProcessingRuntimeInvocationExecutionError` now includes typed variants for setup/discovery/context/database/handler/rollback/terminal/partial-commit outcomes.

## Sensitive diagnostic behavior
- New error display strings use stable categories and avoid leaking request/response bodies, headers, SQL row content, source args, or envelope/context JSON.

## Confirmation that acquisition data was not modified
- No code path was added that mutates acquisition raw-data trees, checkpoints, progress, or acquisition session records.

## Confirmation that foreground ownership remained unchanged
- Existing foreground session binding/lifecycle ownership path remains in use; no alternate invocation route was introduced.

## Confirmation that background supervision and lexicon build were not added
- No background processing host/supervision handoff or build orchestration was added.

## Command and validation execution confirmation
- Per the milestone command constraints and test-skip directive, no test/check/build/fmt/clippy/metadata/rustc command was run.
- No runtime/HTTP/SQLite tooling execution, workspace validation, or bundle/install pipeline execution was run.
- Attempted `lexicon init . telugu-lexicon` from `instructions.md` step 4, but CLI was unavailable in this environment (`lexicon: command not found`).
