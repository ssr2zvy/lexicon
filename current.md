# Runtime information probe implementation report

Implemented the bounded runtime-information subprocess probe in `lexicon-framework/src/build/runtime_probe.rs` and re-exported the public API from `lexicon-framework/src/build/mod.rs`.

Completed work:
- Added `probe_http_runtime_information` plus the fixed 5 second timeout and 64 KiB stderr limit constants.
- Spawned the target executable directly with the reserved Core probe argument, without invoking Cargo or a shell.
- Started concurrent stdout/stderr drainers with bounded retention, continued draining past the limit, and preserved overflow flags.
- Enforced deterministic error precedence: timeout, wait/cleanup errors, read failures, stream overflow, unsuccessful exit, then admission validation.
- Reaped every successfully spawned child before returning and preserved the exit status and bounded stderr for unsuccessful-exit errors.
- Added a test-only fixture script for valid, malformed, incompatible, delayed, oversized, and failing probe modes.

Validation:
- `cargo test --workspace --quiet` passed.
Do not use a shell in the production implementation.

Required tests

Add tests proving:

1. A valid fixture runtime is executed and admitted.
2. The child receives the exact reserved probe argument.
3. The child receives no additional arguments.
4. The admitted identity matches the expected identity.
5. Required and available capabilities survive the process boundary.
6. A missing executable returns Spawn.
7. Malformed stdout returns Admission.
8. Incompatible identity returns Admission.
9. Missing required capabilities return Admission.
10. A nonzero exit returns UnsuccessfulExit.
11. Nonzero exit is rejected even when stdout is valid.
12. Bounded stderr is retained for a nonzero exit.
13. Successful exit with bounded stderr may still be admitted.
14. A delayed child exceeding the deadline returns Timeout.
15. The timed-out child is terminated.
16. The timed-out child is reaped.
17. Oversized stdout returns StdoutTooLarge.
18. Oversized stderr returns StderrTooLarge.
19. Simultaneous noisy stdout and stderr do not deadlock.
20. Retained stream buffers never exceed their limits.
21. Existing pure admission tests remain unchanged.
22. Existing Core probe tests remain unchanged.
23. All workspace tests pass.

Use the internal timeout helper for short timeout tests. Do not make the normal test suite wait five seconds for each timeout case.

Preserve existing behavior

Do not change:

* source scaffolding;
* source implementation crates;
* source create;
* source build;
* Cargo build invocation;
* artifact selection;
* runtime publication;
* CLI behavior;
* Core probe behavior;
* probe JSON schema;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-framework remains library-only.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-framework --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If the known MZA dependency remains unavailable, report it separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* source build integration;
* Cargo metadata changes;
* Cargo artifact-selection changes;
* executable hashing;
* runtime.json;
* staged runtime bundles;
* publication changes;
* managed-runner generation;
* runner main.rs;
* runner::run;
* source workspace migration;
* invocation envelopes;
* acquisition execution;
* resume execution;
* child runtime admission;
* HTTP transport;
* raw recording;
* sessions;
* foreground supervision;
* background supervision;
* __operator-host;
* processing runtime probing.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the public probe-execution API;
* timeout and stream limits;
* exact child invocation;
* concurrent bounded-capture behavior;
* termination and reaping behavior;
* typed execution errors;
* error precedence;
* test-fixture arrangement;
* successful probe result;
* nonzero-exit behavior;
* timeout behavior;
* stdout and stderr overflow behavior;
* confirmation that no Cargo or source build integration was added;
* framework and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not connect the executor to source build or generate managed runners.