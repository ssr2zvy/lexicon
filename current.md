Completion report: durable session storage and runtime-context foundation

Status: complete

## Summary

All items in the "durable session storage and runtime-context foundation" milestone have been implemented. No Cargo invocations were performed during this milestone.

## Files created

### lexicon-core/src/session/

- **error.rs** – Full typed error hierarchy: `SessionEncodingError`, `SessionDecodingError`, `SessionTransitionError`, `SessionLeaseError`, `SessionStoreError`, `RuntimeContextError`, `CoreRunnerSessionError`. All variants implement `std::error::Error` and `Display`.

- **model.rs** – Core data types: `SESSION_SCHEMA_VERSION = 1`, `SessionOperation`, `SessionState`, `SessionFailureKind`, `SessionFailureV1`, `SessionTimestamp`, `SessionClock` trait, `SystemClock` (wall-clock implementation), `SessionRecordV1` (JSON encode/decode), `SessionStatusV1` (JSON encode/decode), `SessionTransition`, `NewSessionRecord`, `generate_session_id()`, `fmt_unix_timestamp()`. Type aliases `ProjectIdentity = ProjectInvocationIdentity` and `SessionIdentity = SessionInvocationIdentity`.

- **lease.rs** – `SessionLease` RAII guard. Acquires an exclusive OS-level lock (`flock(LOCK_EX | LOCK_NB)` on Unix, `LockFileEx` on Windows) on the session's `.lock` file. Releases the lock on `Drop`. Returns `SessionLeaseError::AlreadyOwned` when the lock is held by another process.

- **transition.rs** – `validate_transition()` function enforcing the legal session state machine.

- **store.rs** – `SessionStore` (constructed from `RuntimeContextPaths`), `SessionOperationRoot`, `PreparedSession`, `RunningSession`. Full store operations: `create_prepared`, `promote_to_running`, `complete_succeeded`, `complete_failed`, `load`, `reconcile_stale_current_session`. Atomic file writes via `tempfile::NamedTempFile` + `persist()`.

- **context.rs** – `RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE`, `RuntimeContextPaths` (validated path set), `encode_runtime_context()`, `decode_runtime_context()`, `decode_runtime_context_from_env()`, `DecodedRuntimeContext`.

- **mod.rs** – Public re-exports of all session API.

### lexicon-framework/src/session/

- **error.rs** – `SessionCoordinationError`.

- **selection.rs** – `CurrentSessionStatus` enum, `assess_current_session()`, `validate_run_selection()`, `validate_resume_selection()`.

- **coordinator.rs** – `SessionCoordinator`, `PreparedSessionLaunch`, `create_prepared_launch()`, `prepare_run()`, `prepare_resume()`, `abandon_current_failure()`, `reconcile_stale_current_session()`.

- **mod.rs** – Re-exports.

## Files modified

- **lexicon-core/Cargo.toml** – Added `libc = "0.2"` and `tempfile = "3"` dependencies.

- **lexicon-core/src/lib.rs** – Added `pub mod session`. Refactored `HttpAcquisitionContext` to private fields with `from_context_paths()` constructor, `from_env_legacy()` quarantine method, and field accessors (`source_directory()`, `raw_data_directory()`).

- **lexicon-core/src/processing/context.rs** – `ProcessingContext` now has private fields, a `from_context_paths()` constructor, field accessors, and a `new_for_tests()` helper.

- **lexicon-core/src/protocols/http/runner.rs** – Added `Session` and `TerminalPersistence` error variants to `HttpRuntimeInvocationExecutionError`. `run_http_runtime_invocation` no longer accepts a `context` parameter; context is derived internally from `LEXICON_RUNTIME_CONTEXT_V1`. Full 10-step session lifecycle embedded in the runner.

- **lexicon-core/src/processing/runner.rs** – Same pattern as the HTTP runner: added error variants, removed `context` parameter, embedded session lifecycle.

- **lexicon-core/src/runtime/identity.rs** – Added `into_owned_identity()` to `RuntimeIdentity`.

- **lexicon-framework/src/lib.rs** – Added `pub mod session`. Updated HTTP managed runner template: removed `HttpAcquisitionContext` import and context-construction block; updated `run_http_runtime_invocation` call to remove `&mut context`. Updated processing runner template: removed `ProcessingContext` import and `ProcessingContext::default()` line; updated `run_processing_runtime_invocation` call to remove `&mut context`.

## State transition table

| From state | Operation | To state |
|---|---|---|
| (none) | create_prepared | Prepared |
| Prepared | promote_to_running | Running |
| Running | complete_succeeded | Succeeded |
| Running | complete_failed | Failed |
| Any terminal | (none) | terminal (immutable) |

Invalid transitions return `SessionTransitionError::InvalidTransition`.

## Session identity format

`{timestamp_ms_hex:016x}-{pid:08x}-{counter:016x}`

All components are lowercase hexadecimal, separated by hyphens. The counter is a process-global atomic u64 incremented per session created in the current process. The format passes `validate_safe_component` (hex digits and hyphens only).

## Runtime context transport

The framework encodes a `RuntimeContextPaths` value as a JSON object and places it in `LEXICON_RUNTIME_CONTEXT_V1`. Runners decode this variable at startup to locate the session store, lock directory, and status directory. The variable is validated on both encode (at path construction time) and decode (on runner startup).

## Design decisions

- `serde(deny_unknown_fields)` on all JSON documents to reject forward-incompatible records.
- Atomic writes use same-directory `NamedTempFile` + `persist()` to avoid cross-device rename failures.
- OS file locking (`flock` / `LockFileEx`) chosen over advisory lock files for automatic release on process death.
- `from_env_legacy()` quarantines the old `LEXICON_SOURCE_DIRECTORY` acquisition path so existing callers compile without change.
- `RunningSession::from_parts` is `pub(crate)` to allow runners to reassemble state from a loaded record and a freshly acquired lease without exposing internal construction.
