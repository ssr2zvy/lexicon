Current implementation milestone: background execution, phase 2 — test coverage for the operator-host handoff

Objective

Add the test coverage contract.md and specs.md require for the background-execution path implemented in the prior milestone, and make the handoff timing injectable so that coverage runs fast and deterministically. No behavior change to the handoff protocol itself.

Contract authority

Follow:

workspace/specs/contract.md section 17 ("Testing requirements"), which explicitly requires tests for, among others, "background operator-host handoff," "parent/child identity disagreement," and "stale-session reconciliation."

workspace/specs/specs.md section 18 ("Testing requirements"), which repeats the same "background operator-host handoff" requirement and adds "confirmation that the Lexicon installer payload contains no separate framework executable" (out of scope here; belongs to a bundling-focused milestone).

Repository-grounded current state

1. Zero tests exist for the background-execution code added in the prior milestone

`lexicon-framework/src/data/background.rs`, the `SessionCoordinator::resume_prepared_launch` / `PreparedSessionLaunch::release_for_handoff` methods in `lexicon-framework/src/session/coordinator.rs`, and the CLI wiring in `lexicon-cli/src/cli/mod.rs` / `lexicon-cli/src/cli/operator_host.rs` have no `#[cfg(test)]` coverage at all. The only test coverage touching this milestone's work is the JSON round-trip coverage already in `lexicon-framework/src/supervision/mod.rs`.

More broadly, `lexicon-framework/src/data/` and `lexicon-framework/src/session/` contain no `#[cfg(test)]` modules anywhere (confirmed by search), so this milestone is also the first test coverage for `select_and_prepare_session`, `spawn_and_supervise`, and the coordinator's session-lifecycle methods in general — not only the newly added background pieces.

2. The ownership-handoff wait loop is not test-friendly as written

`execute_background_data_with_re_executor` in `lexicon-framework/src/data/background.rs` uses fixed constants:

```rust
const OWNERSHIP_HANDOFF_TIMEOUT: Duration = Duration::from_secs(10);
const OWNERSHIP_POLL_INTERVAL: Duration = Duration::from_millis(20);
```

A test exercising the timeout path (operator host never acquires the lease) would otherwise have to block for the full 10 seconds. There is no seam to shorten this for tests.

3. Existing testable seams to reuse, not duplicate

* `ForegroundRuntimeLauncher` (`lexicon-framework/src/data/foreground.rs`) already exists precisely so spawning the actual runtime child can be faked in tests without launching a real process.
* `OperatorHostReExecutor` (`lexicon-framework/src/data/background.rs`) already exists for the same reason on the re-exec side; its production impl (`ProcessOperatorHostReExecutor`) re-executes `std::env::current_exe()`, but a test fake can spawn any short-lived process (or none at all) and simulate lease acquisition independently.
* `lexicon_core::session::{SessionLease, inspect_session_lease}` are real cross-platform OS-level locks; tests exercising lease handoff should use real temporary session stores (via `tempfile`, already a dev-dependency of `lexicon-framework`) rather than mocking the lease mechanism itself, so the tests exercise the actual `flock`/`LockFileEx` behavior contract.md relies on.

Required test coverage

Add `#[cfg(test)]` modules covering at least:

1. `lexicon-framework/src/session/coordinator.rs`:
   * `resume_prepared_launch` succeeds for a session in the `Prepared` state and returns a `PreparedSessionLaunch` whose `record()` matches the original.
   * `resume_prepared_launch` returns `SessionCoordinationError::HandoffSessionNotPrepared` when the session is `Running`, `Succeeded`, `Failed`, or `Abandoned`.
   * `resume_prepared_launch` acquires the lease: after a successful call, `inspect_session_lease` on the same path reports `Owned` until the returned `PreparedSessionLaunch` is dropped.
   * `release_for_handoff` releases the lease (`inspect_session_lease` reports `Available` immediately afterward) while the durable record remains `Prepared` (a fresh `store.load` still returns `Prepared`).
   * A `prepare_run` immediately after `release_for_handoff` (simulating an unrelated concurrent invocation racing the handoff) demonstrates the documented race-window limitation from the prior milestone's report: assert what actually happens (today, the session is reconciled to `Failed` via stale-ownership reconciliation) so the limitation is pinned by a test rather than only described in prose. Do not "fix" this in this milestone; the assertion documents current behavior.

2. `lexicon-framework/src/data/session.rs`:
   * `select_and_prepare_session` records `RuntimeSupervisionMode::Background` on the resulting `PreparedSessionLaunch` when called with `RuntimeSupervisionMode::Background`, and `Foreground` when called with `Foreground` (inspect via `prepared.record().supervision_mode()`).
   * Existing acquisition/processing selection-policy behavior (run, resume, abandon-then-run, live-session rejection) continues to pass with the new parameter threaded through — extend rather than replace the scenarios described by the removed defect list in this milestone's predecessor.

3. `lexicon-framework/src/data/background.rs`:
   * Make the handoff timing injectable: add a way to construct the background-execution call with a configurable timeout and poll interval (for example, a `pub(crate)` variant of `execute_background_data_with_re_executor` that accepts `Duration` parameters, with the public `execute_background_data` continuing to use the fixed production constants). Do not change the default production timing.
   * Using a fake `OperatorHostReExecutor` and the injectable timing: a successful handoff, where the fake spawns a short-lived helper process (or simulates one) and a background helper acquires the session lease within the shortened poll window, returns `BackgroundHandoffOutcome` with the correct project/source/operation/session.
   * A fake re-executor whose spawned process exits immediately without acquiring the lease yields `ForegroundDataExecutionError::OperatorHostExitedBeforeOwnership`.
   * A fake re-executor whose spawned process never acquires the lease and never exits yields `ForegroundDataExecutionError::OperatorHostOwnershipTimeout` once the shortened timeout elapses.
   * A fake re-executor that fails to spawn at all yields `ForegroundDataExecutionError::OperatorHostReExec`.
   * `execute_operator_host_with_launcher` successfully resumes a session prepared by `execute_background_data_with_re_executor` (or an equivalent direct `resume_prepared_launch` setup) and, using a fake `ForegroundRuntimeLauncher`, reaches a terminal outcome through the shared `spawn_and_supervise` pipeline with `RuntimeSupervisionMode::Background` recorded in the invocation envelope.
   * `execute_operator_host_with_launcher` returns a typed error (via `SessionPreparation(SessionCoordinationError::HandoffSessionNotPrepared)`) when given a reference to a session that is not `Prepared` (for example, one already resumed and completed by a prior call) — this is the "parent/child identity disagreement" / stale-handoff family of coverage contract.md section 17 calls for, applied to the new resume path.

4. `lexicon-framework/src/supervision/mod.rs`:
   * Keep the existing round-trip, unknown-schema-version, and unknown-operation tests unchanged.
   * Add a test that `from_json` rejects a document with an unrecognized extra field (exercising `#[serde(deny_unknown_fields)]`), and a test that `from_json` rejects a syntactically invalid JSON string with `OperatorHostInvocationDecodingError::JsonSyntax`.

5. `lexicon-cli`:
   * A parsing test confirming `lexicon __operator-host <ref> -- <args>` parses into `RootCommand::OperatorHost` with `reference` and `passthrough` populated correctly, mirroring the existing `DataCommand` passthrough tests in `lexicon-cli/src/cli/data.rs`.
   * A test confirming `__operator-host` does not appear in the rendered `--help` output (asserting on `Cli::command()`'s generated help text), demonstrating the `hide = true` attribute has the intended effect.

Required corrections

* Introduce the injectable timeout/poll-interval seam described above without changing `execute_background_data`'s externally observable default timing (10 seconds / 20 milliseconds).
* Any new test helper for constructing a temporary `SessionStore` / `SessionCoordinator` against a `tempfile::TempDir` should be written once and shared across the new test modules in `session/coordinator.rs`, `data/session.rs`, and `data/background.rs` rather than copy-pasted three times; a `#[cfg(test)]`-only helper module under `lexicon-framework/src/session/` or `lexicon-framework/src/data/` is acceptable.
* Tests must not depend on real network access, real Cargo builds, or a real admitted runtime bundle on disk; use fakes for `ForegroundRuntimeLauncher` and `OperatorHostReExecutor` exactly as the existing seams intend, and construct `AdmittedBundle`/`RuntimeProjectLayout` values the same way any future test of `execute_foreground_data_with_launcher` would need to (if no existing helper exists for this, adding a minimal one is in scope, but keep it as narrow as the tests in this milestone actually require).
* Do not weaken or delete any existing test.

Preserve existing behavior

Do not change:

* the background-execution handoff protocol, lease hand-off sequence, or `OperatorHostInvocationV1` schema from the prior milestone;
* the default production timeout/poll-interval values;
* the source acquisition or processing handler signatures;
* invocation-envelope JSON schema, session schema, or argv transport;
* foreground behavior when `--bg` is absent;
* processing correctness/durability behavior;
* HTTP transport, transaction recording, or redaction behavior;
* managed runner entrypoints, source build, runtime verification, bundle staging, or publication;
* CLI syntax for existing subcommands.

Explicit exclusions

Do not implement in this milestone:

* a fix for the documented handoff race-window limitation (pin it with a test instead; fixing it is a separate future milestone);
* cancellation, signal forwarding, or true OS-level daemonization/detachment;
* lexicon build, automatic build-before-run, or MZA/installer changes;
* new HTTP capabilities, client certificates, or protocol changes;
* changes to acquisition/processing correctness, durability, or error-preservation behavior;
* an exhaustive test suite for every scenario listed in contract.md section 17 / specs.md section 18 — only the background-execution-relevant scenarios enumerated above are in scope for this milestone. Coverage for the remaining listed scenarios (compressed-response preservation, checkpoint/resume behavior, runtime hash mismatch, publication rollback, MZA target coverage, etc.) is deferred to later milestones targeting those specific areas.

Command-execution constraint

This is a source-only milestone.

Do not run:

cargo test
cargo check
cargo build
cargo fmt
cargo clippy
cargo metadata
rustc

Do not execute:

* lexicon CLI commands, including the `__operator-host` entrypoint;
* generated runners;
* processing or acquisition runtimes;
* SQLite tools;
* HTTP servers;
* real or test HTTP requests;
* workspace validation;
* bundle/install automation.

Do not attempt a CLI command merely to confirm whether it is installed.

Full validation, including running the new tests, remains deferred to the final project-wide validation milestone. Any future Cargo invocation must go through the `lexicon-local-test` container per `instructions.md`, and per the standing rule for this workflow, the agent does not run compile/test/build commands itself; the user runs them.

Completion report

After completion, replace current.md with a report containing:

* files changed, including every new or extended `#[cfg(test)]` module;
* the injectable timeout/poll-interval seam added to `background.rs` and confirmation the public default is unchanged;
* a summary of what each new test in `session/coordinator.rs`, `data/session.rs`, `data/background.rs`, `supervision/mod.rs`, and `lexicon-cli` actually asserts;
* explicit confirmation that the race-window test pins current (not fixed) behavior, with the exact assertion described;
* confirmation that no existing test was weakened, removed, or changed in a way that alters its assertions;
* confirmation that the excluded items were not added;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, runtime execution, or workspace/bundle/install automation were run by the agent.

Then stop.

Do not begin further background-execution feature work (cancellation, signal forwarding, daemonization, or the race-window fix) until this test-coverage milestone is complete and the user has had an opportunity to run the containerized validation against the accumulated, still-unverified changes from this and the prior milestone.
