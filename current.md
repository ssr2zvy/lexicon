Current implementation request: HTTP capability availability and compatibility

Objective

Add the in-memory model that distinguishes capabilities required by a source from capabilities available in a compiled managed runtime.

Then add a pure compatibility check that determines whether the runtime satisfies the source descriptor.

This step must not create a runner, execute a source handler, perform a subprocess probe, or admit a runtime.

Architectural requirement

The existing field:

descriptor.required_capabilities

states what the source needs.

It does not state what the compiled Core and runner provide.

The runtime-information model must carry both sets independently:

source requirements
runtime availability

Compatibility is:

every required capability exists in the available set

Required implementation

1. Extend RuntimeInformationV1

Add a private field:

available_capabilities: HttpCapabilitySet,

The representation becomes conceptually:

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeInformationV1 {
    identity: RuntimeIdentity,
    descriptor_contract_version: u32,
    required_capabilities: HttpCapabilitySet,
    available_capabilities: HttpCapabilitySet,
    resume_handler_registered: bool,
}

Do not create a second capability enum or a second bitset type. Both requirement and availability sets contain the same versioned HttpCapability values.

2. Require availability during construction

Update construction to accept the capabilities compiled into the selected runtime:

pub const fn from_http_source(
    identity: RuntimeIdentity,
    source: &HttpSourceContractV1,
    available_capabilities: HttpCapabilitySet,
) -> Self;

Do not infer availability from the source’s requirements.

In particular, this is invalid reasoning:

source requires ClientCertificateV1
therefore runtime provides ClientCertificateV1

The caller must supply the runtime’s actual available set.

3. Add an accessor

Provide:

pub const fn available_capabilities(
    &self,
) -> HttpCapabilitySet;

Preserve the existing:

pub const fn required_capabilities(
    &self,
) -> HttpCapabilitySet;

4. Add capability-set compatibility operations

Add the smallest required operations to HttpCapabilitySet:

pub const fn is_subset_of(
    self,
    available: HttpCapabilitySet,
) -> bool;
pub const fn missing_from(
    self,
    available: HttpCapabilitySet,
) -> HttpCapabilitySet;

Semantics:

required.is_subset_of(available)

is true only when every required capability is available.

required.missing_from(available)

returns only required capabilities absent from the available set.

These operations must remain allocation-free and const-friendly.

5. Add a typed compatibility error

Define:

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingHttpCapabilities {
    missing: HttpCapabilitySet,
}

Provide:

impl MissingHttpCapabilities {
    pub const fn missing(&self) -> HttpCapabilitySet;
}

Do not use String as the compatibility error.

6. Add pure compatibility validation

Provide:

impl RuntimeInformationV1 {
    pub const fn validate_capabilities(
        &self,
    ) -> Result<(), MissingHttpCapabilities>;
}

It must:

* return Ok(()) when every required capability is available;
* return all missing requirements together;
* not invoke acquisition or resume handlers;
* not inspect the filesystem;
* not inspect the installed CLI;
* not use the Core crate version as a proxy for capability availability;
* not mutate the runtime-information object.

This validates capability-set compatibility only. It must not validate identity or contract-version compatibility.

JSON document update

Extend the serialized document to include runtime availability:

{
  "schema_version": 1,
  "identity": {
    "source": "example-source",
    "protocol": "http",
    "operation": "acquisition",
    "source_contract_version": 1
  },
  "descriptor": {
    "contract_version": 1,
    "required_capabilities": [
      "client-certificate-v1"
    ],
    "resume_handler_registered": true
  },
  "runtime": {
    "available_capabilities": [
      "client-certificate-v1"
    ]
  }
}

Because the runtime-information format has not yet been connected to a released probe or published runtime schema, keep:

RUNTIME_INFORMATION_SCHEMA_VERSION == 1

Update the private Serde representation and tests accordingly.

Decoding must reject:

* a missing runtime object;
* a missing available_capabilities field;
* unknown runtime fields;
* unknown available-capability identifiers;
* duplicate available capabilities.

Decoding must not automatically reject missing required capabilities. A structurally valid but incompatible document must decode successfully, after which:

information.validate_capabilities()

reports the incompatibility.

No false capability claims

Do not add a function claiming that ClientCertificateV1 is supplied by the current production HTTP implementation unless that facility actually exists.

Tests may explicitly construct an available set containing it, but production code must not declare it available merely to make compatibility tests pass.

Do not add:

HttpCapabilitySet::all()

or equivalent as the default runtime availability.

An empty available set must be representable.

Required tests

Add tests proving:

1. No requirements plus no availability is compatible.
2. No requirements plus an available capability is compatible.
3. A required and available ClientCertificateV1 is compatible.
4. Required ClientCertificateV1 with empty availability is incompatible.
5. The compatibility error contains ClientCertificateV1.
6. missing_from(...) returns only missing requirements.
7. Repeated capability insertion does not create duplicates.
8. Construction preserves required and available sets independently.
9. Compatibility validation does not invoke acquire.
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