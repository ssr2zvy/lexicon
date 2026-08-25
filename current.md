Current implementation request: processing runtime identity

Objective

Extend the canonical Core runtime identity model to represent HTTP processing runtimes.

This is the first micro-step toward the processing side required before paired acquisition-and-processing publication can be implemented.

Do not add a processing descriptor, runner, build, staging, or publication coordinator yet.

Required changes

Update:

lexicon-core/src/runtime/identity.rs

and its existing exports/tests.

Runtime operation

Extend:

pub enum RuntimeOperation {
    Acquisition,
    Processing,
}

Stable identifiers must be:

Acquisition → "acquisition"
Processing  → "processing"

Update:

RuntimeOperation::identifier()
RuntimeOperation::from_identifier(...)

so "processing" round-trips to RuntimeOperation::Processing.

Continue rejecting:

* unknown identifiers;
* aliases;
* capitalization differences;
* surrounding whitespace.

Processing identity constructor

Add:

impl RuntimeIdentity {
    pub const fn http_processing(
        source: &'static str,
        source_contract_version: u32,
    ) -> Self;
}

It must construct:

source                  = supplied source
protocol                = RuntimeProtocol::Http
operation               = RuntimeOperation::Processing
source_contract_version = supplied version

Retain the existing acquisition constructor unchanged:

RuntimeIdentity::http_acquisition(...)

Accessors and equality

Existing accessors must correctly expose processing identity values:

source()
protocol()
operation()
source_contract_version()

Processing identities must participate in existing Debug, Clone, Copy, PartialEq, and Eq behavior.

An acquisition identity and processing identity for the same source and contract version must not compare equal.

Runtime-information JSON

Because RuntimeInformationV1 already serializes RuntimeIdentity, update its structural encoding and decoding to support:

{
  "operation": "processing"
}

Requirements:

* processing identity must serialize with "processing";
* processing identity must decode successfully;
* a processing identity must survive a JSON round trip;
* unknown operation identifiers must remain rejected.

Do not change:

RUNTIME_INFORMATION_SCHEMA_VERSION

Adding the already-planned processing operation does not create a new document schema.

Compatibility behavior

The existing compatibility validator must compare processing identities exactly like acquisition identities.

Tests must prove:

* expected processing and actual processing can match;
* expected acquisition and actual processing produce IdentityMismatch;
* expected processing and actual acquisition produce IdentityMismatch.

Do not add processing-specific descriptor or capability validation.

HTTP descriptor construction guard

RuntimeInformationV1::from_http_source(...) currently represents an HTTP acquisition descriptor.

Do not silently reinterpret it as a processing descriptor.

If no type-level guard currently prevents supplying a processing identity to from_http_source(...), document that limitation in the completion report. Do not redesign the runtime-information hierarchy in this step.

A later processing-descriptor step will define the proper construction path.

Required tests

Add focused tests proving:

1. RuntimeOperation::Processing.identifier() returns "processing".
2. RuntimeOperation::from_identifier("processing") succeeds.
3. Processing parsing rejects aliases and incorrect capitalization.
4. RuntimeIdentity::http_processing(...) works in a constant.
5. Its protocol is HTTP.
6. Its operation is Processing.
7. It preserves source identity.
8. It preserves source contract version.
9. Acquisition and processing identities are unequal.
10. Processing identity serializes as "processing".
11. Processing identity survives runtime-information JSON round trip.
12. Unknown operation identifiers remain rejected.
13. Matching processing identities pass exact identity comparison.
14. Acquisition-versus-processing compatibility returns IdentityMismatch.
15. Processing-versus-acquisition compatibility returns IdentityMismatch.
16. Existing acquisition identity behavior remains unchanged.
17. Existing runtime probe behavior remains unchanged.
18. Existing framework tests continue to pass.
19. All workspace tests pass.

Do not create fake additional protocols solely for testing.

Preserve existing behavior

Do not change:

* acquisition descriptor behavior;
* acquisition capabilities or resume handler;
* runtime probing;
* hashing;
* verification;
* manifest schema;
* staging;
* runtime-bundle admission;
* reversible publication leg;
* source scaffolding;
* source create;
* source build;
* Cargo invocation;
* legacy publication;
* CLI behavior;
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

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If it remains unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* ProcessingSourceContractV1;
* processing handler signatures;
* processing capabilities;
* processing runtime-information construction;
* processing probes;
* processing verification;
* processing manifests;
* processing staging;
* processing bundle admission;
* paired publication;
* public single-runtime publication;
* integration with source build;
* managed runners;
* runner::run;
* source workspace migration;
* runtime execution;
* invocation envelopes;
* HTTP execution;
* raw recording;
* SQLite processing;
* sessions;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* the updated RuntimeOperation;
* stable processing identifier;
* processing identity constructor;
* constant-construction proof;
* accessor and equality results;
* runtime-information JSON round-trip results;
* acquisition/processing compatibility mismatch results;
* confirmation that acquisition behavior remains unchanged;
* any remaining lack of a type-level guard in from_http_source(...);
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not add the processing descriptor.