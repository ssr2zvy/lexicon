# RuntimeInformationV1 implementation report

## Files changed
- `lexicon-core/Cargo.toml`
- `lexicon-core/src/lib.rs`
- `lexicon-core/src/runtime/mod.rs`
- `lexicon-core/src/runtime/identity.rs`
- `lexicon-core/src/protocols/http/capability.rs`
- `lexicon-core/src/runtime/information.rs`
- `current.md`

## Exact JSON document structure
The serialized runtime-information document is:

```json
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
```

The document is represented internally by a private `serde` model and is not exposed as the canonical public type.

## Schema-version constant
```rust
pub const RUNTIME_INFORMATION_SCHEMA_VERSION: u32 = 1;
```

## Encoding and decoding APIs
```rust
impl RuntimeInformationV1 {
    pub fn to_json(&self) -> Result<String, RuntimeInformationEncodingError>;
    pub fn from_json(input: &str) -> Result<Self, RuntimeInformationDecodingError>;
}
```

The public model remains `RuntimeInformationV1`; the private Serde document is only used as the wire format.

## Typed error representation
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInformationEncodingError {
    Serialization(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInformationDecodingError {
    JsonSyntax(String),
    UnknownSchemaVersion(u32),
    UnknownIdentifier { field: &'static str, value: String },
    DuplicateCapability(String),
    InvalidVersion { field: &'static str, value: u32 },
    StructuralDocument(String),
}
```

The stable identifier parsing errors are represented by:
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeIdentifierError {
    UnknownIdentifier { kind: &'static str, value: String },
}
```

## Stable identifiers accepted
- `RuntimeProtocol::from_identifier("http") -> Ok(RuntimeProtocol::Http)`
- `RuntimeOperation::from_identifier("acquisition") -> Ok(RuntimeOperation::Acquisition)`
- `HttpCapability::from_identifier("client-certificate-v1") -> Ok(HttpCapability::ClientCertificateV1)`

No aliases, case folding, or guessed values are accepted.

## Deterministic capability ordering
`HttpCapabilitySet::ordered_capabilities()` returns capabilities in a fixed order (`ClientCertificateV1` first, and only once). The set is serialized from the bitset into a deduplicated `Vec<String>` before JSON emission.

## Round-trip and rejection results
Round-trip validation passed for equality-preserving JSON serialization and deserialization, including a case where `identity.source_contract_version` is `2` while `descriptor.contract_version` is `1`.

Malformed-document rejection results covered:
- invalid JSON
- missing required fields
- unknown fields
- unknown schema versions
- unknown protocols
- unknown operations
- unknown capabilities
- duplicate capabilities
- zero contract versions

## Handler safety confirmation
- Serialization calls only `identity`, `descriptor`, and capability data.
- No acquisition or resume function pointer is serialized.
- No pointer addresses or debug representations are emitted in JSON.
- The failing acquisition and resume handlers included in tests were not invoked during `to_json()`.

## Compatibility independence confirmation
The parsed runtime information preserves both:
- `identity.source_contract_version`
- `descriptor.contract_version`

They are validated independently and are not required to be equal during this structural decode step.

## Validation results
Executed successfully:
- `cargo test -p lexicon-core --quiet`
- `cargo test --workspace --quiet`

Result: all tests passed.

## Bundle/install status
The known external MZA dependency is unavailable in this environment.
- `automation/build_bundle_install/build_bundle_install.sh` exists, but the required MZA checkout is missing (`automation/build_bundle_mza/mza` is not present).
- The bundle/install script was not run because the external MZA dependency is not available, and no MZA or installer code was changed.
