Implementation report

Completed the framework-level HTTP runtime verification flow and kept the change scoped to the runtime-verification layer.

What changed
- Added/extended `lexicon-framework/src/build/runtime_verification.rs` to compose the executable hash + bounded HTTP probe + post-probe hash sequence.
- Exposed the verified runtime API via `lexicon-framework/src/build/mod.rs`.
- Kept `hash_runtime_executable(...)` and `probe_http_runtime_information(...)` as the single source of hashing/probe behavior instead of duplicating their logic.
- Ensured a `VerifiedHttpRuntime` is only produced after the hash/probe/hash sequence succeeds and the pre-probe and post-probe artifact values agree.
- Returned typed verification failures for initial-hash, probe, final-hash, and changed-during-probe cases without creating a verified result on any failure path.

Also fixed a workspace-breaking test issue unrelated to the runtime flow:
- Serialized all current-directory changes in the CLI/framework tests so parallel test execution no longer races on `std::env::set_current_dir(...)` and leaves later tests in a deleted temp directory.

Validation
- Ran: `cargo test --workspace --quiet`
- Result: all workspace tests passed.

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

If the known external MZA dependency remains unavailable, report it separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* integration with source build;
* Cargo build-plan changes;
* Cargo artifact-selection changes;
* runtime.json;
* runtime bundle directories;
* staging;
* publication changes;
* rollback changes;
* managed-runner generation;
* runner main.rs;
* runner::run;
* source workspace migration;
* invocation envelopes;
* acquisition or resume execution;
* HTTP transport;
* raw recording;
* sessions;
* supervision;
* __operator-host;
* processing-runtime verification.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the public verification API;
* the opaque verified-runtime representation;
* exact verification order;
* initial and final hash behavior;
* probe delegation behavior;
* changed-during-probe detection;
* typed error representation;
* deterministic mutation-test arrangement;
* successful verification results;
* each failure result;
* confirmation that no staging or publication occurred;
* framework and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not integrate candidate verification into source build.