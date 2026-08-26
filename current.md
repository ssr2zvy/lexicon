Current implementation request: in-memory processing runtime.json manifest

Objective

Add the versioned in-memory runtime.json manifest for a verified processing runtime.

This step connects:

VerifiedProcessingRuntime
→ ProcessingRuntimeManifestV1
→ deterministic JSON

Do not write the manifest to disk, stage a processing bundle, publish anything, or integrate with source build.

Required module

Create:

lexicon-framework/src/build/processing_runtime_manifest.rs

Export the public API through:

lexicon-framework/src/build/mod.rs

Shared manifest schema version

Use the existing:

RUNTIME_MANIFEST_SCHEMA_VERSION

with value:

1

Do not introduce a different processing manifest schema version. Acquisition and processing use the same outer runtime.json schema version, while their nested runtime-information documents remain operation-specific.

Exact JSON structure

Use:

{
  "schema_version": 1,
  "artifact": {
    "executable": "example-source-process-data",
    "size": 123456,
    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
  },
  "runtime_information": {
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
}

The nested object must come from:

ProcessingRuntimeInformationV1::to_json()

Do not reconstruct the Core processing schema field-by-field in the framework.

Processing manifest type

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessingRuntimeManifestV1 {
    executable_name: String,
    executable_size: u64,
    executable_sha256: ExecutableSha256,
    runtime_information: ProcessingRuntimeInformationV1,
}

Keep fields private.

Provide:

impl ProcessingRuntimeManifestV1 {
    pub fn executable_name(&self) -> &str;
    pub const fn executable_size(&self) -> u64;
    pub const fn executable_sha256(
        &self,
    ) -> ExecutableSha256;
    pub fn runtime_information(
        &self,
    ) -> &ProcessingRuntimeInformationV1;
}

Do not provide a public unchecked constructor.

Construction API

Provide:

impl ProcessingRuntimeManifestV1 {
    pub fn from_verified_processing_runtime(
        executable_name: &str,
        verified: &VerifiedProcessingRuntime,
    ) -> Result<
        Self,
        ProcessingRuntimeManifestConstructionError,
    >;
}

Construction must:

1. validate the executable name;
2. copy size and SHA-256 from the verified artifact;
3. copy admitted processing runtime information;
4. accept no independently supplied digest, size, or identity;
5. perform no hashing or execution.

Shared executable-name validation

Refactor acquisition and processing manifest construction to use one private executable-name validator.

It must continue rejecting:

* empty names;
* ".";
* "..";
* /;
* \;
* NUL;
* absolute paths;
* drive prefixes;
* colon-containing drive-style names;
* traversal components.

It must accept:

example-source-process-data
example-source-process-data.exe

Preserve acquisition manifest behavior and errors.

Encoding API

Provide:

impl ProcessingRuntimeManifestV1 {
    pub fn to_json(
        &self,
    ) -> Result<
        String,
        ProcessingRuntimeManifestEncodingError,
    >;
}

Requirements:

* deterministic JSON structure;
* lowercase 64-character SHA-256;
* canonical nested Core processing information;
* no candidate build path;
* no debug representations;
* no trailing newline.

Use a private Serde representation.

Decoding API

Provide:

impl ProcessingRuntimeManifestV1 {
    pub fn from_json(
        input: &str,
    ) -> Result<
        Self,
        ProcessingRuntimeManifestDecodingError,
    >;
}

Decoding must reject:

* invalid JSON;
* duplicate fields;
* unknown fields;
* missing fields;
* unknown outer manifest schema versions;
* invalid executable names;
* zero executable size;
* malformed SHA-256;
* malformed nested processing runtime information;
* acquisition runtime information nested in a processing manifest.

Do not perform expected-identity compatibility validation during structural decoding.

A structurally valid but incompatible processing identity may decode successfully and later fail compatibility admission.

Nested processing information

A private Serde representation may store:

serde_json::Value

For encoding:

1. call ProcessingRuntimeInformationV1::to_json();
2. convert that canonical JSON into the nested value.

For decoding:

1. isolate the nested JSON value;
2. convert it back to JSON;
3. call ProcessingRuntimeInformationV1::from_json(...).

Do not define a second framework-side processing identity or descriptor schema.

Typed errors

Define:

#[derive(Debug)]
pub enum ProcessingRuntimeManifestConstructionError {
    InvalidExecutableName,
}

Define an encoding error distinguishing:

* Core processing runtime-information encoding failure;
* outer manifest serialization failure.

Define a decoding error distinguishing:

* JSON syntax or structural failure;
* unknown manifest schema version;
* invalid executable name;
* invalid executable size;
* invalid SHA-256;
* malformed nested processing runtime information.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String from public APIs.

Required tests

Add tests proving:

1. A verified processing runtime constructs a manifest.
2. Size comes from VerifiedProcessingRuntime.
3. SHA-256 comes from VerifiedProcessingRuntime.
4. Runtime information comes from the admitted processing probe.
5. Construction cannot substitute a caller-provided digest.
6. Native executable names are accepted.
7. Windows .exe names are accepted.
8. Every established invalid filename remains rejected.
9. Acquisition and processing use the same private filename rules.
10. JSON contains the shared outer schema version 1.
11. Nested operation is "processing".
12. Encoding contains no temporary candidate path.
13. Encoding adds no final newline.
14. SHA-256 is exactly 64 lowercase hexadecimal characters.
15. JSON round trip preserves equality.
16. Invalid JSON is rejected.
17. Duplicate fields are rejected.
18. Unknown fields are rejected.
19. Missing fields are rejected.
20. Unknown manifest schema versions are rejected.
21. Zero executable size is rejected.
22. Invalid SHA-256 forms are rejected.
23. Malformed processing runtime information is rejected.
24. Acquisition runtime information is rejected as nested processing information.
25. Structurally valid but incompatible processing information can decode.
26. Compatibility validation later rejects that information.
27. Existing acquisition manifest behavior remains unchanged.
28. Existing processing verification tests remain unchanged.
29. All workspace tests pass.

Use a real VerifiedProcessingRuntime from the private verification test seam. Do not add a public unchecked verified-runtime constructor.

Preserve existing behavior

Do not change:

* acquisition runtime manifest JSON or public API;
* processing runtime-information schema;
* processing verification;
* probe behavior;
* hashing;
* acquisition staging;
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

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-framework --quiet

Run:

cargo test --workspace --quiet
cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* writing processing runtime.json;
* processing staging directories;
* executable copying;
* processing bundle admission;
* paired publication;
* source build integration;
* processing runner main.rs;
* processing execution;
* SQLite behavior;
* raw-data discovery;
* sessions;
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
* shared manifest schema version;
* exact processing manifest JSON;
* processing construction API;
* shared executable-name validation;
* encoding and decoding APIs;
* typed errors;
* nested Core processing-information delegation;
* round-trip and malformed-document results;
* acquisition manifest regression results;
* confirmation that no file was written or staged;
* framework and repeated workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not stage or publish a processing runtime.