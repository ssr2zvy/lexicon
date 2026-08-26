Current implementation milestone: Core normal-invocation execution for acquisition and processing

Supersession notice

This request completely replaces the previous “Core child dispatch and handler execution” milestone.

Discard unfinished work from that prior request if it introduced or proposed:

* a generic lexicon-core/src/runtime/runner.rs;
* RuntimeArgumentDispatch;
* ChildRuntimeOutcome;
* generic execution-input containers;
* replacement probe APIs;
* replacement handler error types;
* changes to established handler signatures.

Preserve any unfinished changes only if they directly implement the repository-aligned APIs and behavior defined below.

Objective

Complete Core-owned normal-invocation execution for both:

* HTTP acquisition, including run and resume;
* HTTP processing run.

The completed path is:

RuntimeInvocationEnvelopeV1
→ argv transport
→ ParsedRuntimeInvocation
→ operation-specific admission

This milestone adds:

normal invocation argv
→ existing parser
→ existing operation-specific admission
→ exact admitted handler invocation
→ existing typed handler result

This milestone does not redesign runtime-information probing.

It does not create executable entrypoints, construct operational filesystem state, launch processes, or implement HTTP/SQLite behavior.

Repository-grounded existing APIs

The repository already defines the following exact handler types:

pub type HttpAcquireFn =
    fn(
        &mut HttpAcquisitionContext,
        &[OsString],
    ) -> AcquisitionResult<()>;
pub type HttpResumeFn =
    fn(
        &mut HttpAcquisitionContext,
        &[OsString],
    ) -> AcquisitionResult<()>;
pub type ProcessDataFn =
    fn(
        &mut ProcessingContext,
        &[OsString],
    ) -> ProcessingResult<()>;

The repository already defines:

HttpSourceContractV1
ProcessingSourceContractV1
AdmittedHttpHandler
AdmittedHttpRuntimeInvocation
HttpRuntimeInvocationAdmissionError
admit_http_runtime_invocation(...)
AdmittedProcessingHandler
AdmittedProcessingRuntimeInvocation
ProcessingRuntimeInvocationAdmissionError
admit_processing_runtime_invocation(...)
AcquisitionError
AcquisitionResult<T>
ProcessingError
ProcessingResult<T>

Use these exact established types.

Do not add alternate handler types or change the handler signatures.

Existing runner modules

Extend the existing modules:

lexicon-core/src/protocols/http/runner.rs
lexicon-core/src/processing/runner.rs

Export the new execution APIs through their existing namespaces:

lexicon_core::http
lexicon_core::processing

Do not create:

lexicon-core/src/runtime/runner.rs

Do not introduce a generic cross-operation runner abstraction in this milestone.

Preserve existing probe behavior exactly

The HTTP runner already defines:

try_write_runtime_information_probe(...)
RuntimeInformationProbeOutcome
RuntimeInformationProbeError

The processing runner already defines:

try_write_runtime_information_probe(...)
ProcessingRuntimeInformationProbeOutcome
ProcessingRuntimeInformationProbeError

Preserve these APIs and their behavior unchanged.

In particular, preserve that they:

* recognize only the exact probe argument;
* reject additional probe arguments;
* return NotRequested for normal invocation arguments;
* write the established JSON document and newline through a supplied std::io::Write;
* flush the supplied writer;
* return typed encoding, construction, and output errors;
* do not invoke source handlers.

Do not replace the existing writer-based design with a returned information object.

Do not merge acquisition and processing runtime-information types.

The existing intended future executable sequence remains:

try operation-specific information probe
→ if Written: stop successfully
→ if NotRequested: execute normal invocation

This milestone implements the second step only.

HTTP normal-invocation execution API

Add an operation-specific function equivalent to:

pub fn run_http_runtime_invocation(
    arguments: &[OsString],
    compiled_identity: RuntimeIdentity,
    source: &HttpSourceContractV1,
    available_capabilities: HttpCapabilitySet,
    context: &mut HttpAcquisitionContext,
) -> Result<
    (),
    HttpRuntimeInvocationExecutionError,
>;

Equivalent naming is acceptable, but the function must clearly mean normal HTTP acquisition invocation execution.

The supplied argument slice excludes executable argv[0].

This function handles normal invocation transport only.

It must not recognize or write runtime-information probes. Passing probe arguments to it must fail through the existing invocation parser’s typed unexpected-invocation-argument path.

Exact HTTP execution sequence

Perform these actions in order:

1. Parse arguments using:

parse_runtime_invocation(arguments)

2. Admit the parsed invocation using:

admit_http_runtime_invocation(
    parsed,
    compiled_identity,
    source,
    available_capabilities,
)

3. Read the selected handler from the admitted value.
4. Pass the supplied mutable HttpAcquisitionContext.
5. Pass the admitted source-argument slice exactly.
6. Invoke the selected handler exactly once.
7. Return Ok(()) when the handler succeeds.
8. Return a typed nested handler error when it fails.

For:

AdmittedHttpHandler::Acquire(handler)

call that exact handler.

For:

AdmittedHttpHandler::Resume(handler)

call that exact handler.

Do not select the handler again from the envelope’s execution mode.

The handler selected by admission is authoritative.

HTTP context behavior

Use the &mut HttpAcquisitionContext supplied by the caller.

Do not call:

HttpAcquisitionContext::from_env()

inside the new invocation-execution function.

Do not read:

LEXICON_SOURCE_DIRECTORY

Do not construct paths from project or session identity.

Do not validate that context paths exist.

Do not alter the existing legacy HttpAcquisitionContext::from_env() or run_http_source(...) behavior in this milestone.

Those older APIs remain unchanged until a later migration explicitly replaces them.

Processing normal-invocation execution API

Add an operation-specific function equivalent to:

pub fn run_processing_runtime_invocation(
    arguments: &[OsString],
    compiled_identity: RuntimeIdentity,
    source: &ProcessingSourceContractV1,
    context: &mut ProcessingContext,
) -> Result<
    (),
    ProcessingRuntimeInvocationExecutionError,
>;

Equivalent naming is acceptable.

The supplied argument slice excludes executable argv[0].

This function handles normal invocation transport only.

It must not recognize or write runtime-information probes.

Exact processing execution sequence

Perform these actions in order:

1. Parse arguments using:

parse_runtime_invocation(arguments)

2. Admit the parsed invocation using:

admit_processing_runtime_invocation(
    parsed,
    compiled_identity,
    source,
)

3. Read the exact selected processing handler.
4. Pass the supplied mutable ProcessingContext.
5. Pass the admitted source-argument slice exactly.
6. Invoke the selected handler exactly once.
7. Return Ok(()) when the handler succeeds.
8. Return a typed nested ProcessingError when it fails.

For:

AdmittedProcessingHandler::Process(handler)

call that exact handler.

Do not retrieve the handler again from:

source.process_handler()

after admission.

The admitted handler is authoritative.

Processing context behavior

Use the &mut ProcessingContext supplied by the caller.

The current processing context is an intentionally minimal placeholder:

#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessingContext {
    _private: (),
}

Do not add SQLite, filesystem, raw-transaction, project, or session behavior to it.

Because ProcessingContext already implements Default, tests and later callers can construct it using:

ProcessingContext::default()

Do not add a duplicate constructor solely for this milestone.

Preserve the existing test-only constructor if present.

Source-argument preservation

The existing handler signatures already accept:

&[OsString]

Therefore, no handler-signature change is required or permitted.

The exact preservation chain must be:

OS arguments
→ parse_runtime_invocation
→ ParsedRuntimeInvocation
→ admitted invocation
→ selected handler

Preserve:

* empty argument lists;
* ordering;
* duplicates;
* empty values;
* Unicode values;
* non-UTF-8 Unix values;
* values beginning with -;
* values equal to --;
* values equal to --lexicon-invocation-v1;
* values equal to --lexicon-runtime-information-v1.

After the mandatory transport delimiter has been consumed, the execution function must not interpret source values.

Do not:

* convert them to String;
* parse them with Clap;
* trim them;
* normalize them;
* redact them;
* remove reserved-looking values;
* reorder or deduplicate them;
* persist or log them.

HTTP typed execution error

Define:

#[derive(Debug)]
pub enum HttpRuntimeInvocationExecutionError {
    Transport(
        RuntimeInvocationTransportDecodingError,
    ),
    Admission(
        HttpRuntimeInvocationAdmissionError,
    ),
    Handler(
        AcquisitionError,
    ),
}

Equivalent naming is acceptable.

Implement:

std::fmt::Display
std::error::Error

Use source() for all nested errors.

Map errors without converting them to strings:

parse failure     → Transport
admission failure → Admission
handler failure   → Handler

Do not add a new HttpAcquisitionHandlerError; the established handler error is AcquisitionError.

Processing typed execution error

Define:

#[derive(Debug)]
pub enum ProcessingRuntimeInvocationExecutionError {
    Transport(
        RuntimeInvocationTransportDecodingError,
    ),
    Admission(
        ProcessingRuntimeInvocationAdmissionError,
    ),
    Handler(
        ProcessingError,
    ),
}

Equivalent naming is acceptable.

Implement:

std::fmt::Display
std::error::Error

Use source() for all nested errors.

Map errors without converting them to strings:

parse failure     → Transport
admission failure → Admission
handler failure   → Handler

Do not add a replacement processing handler error type.

Sensitive error handling

Execution error formatting must not reveal:

* source arguments;
* envelope JSON;
* project identity;
* session identity;
* filesystem context paths.

The nested admission and transport errors already have established sanitization behavior. Preserve it.

For handler errors, preserve their existing Display behavior:

* AcquisitionError displays its existing source message;
* ProcessingError displays its existing generic processing failure message.

Do not include source arguments or envelope values when adding execution-layer context.

Handler invocation behavior

A successfully admitted handler must be called exactly once.

An ordinary handler failure must:

* return the corresponding typed Handler(...) error;
* not cause a retry;
* not invoke the other handler;
* not be converted to admission failure;
* not be converted to transport failure;
* not print or exit.

For acquisition/run:

* acquisition is called once;
* resume is not called.

For acquisition/resume:

* resume is called once;
* acquisition is not called.

For processing/run:

* processing is called once.

Panic behavior

Do not introduce catch_unwind.

A source-handler panic continues to unwind according to normal Rust behavior.

Tests may use panic handlers to prove that pre-handler failures do not invoke handlers, but the execution API must not silently catch or convert handler panics.

Document this preserved panic behavior in the completion report.

No process-level behavior

The new normal-invocation execution functions must not:

* call std::env::args_os();
* recognize probe mode;
* write probe output;
* print to stdout or stderr;
* call std::process::exit;
* launch a subprocess;
* access environment variables;
* access files;
* construct project paths;
* create sessions;
* lock sessions;
* perform HTTP;
* record raw transactions;
* open SQLite databases;
* supervise foreground or background execution.

All inputs must be supplied explicitly.

Required HTTP tests

Add tests proving:

1. A matching acquisition/run invocation calls the exact acquisition handler.
2. Acquisition/run calls the handler exactly once.
3. Acquisition/run does not call resume.
4. A matching acquisition/resume invocation calls the exact resume handler.
5. Acquisition/resume calls the handler exactly once.
6. Acquisition/resume does not call acquisition.
7. The exact supplied HttpAcquisitionContext reaches acquisition.
8. The exact supplied HttpAcquisitionContext reaches resume.
9. A handler can mutate the supplied HTTP context.
10. Foreground invocation is accepted and reaches the selected handler.
11. Background invocation is accepted and reaches the selected handler.
12. Project identity remains preserved through admission and execution.
13. Session identity remains preserved through admission and execution.
14. Source arguments reach acquisition in exact order.
15. Source arguments reach resume in exact order.
16. Duplicate source arguments are preserved.
17. Empty source values are preserved.
18. A source value equal to -- is preserved.
19. A source value equal to the invocation flag is preserved.
20. A source value equal to the probe flag is preserved.
21. Unicode source values are preserved.
22. Non-UTF-8 Unix source arguments reach acquisition byte-for-byte.
23. Non-UTF-8 Unix source arguments reach resume byte-for-byte.
24. Acquisition success returns Ok(()).
25. Resume success returns Ok(()).
26. Acquisition failure returns HttpRuntimeInvocationExecutionError::Handler.
27. Resume failure returns HttpRuntimeInvocationExecutionError::Handler.
28. Handler failures do not cause reinvocation.
29. Malformed transport returns the nested transport error.
30. Probe arguments passed to normal invocation execution return a transport error.
31. Identity mismatch returns the nested admission error.
32. Missing HTTP capabilities return the nested admission error.
33. Missing resume returns the nested admission error.
34. Wrong compiled operation returns the nested admission error.
35. Transport failure invokes neither acquisition nor resume.
36. Admission failure invokes neither acquisition nor resume.
37. Error formatting does not expose source arguments.
38. Error formatting does not expose envelope JSON.
39. The execution function does not call HttpAcquisitionContext::from_env().
40. Existing HTTP probe tests remain unchanged.

Required processing tests

Add tests proving:

1. A matching processing/run invocation calls the exact processing handler.
2. Processing calls the handler exactly once.
3. The exact supplied ProcessingContext reaches the handler.
4. A handler can mutate or otherwise use the supplied mutable context according to its current public behavior.
5. Foreground invocation is accepted and reaches processing.
6. Background invocation is accepted and reaches processing.
7. Project identity remains preserved through admission and execution.
8. Session identity remains preserved through admission and execution.
9. Source arguments reach processing in exact order.
10. Duplicate source arguments are preserved.
11. Empty source values are preserved.
12. A source value equal to -- is preserved.
13. A source value equal to the invocation flag is preserved.
14. A source value equal to the probe flag is preserved.
15. Unicode source values are preserved.
16. Non-UTF-8 Unix source arguments reach processing byte-for-byte.
17. Processing success returns Ok(()).
18. Processing failure returns ProcessingRuntimeInvocationExecutionError::Handler.
19. Processing failure does not cause reinvocation.
20. Malformed transport returns the nested transport error.
21. Probe arguments passed to normal invocation execution return a transport error.
22. Identity mismatch returns the nested admission error.
23. Wrong compiled operation returns the nested admission error.
24. Descriptor-version mismatch returns the nested admission error.
25. Processing/resume remains rejected before handler invocation.
26. Transport failure does not invoke processing.
27. Admission failure does not invoke processing.
28. Error formatting does not expose source arguments.
29. Error formatting does not expose envelope JSON.
30. Existing processing probe tests remain unchanged.

Existing behavior tests

Confirm that these existing areas remain green without rewriting them:

* runtime invocation transport;
* HTTP invocation admission;
* processing invocation admission;
* HTTP source contract;
* processing source contract;
* HTTP runtime-information probe;
* processing runtime-information probe;
* HTTP runtime information;
* processing runtime information;
* capability comparison;
* compile-fail handler-signature tests.

Do not remove or weaken existing tests.

Do not mark tests ignored.

Do not add sleeps or global test serialization.

Preserve existing behavior

Do not change:

* RuntimeInvocationEnvelopeV1;
* invocation JSON;
* argv encoding;
* argv parsing;
* reserved argument constants;
* envelope size limits;
* HTTP admission validation order;
* processing admission validation order;
* handler function signatures;
* HttpSourceContractV1;
* ProcessingSourceContractV1;
* HttpAcquisitionContext::from_env();
* run_http_source(...);
* acquisition error semantics;
* processing error semantics;
* probe APIs;
* probe JSON output;
* probe newline and flush behavior;
* runtime-information types;
* runtime identity semantics;
* capability identifiers;
* resume registration;
* hashing;
* verification;
* manifests;
* staging;
* bundle admission;
* paired publication;
* source scaffolding;
* source create;
* source build;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior.

Validation

Run the complete Core suite twice:

cargo test -p lexicon-core --quiet
cargo test -p lexicon-core --quiet

Do not run:

cargo test --workspace

Workspace-wide validation is intentionally excluded.

Do not run the bundle/install pipeline.

This milestone changes only lexicon-core normal-invocation execution.

Explicit exclusions

Do not implement:

* a generic runtime dispatcher;
* a new generic runtime runner module;
* executable main.rs;
* std::env::args_os() collection;
* process exit-code mapping;
* managed runner generation;
* generated runner crates;
* source workspace migration;
* source scaffolding migration;
* subprocess launching;
* parent-side supervision;
* project filesystem validation;
* environment-variable invocation transport;
* session creation;
* session locking;
* session reconciliation;
* checkpoints;
* HTTP transport;
* redirects;
* retries;
* redaction;
* raw transaction recording;
* raw-data discovery;
* SQLite behavior;
* foreground supervision;
* background supervision;
* __operator-host;
* source build integration;
* data --get;
* data --process;
* lexicon build;
* MZA or installer changes.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* HTTP normal-invocation execution API;
* processing normal-invocation execution API;
* exact execution order for each;
* exact existing handler types reused;
* exact existing context types reused;
* typed execution error representations;
* acquisition/run result;
* acquisition/resume result;
* processing/run result;
* exact-once invocation results;
* source-argument delivery results;
* non-UTF-8 Unix preservation results;
* ordinary handler failure results;
* panic behavior;
* probe preservation results;
* confirmation that normal execution rejects probe arguments through transport parsing;
* confirmation that no generic dispatcher was added;
* confirmation that no handler signature changed;
* confirmation that no environment, filesystem, HTTP, SQLite, printing, exit, or subprocess behavior was added;
* first complete lexicon-core test result;
* second complete lexicon-core test result;
* confirmation that workspace and bundle/install tests were intentionally not run.

Then stop.