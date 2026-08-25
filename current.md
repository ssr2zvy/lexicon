Current implementation request: framework-side runtime probe-output admission

Objective

Implement the first parent-side runtime-information admission function in lexicon-framework.

The function receives already-captured probe stdout, validates its exact output boundary, decodes it through lexicon-core, validates compatibility, and returns an opaque admitted result.

Do not launch a subprocess in this step.

Required module

Create:

lexicon-framework/src/build/runtime_probe.rs

Expose it through the framework’s build module.

lexicon-framework must remain library-only.

Maximum output size

Define:

pub const MAX_RUNTIME_INFORMATION_PROBE_BYTES: usize =
    64 * 1024;

The limit includes the final newline.

It is a framework operational policy, not a Core schema rule.

Admitted result

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedRuntimeInformation {
    information: RuntimeInformationV1,
}

Provide:

impl AdmittedRuntimeInformation {
    pub fn information(&self) -> &RuntimeInformationV1;
}

Do not provide a public unchecked constructor.

An AdmittedRuntimeInformation value must prove that decoding and compatibility validation both succeeded.

Admission API

Provide:

pub fn admit_http_runtime_information_probe(
    expected_identity: RuntimeIdentity,
    stdout: &[u8],
) -> Result<
    AdmittedRuntimeInformation,
    RuntimeProbeAdmissionError,
>;

This function must perform no filesystem access and launch no process.

Accepted output

Accept exactly:

<one JSON document>\n

The valid bytes are the JSON produced by Core’s probe handler followed by exactly one ASCII line-feed byte:

0x0A

After validating the boundary, remove that one byte and pass the JSON text to:

RuntimeInformationV1::from_json(...)

Then call:

information.validate_compatibility(expected_identity)

Rejected output

Reject:

* empty output;
* output larger than the maximum;
* any NUL byte;
* invalid UTF-8;
* missing final newline;
* more than one final newline;
* \r\n;
* leading whitespace;
* trailing whitespace before the final newline;
* diagnostic text before the JSON;
* diagnostic text after the JSON;
* multiple JSON documents.

Do not use general-purpose trimming.

Do not silently extract JSON from surrounding output.

Deterministic validation order

Perform checks in this order:

1. Maximum byte length.
2. Empty output.
3. NUL-byte presence.
4. UTF-8 validity.
5. Exactly one final \n.
6. No preceding \n or \r.
7. No leading or trailing JSON whitespace.
8. Core JSON decoding.
9. Core compatibility validation.
10. Construct the admitted wrapper.

Framework code must not duplicate Core’s JSON schema, identifier parsing, or compatibility rules.

Typed error

Define:

#[derive(Debug)]
pub enum RuntimeProbeAdmissionError {
    OutputTooLarge {
        maximum: usize,
        actual: usize,
    },
    EmptyOutput,
    ContainsNul,
    InvalidUtf8(std::str::Utf8Error),
    InvalidOutputBoundary,
    Decode(RuntimeInformationDecodingError),
    Incompatible(RuntimeCompatibilityError),
}

Equivalent names are acceptable, but each category must remain distinguishable.

Implement:

std::fmt::Display
std::error::Error

Expose underlying errors through source() where appropriate.

Do not:

* return String;
* print diagnostics;
* terminate the process;
* convert compatibility errors into decoding errors.

Ownership boundary

lexicon-framework owns:

* the maximum-output policy;
* the exact stdout boundary;
* the admitted wrapper;
* orchestration-level error classification.

lexicon-core owns:

* JSON schema decoding;
* stable identifier parsing;
* identity validation;
* descriptor-version validation;
* capability compatibility.

Do not define a duplicate framework Serde document.

Required tests

Add tests proving:

1. Exact output from Core’s probe handler is admitted.
2. The admitted wrapper exposes the decoded information.
3. Matching identity and capabilities succeed.
4. Empty output is rejected.
5. Oversized output is rejected before decoding.
6. NUL-containing output is rejected.
7. Invalid UTF-8 is rejected.
8. Missing final newline is rejected.
9. Two final newlines are rejected.
10. \r\n is rejected.
11. Leading spaces are rejected.
12. Leading newline is rejected.
13. Trailing spaces before the final newline are rejected.
14. Diagnostic text before JSON is rejected.
15. Diagnostic text after JSON is rejected.
16. Multiple JSON documents are rejected.
17. Structurally invalid JSON produces Decode.
18. An unknown schema version produces Decode.
19. Identity disagreement produces Incompatible.
20. Descriptor-version disagreement produces Incompatible.
21. Missing required capabilities produce Incompatible.
22. The missing capability set remains inspectable.
23. Failed admission cannot construct AdmittedRuntimeInformation.
24. No acquisition handler is invoked.
25. No resume handler is invoked.
26. Existing Core probe and compatibility tests remain unchanged.
27. All workspace tests pass.

Generate valid success input by calling Core’s existing in-memory probe handler. Do not maintain a second handwritten valid document fixture.

Preserve existing behavior

Do not change:

* Core probe behavior;
* probe JSON schema;
* source scaffolding;
* source implementation crates;
* source create;
* source build;
* Cargo invocation;
* runtime publication;
* CLI behavior;
* MZA configuration;
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

If the known MZA checkout is unavailable, report it separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* subprocess spawning;
* executable-path validation;
* probe timeout enforcement;
* stdout or stderr pipe management;
* exit-status validation;
* Cargo build integration;
* artifact hashing;
* runtime.json;
* managed-runner generation;
* runner::run;
* runner main.rs;
* source workspace migration;
* invocation envelopes;
* acquisition or resume execution;
* child admission;
* publication changes;
* HTTP transport;
* raw recording;
* sessions;
* supervision;
* __operator-host;
* processing runtime admission.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the admission API;
* the opaque admitted result;
* the maximum-output policy;
* the exact accepted output boundary;
* every rejected boundary case;
* the typed error representation;
* proof that Core performs decoding and compatibility validation;
* successful and failed admission results;
* confirmation that no subprocess was launched;
* framework and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not implement process execution or managed runners.