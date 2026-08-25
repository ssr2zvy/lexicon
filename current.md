Current implementation request: processing runtime-information JSON contract

Objective

Add strict JSON encoding and decoding for ProcessingRuntimeInformationV1.

This defines the document a future processing runtime-information probe will emit. Do not add the probe handler, subprocess execution, manifests, staging, or runners yet.

Required module

Extend:

lexicon-core/src/processing/runtime_information.rs

Keep the public API under:

lexicon_core::processing

Schema version

Define:

pub const PROCESSING_RUNTIME_INFORMATION_SCHEMA_VERSION: u32 = 1;

This version applies only to the serialized processing runtime-information document.

It remains distinct from:

* acquisition runtime-information schema version;
* processing source contract version;
* Core crate version;
* runtime invocation protocol version;
* runtime manifest version;
* runner-template version;
* session and raw-data schema versions.

Exact JSON structure

Use:

{
  "schema_version": 1,
  "identity": {
    "source": "example-source",
    "protocol": "http",
    "operation": "processing",
    "source_contract_version": 1
  },
  "descriptor": {
    "contract_version": 1
  }
}

Do not add:

* HTTP capabilities;
* resume registration;
* SQLite configuration;
* raw-data paths;
* source arguments;
* handler pointers;
* runtime paths.

Encoding API

Provide:

impl ProcessingRuntimeInformationV1 {
    pub fn to_json(
        &self,
    ) -> Result<
        String,
        ProcessingRuntimeInformationEncodingError,
    >;
}

Requirements:

* valid UTF-8 JSON;
* deterministic field structure;
* "protocol": "http";
* "operation": "processing";
* stable identity identifiers;
* no trailing newline;
* no handler invocation;
* no pointer addresses or debug output.

Use a private Serde representation.

Decoding API

Provide:

impl ProcessingRuntimeInformationV1 {
    pub fn from_json(
        input: &str,
    ) -> Result<
        Self,
        ProcessingRuntimeInformationDecodingError,
    >;
}

Decoding must reject:

* invalid JSON;
* duplicate fields;
* unknown fields;
* missing fields;
* unknown schema versions;
* unknown protocol identifiers;
* unknown operation identifiers;
* acquisition operation identity;
* zero source contract version;
* zero descriptor contract version.

Structural decoding versus compatibility

Structural decoding must require:

protocol  = http
operation = processing

However, it must preserve these values independently:

identity.source_contract_version
descriptor.contract_version

For example:

identity version   = 2
descriptor version = 1

must decode successfully.

The later call:

information.validate_compatibility(
    expected_identity,
)

must report the version incompatibility.

Do not require version equality during JSON decoding.

Internal construction

The existing supported source construction API must remain strict:

ProcessingRuntimeInformationV1::from_processing_source(...)

It continues rejecting an identity whose source contract version does not match ProcessingSourceContractV1::CONTRACT_VERSION.

JSON decoding may use a private internal constructor to preserve structurally valid but incompatible version combinations.

Do not expose that unchecked constructor publicly.

Typed encoding error

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessingRuntimeInformationEncodingError {
    Serialization(String),
}

Equivalent typed representation is acceptable.

Implement:

std::fmt::Display
std::error::Error

Typed decoding error

Define an error that distinguishes at least:

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessingRuntimeInformationDecodingError {
    JsonSyntax(String),
    UnknownSchemaVersion(u32),
    UnknownIdentifier {
        field: &'static str,
        value: String,
    },
    WrongProtocol {
        actual: RuntimeProtocol,
    },
    WrongOperation {
        actual: RuntimeOperation,
    },
    InvalidVersion {
        field: &'static str,
        value: u32,
    },
    StructuralDocument(String),
}

Equivalent organization is acceptable.

Do not return plain String from the public decoding API.

Identity parsing

Reuse the canonical Core parsing APIs:

RuntimeProtocol::from_identifier(...)
RuntimeOperation::from_identifier(...)

Do not duplicate identifier matching inside the processing module.

Do not accept aliases, case folding, or surrounding whitespace.

Required tests

Add tests proving:

1. Valid processing information serializes successfully.
2. The schema version is 1.
3. The protocol identifier is "http".
4. The operation identifier is "processing".
5. Source identity is preserved.
6. Source contract version is preserved.
7. Descriptor contract version is preserved.
8. Encoding does not add a newline.
9. Encoding does not invoke the process handler.
10. A JSON round trip preserves equality.
11. Invalid JSON is rejected.
12. Duplicate fields are rejected.
13. Unknown fields are rejected.
14. Missing fields are rejected.
15. Unknown schema versions are rejected.
16. Unknown protocol identifiers are rejected.
17. Unknown operation identifiers are rejected.
18. "operation": "acquisition" is rejected as WrongOperation.
19. Zero identity contract version is rejected.
20. Zero descriptor contract version is rejected.
21. Identity version 2 and descriptor version 1 decode successfully.
22. The mismatched decoded versions later fail compatibility validation.
23. Private unchecked construction is not publicly accessible.
24. Existing processing descriptor tests remain unchanged.
25. Existing acquisition runtime-information JSON remains unchanged.
26. Existing framework tests remain unchanged.
27. All workspace tests pass.

Preserve existing behavior

Do not change:

* acquisition runtime-information schema or APIs;
* acquisition probe behavior;
* processing descriptor signature;
* strict processing source construction;
* runtime identity identifiers;
* framework probing;
* hashing;
* verification;
* manifests;
* staging;
* bundle admission;
* reversible publication;
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

* processing probe argument;
* processing probe handler;
* framework processing probe admission;
* processing subprocess execution;
* processing verification;
* processing runtime manifests;
* processing staging;
* processing bundle admission;
* paired publication;
* processing runner;
* processing main.rs;
* processing execution;
* raw-transaction discovery;
* SQLite operations;
* processing sessions;
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
* schema-version constant;
* exact JSON structure;
* encoding and decoding APIs;
* typed encoding and decoding errors;
* stable identifier delegation;
* round-trip results;
* every malformed-document rejection result;
* proof that mismatched contract versions remain structurally representable;
* proof that processing handlers are not invoked;
* confirmation that acquisition runtime information remains unchanged;
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not add the processing probe handler.