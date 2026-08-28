Implementation report: background execution, phase 2 — test coverage for the operator-host handoff

Milestone status

Complete, with one scope adjustment: this milestone required constructing a real, on-disk, correctly-hashed `AdmittedBundle` fixture (there is no lightweight/mock constructor by design — admission always goes through the same file-based validation path production code uses). Building that fixture correctly required deep, previously-unresearched knowledge of three additional JSON schemas (`RuntimeManifestV1`, `RuntimeInformationV1`, and the project/layout directory contract). This is now built and documented below.

Files changed

* `lexicon-framework/src/data/test_support.rs` (new) — shared `#[cfg(test)]` fixture: fake on-disk project, real admissible HTTP bundle, `SessionCoordinator`/`SessionStore` construction, and cwd-mutex-guarded test helpers.
* `lexicon-framework/src/data/mod.rs` — registered `test_support` as `pub(crate)` (crate-wide test visibility) under `#[cfg(test)]`.
* `lexicon-framework/src/session/coordinator.rs` — added tests for `resume_prepared_launch`, `release_for_handoff`, and the documented race-window limitation.
* `lexicon-framework/src/data/session.rs` — added tests for supervision-mode threading through `select_and_prepare_session`, plus a live-session-rejection regression test.
* `lexicon-framework/src/data/background.rs` — added the injectable ownership-handoff timing seam and tests for successful handoff, exited-before-ownership, timeout, spawn failure, and operator-host resume rejection.
* `lexicon-framework/src/supervision/mod.rs` — added rejection tests for unknown fields, invalid JSON syntax, and an invalid session identity.
* `lexicon-cli/src/cli/data.rs` — added `__operator-host` parsing and hidden-from-help tests.

Injectable timeout/poll-interval seam

```rust
pub(crate) fn execute_background_data_with_re_executor_and_timing(
    request: ForegroundDataRequest,
    re_executor: &dyn OperatorHostReExecutor,
    ownership_handoff_timeout: Duration,
    ownership_poll_interval: Duration,
) -> Result<BackgroundHandoffOutcome, ForegroundDataExecutionError>
```

`execute_background_data_with_re_executor` (used by `execute_background_data`, in turn used by production `--bg` handling) now simply forwards to this with the unchanged fixed constants `OWNERSHIP_HANDOFF_TIMEOUT = 10s` and `OWNERSHIP_POLL_INTERVAL = 20ms`. Neither public-facing function's signature or default behavior changed; only tests call the `_and_timing` variant directly, using a 300ms timeout / 10ms poll interval.

Test summary by module

`session/coordinator.rs`:
* `resume_prepared_launch_succeeds_for_prepared_session` — resuming an unowned `Prepared` session returns a launch whose record equals the original.
* `resume_prepared_launch_acquires_the_lease` — the lease reports `Owned` after resume and `Available` again after the resumed launch drops.
* `resume_prepared_launch_rejects_non_prepared_session` — resuming a session already advanced to `Abandoned` returns `HandoffSessionNotPrepared`.
* `release_for_handoff_releases_lease_and_preserves_prepared_state` — releasing drops the lease to `Available` while a fresh `store.load` still reports `Prepared`.
* `concurrent_prepare_run_during_handoff_window_reconciles_prepared_session_to_failed` — pins the documented race-window limitation: an unrelated `prepare_run` call issued after `release_for_handoff` but before resume observes the unowned `Prepared` record as stale and reconciles it to `Failed`. The test asserts this is what happens today; it is not corrected here.

`data/session.rs`:
* `records_background_supervision_mode_when_requested` / `records_foreground_supervision_mode_when_requested` — `select_and_prepare_session`'s `supervision` parameter is faithfully recorded on the resulting session record for both modes.
* `processing_operation_records_requested_supervision_mode` — the processing branch (which never inspects `admitted_bundle`) still threads supervision correctly.
* `rejects_selection_when_a_live_session_is_already_active` — a second selection attempt while the first launch's lease is still held is rejected with `SessionSelection(LiveSessionAlreadyActive)`, preserving this pre-existing scenario now that the function takes an added parameter.

`data/background.rs`:
* `successful_handoff_returns_outcome_once_lease_is_owned` — a fake re-executor resumes the just-prepared session (simulating the operator host) and holds the resumed launch for the test's duration; the real function's polling loop observes `Owned` and returns `BackgroundHandoffOutcome` with the correct source/operation.
* `operator_host_exiting_before_ownership_is_a_typed_error` — a fake re-executor whose process exits immediately, before acquiring the lease, yields `OperatorHostExitedBeforeOwnership`.
* `ownership_timeout_is_a_typed_error` — a fake re-executor whose process never acquires the lease and never exits yields `OperatorHostOwnershipTimeout` once the shortened timeout elapses.
* `re_exec_spawn_failure_is_a_typed_error` — a fake re-executor that fails to spawn anything yields `OperatorHostReExec`.
* `operator_host_rejects_a_session_that_is_no_longer_prepared` — `execute_operator_host_with_launcher` given a reference to a session already resumed-and-failed returns `SessionPreparation(HandoffSessionNotPrepared)`.

`supervision/mod.rs`: added `rejects_unknown_field` (deny_unknown_fields), `rejects_syntactically_invalid_json` (JsonSyntax), and `rejects_invalid_session_identity` (empty session id), alongside the three pre-existing round-trip/schema-version/operation tests, all left unchanged.

`lexicon-cli`: added `parses_operator_host_command_with_reference_and_passthrough` and `operator_host_command_is_hidden_from_help_output` (asserts the rendered `--help` text does not contain `__operator-host`), alongside the two pre-existing `DataCommand` tests, left unchanged.

Fixture design (`test_support.rs`)

Building a real `AdmittedBundle` requires: a `lexicon.toml`, the full `sources/<name>/http/{data/raw,data/processed,get-raw-data,process-data}` directory tree, and inside `get-raw-data/runtime/` a real executable file plus a `runtime.json` whose `artifact.size`/`artifact.sha256` match that file exactly (computed via the same `hash_runtime_executable` production admission uses) and whose nested `runtime_information` satisfies `RuntimeInformationV1`'s compatibility checks (matching source/protocol/operation/contract-version, and `descriptor.contract_version == identity.source_contract_version`, with empty required/available capability lists so the trivial subset check passes). The fixture always builds the HTTP/acquisition flavor; `select_and_prepare_processing` never inspects the admitted bundle, so the same fixture value can stand in for `DataOperation::Processing` test scenarios where only the coordinator's own operation matters.

Because `resolve_project_layout` always reads `std::env::current_dir()` with no override seam, the fixture also provides `with_test_cwd`, guarded by a dedicated `TEST_CWD_LOCK` mutex (mirroring the pre-existing `with_test_cwd` pattern already used in `lexicon-cli/src/cli/mod.rs`'s tests), so tests that need a real layout do not race each other over the process-global working directory.

Race-window test: exact assertion

`concurrent_prepare_run_during_handoff_window_reconciles_prepared_session_to_failed` prepares a session, calls `release_for_handoff`, then calls `prepare_run` again on the same coordinator (simulating an unrelated concurrent invocation). It asserts the new call succeeds with a *different* session id, and that the originally handed-off session's durable state is now `Failed` (via `SessionStore::reconcile_stale_current_session`'s stale-ownership path, triggered because the lease is unowned). This is the current, undesirable-but-real behavior described in the prior milestone's report; the test exists to pin it, not to endorse it.

Existing tests

No existing test was weakened, deleted, or had its assertions changed. All additions are new test functions in new or existing `#[cfg(test)]` modules.

Excluded items confirmed not added

* No fix for the race-window limitation.
* No cancellation, signal forwarding, or true OS-level daemonization/detachment.
* No lexicon build, automatic build-before-run, or MZA/installer changes.
* No new HTTP capabilities, client certificates, or protocol changes.
* No changes to acquisition/processing correctness, durability, or error-preservation behavior.
* No attempt at exhaustive coverage of every scenario in contract.md section 17 / specs.md section 18 beyond the background-execution-relevant items enumerated in the milestone brief.

Command-execution confirmation

No `cargo test`, `cargo check`, `cargo build`, `cargo fmt`, `cargo clippy`, `cargo metadata`, or `rustc` invocation was run by the agent. No lexicon CLI command, generated runner, processing or acquisition runtime, SQLite tool, HTTP server, real or test HTTP request, or workspace/bundle/install automation was executed. The new tests do spawn trivial, short-lived OS helper processes (`sh -c`/`cmd /C` with `exit`/`sleep`/`ping`) as fakes standing in for a real operator-host process, exactly as unit tests for process-supervision code must; no lexicon binary, generated runner, or real runtime was invoked.

Accumulated validation risk — recommendation

This is the third consecutive milestone (processing correctness closure, background execution phase 1, and this phase 2) implemented without a single successful compile check, per the standing rule that the agent does not run build/test/compile commands. This milestone's own fixture work required deep schema knowledge (`RuntimeManifestV1`, `RuntimeInformationV1`) that was not previously exercised or verified, and its tests spawn real OS processes and hold real file locks — a category of code that is unusually easy to get subtly wrong without a compiler and test runner.

Before further feature milestones are attempted, running the accumulated changes through the `lexicon-local-test` container (`cargo check` first, then `cargo test --workspace`) would be a high-value checkpoint, since three layers of untested assumptions are now stacked on top of each other. The next `current.md` reflects this.
