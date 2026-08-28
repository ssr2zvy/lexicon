Completion report: HTTP cross-platform publication and partial-failure ownership closure

Status: Source-only milestone implemented in lexicon-core HTTP modules (no tests/build/check/fmt/clippy/cargo metadata/rustc run per instruction).

Implemented:
- Unified publication boundary with typed `HttpTransactionPublicationError` and platform-specific no-replace directory publication for Linux/macOS/Windows.
- Windows publication path added using native wide-path API; Linux uses named `RENAME_NOREPLACE`; macOS keeps `renamex_np(..., RENAME_EXCL)` with typed interior-NUL handling.
- Parent durability step made platform-aware, including Windows directory-handle flush path.
- Managed path validation extracted to typed `HttpManagedPathError` with trusted-root containment and explicit modes: existing/creatable directory/file.
- Metadata persistence converted to typed `HttpMetadataPersistenceError` (no stringification of managed-path causes).
- Incomplete marker persistence converted to typed `HttpIncompleteMarkerError`.
- Incomplete response failure now returns typed `IncompleteHttpResponseFailure`, retaining stream error + partial body sync outcome + marker outcome + partial body digest/length.
- Progress partial commits now preserve ownership in `HttpProgressPartialCommit` with accessors to finalized transaction, identity, path, attempt identity, outcome, and progress error.
- Progress update flow no longer discards finalized owner; wired through `HttpExecutionError::ProgressPartialCommit`.
- Progress validation no longer self-validates revision against itself; invariant validation is internal, revision conflict detection is explicit.
- Progress persistence now revalidates running session + lease + on-disk revision immediately before replacement and returns typed revision conflict without overwrite.
- Redirect/orchestration control flow no longer clones `RecordedTransaction` for missing-location/invalid-target branches.
- Outcome branching now uses `HttpRecordedOutcomeKind`; removed outcome cloning for control flow.
- Transport config error now stores typed reqwest cause (`Arc<reqwest::Error>`); `source()` exposed safely with sanitized display.
- TLS classification no longer parses error strings; transport failure classes now reflect only typed-proven classes.
- `StoredHttpVersion` no longer serializes unknown versions as understood values; unsupported reqwest versions are persisted as `None`.
- Admission tightened: directory name `<timestamp>-<transaction-id>`, immediate-child containment under trusted raw root, top-level/request/response exact managed shape, no unexpected entries/symlinks/nesting, non-regular file rejection, timestamp and attempt/parent invariants, metadata identity consistency.
- HTTP contract test source in `protocols/http/contract.rs` updated to current `HttpAcquisitionContext` API.
- Consolidated stale/duplicate recorder error variants and updated HTTP re-exports/usages to authoritative typed errors.

Files changed:
- lexicon-core/Cargo.toml
- lexicon-core/src/protocols/http/context.rs
- lexicon-core/src/protocols/http/contract.rs
- lexicon-core/src/protocols/http/error.rs
- lexicon-core/src/protocols/http/mod.rs
- lexicon-core/src/protocols/http/transport.rs
- lexicon-core/src/protocols/http/transaction/error.rs
- lexicon-core/src/protocols/http/transaction/metadata.rs
- lexicon-core/src/protocols/http/transaction/mod.rs
- lexicon-core/src/protocols/http/transaction/recorder.rs
