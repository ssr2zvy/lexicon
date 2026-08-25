# Current implementation request: framework-side processing probe-output admission

Objective

Add the framework function that validates already-captured processing probe stdout, decodes it through Core, checks compatibility, and returns an opaque admitted processing result.

Do not launch a subprocess yet.

Required module

Extend:

lexicon-framework/src/build/runtime_probe.rs

Export the API through:

lexicon-framework/src/build/mod.rs

lexicon-framework remains library-only.

Shared output-boundary validation

Refactor the existing acquisition admission code so acquisition and processing share one private boundary validator.

It must enforce:

* maximum size using MAX_RUNTIME_INFORMATION_PROBE_BYTES;
* nonempty output;
* no NUL bytes;
* valid UTF-8;
* exactly one final ASCII newline;
* no \r\n;
* no additional final newline;
* no leading whitespace;
* no trailing whitespace before the newline;
* no diagnostic text or additional JSON documents.

Conceptually:

fn validate_runtime_probe_output(
    stdout: &[u8],
) -> Result<&str, RuntimeProbeOutputBoundaryError>;

The exact private API may differ.

Do not duplicate these checks separately for acquisition and processing.

Preserve the existing acquisition public API and behavior.

Admitted processing result

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedProcessingRuntimeInformation {
    information: ProcessingRuntimeInformationV1,
}

Provide:

impl AdmittedProcessingRuntimeInformation {
    pub fn information(
        &self,
    ) -> &ProcessingRuntimeInformationV1;
}

Do not provide a public unchecked constructor.

Public API

Provide:

pub fn admit_processing_runtime_information_probe(
    expected_identity: RuntimeIdentity,
    stdout: &[u8],
) -> Result<
    AdmittedProcessingRuntimeInformation,
    ProcessingRuntimeProbeAdmissionError,
>;

This function must perform no filesystem access and launch no process.

Admission sequence

Perform checks in this order:

1. Validate the shared output boundary.
2. Remove exactly one final newline.
3. Decode using:

ProcessingRuntimeInformationV1::from_json(...)

4. Validate using:

information.validate_compatibility(
    expected_identity,
)

5. Construct AdmittedProcessingRuntimeInformation.

Do not duplicate Core’s JSON schema, identity parsing, or compatibility rules.

Typed error

Define:

#[derive(Debug)]
pub enum ProcessingRuntimeProbeAdmissionError {
    OutputTooLarge {
        maximum: usize,
        actual: usize,
    },
    EmptyOutput,
    ContainsNul,
    InvalidUtf8(std::str::Utf8Error),
    InvalidOutputBoundary,
    Decode(
        ProcessingRuntimeInformationDecodingError,
    ),
    Incompatible(
        ProcessingRuntimeCompatibilityError,
    ),
}

Equivalent organization is acceptable.

If a private shared boundary error is introduced, translate it into the established acquisition error and new processing error without losing their public distinctions.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String, print diagnostics, or exit.

Compatibility semantics

Structural decoding and compatibility admission remain separate.

A processing document with a different source identity must:

1. decode successfully;
2. fail admission as Incompatible.

A document whose identity and descriptor contract versions disagree must likewise produce the typed compatibility error rather than a decoding error.

An acquisition-operation document must fail processing decoding rather than be admitted.

Handler safety

Admission must not:

* invoke process_handler();
* construct ProcessingContext;
* execute the runtime;
* inspect raw transactions;
* create SQLite databases;
* create sessions.

It validates captured metadata only.

Required tests

Add tests proving:

1. Exact output produced by Core’s processing probe is admitted.
2. The admitted wrapper exposes the decoded information.
3. Matching processing identity succeeds.
4. Empty output is rejected.
5. Oversized output is rejected before decoding.
6. NUL-containing output is rejected.
7. Invalid UTF-8 is rejected.
8. Missing final newline is rejected.
9. Two final newlines are rejected.
10. \r\n is rejected.
11. Leading spaces are rejected.
12. Leading newline is rejected.
13. Trailing spaces before the newline are rejected.
14. Diagnostic text before JSON is rejected.
15. Diagnostic text after JSON is rejected.
16. Multiple JSON documents are rejected.
17. Invalid JSON returns Decode.
18. Unknown processing schema versions return Decode.
19. Acquisition-operation documents return Decode.
20. Source identity disagreement returns Incompatible.
21. Descriptor-version disagreement returns Incompatible.
22. Failed admission cannot construct the admitted wrapper.
23. Admission does not invoke the processing handler.
24. Valid test output is generated using Core’s processing probe handler.
25. Existing acquisition admission succeeds unchanged.
26. Existing acquisition error classifications remain unchanged.
27. Acquisition and processing use the same private boundary rules.
28. Existing Core probe tests remain unchanged.
29. All workspace tests pass.

Do not maintain a separate handwritten valid processing JSON fixture.

Preserve existing behavior

Do not change:

* Core processing probe behavior;
* processing runtime-information schema;
* acquisition probe or admission public APIs;
* acquisition runtime-information schema;
* shared probe argument;
* probe output limit;
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

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-framework --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* processing subprocess execution;
* processing probe timeout handling;
* processing verification;
* processing manifests;
* processing staging;
* processing bundle admission;
* paired publication;
* processing runner main.rs;
* processing execution;
* raw-data discovery;
* SQLite operations;
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
* processing admission API;
* admitted processing wrapper;
* shared private boundary validator;
* exact accepted and rejected output forms;
* typed processing admission errors;
* Core decoding and compatibility delegation;
* successful and failed admission results;
* confirmation that acquisition admission remains unchanged;
* proof that the processing handler was not invoked;
* framework and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not execute a processing runtime subprocess. report

Completed: Core-owned processing runtime-information probe

Summary

The repository already contains the required processing probe implementation and the canonical runtime probe constant, and it matches the behavior described by the request in `current.md`.

What was implemented

- Added the processing runtime probe module at `lexicon-core/src/processing/runner.rs`.
- Exported the module as `lexicon_core::processing::runner`.
- Centralized the reserved probe flag in `lexicon_core::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT`.
- Re-used the canonical value through the existing acquisition re-export and the processing re-export so both code paths share the same literal.
- Implemented `try_write_runtime_information_probe(...)` with the required safety checks:
  - no-op for empty / unrelated / later-position arguments;
  - exact single-argument probe path only;
  - typed `UnexpectedArguments` error for invalid extra args;
  - strict `ProcessingRuntimeInformationV1::from_processing_source(...)` construction path;
  - JSON serialization, newline termination, flush, and `Written` outcome.
- Added deterministic tests covering the requested success and failure cases, including writer and flush errors.

Validation

Executed the repository’s standard validation flow:

```bash
cargo test --workspace --quiet
```

Result: pass.

Notes

- The processing and acquisition probe paths are consistent with the same canonical probe flag.
- Existing acquisition probe behavior remains unchanged.
- The processing runtime-information JSON schema and the processing descriptor contract remain intact.
- No additional framework-side subprocess probing or processing execution was introduced.
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

* framework processing probe-output admission;
* processing subprocess execution;
* processing executable hashing integration;
* processing verification;
* processing manifests;
* processing staging;
* processing bundle admission;
* paired publication;
* processing runner main.rs;
* processing execution;
* raw-transaction discovery;
* SQLite behavior;
* processing sessions;
* source workspace migration;
* acquisition managed runners;
* runner::run;
* invocation envelopes;
* HTTP execution;
* raw recording;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* canonical shared probe argument location;
* preserved acquisition re-export;
* processing probe API;
* outcome and error types;
* exact argument behavior;
* output and newline behavior;
* construction-failure behavior;
* write and flush failure results;
* proof that the process handler was not invoked;
* non-UTF-8 argument behavior;
* acquisition compatibility results;
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not add framework-side processing probing or a managed processing runner.