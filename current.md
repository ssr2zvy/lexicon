# Implementation report

Implemented the verified HTTP runtime candidate flow in `lexicon-framework/src/build/runtime_verification.rs` and exported the public API via `lexicon-framework/src/build/mod.rs`.

What changed
- Added `HashedRuntimeArtifact` and `RuntimeArtifactHashError` support for regular-file hashing and missing-file handling.
- Added `VerifiedHttpRuntime` with a constrained public API and no unchecked constructor.
- Added `HttpRuntimeVerificationError` covering the exact failure phases: initial hash, probe execution/admission, final hash, and changed-during-probe artifact mismatch.
- Implemented `verify_http_runtime_candidate` to enforce the required sequence:
  1. pre-probe hash,
  2. probe execution and admission,
  3. post-probe hash,
  4. compare path/size/SHA-256,
  5. return a verified result only when all checks agree.
- Kept the verification operation isolated from source build, staging, publication, and runtime metadata writing; it only verifies the existing executable at its original path.
- Reused the existing hashing and probe primitives rather than duplicating subprocess or decoding logic.

Validation
- Ran: `cargo test --workspace --quiet`
- Result: all workspace tests passed.

This implementation preserves the existing runtime-information and hashing behavior while providing the framework-level candidate verification step requested by the requirements.
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