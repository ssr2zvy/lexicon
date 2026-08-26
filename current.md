Current implementation request: invocation argv transport and source-argument splitting

Objective

Define the exact native command-line transport for RuntimeInvocationEnvelopeV1 and untouched source-specific arguments.

This step implements argument encoding and parsing only.

Do not launch a runtime, perform child admission, invoke source handlers, or generate managed runners.

Process argument contract

The managed runtime invocation has this exact structure:

--lexicon-invocation-v1 <envelope-json> -- <source-arguments...>

At the operating-system argument level:

[
    OsString::from("--lexicon-invocation-v1"),
    OsString::from(envelope_json),
    OsString::from("--"),
    // Untouched source-specific OsString values.
]

The -- delimiter is mandatory even when the source receives no arguments:

[
    OsString::from("--lexicon-invocation-v1"),
    OsString::from(envelope_json),
    OsString::from("--"),
]

The supplied parser argument slice excludes operating-system argv[0].

Required module

Create:

lexicon-core/src/runtime/invocation_transport.rs

Export its public API through:

lexicon_core::runtime

Canonical reserved arguments

Define:

pub const RUNTIME_INVOCATION_ARGUMENT: &str =
    "--lexicon-invocation-v1";
pub const RUNTIME_SOURCE_ARGUMENT_DELIMITER: &str =
    "--";

These are the canonical values future framework launch code and managed runners must use.

Do not duplicate these string literals elsewhere.

The existing information-probe argument remains separate:

RUNTIME_INFORMATION_PROBE_ARGUMENT

Envelope size limit

Define:

pub const MAX_RUNTIME_INVOCATION_ENVELOPE_JSON_BYTES: usize =
    16 * 1024;

This limit applies only to the UTF-8 serialized invocation envelope.

Do not introduce a Lexicon-specific size limit for source arguments in this step. Operating-system command-line limits still apply.

Encoded invocation type

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedRuntimeInvocation {
    arguments: Vec<OsString>,
}

Keep the field private.

Provide:

impl EncodedRuntimeInvocation {
    pub fn arguments(
        &self,
    ) -> &[OsString];
    pub fn into_arguments(
        self,
    ) -> Vec<OsString>;
}

Do not provide a public unchecked constructor.

Parent-side encoding API

Provide:

pub fn encode_runtime_invocation(
    envelope: &RuntimeInvocationEnvelopeV1,
    source_arguments: &[OsString],
) -> Result<
    EncodedRuntimeInvocation,
    RuntimeInvocationTransportEncodingError,
>;

The function must:

1. Serialize the envelope using:

RuntimeInvocationEnvelopeV1::to_json()

2. Measure the serialized UTF-8 byte length.
3. Reject envelope JSON larger than the configured maximum.
4. Add the canonical invocation argument.
5. Add the envelope as one OsString argument.
6. Add the mandatory source-argument delimiter.
7. Append every source argument without interpretation.
8. Preserve source argument order, duplicates, and native values.

The encoder must not persist, print, redact, normalize, parse, or otherwise interpret source-specific arguments.

Parsed invocation type

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRuntimeInvocation {
    envelope: RuntimeInvocationEnvelopeV1,
    source_arguments: Vec<OsString>,
}

Keep fields private.

Provide:

impl ParsedRuntimeInvocation {
    pub fn envelope(
        &self,
    ) -> &RuntimeInvocationEnvelopeV1;
    pub fn source_arguments(
        &self,
    ) -> &[OsString];
    pub fn into_parts(
        self,
    ) -> (
        RuntimeInvocationEnvelopeV1,
        Vec<OsString>,
    );
}

Do not provide a public unchecked constructor.

Child-side parsing API

Provide:

pub fn parse_runtime_invocation(
    arguments: &[OsString],
) -> Result<
    ParsedRuntimeInvocation,
    RuntimeInvocationTransportDecodingError,
>;

The arguments slice excludes executable argv[0].

Exact parsing rules

Require:

1. Argument zero exists.
2. Argument zero is exactly:

--lexicon-invocation-v1

3. Argument one exists.
4. Argument one is valid UTF-8 envelope JSON.
5. Its UTF-8 byte length does not exceed the configured maximum.
6. Argument two exists.
7. Argument two is exactly:

--

8. The envelope decodes through:

RuntimeInvocationEnvelopeV1::from_json(...)

9. Every later value is copied as an untouched source argument.

Do not search later arguments for a missing internal flag or delimiter.

Do not reorder, trim, guess, or repair malformed internal arguments.

Native source-argument preservation

Only the envelope JSON must be UTF-8.

Values after the delimiter remain native OsString values.

The complete encode/parse round trip must preserve:

* an empty source-argument list;
* empty argument values;
* Unicode values;
* non-UTF-8 Unix values;
* values beginning with -;
* repeated values;
* source argument ordering;
* a source value equal to --;
* a source value equal to --lexicon-invocation-v1;
* a source value equal to --lexicon-runtime-information-v1.

After consuming the first mandatory delimiter, Lexicon must not interpret any later value.

Malformed transport behavior

Reject:

* an empty argument slice;
* the wrong first argument;
* the runtime-information probe argument as the first argument;
* a missing envelope argument;
* a non-UTF-8 envelope argument;
* an oversized envelope;
* a missing delimiter;
* a delimiter in the wrong position;
* an extra value between the envelope and delimiter;
* invalid envelope JSON;
* source arguments placed before the delimiter.

A later occurrence of the invocation flag does not repair an invalid first argument.

Probe separation

Information probing remains:

--lexicon-runtime-information-v1

Normal execution remains:

--lexicon-invocation-v1 <json> -- <source-args...>

parse_runtime_invocation(...) must reject probe mode.

A later runner dispatcher will recognize probe mode before attempting normal invocation parsing.

Do not add that dispatcher now.

Typed encoding error

Define:

#[derive(Debug)]
pub enum RuntimeInvocationTransportEncodingError {
    Envelope(
        RuntimeInvocationEncodingError,
    ),
    EnvelopeTooLarge {
        maximum: usize,
        actual: usize,
    },
}

Equivalent naming is acceptable.

Typed decoding error

Define:

#[derive(Debug)]
pub enum RuntimeInvocationTransportDecodingError {
    MissingInvocationArgument,
    UnexpectedInvocationArgument {
        actual: OsString,
    },
    MissingEnvelope,
    EnvelopeNotUtf8,
    EnvelopeTooLarge {
        maximum: usize,
        actual: usize,
    },
    MissingDelimiter,
    UnexpectedDelimiter {
        actual: OsString,
    },
    Envelope(
        RuntimeInvocationDecodingError,
    ),
}

Equivalent typed organization is acceptable, but callers must distinguish:

* missing or wrong invocation argument;
* missing envelope;
* non-UTF-8 envelope;
* oversized envelope;
* missing or misplaced delimiter;
* envelope decoding failure.

Implement:

std::fmt::Display
std::error::Error

Use source() for nested envelope errors.

Do not return plain String, print arguments, or exit.

Sensitive error handling

Error formatting must not include:

* serialized envelope JSON;
* project identity;
* session identity;
* source argument values;
* raw non-UTF-8 bytes.

An error may identify the failed structural position without echoing its contents.

If the typed error retains an unexpected OsString, its Display implementation must not print that value.

No handler execution

Encoding and parsing must not:

* invoke acquisition;
* invoke resume;
* invoke processing;
* construct acquisition or processing contexts;
* create sessions;
* access the filesystem;
* launch a process.

Required tests

Add tests proving:

1. Acquisition/run encodes into the exact three-element internal prefix.
2. Acquisition/resume encodes correctly.
3. Processing/run encodes correctly.
4. No source arguments still produces the mandatory delimiter.
5. Ordinary source arguments are appended after the delimiter.
6. Parsing recovers the original acquisition envelope.
7. Parsing recovers the original processing envelope.
8. Parsing preserves source argument order.
9. Empty source argument values are preserved.
10. Duplicate source arguments are preserved.
11. A source argument equal to -- is preserved after the delimiter.
12. A source argument equal to the invocation flag is preserved after the delimiter.
13. A source argument equal to the probe flag is preserved after the delimiter.
14. Unicode source arguments round-trip.
15. Non-UTF-8 Unix source arguments round-trip byte-for-byte.
16. Empty input is rejected.
17. Wrong first argument is rejected.
18. Probe mode is rejected.
19. Missing envelope is rejected.
20. Non-UTF-8 envelope is rejected.
21. Oversized envelope is rejected during encoding.
22. Oversized envelope is rejected during parsing.
23. Missing delimiter is rejected.
24. Wrong delimiter position is rejected.
25. Extra internal values before the delimiter are rejected.
26. Invalid envelope JSON returns the nested typed error.
27. Processing/resume remains rejected through envelope decoding.
28. Error display does not reveal envelope JSON.
29. Error display does not reveal source arguments.
30. Encoding and parsing invoke no source handler.
31. Existing envelope JSON tests remain unchanged.
32. Existing runtime-information probe tests remain unchanged.
33. All workspace tests pass repeatedly.

Preserve existing behavior

Do not change:

* invocation-envelope JSON;
* in-memory envelope validation;
* runtime-information probing;
* source descriptors;
* runtime identities;
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

lexicon-framework remains library-only.

lexicon-bundle remains a binary installer compiled through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-core --quiet

Run the workspace suite twice:

cargo test --workspace --quiet
cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* subprocess launching;
* child runtime admission;
* envelope files;
* environment-variable transport;
* project-path transport;
* source-handler selection;
* resume-handler availability validation;
* managed runner generation;
* runner main.rs;
* runner::run;
* acquisition execution;
* processing execution;
* session creation or locking;
* HTTP transport;
* raw transaction recording;
* SQLite behavior;
* foreground supervision;
* background supervision;
* __operator-host;
* source build integration.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* canonical reserved argument constants;
* envelope size limit;
* encoding and parsing APIs;
* exact argv layout;
* typed encoding and decoding errors;
* malformed transport rejection results;
* source-argument preservation results;
* non-UTF-8 Unix round-trip result;
* probe/invocation separation;
* confirmation that error messages do not reveal arguments or envelope contents;
* proof that no source handler was invoked;
* Core and repeated workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not launch or execute a managed runtime.