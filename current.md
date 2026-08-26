Current implementation request: Core child admission for HTTP acquisition invocations

Objective

Add the Core operation that validates a parsed normal invocation against the HTTP acquisition runtime’s compiled identity and source descriptor.

After successful admission, Core must know:

* the envelope matches this compiled runtime;
* the source contract version matches;
* required HTTP capabilities are available;
* the requested acquisition or resume handler exists;
* source arguments remain untouched.

This step selects a typed handler but does not invoke it.

Architectural position

The completed normal-invocation path is:

RuntimeInvocationEnvelopeV1
→ JSON
→ argv transport
→ ParsedRuntimeInvocation

This micro-step adds:

ParsedRuntimeInvocation
+ compiled RuntimeIdentity
+ HttpSourceContractV1
+ available capabilities
→ AdmittedHttpRuntimeInvocation

A later runner::run step will invoke the selected handler.

Required module

Create:

lexicon-core/src/protocols/http/invocation.rs

Export the public API through:

lexicon_core::http

Do not add a runner main.rs.

Selected HTTP handler

Define:

#[derive(Clone, Copy)]
pub enum AdmittedHttpHandler {
    Acquire(HttpAcquireFn),
    Resume(HttpResumeFn),
}

Equivalent naming is acceptable.

Provide:

impl AdmittedHttpHandler {
    pub const fn execution_mode(
        &self,
    ) -> RuntimeExecutionMode;
}

Do not add a method that invokes the function in this step.

The function pointer may remain inspectable through typed enum matching.

Admitted invocation type

Define:

#[derive(Clone)]
pub struct AdmittedHttpRuntimeInvocation {
    envelope: RuntimeInvocationEnvelopeV1,
    source_arguments: Vec<OsString>,
    handler: AdmittedHttpHandler,
    available_capabilities: HttpCapabilitySet,
}

Keep fields private.

Provide:

impl AdmittedHttpRuntimeInvocation {
    pub fn envelope(
        &self,
    ) -> &RuntimeInvocationEnvelopeV1;
    pub fn source_arguments(
        &self,
    ) -> &[OsString];
    pub const fn handler(
        &self,
    ) -> AdmittedHttpHandler;
    pub const fn available_capabilities(
        &self,
    ) -> HttpCapabilitySet;
    pub fn into_parts(
        self,
    ) -> (
        RuntimeInvocationEnvelopeV1,
        Vec<OsString>,
        AdmittedHttpHandler,
        HttpCapabilitySet,
    );
}

Do not provide a public unchecked constructor.

Admission API

Provide:

pub fn admit_http_runtime_invocation(
    parsed: ParsedRuntimeInvocation,
    compiled_identity: RuntimeIdentity,
    source: &HttpSourceContractV1,
    available_capabilities: HttpCapabilitySet,
) -> Result<
    AdmittedHttpRuntimeInvocation,
    HttpRuntimeInvocationAdmissionError,
>;

The parsed invocation is consumed so its native source arguments can move into the admitted value without normalization.

Required validation order

Perform checks in this deterministic order.

1. Compiled protocol

Require:

compiled_identity.protocol()
    == RuntimeProtocol::Http

Return a typed compiled-protocol error otherwise.

2. Compiled operation

Require:

compiled_identity.operation()
    == RuntimeOperation::Acquisition

This closes the previously documented gap where an HTTP descriptor could be paired with a processing identity.

Return a typed compiled-operation error otherwise.

3. Parent/child identity agreement

Require:

parsed.envelope().runtime()
    == compiled_identity

This comparison includes:

* source identity;
* protocol;
* operation;
* source contract version.

Return IdentityMismatch containing expected compiled and actual envelope identities.

4. Descriptor contract version

Require:

compiled_identity.source_contract_version()
    == HttpSourceContractV1::CONTRACT_VERSION

Return a typed descriptor-version mismatch otherwise.

5. Capability availability

Require every capability in:

source.required_capabilities()

to exist in:

available_capabilities

Reuse the existing capability-set comparison and MissingHttpCapabilities.

Do not infer availability from source requirements.

6. Handler selection

For:

RuntimeExecutionMode::Run

select:

source.acquire_handler()

For:

RuntimeExecutionMode::Resume

require:

source.resume_handler()

If no resume handler is registered, return ResumeHandlerUnavailable.

Do not invoke the selected function.

Supervision mode

Both are admitted:

RuntimeSupervisionMode::Foreground
RuntimeSupervisionMode::Background

The selected source handler is unaffected by supervision mode.

Preserve the supervision value in the envelope.

Project and session identities

The parsed envelope has already structurally validated these values.

This step preserves them but performs no filesystem, manifest, session-lock, or ownership validation.

Those checks belong to later parent/session integration.

Source arguments

Source arguments must move from ParsedRuntimeInvocation into the admitted value exactly.

Do not:

* convert them to UTF-8;
* parse them with Clap;
* redact them;
* log them;
* reorder them;
* remove reserved-looking values;
* persist them.

The source implementation will validate its own arguments after handler invocation begins.

Typed admission error

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpRuntimeInvocationAdmissionError {
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
    MissingCapabilities(
        MissingHttpCapabilities,
    ),
    ResumeHandlerUnavailable,
}

Equivalent naming is acceptable.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String, print arguments, or exit.

Error formatting must not reveal:

* source arguments;
* session identity;
* project identity;
* envelope JSON.

Runtime identity values may be represented in diagnostics only through their established non-secret identifiers.

No handler invocation

Admission must not:

* call acquisition;
* call resume;
* construct HttpAcquisitionContext;
* perform HTTP;
* create raw transactions;
* create sessions;
* access files;
* launch processes.

Use handler call counters or panic handlers to prove this.

Required tests

Add tests proving:

1. Matching acquisition/run invocation is admitted.
2. Run selects the exact acquisition function pointer.
3. Matching acquisition/resume invocation is admitted when resume exists.
4. Resume selects the exact resume function pointer.
5. Foreground mode is preserved.
6. Background mode is preserved.
7. Project identity is preserved.
8. Session identity is preserved.
9. Source arguments remain in exact order.
10. Empty source argument values are preserved.
11. Reserved-looking source values are preserved.
12. Non-UTF-8 Unix source arguments are preserved byte-for-byte.
13. Wrong compiled protocol is typed when a real alternate protocol exists.
14. Processing compiled identity returns WrongCompiledOperation.
15. Envelope and compiled source mismatch returns IdentityMismatch.
16. Envelope and compiled operation mismatch returns IdentityMismatch.
17. Envelope and compiled version mismatch returns IdentityMismatch.
18. Compiled descriptor-version mismatch is typed.
19. Missing capability requirements return the complete missing set.
20. Extra available capabilities do not cause rejection.
21. Resume without a registered handler returns ResumeHandlerUnavailable.
22. Resume-handler absence is checked after identity, version, and capabilities.
23. Admission does not invoke acquisition.
24. Admission does not invoke resume.
25. Failed admission cannot construct the admitted value.
26. Existing invocation transport tests remain unchanged.
27. Existing HTTP descriptor and capability tests remain unchanged.
28. Processing descriptor behavior remains unchanged.
29. All workspace tests pass repeatedly.

Do not add a fake protocol solely to test the wrong-protocol branch.

Preserve existing behavior

Do not change:

* invocation JSON or argv transport;
* source descriptor signatures;
* capability identifiers;
* resume registration;
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
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior.

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

* processing child admission;
* handler invocation;
* runner::run;
* runner main.rs;
* managed runner generation;
* process launching;
* project filesystem validation;
* session creation or locking;
* HTTP execution;
* raw recording;
* processing SQLite behavior;
* foreground supervision;
* background supervision;
* __operator-host;
* source workspace migration;
* source build integration.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* admission API;
* admitted invocation and handler representations;
* exact validation order;
* operation-identity guard;
* descriptor-version behavior;
* capability-validation behavior;
* run and resume selection results;
* source-argument preservation results;
* proof that handlers were not invoked;
* typed failure results;
* Core and repeated workspace test results;
* bundle/install result or known external blocker.

Then stop. Do not invoke the selected handler.