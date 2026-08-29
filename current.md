Current milestone: make embedded Core dependency identity fail closed and verify installed-CLI standalone execution
Objective
Ensure that the embedded Core dependency identity (specs.md §8) fails closed at compile time (rejecting missing, empty, or all-zero revisions in `lexicon-framework/build.rs`), and verify through an installed-CLI black-box test that both generated source workspaces resolve and build against the exact declared Core revision without access to the original checkout, `CARGO_MANIFEST_DIR`, or Git executable.
This milestone is derived from:
contract.md §4 (installed and linked components);
specs.md §8 (embedded Core dependency identity: must be embedded at Lexicon build time, must not inspect CARGO_MANIFEST_DIR, must not run git rev-parse, must not require Git on operator machines);
specs.md §44 (Scaffold and validation: installed scaffold generation without original Git checkout);
the prior milestone's completion report identifying fail-closed build-time resolution as the next candidate.
Repository-grounded starting point
`lexicon-framework/build.rs` currently falls back to `0000000000000000000000000000000000000000` if Git fails or `LEXICON_EMBEDDED_CORE_REV` is unset. This fails open rather than failing closed at build time.
Scaffold generation in `lexicon-framework/src/lib.rs` uses `pub const EMBEDDED_CORE_GIT_REV: &str = env!("LEXICON_EMBEDDED_CORE_REV");` to write `get-raw-data/Cargo.toml` and `process-data/Cargo.toml`.
Required implementation
1. Make build.rs fail closed
In `lexicon-framework/build.rs`:
* If `LEXICON_EMBEDDED_CORE_REV` is set, validate that it is a non-empty, valid hex commit SHA (40 characters) or valid version tag. Reject all-zeros (`00000000...`) or empty strings.
* If unset, run `git rev-parse HEAD`. If Git fails or returns a non-40-hex string, panic during `build.rs` with an actionable compile error instructing how to provide `LEXICON_EMBEDDED_CORE_REV`.
* Under no circumstances emit a dummy fallback or placeholder string.
2. Validate embedded revision format
In `lexicon-framework/src/lib.rs`:
* Add compile-time or startup validation verifying `EMBEDDED_CORE_GIT_REV` is not empty, not all zeros, and is a valid 40-character hex revision.
3. End-to-end installed-CLI standalone test
Add a test (in `lexicon-cli` or `lexicon-framework` integration tests) that:
* Uses the built `lexicon` binary in a clean temporary directory outside the repository;
* Verifies `lexicon init <parent> <project>` succeeds;
* Verifies `lexicon source create <source> --protocol http` succeeds without running Git or inspecting `CARGO_MANIFEST_DIR`;
* Asserts that both generated workspaces (`get-raw-data/Cargo.toml` and `process-data/Cargo.toml`) contain `rev = "<embedded_rev>"`;
* Asserts that `get-raw-data/Cargo.lock` and `process-data/Cargo.lock` were generated and exist.
Scope constraints
Do not implement during this milestone:
* changes to runtime admission or HTTP execution;
* changes to background supervision;
* MZA release packaging changes;
* second-protocol support.
Completion criteria
This milestone is complete only when:
* `lexicon-framework/build.rs` fails closed when no valid Core Git revision can be resolved;
* `EMBEDDED_CORE_GIT_REV` is guaranteed to be a valid 40-character hex revision;
* the installed-CLI standalone test verifies source creation outside the repo;
* `cargo check --workspace` passes;
* `cargo test --workspace --quiet` passes;
* no production contract is weakened.
Completion report
When the milestone passes, replace this file with a concise report and continue the loop.
