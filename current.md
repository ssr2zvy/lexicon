Current implementation milestone: session integration closure

Objective

Correct and complete the durable session implementation currently pushed to main.

The session model, store, coordinator, runtime-context document, and child-side binding types now exist, but the implementation contains conflicting ownership models and several incomplete integration paths.

This milestone must produce one coherent source-level session architecture before foreground process launching begins.

Do not implement CLI data-command execution, subprocess launching, HTTP transport, raw transaction recording, SQLite processing, or background supervision.

Repository-grounded defects

The current implementation has the following concrete defects.

1. Parent and child compete for the same lease

SessionCoordinator::prepare_run(...) and prepare_resume(...) return PreparedSessionLaunch, which retains the parent-side SessionLease.

Both Core runtime runners then call:

store.acquire_lease(&session_id)

The child therefore attempts to acquire a lease already held by its supervisor.

A real invocation will receive SessionLeaseError::AlreadyOwned and cannot enter Running.

This is not a usable handoff.

2. Two competing running-session type systems exist

The session module exports both:

RunningSession
RunningRuntimeSession

The store path uses RunningSession, which owns a lease.

The newer binding path uses RunningRuntimeSession, which borrows a store and receives a lease reference for each transition.

The HTTP and processing runners do not use:

bind_runtime_session(...)
BoundRuntimeSession
RunningRuntimeSession

Instead, each runner separately:

* loads the session;
* checks Prepared;
* acquires the lease;
* transitions to Running;
* constructs RunningSession with from_parts.

There must be one supported child lifecycle path.

3. Prepared-session creation does not update root status

SessionStore::create_prepared(...) currently:

* creates the session directory;
* writes sessions/<id>/session.json;
* returns PreparedSession.

It does not write:

session_status.json

The report’s claim that creation updates the root summary is incorrect.

This can leave a durable prepared session invisible to coordinator selection.

4. Scaffold status files may not be compatible with strict decoding

SessionStore::load_status() treats an existing status file as a complete SessionStatusV1.

A generated placeholder such as:

{}

is not a valid empty status.

New source scaffolds and the strict session store must agree on one initial representation.

5. Runtime runners contain an ordinary-path panic

Both runners retrieve session ownership using:

context
    .take_running_session()
    .expect("running session must be present")

Failure to recover an internal lifecycle value must return a typed runtime/session error.

Ordinary execution must not panic merely because a context invariant was violated.

6. Error truncation is incorrectly treated as sanitization

The child binding module persists:

error.to_string()

after truncation.

The HTTP runner persists:

acquisition_error.message().to_string()

Source-authored errors may contain:

* source arguments;
* URLs;
* query parameters;
* credentials;
* cookies;
* response fragments;
* filesystem values.

Truncation limits size. It does not redact sensitive information.

7. Runtime path validation does not bind paths to the project and source

RuntimeContextPaths::new(...) proves relationships below the supplied protocol_root, but it does not prove that the protocol root belongs to:

<project-root>/<configured-sources-directory>/<source>/http

An unrelated absolute directory tree can satisfy the current checks.

8. Runtime-context path encoding is lossy

The context document converts paths using:

path.display().to_string()

This silently changes non-UTF-8 Unix paths.

A path must either round-trip exactly or be rejected explicitly.

It must not be silently normalized through lossy display formatting.

9. Framework session errors are stringified internally

The coordinator currently converts errors using .to_string() for variants such as:

InvalidOperationRoot(String)
ContextEncoding(String)

This discards nested typed causes inside the session pipeline.

10. Launch failure releases ownership before transition

PreparedSessionLaunch::fail_launch(...) currently drops the lease before transitioning the session to Failed.

That introduces an unlocked interval in which another coordinator may observe or mutate the same prepared session.

11. Acquisition context path semantics are inconsistent

HttpAcquisitionContext::source_directory is populated with:

paths.operation_root()

For acquisition this is:

get-raw-data/

It is not the protocol/source root.

The name and value must have one precise meaning.

12. Existing source definitions have drifted

Some existing runner test helpers and internal constructors still reflect the prior context field layout.

This milestone does not run or modify tests, but production source definitions must no longer depend on obsolete context construction.

Correct ownership model

Use the ownership model already stated in the project contract:

supervising Lexicon process
└── owns the session lease for the complete child lifecycle
linked Core inside child
├── binds invocation to the prepared durable session
├── confirms the session is supervisor-owned
├── enters Running
├── invokes the handler
└── persists ordinary completion or failure

Do not transfer the lease to the child.

Do not make the child acquire a second exclusive lease.

In foreground mode, the original Lexicon process will retain the lease.

In background mode, the future operator host will retain the lease.

Process launching remains excluded, but the APIs produced here must support that later model directly.

Supervisor lease lifetime

PreparedSessionLaunch must retain the SessionLease.

It must remain non-Clone.

Provide read-only accessors needed by the future launcher:

impl PreparedSessionLaunch {
    pub fn record(&self) -> &SessionRecordV1;
    pub fn context_document(&self) -> &str;
    pub fn session_identity(&self) -> &SessionIdentity;
    pub fn operation_root(&self) -> &SessionOperationRoot;
}

Do not expose the underlying file or raw operating-system handle.

The future supervisor must keep PreparedSessionLaunch or a consuming successor alive:

before spawn
→ during child startup
→ during handler execution
→ until child exit and reconciliation complete

Define an owning successor if useful:

pub struct SupervisedSession {
    record: SessionRecordV1,
    lease: SessionLease,
    context_document: String,
}

This milestone may define the type and conversion boundary but must not launch or wait for a process.

Dropping the final supervisor-owned value releases the lease.

Child confirmation of supervisor ownership

Add a non-mutating lease-state query.

Representative API:

pub enum SessionLeaseState {
    Available,
    Owned,
}
pub fn inspect_session_lease(
    path: &Path,
) -> Result<SessionLeaseState, SessionLeaseError>;

Equivalent organization is acceptable.

The check may attempt a non-blocking acquisition:

* successful temporary acquisition means no supervisor currently owns the session;
* AlreadyOwned means an exclusive owner exists.

If temporary acquisition succeeds, release it immediately.

Child binding requires Owned.

A prepared child invocation without an active supervisor lease must return a typed error such as:

RuntimeSessionBindingError::SupervisorLeaseUnavailable

Do not treat lock-file existence as ownership.

Do not use PID presence as ownership.

This is a coordination guarantee, not hostile-code confinement. Do not add authorization tokens or cryptographic handoff protocols in this milestone.

One child lifecycle type-state path

Retain one authoritative child type-state sequence:

bind_runtime_session(...)
→ BoundRuntimeSession
→ enter_running(...)
→ RunningRuntimeSession
→ complete(...) or fail_source(...) or fail_runtime(...)

Use this sequence in both runtime runners.

Recommended behavior:

pub fn bind_runtime_session<'store>(
    store: &'store SessionStore,
    envelope: &RuntimeInvocationEnvelopeV1,
) -> Result<BoundRuntimeSession<'store>, RuntimeSessionBindingError>;

Binding must validate:

* project identity;
* runtime source;
* protocol;
* operation;
* source contract version;
* session identity;
* execution mode;
* supervision mode;
* durable state is Prepared;
* supervisor lease is actively owned.

Then:

impl<'store> BoundRuntimeSession<'store> {
    pub fn enter_running(
        self,
    ) -> Result<RunningRuntimeSession<'store>, SessionStoreError>;
}

The child must not receive or acquire a SessionLease.

RunningRuntimeSession must provide consuming terminal operations:

impl<'store> RunningRuntimeSession<'store> {
    pub fn complete(
        self,
    ) -> Result<SessionRecordV1, SessionStoreError>;
    pub fn fail_source(
        self,
        failure: SafeSessionFailure,
    ) -> Result<SessionRecordV1, SessionStoreError>;
    pub fn fail_runtime(
        self,
        failure: SafeSessionFailure,
    ) -> Result<SessionRecordV1, SessionStoreError>;
}

Equivalent names are acceptable.

Remove RunningSession and RunningSession::from_parts(...) if they no longer have a supported caller.

Do not retain two production lifecycle models.

Store transition authorization

Separate supervisor ownership from child transition authorization.

The child transition API may update the durable record only after:

* the invocation was admitted;
* runtime-context identities matched;
* bind_runtime_session(...) matched the detailed record;
* an active external supervisor lease was confirmed.

Keep the type-state constructors private so an ordinary caller cannot fabricate this proof.

Do not require the child to hold the supervisor’s SessionLease value.

The parent-side coordinator still requires an owned SessionLease for:

* prepared-session creation completion;
* launch failure;
* explicit abandonment;
* stale reconciliation;
* later abnormal-exit reconciliation.

Atomic prepared-session publication

Correct SessionStore::create_prepared(...).

A successful return must mean:

1. the session directory exists;
2. session.json is durably written in Prepared;
3. session_status.json identifies the same session and Prepared state;
4. both documents contain matching immutable identities and revision;
5. the returned PreparedSession represents that published state.

If writing session.json succeeds and writing session_status.json fails:

* return SessionStoreError::PartialCommit;
* retain enough structured information for reconciliation;
* do not claim preparation succeeded;
* do not silently delete a durable detailed record.

Add a recovery operation that can rebuild the summary from the authoritative detailed record.

Do not generate a second session identity during recovery.

Root status initial state

Use one canonical initial-state policy:

Before the first session:
session_status.json does not exist.

Update source scaffolding so newly created operation workspaces contain:

sessions/

but do not create an invalid placeholder session_status.json.

The first successful create_prepared(...) creates the valid status document atomically.

If preserving the physical file is required by an established external contract, define a valid explicit empty SessionStatusV1. Do not use {}.

Prefer absence because the store already represents a missing file as:

Ok(None)

Update SourceCreateResult.created_files if it currently reports the removed placeholder file.

Do not change the surrounding operation directory structure.

Safe durable failure representation

Replace arbitrary error-string persistence with an explicitly safe value.

Define a bounded type equivalent to:

pub struct SafeSessionFailure {
    kind: SessionFailureKind,
    code: SessionFailureCode,
}

Representative stable codes:

pub enum SessionFailureCode {
    SourceReturnedError,
    RuntimeInitializationFailed,
    RuntimeContextInvalid,
    HandlerStateUnavailable,
    LaunchFailed,
    AbnormalTermination,
    StaleOwnership,
}

The durable session record may store:

* failure kind;
* stable failure code;
* an optional Core-authored bounded diagnostic.

It must not persist arbitrary source Display output.

fail_source(...) must use a Core-authored value such as:

source handler returned an error

It must not call error.to_string() for durable storage.

The returned runtime error should still preserve the original typed AcquisitionError or ProcessingError through Error::source().

The CLI may eventually display a sanitized top-level diagnostic without placing it in the durable record.

Runtime runner integration

Update both:

run_http_runtime_invocation(...)
run_processing_runtime_invocation(...)

The normal path must be:

parse invocation argv
→ admit invocation
→ decode runtime context
→ validate context identities and paths
→ open SessionStore
→ bind_runtime_session(...)
→ confirm supervisor lease is active
→ BoundRuntimeSession::enter_running()
→ construct operation context
→ invoke exact admitted handler
→ recover RunningRuntimeSession without panic
→ persist Succeeded or typed safe Failed
→ return typed handler result

Remove duplicated manual logic for:

* loading the prepared record;
* checking Prepared;
* child lease acquisition;
* direct SessionTransition::ToRunning;
* RunningSession::from_parts(...).

Probe mode remains independent and unchanged.

Probe mode must not:

* decode runtime context;
* access session files;
* inspect a session lease;
* transition session state;
* invoke a handler.

No panic when recovering lifecycle ownership

Replace:

expect("running session must be present")

with a typed error.

Representative error:

CoreRunnerSessionError::RunningSessionUnavailable

Context APIs may return:

pub(crate) fn take_running_session(
    &mut self,
) -> Result<RunningRuntimeSession<'_>, ContextLifecycleError>;

However, avoid awkward self-referential lifetimes.

A cleaner design is preferred:

* the runner owns RunningRuntimeSession;
* the context borrows only the validated data paths and session identity needed by the handler;
* after the handler returns, the runner still owns the lifecycle value.

If the context does not need to own the running session, remove session ownership from both HttpAcquisitionContext and ProcessingContext.

The supervisor owns the process lease, and the Core runner owns the child lifecycle type state.

Do not hide lifecycle state inside an Option merely to extract it later.

Operation-context structure

Refactor acquisition and processing contexts around SessionDataPaths.

Recommended structure:

pub struct HttpAcquisitionContext {
    paths: SessionDataPaths,
    session: SessionIdentity,
}
pub struct ProcessingContext {
    paths: SessionDataPaths,
    session: SessionIdentity,
}

Equivalent private organization is acceptable.

Provide read-only accessors:

pub fn protocol_root(&self) -> &Path;
pub fn operation_root(&self) -> &Path;
pub fn session_directory(&self) -> &Path;
pub fn raw_data_directory(&self) -> &Path;
pub fn processed_data_directory(&self) -> &Path;
pub fn session_identity(&self) -> &SessionIdentity;

For acquisition, do not call operation_root the source_directory.

If a compatibility accessor must remain, document its exact meaning and make it return the protocol root rather than get-raw-data/.

Do not expose public unchecked constructors.

Remove the production Default path from ProcessingContext.

Keep LEXICON_SOURCE_DIRECTORY quarantined from managed runners.

Complete path binding

Extend runtime-context construction so paths are derived from a validated project/source layout rather than supplied independently.

Define a parent-side input equivalent to:

pub struct RuntimeProjectLayout {
    project_root: PathBuf,
    sources_root: PathBuf,
    source_name: String,
    protocol: RuntimeProtocol,
    operation: RuntimeOperation,
    session: SessionIdentity,
}

Derive:

sources_root
protocol_root
operation_root
session_directory
raw_data_directory
processed_data_directory

Required relationships:

sources_root =
    project_root/<configured-sources-directory>
protocol_root =
    sources_root/<source-name>/http
acquisition operation_root =
    protocol_root/get-raw-data
processing operation_root =
    protocol_root/process-data
session_directory =
    operation_root/sessions/<session-id>
raw_data_directory =
    protocol_root/data/raw
processed_data_directory =
    protocol_root/data/processed

The configured sources directory comes from the already validated project configuration.

Do not hardcode sources when the project configuration provides another relative directory.

Require:

* absolute project root;
* sources root beneath project root;
* protocol root beneath sources root;
* source path component matching runtime source identity;
* protocol path component matching runtime protocol;
* operation root matching runtime operation;
* session path component matching session identity;
* no .. traversal;
* no unrelated absolute path substitution.

Lexical validation is sufficient here.

Do not add hostile symlink sandboxing.

Lossless path transport

Do not serialize paths through:

Path::display()
to_string_lossy()

Choose one explicit policy.

Unix

Encode native path bytes losslessly using a tagged representation such as Base64.

Example structural representation:

{
  "encoding": "unix-bytes-base64",
  "value": "..."
}

Windows

Encode the native UTF-16 code units losslessly.

Example structural representation:

{
  "encoding": "windows-utf16",
  "value": [67, 58, 92]
}

Equivalent lossless versioned encoding is acceptable.

The decoder must reject an encoding intended for another operating-system family.

Do not log encoded raw path values in error messages.

Do not add a second argv transport.

Continue transporting one versioned runtime-context JSON document through:

LEXICON_RUNTIME_CONTEXT_V1

Runtime-context encoding errors

Separate encoding and decoding errors.

Do not map serde_json::to_string(...) failures into:

RuntimeContextError::Decoding(
    SessionDecodingError::JsonSyntax(...)
)

Define typed variants equivalent to:

RuntimeContextEncodingError
RuntimeContextDecodingError

Preserve nested serialization/deserialization errors through source().

Identity mismatch and path mismatch may remain separate validation errors.

Typed framework errors

Replace string variants such as:

InvalidOperationRoot(String)
ContextEncoding(String)

with nested typed variants:

InvalidOperationRoot(RuntimeContextError)
ContextEncoding(RuntimeContextEncodingError)

Equivalent organization is acceptable.

Do not use .to_string() inside:

* SessionCoordinator;
* session selection;
* prepared launch creation;
* context-path derivation;
* abandonment;
* reconciliation.

The eventual CLI boundary may convert the final framework error to the existing CLI representation.

Launch failure ordering

Correct PreparedSessionLaunch::fail_launch(...).

Required order:

retain supervisor lease
→ transition Prepared to Failed(LaunchFailed)
→ update root summary
→ release lease only after transition result is known

If terminal persistence fails, continue holding ownership until the owning value is dropped by the caller.

Do not explicitly drop the lease before the transition.

Use a consuming API whose ownership semantics remain clear on both success and failure.

Coordinator identity validation

SessionCoordinator::new(...) must validate agreement among:

* project identity;
* owned runtime identity;
* session operation;
* operation root;
* runtime-context layout;
* runtime protocol;
* runtime operation;
* runtime source;
* source contract version.

Reject acquisition/processing substitution before creating a session.

Do not accept a caller-provided RuntimeContextPaths whose session directory contains a placeholder session identity.

The coordinator should retain a session-independent validated project/source layout and derive session-specific paths only after the real session identity is generated.

Remove:

placeholder_session_identity()

The caller should not fabricate a placeholder that the store later ignores.

Refactor NewSessionRecord so session identity is either:

* generated before constructing it and retained consistently; or
* absent from the new-session input and generated exactly once by SessionStore.

Prefer:

pub struct NewSessionRecord {
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    operation: SessionOperation,
    execution_mode: RuntimeExecutionMode,
    supervision_mode: RuntimeSupervisionMode,
}

Then SessionStore::create_prepared(...) generates the session identity exactly once.

Existing-session selection consistency

Ensure coordinator selection does not rely only on session_status.json.

When the root summary is missing or inconsistent:

* do not silently assume there is no prior session if detailed session records exist;
* return a typed reconciliation-required condition or rebuild from a uniquely identifiable authoritative detailed record;
* do not guess between multiple conflicting candidate records.

The root summary remains the current index.

Detailed records remain authoritative history.

Do not scan and select sessions based only on lexicographic directory order.

Public API cleanup

After integration:

* remove obsolete RunningSession;
* remove RunningSession::from_parts;
* remove unused lease-taking child transition methods;
* remove unused duplicate lifecycle helpers;
* remove placeholder-session construction;
* remove obsolete context constructors used only by the superseded path;
* keep fields private;
* keep type-state constructors private;
* retain only one supported production execution route.

Do not preserve dead production APIs merely because existing tests reference them.

Tests will be corrected during the final validation phase.

Typed errors

Add or correct typed errors covering:

* supervisor lease unavailable;
* supervisor lease inspection failure;
* prepared-session publication failure;
* root-summary inconsistency;
* ambiguous detailed-session recovery;
* running-session lifecycle unavailable;
* safe failure construction;
* runtime-context encoding;
* runtime-context decoding;
* native-path encoding mismatch;
* source-layout mismatch;
* coordinator identity mismatch;
* launch failure persistence;
* child binding failure.

Implement:

std::fmt::Display
std::error::Error

Preserve nested causes through source().

Do not return plain String from the session pipeline.

Diagnostic constraints

Session and runtime errors must not reveal:

* source arguments;
* invocation-envelope JSON;
* runtime-context JSON;
* native encoded path bytes;
* credentials;
* cookies;
* request or response bodies;
* arbitrary source error messages.

Established non-secret identifiers may appear:

* project identifier;
* source identifier;
* protocol;
* operation;
* session identifier;
* state;
* revision;
* schema version;
* stable failure code.

Direct filesystem errors may identify the affected logical path or path field, but must not print the complete serialized context document.

Source-code-only constraint

This milestone is source implementation only.

Do not run:

cargo test
cargo check
cargo build
cargo fmt
cargo clippy
cargo metadata
rustc

Do not execute generated runners.

Do not run workspace validation.

Do not run the bundle/install pipeline.

Do not add or modify tests in this milestone.

Comprehensive test correction and execution are deferred to the final validation phase.

Use static source inspection only.

Preserve existing behavior

Do not change:

* CLI command names or syntax;
* lexicon init;
* project configuration schema;
* source.toml;
* managed implementation handler signatures;
* managed runner package names;
* managed runner binary names;
* immutable Core dependency pin;
* invocation-envelope JSON;
* invocation argv layout;
* source-argument transport;
* runtime-information JSON;
* probe stdout behavior;
* probe limits or timeout;
* admission order;
* HTTP capability identifiers;
* runtime hashing;
* runtime manifests;
* verification;
* staging;
* bundle admission;
* paired publication;
* MZA;
* Protocol 1;
* installer behavior.

Explicit exclusions

Do not implement:

* lexicon data --get;
* lexicon data --process;
* lexicon build;
* process spawning;
* process waiting;
* exit-status reconciliation;
* signal forwarding;
* cancellation;
* foreground supervision;
* background supervision;
* __operator-host;
* HTTP requests;
* redirects;
* retries;
* rate limiting;
* redaction;
* raw transaction recording;
* checkpoints;
* resume checkpoint interpretation;
* SQLite behavior;
* processing raw transactions;
* automatic legacy-project migration;
* cross-compilation;
* MZA or installer changes.

Completion report

After implementation, replace current.md with a report containing:

* files created and changed;
* each repository defect corrected;
* final supervisor lease ownership model;
* confirmation that the child no longer acquires the supervisor lease;
* supervisor lease inspection behavior;
* final lease lifetime;
* final authoritative child type-state sequence;
* duplicate lifecycle types and APIs removed;
* prepared-session detailed-record publication;
* prepared-session root-summary publication;
* partial-publication recovery behavior;
* initial session_status.json scaffold behavior;
* root-summary inconsistency behavior;
* safe durable failure representation;
* confirmation that arbitrary source error text is not persisted;
* final HTTP runner session flow;
* final processing runner session flow;
* removal of ordinary-path expect or panic behavior;
* final acquisition context representation;
* final processing context representation;
* project/source/protocol/operation path-binding behavior;
* configured sources-directory behavior;
* native Unix path round-trip representation;
* native Windows path round-trip representation;
* runtime-context encoding and decoding error separation;
* coordinator typed-error behavior;
* launch-failure transition ordering;
* placeholder session identity removal;
* legacy session APIs removed or intentionally retained;
* confirmation that probes remain session-independent;
* confirmation that process launching, HTTP transport, raw recording, checkpoints, and SQLite were not implemented;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin foreground data-command execution until this session integration closure is complete.