# Implementation report

Implemented the Core-owned processing runtime-information probe without adding a runner or framework-side probing logic.

Summary
- Added the canonical shared probe argument under `lexicon_core::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT`.
- Kept the acquisition path stable via the existing `lexicon_core::http::runner::RUNTIME_INFORMATION_PROBE_ARGUMENT` re-export.
- Added `lexicon_core::processing::runner` and exported the processing probe API and constant from that module.
- Implemented `try_write_runtime_information_probe(...)` with typed errors and exact newline/flush behavior for successful output.
- Added focused tests covering probe detection, serialization, error handling, and canonical constant sharing.

Files changed
- `lexicon-core/src/runtime/mod.rs`
- `lexicon-core/src/protocols/http/runner.rs`
- `lexicon-core/src/processing/mod.rs`
- `lexicon-core/src/processing/runner.rs`

Validation
- Ran: `cargo test --workspace --quiet`
- Result: all workspace tests passed.
* installed paths.

lexicon-framework remains library-only.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If it remains unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* framework processing probe-output admission;
* processing subprocess execution;
* processing executable hashing integration;
* processing verification;
* processing manifests;
* processing staging;
* processing bundle admission;
* paired publication;
* processing runner main.rs;
* processing execution;
* raw-transaction discovery;
* SQLite behavior;
* processing sessions;
* source workspace migration;
* acquisition managed runners;
* runner::run;
* invocation envelopes;
* HTTP execution;
* raw recording;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* canonical shared probe argument location;
* preserved acquisition re-export;
* processing probe API;
* outcome and error types;
* exact argument behavior;
* output and newline behavior;
* construction-failure behavior;
* write and flush failure results;
* proof that the process handler was not invoked;
* non-UTF-8 argument behavior;
* acquisition compatibility results;
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not add framework-side processing probing or a managed processing runner.