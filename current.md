Implementation report: background execution, phase 1 — operator-host re-execution and durable session handoff

Milestone status

Complete. `--bg` now performs a real background handoff instead of returning `BackgroundModeUnsupported` unconditionally.

Files changed

* `lexicon-framework/src/supervision/mod.rs` (new) — `OperatorHostInvocationV1` versioned internal protocol.
* `lexicon-framework/src/data/background.rs` (new) — initiating-process handoff and operator-host entrypoint.
* `lexicon-framework/src/data/foreground.rs` — extracted the shared `spawn_and_supervise` pipeline; threaded `RuntimeSupervisionMode` through envelope construction; made `PreparedForegroundExecution::new` `pub(crate)`.
* `lexicon-framework/src/data/session.rs` — threaded `supervision: RuntimeSupervisionMode` through `select_and_prepare_session` and its acquisition/processing helpers.
* `lexicon-framework/src/data/error.rs` — new background-execution error variants; repurposed `BackgroundModeUnsupported` as a misuse guard.
* `lexicon-framework/src/data/outcome.rs` — new `BackgroundHandoffOutcome`.
* `lexicon-framework/src/data/mod.rs` — registered `background` module and its exports.
* `lexicon-framework/src/session/coordinator.rs` — `PreparedSessionLaunch::release_for_handoff`; `SessionCoordinator::resume_prepared_launch`.
* `lexicon-framework/src/session/error.rs` — new `SessionCoordinationError::HandoffSessionNotPrepared` variant.
* `lexicon-framework/src/lib.rs` — registered the `supervision` module.
* `lexicon-cli/src/cli/operator_host.rs` (new) — hidden `__operator-host` command definition.
* `lexicon-cli/src/cli/mod.rs` — registered the hidden subcommand; `--bg` now routes to the background path.

`OperatorHostInvocationV1` schema and versioning

```rust
pub struct OperatorHostInvocationV1 {
    source_name: String,
    operation: DataOperation,
    session: SessionIdentity,
}
pub const OPERATOR_HOST_INVOCATION_SCHEMA_VERSION: u32 = 1;
```

This is leaner than the milestone brief's illustrative field list (project name, protocol, abandon-past-failure were also listed there). During implementation I found each of those three is unnecessary and dropped it, rather than carrying dead state:

* **project name** — not needed by the operator host. `resolve_project_layout` does not take a project name; it re-discovers the project from the process's working directory, exactly as the initiating process did. The re-exec explicitly sets the operator host's working directory to the already-resolved `layout.project_root()`, so re-discovery is deterministic.
* **protocol** — the codebase has exactly one protocol today (`resolve_project_layout` hardcodes `"http"`); carrying a field with only one possible value would be dead weight. Left as a documented limitation below.
* **abandon_past_failure** — already consumed. The initiating process applies the abandon-then-run policy once, during its own `select_and_prepare_session` call, before the reference is ever built. The operator host only resumes the resulting `Prepared` session; it never re-runs selection policy.

What remains (`source_name`, `operation`, `session`) is exactly what the operator host cannot otherwise obtain: which source, which operation, and which already-prepared session to resume. The schema version constant is distinct from `RUNTIME_INVOCATION_PROTOCOL_VERSION`, the session schema version, and the source contract version, per the required distinct-versioning discipline. Decoding rejects unknown schema versions, unknown operation identifiers, invalid session identities, and unknown JSON fields (`#[serde(deny_unknown_fields)]`), matching the decoding strictness of `RuntimeInvocationEnvelopeV1::from_json`.

Raw source arguments are never persisted

`OperatorHostInvocationV1` has no field for source arguments. In `execute_background_data`, the source arguments are appended directly to the operator-host process's own argv after a `--` separator:

```rust
let mut operator_host_arguments: Vec<OsString> =
    vec![OsString::from("__operator-host"), OsString::from(encoded_reference), OsString::from("--")];
operator_host_arguments.extend(request.source_arguments.iter().cloned());
```

They are read back by the operator-host entrypoint from its own `passthrough` argv (`OperatorHostCommand.passthrough`) and forwarded to `execute_operator_host` unchanged — never round-tripped through the encoded reference, a file, or any other durable store.

Supervision-mode threading through session selection

`select_and_prepare_session`, `select_and_prepare_acquisition`, and `select_and_prepare_processing` in `lexicon-framework/src/data/session.rs` now take a `supervision: RuntimeSupervisionMode` parameter, forwarded to every `coordinator.prepare_run(...)` / `prepare_resume(...)` call site (previously hardcoded to `RuntimeSupervisionMode::Foreground`). `execute_foreground_data_with_launcher` passes `Foreground`; `execute_background_data` passes `Background`. `build_invocation_envelope` in `foreground.rs` also now takes and forwards this parameter instead of hardcoding `Foreground`, so the runtime invocation envelope built by the operator host correctly carries `Background`.

Lease hand-off protocol

1. The initiating process calls `select_and_prepare_session(..., RuntimeSupervisionMode::Background)`, which creates a `Prepared` session record and acquires the lease, exactly like the foreground path.
2. It immediately calls the new `PreparedSessionLaunch::release_for_handoff(self) -> SessionRecordV1`, which drops the held `SessionLease` (releasing the OS-level advisory lock) without transitioning the session away from `Prepared`.
3. It re-executes the operator host (see below) and polls `coordinator.store().inspect_lease_state(&session_id)` in a bounded loop (10-second timeout, 20ms interval), also checking `Child::try_wait()` each iteration so a prematurely-exited operator host is detected immediately rather than only after a timeout.
4. The operator host calls the new `SessionCoordinator::resume_prepared_launch(&self, session_id)`, which loads the existing record, requires it still be exactly `Prepared` (returning `SessionCoordinationError::HandoffSessionNotPrepared` otherwise), and acquires the lease for itself — winning the race deterministically in the ordinary case, since it is the very next process to attempt acquisition after release.
5. Once the operator host holds the lease, `inspect_lease_state` observes `Owned` and the initiating process returns `BackgroundHandoffOutcome`.

`resume_prepared_launch` deliberately does not go through `assess_current_session` / `reconcile_stale_current_session`: an unowned `Prepared` record during handoff is the expected valid state, not evidence of a dead owner. Reusing the stale-reconciliation path here would have raced against the handoff itself and incorrectly failed it.

Known limitation (documented, not fixed in this phase): there is a narrow window between step 2 and step 4 during which an unrelated concurrent `lexicon data` invocation against the same source/operation could call `assess_current_session`, observe the same unowned `Prepared` record, and reconcile it to `Failed` as stale ownership, pre-empting the handoff. This requires a second process racing during a normally millisecond-scale window and is out of scope for phase 1 (see Explicit exclusions in the prior milestone brief regarding concurrency hardening).

Re-execution argv construction

```rust
lexicon __operator-host <encoded-reference> -- <source-args...>
```

`ProcessOperatorHostReExecutor::spawn_operator_host` re-executes `std::env::current_exe()` with this argv and sets the child's working directory to the already-resolved `layout.project_root()`, so the operator host's own `resolve_project_layout` call deterministically re-discovers the same project regardless of the shell's cwd at invocation time.

Shared spawn-and-supervise pipeline

`lexicon-framework/src/data/foreground.rs` now exposes `pub(crate) fn spawn_and_supervise(owner, layout, admitted, coordinator, source_arguments, supervision, launcher)`, extracted verbatim from the former tail of `execute_foreground_data_with_launcher` (envelope construction, argv encoding, pre-launch integrity recheck, spawn, and `wait_and_reconcile`), parametrized only by `RuntimeSupervisionMode`. Both `execute_foreground_data_with_launcher` (passing `Foreground`) and `execute_operator_host_with_launcher` (passing `Background`) call it. No reconciliation, wait-recovery, or termination logic was duplicated. `PreparedForegroundExecution::new` was widened from module-private to `pub(crate)` so the operator-host path (a different module) can construct the same owner type from a *resumed* `PreparedSessionLaunch`, rather than the freshly-created one the foreground path produces.

`__operator-host` CLI wiring and internal status

`lexicon-cli/src/cli/operator_host.rs` defines `OperatorHostCommand` with `#[command(name = "__operator-host", hide = true, about = "Reserved internal entrypoint. Do not invoke directly.")]`. It is registered as `RootCommand::OperatorHost(OperatorHostCommand)` without a variant-level `#[command(...)]` override, matching the existing pattern where every other subcommand's naming and visibility come from the wrapped struct's own attribute rather than being redeclared at the enum-variant level. It takes the encoded reference as a positional argument and forwards everything after `--` as `passthrough: Vec<OsString>`, mirroring `DataCommand`'s existing passthrough handling.

`--bg`-absent behavior is unchanged

`execute_foreground_data_with_launcher`'s steps 1–5 (project discovery, bundle admission, project identity, coordinator, session selection) are untouched apart from the added `RuntimeSupervisionMode::Foreground` argument at the two call sites that previously hardcoded it internally. Its defensive `if request.background { return Err(BackgroundModeUnsupported) }` guard is untouched. `lexicon-cli`'s dispatch now branches on `command.bg` before constructing the call, but the non-`--bg` branch calls `execute_foreground_data` exactly as before with an identical request and identical success/error rendering.

Excluded items confirmed not added

* No cancellation, stop, or attach/status command was added.
* No signal forwarding (operator host to child, or external process to operator host) was added.
* No true OS-level daemonization/detachment (`setsid`, new process group, `DETACHED_PROCESS`, job objects) was added; the operator host is spawned as an ordinary child via `std::process::Command`. The initiating process does not `wait()` on it and does not gate its own return on the operator host's continued execution past the ownership handshake.
* No lexicon build, automatic build-before-run, MZA, or installer change was made.
* No new HTTP capability, client certificate, or protocol change was made.
* No change to the already-closed processing correctness/durability/error-preservation behavior.
* No fixed source schema, ORM behavior, or decoded response reader was added.

Command-execution confirmation

No `cargo test`, `cargo check`, `cargo build`, `cargo fmt`, `cargo clippy`, `cargo metadata`, or `rustc` invocation was run. No lexicon CLI command (including the new `__operator-host` entrypoint), generated runner, processing or acquisition runtime, SQLite tool, HTTP server, real or test HTTP request, workspace validation, or bundle/install automation was executed. Full validation, including a first-time compile of this milestone's changes, remains deferred to the final project-wide validation milestone; any future Cargo invocation must go through the `lexicon-local-test` container per `instructions.md`.

Since this milestone could not be compiled during implementation, every cross-module call site's visibility and signature was manually traced against its declaration before use (documented inline in the new/changed source), and one real visibility bug found this way — `PreparedForegroundExecution::new` being module-private while needed from the new `background` module — was corrected by widening it to `pub(crate)`. The next iteration's foreground/background execution attempt (or the eventual project-wide validation milestone) should treat a first successful `cargo check` of this milestone's code as a meaningful open item, not an assumed given.

Next step

Background execution phase 1 (handoff mechanics) is complete. True daemonization/detachment, signal forwarding, and explicit cancellation remain open for a future milestone, as does closing the documented handoff race-window limitation if concurrent invocations become a real concern.
