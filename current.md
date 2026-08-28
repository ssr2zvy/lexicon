Current milestone: validation checkpoint — compile and test the accumulated unverified work

Objective

Three consecutive milestones have been implemented back-to-back with zero compilation checks:

1. Processing correctness, durability, and error-preservation closure.
2. Background execution phase 1 (operator-host re-execution and session handoff).
3. Background execution phase 2 (test coverage for the handoff).

Per this workflow's standing rule, the agent does not run compile, test, or build commands; the user runs them. This milestone is a mandatory checkpoint: it requires the user to run the containerized build/test cycle and, if needed, direct the agent to fix whatever compile or test failures surface, before any further feature milestone is planned or implemented.

This is not a contract- or spec-derived feature gap. It exists because continuing to layer additional unverified source changes on top of three already-unverified layers would compound risk rather than close it, and because the required corrective action (running Cargo) is one the agent is structurally unable to perform itself in this workflow.

Why this is necessary now

* `lexicon-core`'s processing module was substantially rewritten (new error hierarchy, new discovery/provenance logic, new context invariants, new runner sequencing) without ever compiling.
* `lexicon-framework` gained a new `supervision` module, a new `data::background` module, new `SessionCoordinator` methods, and threaded a new parameter through `select_and_prepare_session` and `build_invocation_envelope` — all unverified.
* This milestone's own test fixture (`lexicon-framework/src/data/test_support.rs`) required hand-constructing JSON matching `RuntimeManifestV1` and `RuntimeInformationV1`'s schemas from source-code inspection alone, with no ability to confirm the fixture actually admits successfully.
* Tests were added that spawn real OS processes and take real file-based locks — exactly the kind of code most likely to contain a subtle, non-obvious bug that only a real test run will reveal.

Required action (user)

Using `instructions.md`'s containerized Cargo workflow:

```bash
podman build -f containerization/test-container/Containerfile -t lexicon-local-test-image .
podman run -d --name lexicon-local-test -v "$PWD":/lexicon --workdir /lexicon lexicon-local-test-image
podman exec lexicon-local-test bash -lc 'cd /lexicon && cargo check --workspace'
podman exec lexicon-local-test bash -lc 'cd /lexicon && cargo test --workspace --quiet'
```

(Substitute `podman start lexicon-local-test` instead of `podman run` if the container already exists.)

Then either:

* if both commands succeed, replace this file with a short `current.md` confirming a clean `cargo check` and `cargo test --workspace` pass, so the next loop iteration can resume deriving feature milestones from the contract and specs with confidence in the baseline; or
* if either command fails, paste (or otherwise make available) the exact failing output, and direct the agent to fix the reported errors as a source-only corrective pass against this exact `current.md`, re-stating the failing output inside it so the agent has the precise compiler/test diagnostics to work from. The agent cannot guess at compiler errors it has not seen.

Required corrections (agent, once failures are known)

If invoked to fix compile or test failures:

* Fix only what the reported diagnostics require. Do not use this pass to add new features, refactor unrelated code, or expand scope.
* Preserve every behavioral guarantee documented in the three prior milestones' completion reports (now folded into this repository's git history) unless a diagnostic proves one of them was never actually achievable as described, in which case document the discrepancy explicitly in the new completion report rather than silently changing the claim.
* Do not weaken, delete, or skip a failing test to make the suite pass; fix the code or, if the test itself is wrong, fix the test's logic while preserving its intent.
* If a test fails due to a genuine environmental limitation of the container (for example, a missing `sh`/`cmd` shell assumed by `lexicon-framework/src/data/background.rs`'s test helpers), adapt the test's process-spawning approach rather than deleting coverage.

Preserve existing behavior

Do not change:

* any public API surface introduced by the prior three milestones, unless a compiler error proves it cannot work as written;
* the background-execution handoff protocol or `OperatorHostInvocationV1` schema;
* the processing runner sequence, error hierarchy, or durability guarantees;
* CLI syntax for any existing subcommand.

Explicit exclusions

Do not implement in this milestone:

* any new feature work (cancellation, signal forwarding, daemonization, further processing corrections, or new contract/spec-derived milestones);
* lexicon build, automatic build-before-run, or MZA/installer changes;
* a fix for the previously documented handoff race-window limitation, unless it is the direct cause of a reported test failure.

Command-execution constraint

This milestone is the explicit exception to the usual "source-only, no Cargo" pattern used by prior milestones, but only for the user. The agent still does not run `cargo`, `rustc`, or any lexicon CLI/runtime command itself, per the standing rule for this workflow. The agent's role in this milestone is limited to:

* waiting for the user to report compile/test results;
* if given failing diagnostics, making the minimal source corrections they require;
* if given a clean pass, writing the completion report and resuming the normal contract/spec-derived loop next iteration.

Completion report

Once `cargo check --workspace` and `cargo test --workspace` both pass (confirmed by the user), replace current.md with a report containing:

* confirmation that both commands were run by the user (not the agent) and passed;
* a list of any source corrections the agent made to reach a passing state, with a one-line rationale per correction tied to the specific diagnostic it fixed;
* confirmation that no new feature scope was added during this corrective pass;
* confirmation that no existing test was weakened or deleted to reach a passing state.

Then stop, and resume deriving the next feature milestone from workspace/specs/contract.md and workspace/specs/specs.md on the following loop iteration.
