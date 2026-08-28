Current milestone: implement and verify the source-owned SQLite work-ledger and durable state pattern
Objective
Implement the canonical source-owned SQLite work-ledger pattern on top of `HttpAcquisitionContext::source_state_directory()` per specs.md §13-§15, re-export `rusqlite` from `lexicon_core::http` (matching `lexicon_core::processing`), and provide complete fixture-backed test coverage for the required durable source state invariants in specs.md §44.
This milestone is derived from:
contract.md §2 (a source implementation may preserve source-specific durable state, resume partially completed work), §3 (trust model: ordinary Rust), §9 (durable source state and work ledgers: Core provides the directory handle, source manages its schema);
specs.md §12 (durable source state), §13 (work ledger schema & deduplication), §14 (discovery and fan-out), §15 (work execution and checkpoint composition), §16 (source phases), and §44 (Required Tests under "Durable source state": work insertion deduplication, repeated discovery convergence, crash after checkpoint before work completion, recovery marks checkpointed work complete, SQLite schema migration, simultaneous unsupported writer rejection).
Repository-grounded starting point
`HttpAcquisitionContext::source_state_directory()` (lexicon-core/src/protocols/http/context.rs) was added in Milestone 2 and verified to persist across sequential sessions.
`rusqlite` is a bundled dependency of `lexicon-core` (Cargo.toml) and is already re-exported on the processing side (`lexicon_core::processing::rusqlite`), but is not yet re-exported on the acquisition side (`lexicon_core::http::rusqlite`).
`lexicon-core/src/protocols/http/runner.rs` execution_tests contains fixture-backed tests for HTTP execution, session state, and checkpoints, but does not yet contain tests for the source-owned SQLite work-ledger composition, deduplication, discovery convergence, crash-reconciliation, and schema migration invariants defined in specs.md §13-§15 and §44.
Required implementation
1. Re-export rusqlite for HTTP acquisition sources
In `lexicon-core/src/protocols/http/mod.rs` (and `lexicon-core/src/lib.rs` / `lexicon-core/src/protocols/mod.rs` if appropriate), re-export `rusqlite` so HTTP acquisition sources can use the framework's compatible bundled SQLite driver.
2. Implement canonical WorkLedger test fixture and support types
In `lexicon-core/src/session/test_support.rs` or `lexicon-core/src/protocols/http/runner.rs` (in test modules), implement a canonical `WorkLedger` helper conforming to specs.md §13-§15:
* `work_items` table with `(kind, stable_key, payload_version, payload, status, attempt_count, last_error, origin_transaction_id, created_at, updated_at)` with primary key `(kind, stable_key)`;
* methods for `insert_if_absent`, `mark_active`, `mark_complete`, `mark_failed`, `pending_items`, `migrate_schema`;
* transactional updates using SQLite transactions.
3. Add required fixture-backed tests for specs.md §44 "Durable source state"
Add execution tests using `RuntimeInvocationFixture`:
* `work_insertion_deduplication`: prove multiple inserts of the same `(kind, stable_key)` work item converge without duplicating rows;
* `repeated_discovery_convergence`: prove discovery interrupted before checkpoint commit can run again in a second session against the same state directory and converge without duplicate work items;
* `crash_after_checkpoint_before_work_completion`: prove that if an execution crashes after `context.commit_checkpoint(&key)` but before marking work complete in SQLite, a subsequent session uses `context.has_checkpoint(&key)` to observe the committed checkpoint and mark the item complete without repeating the HTTP request;
* `sqlite_schema_migration`: prove opening a state database with schema version 1 and migrating to version 2 inside a transaction upgrades columns, updates metadata, and preserves all existing work item records;
* `simultaneous_unsupported_writer_rejection`: prove SQLite file locking rejects concurrent write transactions on the same database file (e.g. `sqlite3_busy` / locked error).
Scope constraints
Do not implement during this milestone:
* Core-owned task queues or shared `durable-work-v1` capability (explicitly deferred per specs.md §46);
* automatic schema migration or Core interpretation of the source's SQLite files (source owns its schema, per contract.md §9);
* MZA Protocol 1 release construction;
* changes to `lexicon build` or `lexicon source build`;
* second protocol support.
Completion criteria
This milestone is complete only when:
* `rusqlite` is re-exported from `lexicon_core::http`;
* all 6 required durable source state test categories from specs.md §44 are covered by fixture-backed execution tests;
* `cargo check --workspace` passes;
* `cargo test --workspace --quiet` passes;
* no production contract is weakened.
Completion report
When the milestone passes, replace this file with a concise report containing:
* the exact commit tested;
* confirmation that `cargo check --workspace` passed;
* confirmation that `cargo test --workspace --quiet` passed;
* where `rusqlite` was re-exported and how the WorkLedger tests were structured;
* the list of new tests added corresponding to specs.md §44;
* confirmation that no required test remains ignored, deleted, or falsely successful.
Then stop.
The following milestone should be derived from the updated contract and specification once this one lands.
