Implementation report

Files changed
- lexicon-core/src/processing/runtime_information.rs
- lexicon-core/src/processing/mod.rs
- current.md

Schema-version constant
- PROCESSING_RUNTIME_INFORMATION_SCHEMA_VERSION: u32 = 1

Exact JSON structure
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

Encoding and decoding APIs
- impl ProcessingRuntimeInformationV1 {
    pub fn to_json(&self) -> Result<String, ProcessingRuntimeInformationEncodingError>
  }
- impl ProcessingRuntimeInformationV1 {
    pub fn from_json(input: &str) -> Result<Self, ProcessingRuntimeInformationDecodingError>
  }

Typed encoding and decoding errors
- ProcessingRuntimeInformationEncodingError::Serialization(String)
- ProcessingRuntimeInformationDecodingError::{
    JsonSyntax(String),
    UnknownSchemaVersion(u32),
    UnknownIdentifier { field: &'static str, value: String },
    WrongProtocol { actual: RuntimeProtocol },
    WrongOperation { actual: RuntimeOperation },
    InvalidVersion { field: &'static str, value: u32 },
    StructuralDocument(String),
  }

Stable identifier delegation
- RuntimeProtocol::from_identifier(...)
- RuntimeOperation::from_identifier(...)
- The processing JSON contract does not duplicate identifier matching logic locally.

Round-trip results
- Valid processing documents serialize to JSON successfully.
- The generated JSON has no trailing newline.
- A JSON round trip preserves equality for the same processing runtime identity/descriptor.
- Source name, source contract version, and descriptor contract version are preserved.

Malformed-document rejection results
- invalid JSON rejected as JsonSyntax
- duplicate fields rejected as StructuralDocument
- unknown fields rejected as StructuralDocument
- missing fields rejected as StructuralDocument
- unknown schema versions rejected as UnknownSchemaVersion
- unknown protocol identifiers rejected as UnknownIdentifier
- unknown operation identifiers rejected as UnknownIdentifier
- operation = acquisition rejected as WrongOperation
- zero identity source contract version rejected as InvalidVersion
- zero descriptor contract version rejected as InvalidVersion
- incompatible but structurally valid versions decode successfully, and later compatibility validation fails as expected

Mismatched contract version behavior
- source_contract_version = 2 and descriptor.contract_version = 1 are structurally representable in JSON
- validate_compatibility(expected_identity) reports the version mismatch after decoding

Proof that processing handlers are not invoked
- Existing strict construction ensures processing source handling remains function-pointer-only and the JSON methods never call the process handler.
- The new tests assert that encoding and construction do not increment the process-handler call counter.

Acquisition runtime information remains unchanged
- No acquisition runtime-information schema, JSON docs, or acquisition APIs were modified.
- The processing contract remains distinct and isolated from acquisition runtime metadata.

Core and workspace test results
- cargo test -p lexicon-core --quiet: passed
- cargo test --workspace --quiet: passed

Bundle/install result
- Bundle/install script was attempted: bash automation/build_bundle_install/build_bundle_install.sh
- Blocked by external MZA checkout missing: /home/runner/work/lexicon/lexicon/automation/build_bundle_mza/mza/make-artifact.sh: No such file or directory
- This is the known external blocker; no MZA or installer code was modified.
