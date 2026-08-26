Current implementation request: Core child admission for HTTP processing invocations

Objective

Add the Core operation that validates a parsed normal invocation against the HTTP processing runtime’s compiled identity and processing source descriptor.

After successful admission, Core must know:

* the compiled runtime is for the HTTP protocol;
* the compiled runtime operation is processing;
* the envelope identifies this exact compiled runtime;
* the source contract version matches the processing descriptor contract;
* the invocation requests the supported processing execution mode;
* the exact processing handler has been selected;
* source arguments remain untouched.

This step selects a typed processing handler but does not invoke it.

Architectural position

The completed normal-invocation path is:

RuntimeInvocationEnvelopeV1
→ JSON
→ argv transport
→ ParsedRuntimeInvocation

HTTP acquisition child admission now adds:

ParsedRuntimeInvocation
+ compiled acquisition RuntimeIdentity
+ HTTP acquisition source contract
+ available HTTP capabilities
→ AdmittedHttpRuntimeInvocation

This micro-step adds the corresponding processing boundary:

ParsedRuntimeInvocation
+ compiled processing RuntimeIdentity
+ processing source contract
→ AdmittedProcessingRuntimeInvocation

A later Core-owned processing runner will invoke the selected handler.

Do not implement that runner in this step.

Use the established processing types

Use the processing descriptor and handler types already defined by lexicon-core.

The representative names in this request are:

ProcessingSourceContractV1
ProcessingFn

If the repository uses equivalent established names, use those exact existing names.

Do not:

* create a second processing descriptor model;
* rename existing public processing APIs;
* alter the existing processing handler signature;
* add acquisition capabilities to processing;
* introduce a new processing capability system.

Required module

Create:

lexicon-core/src/processing/invocation.rs

If processing is currently organized beneath another established module path, place the implementation beside the existing processing descriptor and follow that module’s organization.

Export the public API through the existing processing namespace, expected to be:

lexicon_core::processing

Do not export processing admission through lexicon_core::http merely because the source protocol is HTTP.

Do not add a runner main.rs.

Selected processing handler

Define:

#[derive(Clone, Copy)]
pub enum AdmittedProcessingHandler {
    Process(ProcessingFn),
}

Equivalent naming consistent with the existing processing API is acceptable.

Provide:

impl AdmittedProcessingHandler {
    pub const fn execution_mode(
        &self,
    ) -> RuntimeExecutionMode;
}

For the processing handler, this must return:

RuntimeExecutionMode::Run

Do not add a method that invokes the function.

The exact function pointer must remain inspectable through typed enum matching so tests and the later runner can verify which registered handler was selected.

Do not erase the handler behind:

* Box<dyn Fn...>;
* a string name;
* an integer identifier;
* an untyped pointer;
* a closure allocated during admission.

Admitted processing invocation

Define:

#[derive(Clone)]
pub struct AdmittedProcessingRuntimeInvocation {
    envelope: RuntimeInvocationEnvelopeV1,
    source_arguments: Vec<OsString>,
    handler: AdmittedProcessingHandler,
}

Keep all fields private.

Provide:

impl AdmittedProcessingRuntimeInvocation {
    pub fn envelope(
        &self,
    ) -> &RuntimeInvocationEnvelopeV1;
    pub fn source_arguments(
        &self,
    ) -> &[OsString];
    pub const fn handler(
        &self,
    ) -> AdmittedProcessingHandler;
    pub fn into_parts(
        self,
    ) -> (
        RuntimeInvocationEnvelopeV1,
        Vec<OsString>,
        AdmittedProcessingHandler,
    );
}

Do not provide a public unchecked constructor.

Only the admission function may construct this value.

Do not add fields for:

* acquisition capabilities;
* filesystem paths derived from project identity;
* open files;
* SQLite connections;
* sessions or locks;
* processing context;
* supervision state duplicated outside the envelope.

The complete envelope remains the canonical preserved invocation metadata.

Admission API

Provide:

pub fn admit_processing_runtime_invocation(
    parsed: ParsedRuntimeInvocation,
    compiled_identity: RuntimeIdentity,
    source: &ProcessingSourceContractV1,
) -> Result<
    AdmittedProcessingRuntimeInvocation,
    ProcessingRuntimeInvocationAdmissionError,
>;

Use the established processing source-contract type if its exact name differs.

The parsed invocation is consumed so its native source arguments can move into the admitted value without conversion or normalization.

Required validation order

Perform the checks in this exact deterministic order.

1. Compiled protocol

Require:

compiled_identity.protocol()
    == RuntimeProtocol::Http

The processing runtime belongs to a source scoped to the HTTP protocol even though processing itself performs no HTTP acquisition.

Return a typed compiled-protocol error otherwise.

Do not perform this check by comparing debug strings or serialized values.

2. Compiled operation

Require:

compiled_identity.operation()
    == RuntimeOperation::Processing

Return a typed compiled-operation error otherwise.

In particular, an otherwise matching HTTP acquisition identity must be rejected here.

3. Parent/child identity agreement

Require:

parsed.envelope().runtime()
    == compiled_identity

This full equality comparison includes:

* source identity;
* protocol;
* operation;
* source contract version.

Return IdentityMismatch containing the compiled identity and envelope identity.

Do not compare only the source name or only the operation.

4. Descriptor contract version

Require:

compiled_identity.source_contract_version()
    == ProcessingSourceContractV1::CONTRACT_VERSION

Use the established processing contract-version constant if named differently.

Return a typed descriptor-version mismatch otherwise.

This check establishes that the compiled identity claims the exact processing source-contract version represented by the descriptor passed to admission.

Do not:

* silently accept older versions;
* coerce versions;
* fall back to an acquisition contract version;
* derive the accepted version from the envelope alone.

5. Execution-mode guard

Processing currently supports:

RuntimeExecutionMode::Run

Require the admitted envelope to request that mode.

A processing/resume envelope should already be impossible to construct or decode through the established RuntimeInvocationEnvelopeV1 validation.

Nevertheless, admission must not use an unreachable wildcard that would silently accept a future unsupported processing execution mode.

Use exhaustive matching where the current enum permits it.

If the existing type system makes a non-run processing envelope impossible, preserve that guarantee and do not weaken envelope validation merely to manufacture an admission error.

Do not add processing resume support.

6. Handler selection

Select the exact processing handler registered in the processing source contract:

source.process_handler()

Wrap it as:

AdmittedProcessingHandler::Process(...)

Use the existing accessor’s exact established name if different.

Do not invoke the selected function.

If the established processing descriptor structurally guarantees that a processing handler exists, retain that guarantee. Do not change it into an optional handler merely to mirror HTTP resume behavior.

Supervision mode

Both supervision values are admissible:

RuntimeSupervisionMode::Foreground
RuntimeSupervisionMode::Background

The selected processing handler is unaffected by supervision mode.

Preserve the supervision mode inside the envelope exactly.

Do not implement foreground or background supervision here.

Project and session identities

The parsed envelope has already structurally validated project and session identity values.

Admission must preserve them exactly but perform no:

* project-directory lookup;
* manifest validation;
* project ownership check;
* session-directory creation;
* session-state transition;
* session locking;
* raw-data discovery;
* processed-data path construction.

Those operations belong to later execution and session integration.

Source arguments

Move source arguments from ParsedRuntimeInvocation into the admitted processing value exactly.

Do not:

* require UTF-8;
* parse them with Clap;
* normalize them;
* redact them;
* log them;
* reorder them;
* deduplicate them;
* remove empty values;
* treat -- specially after transport parsing;
* remove Lexicon-looking reserved values;
* persist them.

The processing implementation will interpret its own source-specific arguments only after the later runner invokes the handler.

Typed admission error

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessingRuntimeInvocationAdmissionError {
    WrongCompiledProtocol {
        actual: RuntimeProtocol,
    },
    WrongCompiledOperation {
        actual: RuntimeOperation,
    },
    IdentityMismatch {
        compiled: RuntimeIdentity,
        envelope: RuntimeIdentity,
    },
    DescriptorContractVersionMismatch {
        identity_version: u32,
        descriptor_version: u32,
    },
}

Equivalent naming consistent with the processing module is acceptable.

Do not add capability or missing-handler variants unless the existing processing contract genuinely requires them.

Implement:

std::fmt::Display
std::error::Error

Do not:

* return plain String;
* print errors;
* exit;
* log the envelope;
* expose source arguments.

Sensitive error handling

Error formatting must not reveal:

* source arguments;
* session identity;
* project identity;
* serialized envelope JSON;
* filesystem paths derived from invocation metadata.

Runtime identities may be represented only through their established non-secret identifiers and existing safe diagnostic behavior.

If displaying the full RuntimeIdentity would reveal project or session information under the actual type model, format only the established source/protocol/operation/version identifiers.

Tests must verify that error text does not contain project identity, session identity, envelope JSON, or source argument values.

No handler invocation

Admission must not:

* call the processing handler;
* construct a processing context;
* create or open SQLite databases;
* enumerate raw transactions;
* read request or response bodies;
* access project files;
* create sessions;
* acquire locks;
* perform HTTP;
* launch processes.

Use a processing handler that panics or increments a counter if called to prove admission only stores the function pointer.

A successful test must inspect the selected pointer without invoking it.

Function-pointer verification

Tests must prove that admission selects the exact registered processing function pointer.

Compare function pointers using the repository’s established safe test technique.

For example, if compatible with the actual function type:

let selected = match admitted.handler() {
    AdmittedProcessingHandler::Process(handler) => handler,
};
assert!(std::ptr::fn_addr_eq(
    selected,
    expected_processing_handler,
));

Use std::ptr::fn_addr_eq rather than relying on raw function-pointer equality if the compiler warns that direct equality is unreliable.

Do not call either function to establish identity.

Required tests

Add tests proving:

1. A matching HTTP processing/run invocation is admitted.
2. Admission selects the exact registered processing function pointer.
3. The admitted handler reports RuntimeExecutionMode::Run.
4. Foreground supervision mode is preserved.
5. Background supervision mode is preserved.
6. Project identity is preserved.
7. Session identity is preserved.
8. The complete envelope is preserved.
9. Source arguments remain in exact order.
10. Duplicate source arguments are preserved.
11. Empty source argument values are preserved.
12. A source value equal to -- is preserved.
13. A source value equal to --lexicon-invocation-v1 is preserved.
14. A source value equal to --lexicon-runtime-information-v1 is preserved.
15. Unicode source arguments are preserved.
16. Non-UTF-8 Unix source arguments are preserved byte-for-byte.
17. A wrong compiled protocol returns the typed protocol error when a real alternate protocol already exists.
18. An acquisition compiled identity returns WrongCompiledOperation.
19. An envelope/compiled source mismatch returns IdentityMismatch.
20. An envelope/compiled protocol mismatch returns IdentityMismatch when representable after the compiled-protocol guard.
21. An envelope/compiled operation mismatch returns IdentityMismatch.
22. An envelope/compiled contract-version mismatch returns IdentityMismatch.
23. A compiled processing descriptor-version mismatch returns the typed descriptor-version error.
24. The compiled-operation check occurs before identity comparison.
25. Identity agreement is checked before descriptor-version validation.
26. Processing admission does not invoke the selected processing handler.
27. A failed admission does not invoke the processing handler.
28. A failed admission cannot construct an admitted value.
29. Error formatting does not expose source arguments.
30. Error formatting does not expose project identity.
31. Error formatting does not expose session identity.
32. Error formatting does not expose envelope JSON.
33. Processing/resume remains rejected by the existing envelope model.
34. Existing invocation transport tests remain unchanged.
35. Existing HTTP acquisition-admission tests remain unchanged.
36. Existing HTTP descriptor and capability tests remain unchanged.
37. Existing processing descriptor tests remain unchanged.
38. Existing runtime-information probe tests remain unchanged.
39. All workspace tests pass repeatedly.

If a particular mismatch is impossible because earlier typed constructors enforce it, test the earliest public boundary where it is rejected. Do not weaken constructors or add unchecked constructors solely to reach an impossible state.

Do not add a fake protocol solely to test the wrong-protocol branch.

If no real alternate RuntimeProtocol currently exists, document that the branch is exhaustively implemented but cannot be directly constructed in a test. Do not expand the public protocol enum for test coverage.

Validation-order tests

Where multiple conditions are invalid simultaneously, prove deterministic precedence.

At minimum:

* an acquisition compiled identity paired with a mismatching envelope returns WrongCompiledOperation, not IdentityMismatch;
* a compiled identity mismatching the envelope and descriptor version returns IdentityMismatch, not DescriptorContractVersionMismatch;
* a matching compiled/envelope identity using a non-current processing contract version returns DescriptorContractVersionMismatch.

Do not change production types merely to make invalid combinations easier to construct.

Preserve existing behavior

Do not change:

* invocation-envelope JSON;
* argv transport;
* source-argument splitting;
* HTTP acquisition admission;
* source descriptor signatures;
* processing handler signatures;
* capability identifiers;
* HTTP resume registration;
* runtime identity semantics;
* runtime-information probes;
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

Do not refactor the completed HTTP admission implementation except for a minimal shared helper that is clearly justified and leaves its public API and validation behavior unchanged.

Prefer a direct processing implementation over premature generic admission abstraction.

Validation

Run:

cargo test -p lexicon-core --quiet

Run the workspace suite twice:

cargo test --workspace --quiet
cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If it is unavailable, report the known external blocker separately.

Do not modify MZA, bundling, or installer code to make that optional validation available.

Explicit exclusions

Do not implement:

* processing handler invocation;
* acquisition handler invocation;
* runner::run;
* runner main.rs;
* managed runner generation;
* process launching;
* project filesystem validation;
* session creation or locking;
* raw transaction discovery;
* HTTP execution;
* raw recording;
* processing context construction;
* SQLite connection creation;
* SQLite schema management;
* processed-output publication;
* foreground supervision;
* background supervision;
* __operator-host;
* source workspace migration;
* source scaffolding migration;
* source build integration;
* data --get;
* data --process;
* lexicon build.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the processing admission API;
* admitted invocation and handler representations;
* exact validation order;
* compiled-protocol guard;
* compiled-operation guard;
* parent/child identity behavior;
* processing descriptor-version behavior;
* processing execution-mode behavior;
* exact handler-selection result;
* supervision preservation results;
* project- and session-identity preservation results;
* source-argument preservation results;
* non-UTF-8 Unix preservation result;
* proof that the processing handler was not invoked;
* typed failure results;
* sensitive error-formatting results;
* Core test result;
* both workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop.

Do not invoke the selected processing handler.