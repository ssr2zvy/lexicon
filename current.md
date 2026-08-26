Current implementation request: runtime invocation-envelope JSON contract

Objective

Add strict JSON encoding and decoding for RuntimeInvocationEnvelopeV1.

This defines the versioned parent-to-child invocation document while keeping source-specific arguments outside the envelope.

Do not add command-line transport, runner execution, sessions, or subprocess launching yet.

Required module

Extend:

lexicon-core/src/runtime/invocation.rs

Keep the public API under:

lexicon_core::runtime

Exact JSON structure

Use:

{
  "schema_version": 1,
  "project": {
    "name": "telugu-lexicon"
  },
  "runtime": {
    "source": "example-source",
    "protocol": "http",
    "operation": "acquisition",
    "source_contract_version": 1
  },
  "session": {
    "id": "session-000001"
  },
  "execution": {
    "mode": "run",
    "supervision": "foreground"
  }
}

A processing example differs only in the runtime operation:

{
  "schema_version": 1,
  "project": {
    "name": "telugu-lexicon"
  },
  "runtime": {
    "source": "example-source",
    "protocol": "http",
    "operation": "processing",
    "source_contract_version": 1
  },
  "session": {
    "id": "session-000002"
  },
  "execution": {
    "mode": "run",
    "supervision": "background"
  }
}

Schema version

Use:

RUNTIME_INVOCATION_PROTOCOL_VERSION

as the serialized schema_version.

Do not define a second envelope schema-version constant.

Encoding API

Provide:

impl RuntimeInvocationEnvelopeV1 {
    pub fn to_json(
        &self,
    ) -> Result<
        String,
        RuntimeInvocationEncodingError,
    >;
}

Requirements:

* deterministic structure;
* valid UTF-8 JSON;
* canonical runtime identifiers;
* canonical execution and supervision identifiers;
* no trailing newline;
* no source arguments;
* no project path;
* no session path;
* no environment variables;
* no handler pointers;
* no sensitive argument data.

Use a private Serde representation.

Decoding API

Provide:

impl RuntimeInvocationEnvelopeV1 {
    pub fn from_json(
        input: &str,
    ) -> Result<
        Self,
        RuntimeInvocationDecodingError,
    >;
}

Decoding must reconstruct values through the existing validated APIs:

ProjectInvocationIdentity::new(...)
SessionInvocationIdentity::new(...)
RuntimeProtocol::from_identifier(...)
RuntimeOperation::from_identifier(...)
RuntimeExecutionMode::from_identifier(...)
RuntimeSupervisionMode::from_identifier(...)
RuntimeInvocationEnvelopeV1::new(...)

Do not bypass constructor validation.

Strict decoding requirements

Reject:

* invalid JSON;
* duplicate fields;
* unknown fields;
* missing fields;
* unknown schema versions;
* invalid project identity;
* invalid session identity;
* unknown protocol identifiers;
* unknown operation identifiers;
* unknown execution-mode identifiers;
* unknown supervision-mode identifiers;
* zero source contract version;
* processing plus resume;
* any structurally invalid nested object.

Do not accept:

* aliases;
* case folding;
* surrounding whitespace in identifiers;
* numeric strings in place of versions;
* unknown compatibility fields for future versions.

Source arguments remain absent

The JSON document must not contain:

args
arguments
source_args
command_line

Unknown-field rejection must reject attempts to add them.

The later process syntax will carry source arguments separately after -- as untouched OsString values.

Typed encoding error

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInvocationEncodingError {
    Serialization(String),
}

Equivalent typed representation is acceptable.

Implement Display and Error.

Typed decoding error

Define an error that distinguishes at least:

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInvocationDecodingError {
    JsonSyntax(String),
    UnknownSchemaVersion(u32),
    UnknownIdentifier {
        field: &'static str,
        value: String,
    },
    InvalidProjectIdentity(
        RuntimeInvocationValueError,
    ),
    InvalidSessionIdentity(
        RuntimeInvocationValueError,
    ),
    InvalidVersion {
        field: &'static str,
        value: u32,
    },
    InvalidConstruction(
        RuntimeInvocationConstructionError,
    ),
    StructuralDocument(String),
}

Equivalent organization is acceptable.

Do not collapse identifier, value, construction, and structural failures into one string.

Runtime identity reconstruction

The decoded RuntimeIdentity must preserve:

* source;
* protocol;
* operation;
* source contract version.

Use the canonical identity types and identifier parsers.

Do not define a second runtime identity model inside the invocation module.

Required tests

Add tests proving:

1. A foreground acquisition/run envelope serializes successfully.
2. A background acquisition/resume envelope serializes successfully.
3. A foreground processing/run envelope serializes successfully.
4. The serialized schema version is 1.
5. Runtime identifiers use canonical strings.
6. Execution identifiers use canonical strings.
7. Encoding adds no final newline.
8. Encoding contains no source arguments or paths.
9. Acquisition/run round trip preserves equality.
10. Acquisition/resume round trip preserves equality.
11. Processing/run round trip preserves equality.
12. Invalid JSON is rejected.
13. Duplicate fields are rejected.
14. Unknown top-level fields are rejected.
15. Unknown nested fields are rejected.
16. Missing fields are rejected.
17. Unknown schema versions are rejected.
18. Invalid project identity is rejected.
19. Invalid session identity is rejected.
20. Unknown protocol is rejected.
21. Unknown operation is rejected.
22. Unknown execution mode is rejected.
23. Unknown supervision mode is rejected.
24. Identifier capitalization and whitespace are rejected.
25. Zero source contract version is rejected.
26. Processing/resume is rejected through construction validation.
27. An args field is rejected as unknown.
28. Encoding and decoding invoke no acquisition handler.
29. Encoding and decoding invoke no processing handler.
30. Existing in-memory invocation tests remain unchanged.
31. Existing runtime identity and runtime-information JSON tests pass.
32. All workspace tests pass repeatedly.

Preserve existing behavior

Do not change:

* in-memory envelope construction rules;
* runtime identity behavior;
* source descriptors;
* runtime-information schemas;
* probe behavior;
* hashing;
* verification;
* manifests;
* staging;
* bundle admission;
* paired publication;
* source scaffolding;
* source create;
* source build;
* Cargo invocation;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet
cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* invocation command-line syntax;
* base64 or other argv encoding;
* envelope files;
* source-argument splitting;
* child runtime admission;
* descriptor compatibility checks against the envelope;
* resume-handler presence validation;
* managed runner generation;
* runner main.rs;
* runner::run;
* runtime execution;
* project-path transport;
* session creation or locking;
* HTTP execution;
* raw recording;
* processing SQLite behavior;
* foreground supervision;
* background supervision;
* __operator-host;
* source build integration.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* exact invocation JSON;
* use of the invocation protocol version;
* encoding and decoding APIs;
* typed encoding and decoding errors;
* constructor and identifier delegation;
* successful round trips;
* every malformed-document rejection result;
* confirmation that source arguments and paths are absent;
* proof that no handler was invoked;
* Core and repeated workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not transport or execute the envelope.