# Implementation report

Implemented the in-memory runtime.json manifest contract in `lexicon-framework` without writing runtime manifests to disk or integrating them into source build.

Changes completed
- Added `lexicon-framework/src/build/runtime_manifest.rs` with the `RuntimeManifestV1` API, schema version constant, strict executable-name validation, strict SHA-256 parsing, deterministic JSON encoding, and strict JSON decoding.
- Exported the public manifest API from `lexicon-framework/src/build/mod.rs`.
- Reused the existing Core `RuntimeInformationV1` JSON contract for the nested `runtime_information` object instead of duplicating the schema.
- Kept the construct path limited to `VerifiedHttpRuntime` to ensure the manifest integrity fields are copied only from the verified runtime state.
- Added duplicate-field rejection and structural validation around invalid schema versions, executable names, zero-size artifacts, malformed digests, and malformed nested runtime data.

Validation
- Ran: `cargo test --workspace --quiet`
- Result: pass (all workspace tests succeeded)

Notes
- This step remains in-memory only and does not stage a bundle or write `runtime.json` to disk, as required by the current implementation contract.

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