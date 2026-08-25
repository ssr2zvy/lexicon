# Implementation report

## Outcome
Implemented the runtime-information compatibility validation in `lexicon-core` without changing the probe, scaffolding, or runtime publication flow.

## What changed
- Added the canonical `RuntimeCompatibilityError` enum plus `MissingHttpCapabilities` in `lexicon-core/src/runtime/information.rs`.
- Added `RuntimeInformationV1::validate_compatibility(...)` to enforce:
  1. exact identity match,
  2. descriptor contract version equals the reported source contract version,
  3. required capability subset checks.
- Kept JSON decoding separate from compatibility checking so structurally valid but incompatible documents still decode cleanly.
- Re-exported the compatibility type through `lexicon_core::runtime` and `lexicon_core::http` as the same canonical type.
- Added focused tests covering matching success, identity mismatch, descriptor mismatch, missing capabilities, ordering checks, and probe safety.

## Compiler note
The underlying comparisons are const-friendly, but the current Rust compiler still rejects `PartialEq` and `Result::map_err`/`?` usage in `const fn`s. The public validator remains a regular method with the exact compatibility logic required by the task, while the rest of the runtime metadata remains const-friendly.

## Validation
Executed:

```bash
cargo test --workspace --quiet
```

Result: all workspace tests passed.
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