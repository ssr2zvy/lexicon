# Implementation report

Completed the verified processing runtime candidate work in `lexicon-framework`.

What changed
- Added `VerifiedProcessingRuntime` and `ProcessingRuntimeVerificationError` to `lexicon-framework/src/build/runtime_verification.rs`.
- Implemented `verify_processing_runtime_candidate(...)` and a private deterministic seam (`verify_processing_runtime_candidate_with(...)`) alongside the existing HTTP acquisition verification.
- Reused a shared private `verify_runtime_candidate_with(...)` orchestration helper so the hash -> probe -> hash flow is shared where practical, while preserving the existing HTTP acquisition API and behavior.
- Exported the processing verification API through `lexicon-framework/src/build/mod.rs`.
- Added coverage for successful processing verification and the main failure branches (initial hash, probe failure, final hash failure, and artifact mutation).

Validation
- Ran the required workspace validation command:
  `cargo test --workspace --quiet`
- Result: all workspace tests passed.

Notes
- No manifest/staging/publication/source-build changes were introduced; this remains a verification-only change.
- The candidate file is hashed before and after the probe and is rejected if the bytes change during the probe.

* legacy publication;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-framework --quiet

Run the workspace suite twice:

cargo test --workspace --quiet
cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* processing runtime manifests;
* processing staging;
* processing bundle admission;
* paired publication;
* source build integration;
* processing runner main.rs;
* processing execution;
* SQLite behavior;
* raw-data discovery;
* sessions;
* source workspace migration;
* managed acquisition runners;
* runner::run;
* invocation envelopes;
* HTTP execution;
* raw recording;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* processing verification API;
* opaque verified processing type;
* shared private orchestration;
* exact hash/probe/hash order;
* typed verification errors;
* successful processing verification;
* initial-hash, probe, final-hash, and mutation failures;
* same-size mutation detection;
* acquisition regression results;
* confirmation that no manifest, staging, or publication occurred;
* repeated workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not add processing manifests or staging.