Current implementation request: in-memory runtime.json manifest contract

Objective

Define the versioned runtime.json document created from a successfully verified HTTP runtime.

This step must provide strict construction, deterministic JSON encoding, and strict decoding entirely in memory.

Do not write runtime.json to disk, create runtime bundles, or integrate the manifest into source build yet.

Architectural position

The completed flow is:

candidate executable
→ initial hash
→ bounded runtime-information probe
→ compatibility admission
→ final hash
→ VerifiedHttpRuntime

This micro-step adds:

VerifiedHttpRuntime
→ RuntimeManifestV1
→ deterministic runtime.json bytes

A later step will stage the executable and manifest together as a runtime bundle.

Required module

Create:

lexicon-framework/src/build/runtime_manifest.rs

Export its public API through:

lexicon-framework/src/build/mod.rs

lexicon-framework remains library-only.

Manifest schema version

Define:

pub const RUNTIME_MANIFEST_SCHEMA_VERSION: u32 = 1;

This version applies only to runtime.json.

It remains distinct from:

* runtime-information schema version;
* source contract version;
* runtime invocation protocol version;
* Core crate version;
* runner-template version;
* project and source manifest versions;
* raw-data and session schema versions.

JSON document

Use this structure:

{
  "schema_version": 1,
  "artifact": {
    "executable": "example-source-get-raw-data",
    "size": 123456,
    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
  },
  "runtime_information": {
    "schema_version": 1,
    "identity": {
      "source": "example-source",
      "protocol": "http",
      "operation": "acquisition",
      "source_contract_version": 1
    },
    "descriptor": {
      "contract_version": 1,
      "required_capabilities": []
    },
    "runtime": {
      "available_capabilities": []
    }
  }
}

The exact nested runtime_information object must be the existing Core RuntimeInformationV1 JSON document.

If the current Core document includes additional established fields such as resume-handler registration, preserve them exactly. Do not maintain a second manually duplicated runtime-information schema in the framework.

Public manifest type

Define an opaque type:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeManifestV1 {
    executable_name: String,
    executable_size: u64,
    executable_sha256: ExecutableSha256,
    runtime_information: RuntimeInformationV1,
}

Provide accessors:

impl RuntimeManifestV1 {
    pub fn executable_name(&self) -> &str;
    pub const fn executable_size(&self) -> u64;
    pub const fn executable_sha256(
        &self,
    ) -> ExecutableSha256;
    pub fn runtime_information(
        &self,
    ) -> &RuntimeInformationV1;
}

Do not provide a public unchecked field constructor.

Construction from verified state

Provide:

pub fn from_verified_http_runtime(
    executable_name: &str,
    verified: &VerifiedHttpRuntime,
) -> Result<
    RuntimeManifestV1,
    RuntimeManifestConstructionError,
>;

Construction must:

1. validate that executable_name is exactly one safe filename;
2. reject empty names;
3. reject "." and "..";
4. reject /, \, or NUL;
5. reject absolute paths;
6. copy the verified artifact size and digest;
7. copy the admitted runtime information;
8. not rehash or execute the artifact;
9. not accept independently supplied size, digest, or runtime information.

The manifest must derive integrity and identity information only from VerifiedHttpRuntime.

Executable filename semantics

artifact.executable is a filename relative to its runtime bundle directory.

It must never contain:

* a parent directory;
* an absolute path;
* a drive prefix;
* path separators;
* traversal components.

Windows executable names such as:

example-source-get-raw-data.exe

must be accepted.

Do not store the candidate’s temporary absolute build path in the manifest.

SHA-256 parsing

Add strict hexadecimal parsing to ExecutableSha256 if it does not already exist:

pub fn from_hex(
    value: &str,
) -> Result<
    ExecutableSha256,
    ExecutableSha256ParseError,
>;

Accept exactly:

* 64 characters;
* lowercase hexadecimal digits 0-9 and a-f.

Reject:

* uppercase;
* prefixes such as sha256:;
* whitespace;
* incorrect length;
* non-hexadecimal characters.

Use a typed parse error.

JSON encoding

Provide:

impl RuntimeManifestV1 {
    pub fn to_json(
        &self,
    ) -> Result<String, RuntimeManifestEncodingError>;
}

Requirements:

* deterministic field structure;
* valid UTF-8 JSON;
* lowercase SHA-256;
* no absolute candidate path;
* no debug representations;
* no duplicate runtime-information schema implementation;
* no trailing newline added by this method.

The later file-writing layer may add or omit a final newline as part of its explicit disk contract.

Strict JSON decoding

Provide:

impl RuntimeManifestV1 {
    pub fn from_json(
        input: &str,
    ) -> Result<Self, RuntimeManifestDecodingError>;
}

Decoding must reject:

* invalid JSON;
* unknown fields;
* missing fields;
* unknown manifest schema versions;
* invalid executable names;
* zero executable size;
* malformed SHA-256;
* malformed nested runtime information;
* unknown nested runtime-information versions;
* duplicate JSON fields.

Use Core’s existing runtime-information decoder for the nested object.

Do not perform expected-identity compatibility admission during structural decoding. The manifest may be decoded before a caller knows which source was requested.

Nested runtime-information handling

The framework must not define a second Rust representation of Core’s identity, descriptor, capability, or resume fields.

A private Serde document may hold the nested value as:

serde_json::Value

The nested value must then be converted through the existing Core JSON APIs.

Encoding must likewise obtain the canonical Core representation through:

RuntimeInformationV1::to_json()

Do not reconstruct Core’s JSON field-by-field in framework code.

Typed errors

Define separate typed errors for:

Construction

pub enum RuntimeManifestConstructionError {
    InvalidExecutableName,
}

Encoding

pub enum RuntimeManifestEncodingError {
    RuntimeInformation(
        RuntimeInformationEncodingError,
    ),
    Serialization(String),
}

Decoding

The decoding error must distinguish at least:

* JSON syntax or structural failure;
* unknown manifest schema version;
* invalid executable name;
* invalid executable size;
* invalid SHA-256;
* malformed nested runtime information.

Equivalent representations are acceptable.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String from public manifest APIs.

Required tests

Add tests proving:

1. A verified runtime constructs a manifest successfully.
2. Executable size comes from the verified artifact.
3. SHA-256 comes from the verified artifact.
4. Runtime information comes from the admitted probe result.
5. Construction cannot independently substitute another digest.
6. A simple native filename is accepted.
7. A Windows .exe filename is accepted.
8. Empty filename is rejected.
9. "." is rejected.
10. ".." is rejected.
11. Forward-slash paths are rejected.
12. Backslash paths are rejected.
13. Absolute paths are rejected.
14. NUL-containing names are rejected.
15. JSON encoding contains no temporary candidate path.
16. SHA-256 encoding is exactly 64 lowercase characters.
17. A manifest JSON round trip preserves equality.
18. Invalid JSON is rejected.
19. Unknown fields are rejected.
20. Missing fields are rejected.
21. Unknown manifest schema versions are rejected.
22. Zero executable size is rejected.
23. Short SHA-256 values are rejected.
24. Uppercase SHA-256 values are rejected.
25. Non-hexadecimal SHA-256 values are rejected.
26. Malformed nested runtime information is rejected through the Core decoder.
27. Structurally valid incompatible runtime information may decode successfully.
28. Existing verification, hashing, and probing tests remain unchanged.
29. All workspace tests pass.

For successful construction tests, use a real VerifiedHttpRuntime produced through the existing verification test seam. Do not add a public unchecked verified-runtime constructor.

Preserve existing behavior

Do not change:

* Core runtime-information schema;
* runtime probing;
* compatibility validation;
* executable hashing;
* candidate verification;
* source scaffolding;
* source implementation crates;
* source create;
* source build;
* Cargo invocation;
* artifact selection;
* runtime publication;
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

cargo test -p lexicon-framework --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If the known MZA dependency remains unavailable, report it separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* writing runtime.json;
* runtime bundle directories;
* executable copying;
* staging;
* publication;
* rollback;
* integration with source build;
* runtime admission from disk;
* executable hash comparison against a published manifest;
* Cargo build-plan changes;
* managed-runner generation;
* runner main.rs;
* runner::run;
* source workspace migration;
* invocation envelopes;
* acquisition or resume execution;
* HTTP transport;
* raw recording;
* sessions;
* supervision;
* __operator-host;
* processing runtime manifests.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* manifest schema version;
* exact JSON structure;
* manifest construction API;
* executable-name validation;
* digest parsing behavior;
* encoding and decoding APIs;
* typed errors;
* nested Core runtime-information delegation;
* round-trip results;
* every malformed manifest rejection result;
* confirmation that no file was written or published;
* framework and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not write or publish a runtime bundle.