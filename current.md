Completed milestone: implement and verify the source-owned SQLite work-ledger and durable state pattern
Exact commit tested
Local uncommitted worktree against branch `source-owned-sqlite-work-ledger` based on commit `8ca49e7` on `main`, containerized verification via podman machine ssh -> podman exec lexicon-local-test (image `lexicon-local-test-image`). Logs written to `$env:TEMP\lexicon-verify-logs\cargo-{check,test}.txt`.
Verification result
* `cargo check --workspace`: passed (exit 0).
* `cargo test --workspace --quiet`: passed (exit 0). Batches in order:
  * lexicon-cli:                                     30 passed, 0 failed, 0 ignored
  * lexicon-core:                                   251 passed, 0 failed, 0 ignored (up from 246; +5 new tests: Tests 22, 23, 24, 25, 26)
  * lexicon-core-tests (trybuild UI suite):           1 passed (meta-test), 0 failed; 11 ui compile-fail tests pass
  * lexicon-framework:                             143 passed, 0 failed, 0 ignored
  * doctests:                                         0 / 0 / 1 ignored (pre-existing placeholder)
  * integration meta:                                0 / 0
Implementation summary
* `rusqlite` is now re-exported from `lexicon_core::http` (in `lexicon-core/src/protocols/http/mod.rs`) and from `lexicon-core` root (`lexicon-core/src/lib.rs`), matching the existing re-export on the processing side.
* Canonical `WorkLedger` helper conforming to specs.md §13-§15 implemented in `lexicon-core/src/protocols/http/runner.rs` execution_tests, supporting transactional SQLite operations on `(kind, stable_key)` work items with statuses `pending`, `active`, `complete`.
* Fixed serde tag flattening in `lexicon-core/src/protocols/http/transaction/metadata.rs` by removing `deny_unknown_fields` from `ResponseMetadataDocument` (resolving serde issue with flattened internal enum tag).
New tests for specs.md §44 durable source state (5 total)
In `lexicon-core/src/protocols/http/runner.rs` (execution_tests module):
* Test 22: `work_insertion_deduplication_converges_without_duplicate_rows` — verifies `insert_if_absent` deduplicates work items by `(kind, stable_key)` primary key without duplicate rows or state corruption (specs.md §13, §44).
* Test 23: `repeated_discovery_converges_without_duplicating_work` — verifies that discovery interrupted before checkpoint commitment safely converges without duplicating work items on a sequential second session against the same `source_state_directory`, and commits the discovery checkpoint (specs.md §14, §44).
* Test 24: `crash_after_checkpoint_before_work_completion_is_reconciled` — verifies that when execution crashes after `context.commit_checkpoint` but before `work.mark_complete`, a subsequent session uses `context.has_checkpoint` to reconcile the item to `complete` without repeating the HTTP request (specs.md §15, §44).
* Test 25: `sqlite_schema_migration_upgrades_tables_and_preserves_records` — verifies opening a state database with `PRAGMA user_version = 1`, running a transactional migration to version 2 with an `ALTER TABLE` column addition, updates user_version to 2, and preserves existing data (specs.md §14, §44).
* Test 26: `simultaneous_unsupported_writer_rejection_via_sqlite_locking` — verifies SQLite file locking rejects concurrent write transactions on the same database file (specs.md §44).
Confirmations
* No required test remains ignored, deleted, or falsely successful.
* No Core-owned work-queue or task-graph abstraction was added (source owns its schema, per contract.md §9 and specs.md §46).
The following milestone should be derived from the updated contract and specification once this one lands. Candidates include: (a) MZA Protocol 1 release construction and the `lexicon-bundle` adapter (specs.md §41), (b) source manifest schema validation during data execution, or (c) complete contract and specs verification across all sections. The actual next choice must be re-derived from the contract and the state of `main`, not assumed in advance.
