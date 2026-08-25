Current implementation request: versioned runtime-information JSON document

Objective

Add deterministic JSON serialization and parsing for RuntimeInformationV1.

This is the next narrow step toward the bounded runtime-information probe required by the architecture. It defines the parent/child data format but does not add a runner, command-line probe, subprocess execution, or runtime admission.

Required implementation

1. Add the runtime-information schema version

Define a distinct schema version:

pub const RUNTIME_INFORMATION_SCHEMA_VERSION: u32 = 1;

This version applies only to the serialized runtime-information document.

It is distinct from:

* source contract version;
* Core crate version;
* runtime invocation protocol version;
* runner-template version;
* project and source manifest versions;
* raw-data and session schema versions.

2. Define the serialized document

Add a private serialization representation corresponding to:

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
  }
}

Use serde and serde_json internally.

Do not expose the private Serde representation as the canonical runtime-information type. RuntimeInformationV1 remains the typed public model.

3. Add deterministic encoding

Provide a public API such as:

impl RuntimeInformationV1 {
    pub fn to_json(&self) -> Result<String, RuntimeInformationEncodingError>;
}

Requirements:

* output must be valid UTF-8 JSON;
* stable enum and capability identifiers must be used;
* capabilities must have deterministic ordering;
* duplicate capabilities must not appear;
* handler function pointers must never be serialized;
* no memory addresses or debug representations may appear;
* encoding must not execute acquisition or resume handlers.

Compact JSON is sufficient. Pretty-printing is not required.

4. Add strict decoding

Provide:

impl RuntimeInformationV1 {
    pub fn from_json(
        input: &str,
    ) -> Result<Self, RuntimeInformationDecodingError>;
}

Decoding must reject:

* invalid JSON;
* missing required fields;
* unknown schema versions;
* unknown protocol identifiers;
* unknown operation identifiers;
* unknown capability identifiers;
* duplicate capability identifiers;
* invalid or zero contract versions;
* unknown fields.

Do not silently ignore incompatible or malformed information.

5. Add stable identifier parsing

Add narrow parsing functions for the existing typed values:

impl RuntimeProtocol {
    pub fn from_identifier(
        value: &str,
    ) -> Result<Self, RuntimeIdentifierError>;
}
impl RuntimeOperation {
    pub fn from_identifier(
        value: &str,
    ) -> Result<Self, RuntimeIdentifierError>;
}
impl HttpCapability {
    pub fn from_identifier(
        value: &str,
    ) -> Result<Self, RuntimeIdentifierError>;
}

Parsing must use the same identifiers already returned by their identifier accessors.

Do not implement FromStr unless it materially simplifies the existing API. Do not permit aliases, case folding, or guessed values.

6. Preserve independent compatibility values

Decoding must preserve both:

identity.source_contract_version
descriptor.contract_version

It must not require them to be equal.

Compatibility validation belongs to a later admission step. This step only parses structurally valid information.

Error requirements

Define typed errors rather than returning String.

The exact internal representation is flexible, but callers must be able to distinguish at least:

* JSON syntax failure;
* unknown schema version;
* unknown identifier;
* duplicate capability;
* invalid version;
* structural document failure.

Errors must not panic and must not include source handler arguments or other unrelated sensitive data.

Required tests

Add tests proving:

1. A minimal RuntimeInformationV1 serializes successfully.
2. The serialized document contains schema version 1.
3. Runtime identity fields use their stable identifiers.
4. ClientCertificateV1 serializes as "client-certificate-v1".
5. Capability ordering is deterministic.
6. No capability appears more than once.
7. Resume registration serializes as true or false correctly.
8. Serialization does not invoke acquisition or resume handlers.
9. A serialize/deserialize round trip preserves equality.
10. Invalid JSON is rejected.
11. Missing required fields are rejected.
12. Unknown fields are rejected.
13. Unknown schema versions are rejected.
14. Unknown protocols are rejected.
15. Unknown operations are rejected.
16. Unknown capabilities are rejected.
17. Duplicate capabilities are rejected.
18. Zero contract versions are rejected.
19. Identity contract version 2 and descriptor contract version 1 survive a round trip without compatibility rejection.
20. Existing descriptor, capability, resume, identity, and in-memory runtime-information tests continue to pass.

Preserve existing external behavior

Do not change:

* source scaffolding;
* generated implementation crates;
* source create;
* source build;
* runtime publication;
* CLI commands or output;
* MZA configuration;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-bundle remains a binary installer compiled through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If the known MZA dependency is unavailable, report that blocker separately without changing MZA or installer code.

Explicit exclusions

Do not implement:

* runner::run;
* managed-runner generation;
* a --runtime-information command;
* writing runtime information to stdout;
* subprocess probing;
* probe timeouts;
* Core capability availability;
* capability compatibility checking;
* build-time admission;
* parent runtime admission;
* child runtime admission;
* invocation envelopes;
* runtime.json;
* executable hashing;
* workspace migration;
* source-library scaffolding;
* processing runtime information;
* HTTP transport;
* raw recording;
* sessions;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the exact JSON document structure;
* the schema-version constant;
* encoding and decoding APIs;
* typed error representation;
* every stable identifier accepted;
* deterministic capability-ordering behavior;
* round-trip test results;
* every malformed-document rejection result;
* confirmation that handlers are never serialized or invoked;
* confirmation that compatibility values remain independent;
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not add the runtime probe or managed runner.