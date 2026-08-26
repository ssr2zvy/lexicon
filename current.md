Current implementation request: in-memory runtime invocation envelope

Objective

Define the typed in-memory invocation envelope that the parent lexicon process will eventually send to a managed acquisition or processing runtime.

This step establishes the invocation identity and execution-mode contract only.

Do not add JSON, command-line parsing, runner execution, sessions, or subprocess launching yet.

Architectural position

The build and publication path now exists independently of runtime execution.

The next runtime path will be:

parent framework
→ construct invocation envelope
→ launch managed runtime
→ child Core validates envelope
→ child Core calls source handler

This micro-step implements only:

construct and validate typed invocation envelope

Required module

Create:

lexicon-core/src/runtime/invocation.rs

Export its public API through:

lexicon_core::runtime

Invocation protocol version

Define:

pub const RUNTIME_INVOCATION_PROTOCOL_VERSION: u32 = 1;

This version applies only to the parent-to-child runtime invocation contract.

It remains distinct from:

* runtime-information schema version;
* runtime manifest schema version;
* source contract version;
* runner-template version;
* Core crate version;
* project schema version;
* session schema version.

Execution mode

Define:

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeExecutionMode {
    Run,
    Resume,
}

Stable identifiers:

Run    → "run"
Resume → "resume"

Provide:

impl RuntimeExecutionMode {
    pub const fn identifier(&self) -> &'static str;
    pub fn from_identifier(
        value: &str,
    ) -> Result<Self, RuntimeInvocationIdentifierError>;
}

Do not accept aliases, capitalization differences, or surrounding whitespace.

Semantics:

* acquisition plus Run selects the mandatory acquisition handler;
* acquisition plus Resume will later select the optional resume handler;
* processing plus Run selects the mandatory processing handler;
* processing plus Resume is invalid in version 1.

Do not execute handlers in this step.

Supervision mode

Define:

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeSupervisionMode {
    Foreground,
    Background,
}

Stable identifiers:

Foreground → "foreground"
Background → "background"

Provide identifier() and strict from_identifier(...).

This records whether the supervising parent is the original CLI process or the same-binary operator host.

It does not change the source handler signature.

Project invocation identity

Define an opaque owned type:

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectInvocationIdentity {
    name: String,
}

Provide:

impl ProjectInvocationIdentity {
    pub fn new(
        name: impl Into<String>,
    ) -> Result<Self, RuntimeInvocationValueError>;
    pub fn name(&self) -> &str;
}

For version 1, project identity is the validated project name from lexicon.toml.

Do not add project paths to this type yet.

Validation must reject:

* empty names;
* ".";
* "..";
* /;
* \;
* NUL;
* ASCII control characters.

Do not silently trim.

Session invocation identity

Define:

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionInvocationIdentity {
    id: String,
}

Provide:

impl SessionInvocationIdentity {
    pub fn new(
        id: impl Into<String>,
    ) -> Result<Self, RuntimeInvocationValueError>;
    pub fn id(&self) -> &str;
}

Apply the same safe-component validation as project identity.

This step does not define how session IDs are generated.

Invocation envelope

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInvocationEnvelopeV1 {
    project: ProjectInvocationIdentity,
    runtime: RuntimeIdentity,
    session: SessionInvocationIdentity,
    execution_mode: RuntimeExecutionMode,
    supervision_mode: RuntimeSupervisionMode,
}

Keep fields private.

Construction API

Provide:

impl RuntimeInvocationEnvelopeV1 {
    pub fn new(
        project: ProjectInvocationIdentity,
        runtime: RuntimeIdentity,
        session: SessionInvocationIdentity,
        execution_mode: RuntimeExecutionMode,
        supervision_mode: RuntimeSupervisionMode,
    ) -> Result<Self, RuntimeInvocationConstructionError>;
}

Construction must validate:

1. source contract version is nonzero;
2. acquisition permits Run;
3. acquisition permits Resume;
4. processing permits Run;
5. processing rejects Resume.

Do not validate whether an acquisition descriptor actually registered a resume handler. That requires the descriptor during child admission.

Accessors

Provide:

impl RuntimeInvocationEnvelopeV1 {
    pub fn project(
        &self,
    ) -> &ProjectInvocationIdentity;
    pub const fn runtime(
        &self,
    ) -> RuntimeIdentity;
    pub fn session(
        &self,
    ) -> &SessionInvocationIdentity;
    pub const fn execution_mode(
        &self,
    ) -> RuntimeExecutionMode;
    pub const fn supervision_mode(
        &self,
    ) -> RuntimeSupervisionMode;
}

Source arguments remain separate

Do not include source-specific arguments inside the envelope.

The eventual process boundary remains:

<internal invocation envelope>
--
<untouched source OsString arguments>

This prevents the versioned envelope from imposing a universal source-argument schema or persisting potentially sensitive arguments.

Typed errors

Define an identifier error:

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInvocationIdentifierError {
    UnknownIdentifier {
        kind: &'static str,
        value: String,
    },
}

Define a value error distinguishing invalid project and session components.

Define a construction error distinguishing:

* zero source contract version;
* unsupported execution mode for the runtime operation.

Equivalent typed representations are acceptable.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String, print diagnostics, or exit.

Required tests

Add tests proving:

1. Invocation protocol version is 1.
2. Every execution-mode identifier round-trips.
3. Every supervision-mode identifier round-trips.
4. Unknown identifiers are rejected.
5. Aliases, capitalization changes, and whitespace are rejected.
6. Valid project identity constructs successfully.
7. Valid session identity constructs successfully.
8. Every unsafe component form is rejected.
9. Acquisition plus Run constructs successfully.
10. Acquisition plus Resume constructs successfully.
11. Processing plus Run constructs successfully.
12. Processing plus Resume is rejected.
13. Zero source contract version is rejected.
14. Every accessor preserves its supplied value.
15. Foreground and background envelopes remain distinct.
16. Source arguments are absent from the envelope type.
17. Construction invokes no acquisition handler.
18. Construction invokes no processing handler.
19. Existing runtime identity behavior remains unchanged.
20. Existing build, publication, and probe tests remain unchanged.
21. All workspace tests pass.

Preserve existing behavior

Do not change:

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

* invocation JSON;
* invocation command-line syntax;
* envelope files;
* source-argument splitting;
* child runtime admission;
* resume-handler availability validation;
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
* invocation protocol version;
* execution and supervision modes;
* stable identifiers;
* project and session identity representations;
* validation rules;
* envelope representation and constructor;
* operation/mode compatibility results;
* accessor results;
* proof that source arguments are not included;
* proof that no handler was invoked;
* Core and repeated workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not serialize or execute the invocation envelope.