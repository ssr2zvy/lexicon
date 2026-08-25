# Implementation report

Completed: Core-owned processing runtime-information probe

Summary

The repository already contains the required processing probe implementation and the canonical runtime probe constant, and it matches the behavior described by the request in `current.md`.

What was implemented

- Added the processing runtime probe module at `lexicon-core/src/processing/runner.rs`.
- Exported the module as `lexicon_core::processing::runner`.
- Centralized the reserved probe flag in `lexicon_core::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT`.
- Re-used the canonical value through the existing acquisition re-export and the processing re-export so both code paths share the same literal.
- Implemented `try_write_runtime_information_probe(...)` with the required safety checks:
  - no-op for empty / unrelated / later-position arguments;
  - exact single-argument probe path only;
  - typed `UnexpectedArguments` error for invalid extra args;
  - strict `ProcessingRuntimeInformationV1::from_processing_source(...)` construction path;
  - JSON serialization, newline termination, flush, and `Written` outcome.
- Added deterministic tests covering the requested success and failure cases, including writer and flush errors.

Validation

Executed the repository’s standard validation flow:

```bash
cargo test --workspace --quiet
```

Result: pass.

Notes

- The processing and acquisition probe paths are consistent with the same canonical probe flag.
- Existing acquisition probe behavior remains unchanged.
- The processing runtime-information JSON schema and the processing descriptor contract remain intact.
- No additional framework-side subprocess probing or processing execution was introduced.
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