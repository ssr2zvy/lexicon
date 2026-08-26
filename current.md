# Implementation report

Completed the in-memory processing runtime manifest work required by the current request.

What was implemented
- Added `lexicon-framework/src/build/processing_runtime_manifest.rs` with the processing manifest model and typed validation/encoding/decoding behavior.
- Exported the public API through `lexicon-framework/src/build/mod.rs`.
- Reused the shared executable-name guard and runtime manifest schema version (`RUNTIME_MANIFEST_SCHEMA_VERSION = 1`) without introducing a second schema version.
- Kept the nested JSON payload sourced from `ProcessingRuntimeInformationV1::to_json()` and decoded back through `ProcessingRuntimeInformationV1::from_json(...)`.
- Added validation for executable names, size, SHA-256, duplicate keys, and malformed nested runtime information.
- Added tests covering manifest construction, invalid names, and JSON round-tripping.

Validation
- Ran: `cargo test --workspace --quiet`
- Result: all workspace tests passed.

Notes
- This work stays in-memory only and does not write runtime manifests to disk, stage bundles, or integrate with source build publication.
- The implementation matches the requested VerifiedProcessingRuntime → ProcessingRuntimeManifestV1 → deterministic JSON flow.
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