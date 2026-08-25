Current implementation request: Core-owned runtime-information probe handler

Objective

Implement the Core-owned handler for the reserved runtime-information probe.

This connects the completed runtime identity, source descriptor, required and available capability sets, and JSON runtime-information document.

The handler must be testable in-process. Do not generate or execute a managed runner yet.

Required module

Create:

lexicon-core/src/protocols/http/runner.rs

Export it through:

lexicon_core::http::runner

This module begins the Core-owned runner support API, but this step implements only runtime-information probing.

Reserved probe argument

Define:

pub const RUNTIME_INFORMATION_PROBE_ARGUMENT: &str =
    "--lexicon-runtime-information-v1";

This is a reserved internal runner argument.

It must not be added to the public lexicon CLI or displayed in normal CLI help.

Probe outcome

Define:

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeInformationProbeOutcome {
    NotRequested,
    Written,
}

* NotRequested means the arguments do not select probe mode.
* Written means the complete runtime-information document was written and flushed.

Probe API

Provide:

pub fn try_write_runtime_information_probe<W: std::io::Write>(
    identity: RuntimeIdentity,
    source: &HttpSourceContractV1,
    available_capabilities: HttpCapabilitySet,
    arguments: &[OsString],
    output: &mut W,
) -> Result<
    RuntimeInformationProbeOutcome,
    RuntimeInformationProbeError,
>;

The function must not read global process arguments or write directly to global stdout. The future managed runner will supply those values.

Argument behavior

No probe requested

These return:

Ok(RuntimeInformationProbeOutcome::NotRequested)

Examples:

<no arguments>
--some-other-mode
--ordinary-source-value

The function must write nothing.

Exact probe request

Exactly this one-element argument list selects the probe:

--lexicon-runtime-information-v1

Core must:

1. Construct RuntimeInformationV1 using the supplied identity, descriptor, and available capabilities.
2. Encode it through the existing to_json() implementation.
3. Write the JSON document.
4. Append exactly one newline.
5. Flush the writer.
6. Return Written.

The output must contain no heading, logging, diagnostic prefix, or other text.

Additional probe arguments

This is invalid:

--lexicon-runtime-information-v1 extra

Return a typed unexpected-arguments error.

Do not treat the additional value as a source argument.

Probe argument in another position

This does not select probe mode:

--another-mode --lexicon-runtime-information-v1

Return NotRequested.

Only the first argument may select the reserved probe mode.

Native argument handling

Accept:

&[OsString]

Do not convert the complete argument list to UTF-8.

Compare the reserved ASCII argument using OsStr.

An unrelated non-UTF-8 argument must safely return NotRequested on platforms where such an argument can be constructed.

Typed error

Define:

#[derive(Debug)]
pub enum RuntimeInformationProbeError {
    UnexpectedArguments,
    Encoding(RuntimeInformationEncodingError),
    Output(std::io::Error),
}

Equivalent naming is acceptable, but callers must be able to distinguish:

* invalid probe argument shape;
* runtime-information encoding failure;
* writer or flush failure.

Implement Display and std::error::Error consistently with existing Core errors.

Do not:

* return String;
* panic;
* print errors;
* call std::process::exit.

Capability behavior

The emitted document must preserve independently:

descriptor.required_capabilities
runtime.available_capabilities

The probe must not call:

RuntimeInformationV1::validate_capabilities()

A runtime with missing required capabilities must still be able to report its information. The future build verifier or runtime admission layer will perform compatibility rejection.

The probe must not infer, add, or otherwise manufacture available capabilities.

Handler safety

The probe must not:

* invoke acquire;
* invoke resume;
* create HttpAcquisitionContext;
* execute HTTP;
* create or modify a session;
* inspect source-specific arguments;
* perform runtime admission.

It reports compiled metadata only.

Required tests

Add tests proving:

1. Empty arguments return NotRequested.
2. An unrelated argument returns NotRequested.
3. The exact probe argument returns Written.
4. NotRequested writes no bytes.
5. Successful output parses through RuntimeInformationV1::from_json().
6. Successful output ends with exactly one newline.
7. Successful output contains only the JSON document and newline.
8. Runtime identity is preserved.
9. Descriptor contract version is preserved.
10. Required capabilities are preserved.
11. Available capabilities are preserved independently.
12. Resume registration is preserved.
13. An incompatible capability combination is successfully reported.
14. Probe execution does not invoke acquire.
15. Probe execution does not invoke resume.
16. Additional arguments after the probe flag return UnexpectedArguments.
17. A probe flag in a later position returns NotRequested.
18. A writer failure returns RuntimeInformationProbeError::Output.
19. A flush failure returns RuntimeInformationProbeError::Output.
20. An unrelated non-UTF-8 Unix argument returns NotRequested.
21. Existing runtime-information serialization and capability-validation tests pass.
22. All workspace tests pass.

Use injected test writers to exercise write and flush failures deterministically.

Preserve existing external behavior

Do not change:

* source scaffolding;
* source implementation crates;
* source create;
* source build;
* Cargo build invocation;
* runtime publication;
* public CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-bundle remains a binary installer compiled through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If the known external MZA dependency remains unavailable, report it separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* runner::run;
* a generated runner;
* runner main.rs;
* source workspace migration;
* process exit-code mapping;
* acquisition execution;
* resume execution;
* invocation envelopes;
* parent-side subprocess probing;
* probe timeouts;
* build-time probe validation;
* runtime admission;
* runtime.json;
* executable hashing;
* publication changes;
* HTTP transport;
* transaction recording;
* sessions;
* supervision;
* __operator-host;
* processing runtime probing.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the reserved probe argument;
* the exact probe API;
* outcome and error representations;
* argument-recognition behavior;
* output and newline behavior;
* write and flush failure results;
* required and available capability results;
* proof that incompatible metadata can still be reported;
* proof that acquisition and resume handlers were not invoked;
* non-UTF-8 argument behavior;
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not generate or execute a managed runner.