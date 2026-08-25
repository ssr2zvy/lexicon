Current implementation request: Core-owned processing runtime-information probe

Objective

Add the Core-owned handler that serves runtime-information probe requests from a future managed processing runner.

This connects:

* RuntimeIdentity::http_processing(...);
* ProcessingSourceContractV1;
* ProcessingRuntimeInformationV1;
* processing runtime-information JSON.

Do not generate a runner, execute processing logic, or add framework-side subprocess probing yet.

Shared reserved argument

The acquisition and processing runtimes must use the same internal probe argument:

--lexicon-runtime-information-v1

Move or define the canonical constant under the shared runtime namespace:

lexicon_core::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT

Preserve the existing acquisition path through a re-export:

lexicon_core::http::runner::
    RUNTIME_INFORMATION_PROBE_ARGUMENT

Also expose the same constant through:

lexicon_core::processing::runner::
    RUNTIME_INFORMATION_PROBE_ARGUMENT

All paths must refer to the same canonical constant. Do not duplicate the string literal.

Required module

Create:

lexicon-core/src/processing/runner.rs

Export it as:

lexicon_core::processing::runner

Do not add a binary target or main.rs.

Probe outcome

Reuse the existing shared outcome type if it is already operation-neutral.

Otherwise define a processing outcome:

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingRuntimeInformationProbeOutcome {
    NotRequested,
    Written,
}

Do not force acquisition and processing errors into one type if their construction and encoding errors differ.

Probe API

Provide:

pub fn try_write_runtime_information_probe<
    W: std::io::Write,
>(
    identity: RuntimeIdentity,
    source: &ProcessingSourceContractV1,
    arguments: &[OsString],
    output: &mut W,
) -> Result<
    ProcessingRuntimeInformationProbeOutcome,
    ProcessingRuntimeInformationProbeError,
>;

Equivalent line wrapping is acceptable.

Argument behavior

Probe not requested

These return NotRequested and write nothing:

<no arguments>
--some-other-mode
ordinary-value

A probe argument in a later position also returns NotRequested:

--another-mode --lexicon-runtime-information-v1

Exact probe request

Exactly this one-element argument list selects probe mode:

--lexicon-runtime-information-v1

Core must:

1. Construct ProcessingRuntimeInformationV1 from the supplied identity and descriptor.
2. Encode it through ProcessingRuntimeInformationV1::to_json().
3. Write the JSON document.
4. Append exactly one ASCII newline.
5. Flush the writer.
6. Return Written.

Output must contain only:

<processing-runtime-information JSON>\n

Additional arguments

This is invalid:

--lexicon-runtime-information-v1 extra

Return a typed UnexpectedArguments error.

Do not treat extra as a processing source argument.

Native argument handling

Accept:

&[OsString]

Do not convert the entire argument list to UTF-8.

Compare only the reserved ASCII probe argument using OsStr.

An unrelated non-UTF-8 argument must safely return NotRequested on Unix.

Typed probe error

Define:

#[derive(Debug)]
pub enum ProcessingRuntimeInformationProbeError {
    UnexpectedArguments,
    Construction(
        ProcessingRuntimeInformationConstructionError,
    ),
    Encoding(
        ProcessingRuntimeInformationEncodingError,
    ),
    Output(std::io::Error),
}

Equivalent naming is acceptable, but callers must distinguish:

* malformed probe invocation;
* invalid processing identity/descriptor construction;
* JSON encoding failure;
* writer or flush failure.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String, print diagnostics, panic, or call std::process::exit.

Construction failure behavior

Probe handling must use:

ProcessingRuntimeInformationV1::
    from_processing_source(...)

Therefore:

* acquisition identity returns the typed construction error;
* incompatible source contract version returns the typed construction error;
* no JSON is written when construction fails.

Do not bypass strict construction through the private JSON-decoding constructor.

Handler safety

The probe must not:

* invoke process_handler();
* construct ProcessingContext;
* inspect raw transactions;
* open SQLite;
* create sessions;
* interpret source-specific arguments.

It reports compiled metadata only.

Required tests

Add tests proving:

1. Empty arguments return NotRequested.
2. An unrelated argument returns NotRequested.
3. The exact reserved argument returns Written.
4. NotRequested writes no bytes.
5. Successful output parses through ProcessingRuntimeInformationV1::from_json().
6. Successful output ends with exactly one newline.
7. Successful output contains only JSON and the newline.
8. Processing identity is preserved.
9. Descriptor contract version is preserved.
10. The process handler is not invoked.
11. Additional arguments return UnexpectedArguments.
12. A later-position probe argument returns NotRequested.
13. Acquisition identity returns the typed construction error.
14. Incorrect source contract version returns the typed construction error.
15. Construction failure writes no bytes.
16. Writer failure returns the typed output error.
17. Flush failure returns the typed output error.
18. An unrelated non-UTF-8 Unix argument returns NotRequested.
19. Acquisition and processing modules expose the same canonical probe argument.
20. Existing acquisition probe behavior remains unchanged.
21. Existing processing JSON tests remain unchanged.
22. All workspace tests pass.

Use injected writers to test write and flush failure deterministically.

Preserve existing behavior

Do not change:

* acquisition probe argument value;
* acquisition probe behavior or output;
* acquisition runtime-information schema;
* processing runtime-information schema;
* processing descriptor signature;
* framework acquisition probing;
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