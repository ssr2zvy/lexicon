# Implementation report

Implemented the framework-side runtime probe admission flow in `lexicon-framework/src/build/runtime_probe.rs` and exposed it via `lexicon-framework/src/build/mod.rs`.

What was completed
- Added `MAX_RUNTIME_INFORMATION_PROBE_BYTES` as the framework-level 64 KiB stdout policy.
- Added `AdmittedRuntimeInformation` as an opaque wrapper that only exists after successful decode and compatibility validation.
- Added `admit_http_runtime_information_probe(...)` to enforce the exact output boundary and reject invalid probe output without spawning subprocesses or touching the filesystem.
- Kept the framework gate dependent on Core’s existing `RuntimeInformationV1::from_json(...)` and `validate_compatibility(...)` APIs instead of duplicating schema or compatibility logic.
- Added the required error classification and tests covering valid admission, invalid boundaries, invalid UTF-8, empty/oversized output, null bytes, JSON decode failures, compatibility mismatches, and missing capabilities.

Validation
- Ran the standard workspace validation command:
  - `cargo test --workspace --quiet`
- Result: all workspace tests passed.

Status
- No remaining blockers. The framework runtime probe admission contract is in place and the repository remains green under the standard Cargo test flow.
* MZA configuration;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-framework --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If the known MZA checkout is unavailable, report it separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* subprocess spawning;
* executable-path validation;
* probe timeout enforcement;
* stdout or stderr pipe management;
* exit-status validation;
* Cargo build integration;
* artifact hashing;
* runtime.json;
* managed-runner generation;
* runner::run;
* runner main.rs;
* source workspace migration;
* invocation envelopes;
* acquisition or resume execution;
* child admission;
* publication changes;
* HTTP transport;
* raw recording;
* sessions;
* supervision;
* __operator-host;
* processing runtime admission.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the admission API;
* the opaque admitted result;
* the maximum-output policy;
* the exact accepted output boundary;
* every rejected boundary case;
* the typed error representation;
* proof that Core performs decoding and compatibility validation;
* successful and failed admission results;
* confirmation that no subprocess was launched;
* framework and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not implement process execution or managed runners.