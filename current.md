# Implementation report

Implemented the HTTP capability compatibility model in the core runtime metadata.

Changes made:
- Added `available_capabilities` to `RuntimeInformationV1` and updated the constructor to accept the runtime’s actual compiled capability set instead of inferring it from the source descriptor.
- Added `HttpCapabilitySet::is_subset_of` and `HttpCapabilitySet::missing_from` as const-friendly, allocation-free compatibility helpers.
- Added `MissingHttpCapabilities` plus `RuntimeInformationV1::validate_capabilities()` to perform pure compatibility checks without acquiring or resuming handlers.
- Extended the JSON document to include `runtime.available_capabilities` and enforced strict decoding rules for missing/unknown/duplicate runtime capability entries.
- Updated and added tests covering empty/compatible/incompatible capability sets, duplicate insertion prevention, independent requirement-vs-availability tracking, and validation behavior.

Validation:
- `cargo test --workspace --quiet` ✅

10. Compatibility validation does not invoke resume.
11. Runtime information with missing capabilities serializes successfully.
12. Runtime information with missing capabilities deserializes successfully.
13. Compatibility is rejected only when validate_capabilities() is called.
14. Available capabilities serialize with stable deterministic ordering.
15. Available capabilities survive a JSON round trip.
16. Duplicate available capabilities in JSON are rejected.
17. Unknown available capabilities in JSON are rejected.
18. Missing runtime availability fields are rejected structurally.
19. Existing identity and descriptor contract-version independence remains unchanged.
20. All existing Core and workspace tests continue to pass.

Preserve existing behavior

Do not change:

* source scaffolding;
* source implementation crates;
* source create;
* source build;
* Cargo build invocation;
* runtime publication;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-bundle remains a binary installer using cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If it remains unavailable, report the known external dependency blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* production availability for ClientCertificateV1;
* capability negotiation;
* automatic capability inference;
* runner generation;
* runner::run;
* runtime-information CLI output;
* subprocess probing;
* build-time capability validation;
* parent admission;
* child admission;
* identity validation;
* contract-version validation;
* invocation envelopes;
* runtime.json;
* executable hashing;
* source workspace migration;
* processing capabilities;
* HTTP transport;
* raw recording;
* sessions;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a focused report containing:

* files changed;
* the updated RuntimeInformationV1 representation;
* the updated constructor signature;
* capability-set operations;
* the typed compatibility error;
* compatibility-validation behavior;
* the updated JSON structure;
* compatible and incompatible test results;
* malformed availability-document rejection results;
* confirmation that incompatibility is not confused with structural decoding failure;
* confirmation that no production capability was falsely declared available;
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not implement the runtime probe or managed runner.