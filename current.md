Current implementation milestone: Core child dispatch and handler execution

Objective

Complete the Core-owned in-process child runtime path for both HTTP acquisition and processing.

The completed foundation now includes:

RuntimeInvocationEnvelopeV1
→ argv encoding
→ argv parsing
→ HTTP acquisition admission
→ HTTP processing admission

This milestone adds:

child argv
→ probe-or-invocation dispatch
→ invocation parsing
→ operation-specific admission
→ exact handler invocation
→ typed completion or failure

Implement this as one cohesive milestone rather than splitting probe dispatch, acquisition execution, processing execution, and error propagation into separate micro-requests.

This milestone does not generate managed runner crates or launch subprocesses.

Required outcomes

After completion, Core must be able to:

1. Distinguish runtime-information probe mode from normal invocation mode.
2. Reject malformed top-level child arguments deterministically.
3. Parse a normal invocation through the existing transport API.
4. Admit an HTTP acquisition invocation through the existing HTTP admission API.
5. Admit an HTTP processing invocation through the existing processing admission API.
6. Invoke the exact handler selected by admission.
7. Make source-specific arguments available to the handler unchanged.
8. Preserve foreground/background, project identity, and session identity.
9. Return handler failures as typed errors.
10. Return handler success without printing or exiting.
11. Answer information probes without invoking a source handler.
12. Avoid filesystem, session, HTTP, SQLite, and subprocess behavior that belongs to later milestones.

Architectural boundary

Core owns:

* top-level child argument dispatch;
* probe-versus-normal-invocation selection;
* invocation parsing;
* operation selection;
* admission;
* selected-handler invocation;
* typed error propagation;
* handler completion reporting.

The caller still owns:

* collecting std::env::args_os();
* deciding how an executable maps a final result to an exit status;
* constructing or supplying any execution resources not yet defined by Core;
* project filesystem validation;
* session creation and locking;
* HTTP transport;
* raw transaction recording;
* SQLite creation;
* process supervision.

Do not put std::process::exit inside Core.

Module organization

Add or extend a Core runtime runner module, expected at:

lexicon-core/src/runtime/runner.rs

Export the public API through:

lexicon_core::runtime

Keep operation-specific execution adapters beside their established domains where useful:

lexicon-core/src/protocols/http/runner.rs
lexicon-core/src/processing/runner.rs

Equivalent organization is acceptable if it follows the repository’s current module structure.

Do not create an executable main.rs in this milestone.

Preserve the established admission APIs

Normal invocation execution must call the completed admission functions:

admit_http_runtime_invocation(...)
admit_processing_runtime_invocation(...)

Do not duplicate their validation logic inside the runner.

In particular, do not independently reimplement:

* compiled protocol validation;
* compiled operation validation;
* parent/child identity agreement;
* descriptor contract-version validation;
* HTTP capability validation;
* acquisition/resume handler selection;
* processing handler selection.

Admission remains the only public constructor of the admitted invocation values.

Top-level child dispatch

Define a typed dispatcher that receives the child argument slice excluding argv[0].

Representative API:

pub fn dispatch_runtime_arguments(
    arguments: &[OsString],
) -> Result<
    RuntimeArgumentDispatch,
    RuntimeArgumentDispatchError,
>;

Define an opaque or closed typed result equivalent to:

pub enum RuntimeArgumentDispatch {
    InformationProbe,
    Invocation(ParsedRuntimeInvocation),
}

The exact naming may follow the existing runtime module.

Dispatch rules

Apply these rules exactly:

1. If the argument slice is exactly:

["--lexicon-runtime-information-v1"]

return information-probe mode.

2. Otherwise, parse the entire slice through:

parse_runtime_invocation(arguments)

3. Do not search later arguments for either reserved flag.
4. Do not interpret source arguments after the invocation delimiter.
5. Do not accept extra arguments in information-probe mode.
6. Do not silently treat malformed probe mode as normal probe mode.
7. Do not reproduce invocation transport parsing inside the dispatcher.

Examples:

--lexicon-runtime-information-v1

is a probe.

--lexicon-invocation-v1 <json> -- ...

is a normal invocation.

--lexicon-runtime-information-v1 extra

is not a valid probe and must fail through a typed error path.

A source argument equal to the probe flag after the normal invocation delimiter remains an untouched source argument.

Runtime-specific execution entry points

Provide Core-owned execution entry points for:

* HTTP acquisition runtimes;
* HTTP processing runtimes.

Representative APIs:

pub fn run_http_acquisition_child(
    arguments: &[OsString],
    compiled_identity: RuntimeIdentity,
    source: &HttpSourceContractV1,
    available_capabilities: HttpCapabilitySet,
    execution: HttpAcquisitionExecutionInputs<'_>,
) -> Result<
    ChildRuntimeOutcome,
    HttpAcquisitionChildError,
>;
pub fn run_processing_child(
    arguments: &[OsString],
    compiled_identity: RuntimeIdentity,
    source: &ProcessingSourceContractV1,
    execution: ProcessingExecutionInputs<'_>,
) -> Result<
    ChildRuntimeOutcome,
    ProcessingChildError,
>;

These signatures are representative.

Use the repository’s established context and handler types rather than introducing parallel replacements.

If an existing handler requires a context reference directly, accept that context through the execution inputs.

If multiple established values are required to invoke a handler correctly, group them in a typed execution-input structure with private fields and checked construction where appropriate.

Do not use:

* Box<dyn Any>;
* stringly typed context maps;
* global mutable context;
* environment variables;
* JSON to pass in-process execution values;
* unchecked raw pointers.

Information-probe behavior

When the dispatcher selects information-probe mode:

1. Return the runtime information derived from the compiled identity and the established descriptor information.
2. Reuse the existing runtime-information model and encoding behavior.
3. Do not parse a normal invocation.
4. Do not perform admission.
5. Do not invoke acquisition, resume, or processing.
6. Do not inspect source arguments because probe mode has none.
7. Do not print the response inside the reusable Core function.
8. Do not exit.

The result should allow a later thin executable entrypoint to serialize or print the established probe response.

Do not change:

* the probe flag;
* runtime-information JSON;
* descriptor-derived runtime information;
* hashing or verification behavior.

Normal acquisition execution

For a normal HTTP acquisition invocation:

1. Parse through parse_runtime_invocation.
2. Admit through admit_http_runtime_invocation.
3. Match the admitted handler.
4. For AdmittedHttpHandler::Acquire, invoke the exact acquisition function pointer.
5. For AdmittedHttpHandler::Resume, invoke the exact resume function pointer.
6. Supply the admitted envelope and source arguments through the established handler/context boundary.
7. Preserve source arguments exactly as OsString values until the source implementation chooses to interpret them.
8. Convert the handler’s established return value into typed child completion or failure.
9. Invoke the selected handler exactly once.

Do not select the handler again from execution mode after admission. Admission’s selected handler is authoritative.

Do not silently substitute acquisition when resume was admitted.

Normal processing execution

For a normal processing invocation:

1. Parse through parse_runtime_invocation.
2. Admit through admit_processing_runtime_invocation.
3. Match AdmittedProcessingHandler::Process.
4. Invoke that exact registered processing function pointer.
5. Supply the admitted envelope and source arguments through the established handler/context boundary.
6. Preserve native source arguments until the processing implementation interprets them.
7. Convert the handler’s established return value into typed child completion or failure.
8. Invoke the selected handler exactly once.

Do not add processing resume behavior.

Processing/resume must remain rejected by the existing envelope model.

Source-argument delivery

This milestone must close the argument-preservation chain:

OS argv
→ ParsedRuntimeInvocation
→ admitted invocation
→ selected handler boundary

The handler must receive or be able to access the exact admitted source arguments.

Preserve:

* ordering;
* duplicates;
* empty values;
* Unicode values;
* non-UTF-8 Unix values;
* values beginning with -;
* values equal to --;
* values equal to --lexicon-invocation-v1;
* values equal to --lexicon-runtime-information-v1.

Do not:

* convert all arguments to String;
* parse with Clap before handler entry;
* remove the delimiter-looking source value;
* normalize or trim values;
* log arguments;
* persist arguments;
* expose them in error formatting.

If the current handler signature cannot receive source arguments without losing native values, make the smallest necessary typed signature adjustment across the descriptor, admission, and tests.

Do not redesign unrelated source contracts.

Document any handler-signature change explicitly in the completion report.

Context handling

Use existing acquisition and processing context types where they already exist.

This milestone may add the minimum Core-owned execution metadata necessary for a selected handler to observe:

* the preserved invocation envelope;
* source arguments;
* execution mode;
* supervision mode;
* project identity;
* session identity.

However, do not falsely construct operational contexts that claim unavailable resources exist.

Specifically, do not create placeholder:

* HTTP clients;
* raw transaction writers;
* SQLite connections;
* project roots;
* session locks.

If the established handler context currently requires one of those unavailable resources, separate invocation metadata from operational resources with a typed input boundary. The operational resource must be supplied by the caller in tests.

Do not perform the resource’s real behavior in this milestone.

Child outcome

Define a typed, non-printing result representing successful dispatch.

Representative form:

pub enum ChildRuntimeOutcome {
    RuntimeInformation(RuntimeInformationV1),
    InvocationCompleted {
        operation: RuntimeOperation,
        execution_mode: RuntimeExecutionMode,
    },
}

Equivalent naming is acceptable.

The successful invocation outcome must not contain:

* source arguments;
* envelope JSON;
* project paths;
* secret-bearing handler state.

Do not use a numeric exit code as Core’s primary result type.

A later executable entrypoint may map this typed outcome to output and exit status.

Typed errors

Define operation-specific top-level errors.

Representative forms:

#[derive(Debug)]
pub enum HttpAcquisitionChildError {
    Dispatch(RuntimeArgumentDispatchError),
    Transport(RuntimeInvocationTransportDecodingError),
    Admission(HttpRuntimeInvocationAdmissionError),
    Handler(HttpAcquisitionHandlerError),
}
#[derive(Debug)]
pub enum ProcessingChildError {
    Dispatch(RuntimeArgumentDispatchError),
    Transport(RuntimeInvocationTransportDecodingError),
    Admission(ProcessingRuntimeInvocationAdmissionError),
    Handler(ProcessingHandlerError),
}

Adjust nesting to avoid representing impossible duplicate dispatch/transport states.

Use existing handler error types if present.

If handlers currently return another established error type, preserve it as the nested source rather than converting it to String.

Implement:

std::fmt::Display
std::error::Error

Use source() for nested errors.

Do not:

* return plain String;
* print;
* log;
* exit;
* discard the underlying typed error category.

Handler failure behavior

A handler returning an ordinary failure must produce the corresponding typed Handler(...) child error.

It must not:

* be reported as admission failure;
* be reported as transport failure;
* be converted into success;
* trigger a second invocation;
* print source arguments.

Preserve the existing handler error model.

If the current handler contract returns Result<(), String>, replace that string error only if a repository-established typed handler error already exists or a minimal typed wrapper can be added without inventing execution semantics.

Do not broadly redesign all error types during this milestone.

Panic behavior

Do not silently swallow panics.

Use the repository’s established panic policy if one exists.

If no panic-conversion policy exists, let panics propagate from the reusable Core execution function and prove separately that ordinary typed handler failures are handled correctly.

Do not introduce an arbitrary catch_unwind boundary unless the existing runtime contract explicitly requires panic conversion.

The completion report must state the behavior used.

No process-level side effects

Core runner functions must not:

* call std::env::args_os() internally;
* print to stdout or stderr;
* call std::process::exit;
* spawn a process;
* create a runner executable;
* access a project directory;
* create a session;
* acquire a session lock;
* perform HTTP;
* record raw transactions;
* open SQLite;
* supervise foreground or background children.

Accept arguments and execution resources explicitly.

This keeps the Core path deterministic and directly testable.

Required tests

Add focused tests proving:

Dispatch

1. The exact single probe argument selects information mode.
2. Normal invocation arguments select invocation mode.
3. Empty arguments are rejected through the typed path.
4. Probe mode with an extra argument is rejected.
5. A wrong first argument is rejected.
6. A probe-looking source argument after the delimiter remains a source argument.
7. The dispatcher does not search later arguments for reserved flags.

Probe execution

8. Acquisition runtime probe returns the established runtime information.
9. Processing runtime probe returns the established runtime information.
10. Acquisition is not invoked during a probe.
11. Resume is not invoked during a probe.
12. Processing is not invoked during a probe.
13. Probe execution does not construct an operational context.
14. Probe execution does not inspect or expose invocation arguments.

Acquisition run

15. A matching acquisition/run invocation calls the exact acquisition handler.
16. The acquisition handler is called exactly once.
17. The resume handler is not called for run.
18. Foreground mode reaches the execution boundary unchanged.
19. Background mode reaches the execution boundary unchanged.
20. Project identity reaches the execution boundary unchanged.
21. Session identity reaches the execution boundary unchanged.
22. Source arguments reach the handler boundary in exact order.
23. Empty source values are preserved.
24. Reserved-looking values are preserved.
25. Non-UTF-8 Unix values are preserved byte-for-byte.
26. Acquisition success returns typed invocation completion.
27. Acquisition typed failure returns the nested handler error.

Acquisition resume

28. A matching acquisition/resume invocation calls the exact resume handler.
29. The resume handler is called exactly once.
30. The acquisition handler is not called for resume.
31. Resume receives the unchanged invocation metadata.
32. Resume receives unchanged source arguments.
33. Resume typed failure returns the nested handler error.
34. Missing resume remains an admission error and invokes neither handler.

Processing

35. A matching processing/run invocation calls the exact processing handler.
36. The processing handler is called exactly once.
37. Processing receives foreground supervision unchanged.
38. Processing receives background supervision unchanged.
39. Processing receives project identity unchanged.
40. Processing receives session identity unchanged.
41. Processing receives source arguments in exact order.
42. Processing preserves empty and reserved-looking values.
43. Processing preserves non-UTF-8 Unix values byte-for-byte.
44. Processing success returns typed invocation completion.
45. Processing typed failure returns the nested handler error.
46. Processing/resume remains rejected before handler invocation.

Validation and side-effect boundaries

47. Transport failure invokes no handler.
48. Identity failure invokes no handler.
49. Descriptor-version failure invokes no handler.
50. Missing HTTP capabilities invoke no handler.
51. Wrong compiled operation invokes no handler.
52. Handler failures do not cause reinvocation.
53. Error formatting does not expose source arguments.
54. Error formatting does not expose envelope JSON.
55. Error formatting does not expose project identity.
56. Error formatting does not expose session identity.
57. Core runner functions do not print or exit.
58. Core runner functions do not access files or launch processes.
59. Existing invocation transport tests remain unchanged.
60. Existing acquisition admission tests remain unchanged.
61. Existing processing admission tests remain unchanged.
62. Existing runtime-information tests remain unchanged.

Use counters, captured execution inputs, or panic handlers to prove exact invocation behavior.

Do not use sleeps.

Do not serialize the entire test suite behind a global lock.

Preserve existing behavior

Do not change:

* invocation-envelope JSON;
* argv encoding;
* argv parsing;
* reserved argument constants;
* envelope size limits;
* acquisition admission validation order;
* processing admission validation order;
* runtime identity semantics;
* source contract versions;
* capability identifiers;
* HTTP capability comparison;
* resume registration semantics;
* runtime-information JSON;
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
* installed paths.

A minimal handler-signature adjustment is allowed only if required to preserve native source arguments through actual invocation. Report it explicitly.

Validation

Run the complete Core suite:

cargo test -p lexicon-core --quiet

Run it a second time to catch test-state leakage:

cargo test -p lexicon-core --quiet

Do not run:

cargo test --workspace

Workspace-wide testing is intentionally excluded from this milestone.

Do not run the bundle/install pipeline.

This milestone does not modify bundling, installation, the framework build pipeline, or MZA, so those validations are outside its scope.

If targeted tests are useful during implementation, run them freely, but the completion report must include both complete lexicon-core test results.

Explicit exclusions

Do not implement:

* executable main.rs;
* managed runner crate generation;
* source workspace generation;
* source scaffold migration;
* subprocess launching;
* parent-side child supervision;
* source build integration;
* project filesystem validation;
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
* SQLite creation;
* processing database behavior;
* foreground supervision;
* background supervision;
* __operator-host;
* data --get;
* data --process;
* lexicon build;
* MZA or installer changes.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* child dispatch API;
* exact probe-versus-invocation dispatch rules;
* acquisition child execution API;
* processing child execution API;
* execution-input representations;
* child outcome representation;
* typed error hierarchy;
* acquisition/run invocation result;
* acquisition/resume invocation result;
* processing/run invocation result;
* source-argument delivery result;
* non-UTF-8 Unix preservation result;
* probe results;
* proof that probes invoke no handlers;
* proof that admitted handlers are invoked exactly once;
* ordinary handler failure behavior;
* panic behavior;
* confirmation that Core does not print or exit;
* confirmation that no filesystem, HTTP, SQLite, or subprocess behavior was added;
* first complete lexicon-core test result;
* second complete lexicon-core test result;
* confirmation that workspace tests and bundle/install validation were intentionally not run.

Then stop.