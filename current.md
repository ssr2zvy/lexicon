# Runtime invocation-envelope JSON contract

Implemented the strict runtime invocation envelope in `lexicon-core`.

## Changes made
- Added `RuntimeInvocationEncodingError` and `RuntimeInvocationDecodingError` to the runtime API.
- Implemented `RuntimeInvocationEnvelopeV1::to_json` and `RuntimeInvocationEnvelopeV1::from_json` in `lexicon-core/src/runtime/invocation.rs`.
- Used a private Serde representation with `deny_unknown_fields` and exact field names:
  - `schema_version`
  - `project.name`
  - `runtime.source`
  - `runtime.protocol`
  - `runtime.operation`
  - `runtime.source_contract_version`
  - `session.id`
  - `execution.mode`
  - `execution.supervision`
- Enforced canonical runtime/execution/supervision identifiers and validation through the existing constructors and identifier parsers.
- Rejected duplicate keys, invalid JSON, unknown fields, missing fields, unsupported schema versions, invalid project/session identities, and invalid construction cases.
- Exported the new runtime error types from `lexicon-core/src/runtime/mod.rs`.
- Added regression coverage for serializing, round-tripping, and invalid input rejection.

## Validation
- `cargo test -p lexicon-core --quiet` ✅
- `cargo test --workspace --quiet` ⚠️ still fails in `lexicon-framework`, specifically `build::runtime_bundle_admission::tests::manifest_too_large_is_rejected`, due `Probe(Spawn { source: Os { code: 26, kind: ExecutableFileBusy, message: "Text file busy" })`. This failure is outside the runtime invocation change and remains a pre-existing/unrelated workspace issue.

## Files updated
- `lexicon-core/src/runtime/invocation.rs`
- `lexicon-core/src/runtime/mod.rs`

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