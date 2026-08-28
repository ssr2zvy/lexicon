Current implementation milestone: background execution, phase 1 — operator-host re-execution and durable session handoff

Objective

Replace the current hard-stubbed `--bg` failure with a real background execution path: the initiating `lexicon` process re-executes itself in a reserved internal `__operator-host` role, hands off durable session ownership to that process, and exits once ownership is confirmed. The operator host then performs the same spawn-and-supervise pipeline the foreground path already implements.

This is a corrective/additive milestone, not a rewrite. It must not change acquisition, processing, or the runtime invocation contract for managed source runtimes.

Contract authority

Follow:

workspace/specs/contract.md, sections 1 ("Goals"), 3 ("Command routing and process model" — Background execution), 9 ("Source-specific arguments"), 12 ("Sessions and supervision"), 16 ("Versioning"), 18 ("Explicit non-goals").

workspace/specs/specs.md, sections 8.2 ("Background execution"), 13 ("Sessions"), 16 ("Versioning"), 20 non-negotiable invariants 8–9.

The governing division restated by both documents: Lexicon controls the supported entrypoint, build, runtime admission, HTTP-and-recording effect, and session supervision. This milestone extends session supervision to the background case; it does not change source-facing contracts.

Repository-grounded current state

1. `--bg` is parsed and threaded but hard-stubbed as unsupported

`lexicon-cli/src/cli/data.rs` defines `DataCommand.bg: bool`, and `lexicon-cli/src/cli/mod.rs` copies it into `lexicon_framework::data::ForegroundDataRequest.background`. `lexicon-framework/src/data/foreground.rs::execute_foreground_data_with_launcher` begins with:

```rust
if request.background {
    return Err(ForegroundDataExecutionError::BackgroundModeUnsupported);
}
```

Background execution is not attempted in any form today; every `--bg` invocation fails immediately.

2. Supervision-mode plumbing exists but is never set to `Background`

`lexicon_core::runtime::RuntimeSupervisionMode` already has `Foreground` and `Background` variants, and `lexicon-core/src/session/binding.rs::bind_runtime_session` already validates supervision-mode agreement between the invocation envelope and the durable session record (`RuntimeSessionBindingError::SupervisionModeMismatch`). `lexicon_framework::session::SessionCoordinator::prepare_run` / `prepare_resume` already accept a `supervision: RuntimeSupervisionMode` parameter. However, the only caller — `select_and_prepare_session` in `lexicon-framework/src/data/session.rs` — always passes `RuntimeSupervisionMode::Foreground`, regardless of `request.background`. The runtime-level supervision-mode contract is real but currently unreachable for `Background`.

3. No operator-host process, module, or entrypoint exists

`lexicon-cli/src/cli/mod.rs` recognizes only `Data`, `Source`, `Init`, `Build` subcommands. There is no `__operator-host` entrypoint, no `frontend.rs`/`operator_host.rs` split as sketched in contract.md's package-boundaries example, and no `lexicon-framework::supervision` module (`lexicon-framework/src` contains only `build/`, `data/`, `publication/`, `session/`). Background supervision has zero implementation surface today, not a partial or buggy one.

4. Reusable primitives already exist and must not be duplicated

* `lexicon_core::session::{SessionLease, inspect_session_lease, SessionLeaseState}` (`lexicon-core/src/session/lease.rs`) is already a cross-platform exclusive advisory lock (`flock` on Unix, `LockFileEx` on Windows) with a non-consuming inspection function. The operator host must reuse this exact primitive for lease handoff; it must not introduce a second locking mechanism.
* `lexicon_framework::session::{SessionCoordinator, PreparedSessionLaunch}` (`lexicon-framework/src/session/coordinator.rs`) already prepares sessions, retains the lease, and exposes `record()`, `session()`, `context_document()`, `operation_root()`, and `fail_launch(...)`.
* `lexicon_framework::data::foreground::{PreparedForegroundExecution, RunningForegroundExecution, execute_foreground_data_with_launcher}` (`lexicon-framework/src/data/foreground.rs`) already implements project discovery, bundle admission, coordinator construction, session selection, invocation-envelope construction (`build_invocation_envelope`), argv encoding (`encode_runtime_invocation`), pre-launch executable integrity recheck, spawning through the `ForegroundRuntimeLauncher` seam, and `wait_and_reconcile` termination handling (including the `handle_wait_error` / `ownership_uncertain` recovery paths).
* Raw source arguments are already deliberately not persisted to durable storage anywhere in the codebase (contract.md section 9); this invariant must be preserved by the new operator-host handoff.

Required implementation

1. Operator-host invocation reference (new, versioned, internal protocol)

Introduce a small versioned type, `OperatorHostInvocationV1` (or equivalently named), in `lexicon-framework` (this is a framework/CLI-level internal protocol, not a source-facing Core contract, so it does not belong in `lexicon-core::runtime`). It must carry exactly what is needed to relocate and rebuild the prepared session deterministically:

* project name;
* source name;
* protocol identifier;
* operation (`Acquisition` | `Processing`);
* `abandon_past_failure`;
* the already-generated session identity (the session the initiating process already prepared).

It must carry its own schema version constant (for example `OPERATOR_HOST_INVOCATION_SCHEMA_VERSION`), separate from `RUNTIME_INVOCATION_PROTOCOL_VERSION`, per the distinct-versioning requirement in contract.md section 16 / specs.md section 16.

Required exclusion: this type must never carry raw source arguments. Source arguments continue to travel only as the operator-host process's own trailing argv after `--`, exactly as `lexicon data --get ... -- <source-args>` already does. Do not persist source arguments to any file as part of this reference.

Provide encode/decode functions for this reference (JSON is acceptable, consistent with existing envelope encoding style) and reject unknown schema versions the same way `RuntimeInvocationEnvelopeV1::from_json` already does for the runtime invocation envelope.

2. Session preparation with `Background` supervision mode

Thread `RuntimeSupervisionMode` through `select_and_prepare_session` (`lexicon-framework/src/data/session.rs`) and its `select_and_prepare_acquisition` / `select_and_prepare_processing` helpers, so the caller controls whether `Foreground` or `Background` is requested, instead of the mode being hardcoded.

3. Initiating-process background path

Add a background counterpart to `execute_foreground_data_with_launcher` (a new function, e.g. `execute_background_data_with_launcher`, or an internal branch reached before the current unconditional rejection) that:

* performs the same project discovery, bundle admission, project-identity construction, and coordinator construction already used by the foreground path;
* calls `coordinator.prepare_run(RuntimeSupervisionMode::Background)` (or `prepare_resume`, following the same selection policy already implemented in `select_and_prepare_acquisition` / `select_and_prepare_processing`);
* releases the lease held by the just-created `PreparedSessionLaunch` before re-execution, so the operator-host process can acquire it itself (the initiating process must not hold the lease across the re-exec boundary);
* builds the `OperatorHostInvocationV1` reference for the now-Prepared session and re-executes the current binary (`std::env::current_exe()`) as `lexicon __operator-host <encoded-reference> -- <source-args>`, forwarding the untouched source arguments exactly as received;
* waits, with a bounded timeout, by polling `inspect_session_lease` on the session's lease path, until the spawned operator-host process has acquired the lease (`SessionLeaseState::Owned`), confirming durable session ownership;
* returns a typed background-handoff outcome distinct from `ForegroundDataOutcome` once ownership is confirmed, without waiting for the operator host or its child runtime to finish;
* returns a typed error if the operator-host process exits, or the timeout elapses, before ownership is observed — never silently reports success in that case.

4. Operator-host entrypoint

Add a reserved internal subcommand to `lexicon-cli` for `__operator-host <encoded-reference> [-- <source-args>]`. It must not be advertised as ordinary CLI surface (hide it from `--help`, or otherwise mark it as internal, consistent with contract.md's statement that this is "an internal protocol, not a public framework API").

The handler decodes `OperatorHostInvocationV1`, re-runs project discovery / bundle admission / coordinator construction for the exact project, source, protocol, and operation named in the reference, then re-`prepare_run`/`prepare_resume`s with `RuntimeSupervisionMode::Background` for the same session identity (this succeeds because the initiating process already released the lease). From there it reuses the existing spawn-and-supervise pipeline (`build_invocation_envelope`, `encode_runtime_invocation`, `recheck_executable_integrity_typed`, the `ForegroundRuntimeLauncher` seam, `wait_and_reconcile`) rather than duplicating it. Prefer generalizing `PreparedForegroundExecution` / `RunningForegroundExecution` (and any misleadingly-named helpers) into shared internal types used by both the foreground and operator-host callers, over forking a second copy of the state machine.

5. CLI wiring

`lexicon data --get/--process ... --bg` in `lexicon-cli/src/cli/mod.rs` must call the new background path instead of unconditionally forwarding into `execute_foreground_data`. Without `--bg`, behavior must remain byte-identical to today.

Required corrections / discipline

* The operator-host invocation reference is a small, versioned, internal protocol distinct from the runtime invocation envelope, the session schema, and the source contract version, per the distinct-compatibility-surfaces requirement in contract.md section 16 and specs.md section 16.
* Do not persist raw source arguments anywhere as part of this milestone's new durable state.
* `bind_runtime_session`'s existing `SupervisionModeMismatch` check must remain meaningful end-to-end: when the operator host launches the managed source runtime, that runtime's own invocation envelope must carry `RuntimeSupervisionMode::Background`, matching the session record the operator host just prepared.
* Do not introduce a second session-lease mechanism; reuse `SessionLease` / `inspect_session_lease` exactly as implemented today.
* Do not change the source acquisition or processing handler signatures, the runtime invocation envelope schema, the managed-runner entrypoints, or the existing foreground observation/reconciliation logic's behavior for `--bg`-absent invocations.

Preserve existing behavior

Do not change:

* processing correctness/durability behavior closed in the prior milestone;
* the source acquisition or processing handler signatures;
* invocation-envelope JSON schema for the managed source runtime;
* argv transport to source implementations;
* source argument preservation and non-persistence;
* acquisition or processing admission;
* runtime-information probes;
* the session schema (only an additive, separate operator-host invocation-reference schema is introduced; the session record schema itself is unchanged);
* `SessionLease` / `inspect_session_lease` locking behavior;
* foreground behavior when `--bg` is absent;
* HTTP transport, retries, redirects, raw transaction formats, raw-byte fidelity, header redaction;
* managed runner entrypoints, source build, runtime verification, bundle staging, paired publication;
* CLI syntax for existing subcommands other than the new internal `__operator-host` entrypoint;
* MZA, Protocol 1, installer behavior.

Explicit exclusions

Do not implement in this milestone:

* explicit cancellation, stop, or attach/status commands for a running background session;
* signal forwarding from the operator host to the child runtime, or from any external process to the operator host;
* true OS-level daemonization or detachment semantics (Unix `setsid`/new session group, Windows `DETACHED_PROCESS`/job objects); the operator host may be spawned as an ordinary child process for this phase, but the initiating process must not `wait()` on it or otherwise gate its own exit on the operator host's continued execution after the ownership handshake completes;
* lexicon build, automatic build-before-run, or MZA/installer changes;
* new HTTP capabilities, client certificates, or protocol changes;
* changes to acquisition/processing correctness, durability, or error-preservation behavior (already closed);
* fixed source schemas, ORM behavior, or decoded response readers.

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

* lexicon CLI commands;
* generated runners;
* the new `__operator-host` entrypoint;
* processing or acquisition runtimes;
* SQLite tools;
* HTTP servers;
* real or test HTTP requests;
* workspace validation;
* bundle/install automation.

Do not attempt a CLI command merely to confirm whether it is installed.

Existing test source may be adjusted only when necessary to align with changed production APIs (for example, `select_and_prepare_session`'s new supervision-mode parameter).

Full validation remains deferred to the final project-wide validation milestone. Any future Cargo invocation must go through the `lexicon-local-test` container per `instructions.md`.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* the `OperatorHostInvocationV1` schema and its versioning;
* confirmation that raw source arguments are never persisted by the new handoff;
* the supervision-mode threading change through `select_and_prepare_session`;
* the lease hand-off protocol (release by the initiating process, acquisition by the operator host, polling/timeout behavior);
* the re-execution argv construction (`__operator-host <reference> -- <source-args>`);
* the new/generalized shared state machine used by both foreground and operator-host callers;
* the `__operator-host` CLI wiring and its internal/hidden status;
* confirmation that `--bg`-absent behavior is unchanged;
* confirmation that the excluded items (cancellation, signal forwarding, true daemonization, build/installer changes) were not added;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, runtime execution, or workspace/bundle/install automation were run.

Then stop.

Do not begin cancellation, signal forwarding, or true daemonization work until this phase-1 handoff closure is complete and reviewed.
