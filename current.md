# Foreground Data Execution — Implementation Complete

## Summary

The full foreground data-command execution path has been implemented and the workspace builds cleanly.

## What was implemented

### lexicon-core fixes (pre-existing compilation failures resolved)

- **`lexicon-core/src/session/model.rs`** — `SessionRecordV1` mutable fields (`state`, `revision`, `updated_at`, `started_at`, `finished_at`, `failure`) promoted to `pub(super)` so that sibling module `store.rs` can apply session transitions. The partial-move borrow error in `SessionRecordV1::from_json` was fixed by moving the `validate_record_invariants` call before the partial moves.
- **`lexicon-core/src/session/lease.rs`** — Added the free function `inspect_session_lease` delegating to `SessionLease::inspect_session_lease` to satisfy the existing re-exports in `mod.rs` and `store.rs`.

### lexicon-framework — Session layer changes

- **`lexicon-framework/src/session/error.rs`** — Fixed malformed `Error::source()` match.
- **`lexicon-framework/src/session/coordinator.rs`** — Rewritten to decouple from `RuntimeContextPaths` construction-time identity. `SessionCoordinator::new` now takes `project_root` and `protocol_root` and derives all paths lazily inside `create_prepared_launch`. `PreparedSessionLaunch` extended with `operation_root` field, `session()` getter, and `fail_launch()` method. Added `store()` accessor on `SessionCoordinator`.

### lexicon-framework — Build module changes

- **`lexicon-framework/src/build/runtime_bundle_admission.rs`** — Added `IncompatibleOwned(String)` variant to both `RuntimeBundleAdmissionError` and `ProcessingRuntimeBundleAdmissionError` (with `Display` and `source()` impls). Added `admit_http_runtime_bundle_owned` and `admit_processing_runtime_bundle_owned` functions accepting `&OwnedRuntimeIdentity`.

### lexicon-framework — `lib.rs` changes

- Exported `pub mod data`.
- Made `find_project_root` and `validate_source_name` `pub(crate)`.
- Added `ProjectConfigData` struct and `load_project_config` fn.

### lexicon-framework — New `data/` module

- **`data/request.rs`** — `ForegroundDataRequest`, `DataOperation`.
- **`data/error.rs`** — Complete `ForegroundDataExecutionError` typed hierarchy (~95 variants).
- **`data/outcome.rs`** — `ForegroundDataOutcome`, `ObservedChildTermination`.
- **`data/project.rs`** — `RuntimeProjectLayout`, `resolve_project_layout`.
- **`data/runtime.rs`** — `AdmittedBundle` enum, admission dispatch (`admit_bundle`), integrity check (`recheck_executable_integrity`), resume check (`acquisition_bundle_has_resume`).
- **`data/session.rs`** — `build_coordinator`, `build_project_identity`, `select_and_prepare_session` (full acquisition/processing selection policy), `load_terminal_session`, `persist_abnormal_termination`.
- **`data/foreground.rs`** — `execute_foreground_data` main pipeline: project discovery → bundle admission → project identity → session coordinator → session selection → invocation envelope → argv encoding → integrity recheck → process launch → child wait → termination reconciliation.
- **`data/mod.rs`** — Public re-exports.

### lexicon-cli

- **`lexicon-cli/src/cli/data.rs`** — `passthrough` field changed to `Vec<OsString>`.
- **`lexicon-cli/src/cli/mod.rs`** — `RootCommand::Data` dispatch now calls `lexicon_framework::data::execute_foreground_data`. Constructs `ForegroundDataRequest` from the parsed CLI command and prints a concise result on success or an error string on failure.

## Execution path

```
lexicon data --get <source> [-- <args>]
  → DataCommand parsed by clap
  → execute_foreground_data(ForegroundDataRequest)
    → resolve_project_layout   (find project root, validate source, build layout)
    → admit_bundle             (admit HTTP/processing bundle, identity check)
    → build_project_identity
    → build_coordinator        (SessionCoordinator, SessionStore)
    → select_and_prepare_session
        reconcile_stale_current_session
        → select policy: run / resume / abandon-then-run / error
        → create_prepared_launch (NewSessionRecord, lease, context document)
    → build_invocation_envelope (RuntimeInvocationEnvelopeV1)
    → encode_runtime_invocation → argv
    → recheck_executable_integrity
    → Command::spawn with LEXICON_RUNTIME_CONTEXT_V1 env + cwd=protocol_root
    → child.wait()
    → reconcile_termination
        zero exit → load session record → Succeeded OK / Failed Err / Incomplete Err
        nonzero exit → load session record → Failed Err / Disagreement Err / Abnormal Err
        signal → best-effort persist AbnormalTermination → AbnormalTermination Err
```

## Build status

Workspace builds cleanly with no errors. Remaining warnings are all pre-existing in `lexicon-core` (unused imports in `store.rs`, `context.rs`, `protocols/http/runner.rs`).
