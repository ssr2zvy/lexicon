# Runtime information compatibility validation report

Implemented the required pure Core compatibility validation for `RuntimeInformationV1` without changing the probe or source scaffolding behavior.

Summary
- Added the canonical compatibility error enum in `lexicon_core::runtime`:
  - `RuntimeCompatibilityError::IdentityMismatch { expected, actual }`
  - `RuntimeCompatibilityError::DescriptorContractVersionMismatch { identity_version, descriptor_version }`
  - `RuntimeCompatibilityError::MissingCapabilities(MissingHttpCapabilities)`
- Added `RuntimeInformationV1::validate_compatibility(expected_identity)` with the required validation order:
  1. identity equality
  2. descriptor contract version vs. source contract version
  3. required-capability subset check
- Kept JSON decoding separate from compatibility validation so decoding continues to return `RuntimeInformationDecodingError` and compatibility failures remain typed runtime compatibility errors.
- Preserved the existing probe semantics: structurally valid runtime information may still be incompatible, and the probe does not perform compatibility validation itself.

Implementation details
- The compatibility checks are restricted to pure Core logic and do not invoke acquire/resume, create contexts, inspect files, mutate data, or infer capabilities.
- The public validator is a regular method rather than a `const fn` because the compiler still rejects the `PartialEq`/result pattern in const evaluation; the underlying comparisons remain const-friendly while the compatibility decision is still exposed at runtime.
- The compatibility error is exposed through the canonical runtime path and remains a single typed error object without collapsing to formatted strings before returning.

Validation
- Ran: `cargo test --workspace --quiet`
- Result: all workspace tests passed.
- This includes the runtime information, capability, and probe test coverage for successful validation, mismatched identities and versions, missing capabilities, and the unchanged probe behavior.

* bundle inputs;
* installed paths.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If the known MZA dependency remains unavailable, report it separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* parent-side subprocess probing;
* probe timeout enforcement;
* Cargo build integration;
* build-time artifact verification;
* generated runners;
* runner::run;
* runner main.rs;
* source workspace migration;
* invocation envelopes;
* acquisition or resume execution;
* child runtime admission;
* runtime.json;
* executable hashing;
* publication changes;
* HTTP transport;
* raw recording;
* sessions;
* supervision;
* __operator-host;
* processing runtime compatibility.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* the exact compatibility API;
* the compatibility error representation;
* validation order;
* successful compatibility results;
* each typed incompatibility result;
* proof that structural decoding remains separate from compatibility validation;
* proof that the probe still reports incompatible information;
* proof that handlers were not invoked;
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not add parent-side probing or managed runners.