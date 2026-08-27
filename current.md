Current implementation milestone: durable session storage and runtime-context foundation

Objective

Implement the durable session model shared by acquisition and processing operations.

The repository currently has:

* managed acquisition and processing runners;
* runtime-information probing;
* invocation-envelope JSON and native argv transport;
* HTTP acquisition admission and execution;
* processing admission and execution;
* generated operation workspaces containing sessions/ and session_status.json;
* parsed CLI flags for foreground/background operation and abandoning a previous failure.

It does not yet have:

* a durable session record;
* legal session-state transitions;
* operation-level session status;
* cross-process session ownership;
* stale-owner reconciliation;
* trusted filesystem paths bound to a runtime invocation;
* functional acquisition or processing contexts backed by the selected session.

This milestone implements those foundations in Core and Framework.

Do not implement CLI data-command execution, child process launching, HTTP transport, raw transaction recording, SQLite processing, or background supervision yet.

Repository-grounded execution boundary

The contract assigns responsibilities as follows:

supervising Lexicon process
├── select, create, or resume a session
├── acquire session ownership
├── apply abandon-past-failure policy
├── launch the source runtime
├── observe process termination
└── reconcile abnormal termination
linked Core inside the source runtime
├── validate the invocation
├── enter running state
├── maintain durable session state
├── record ordinary source failure
└── record normal completion

This milestone implements the reusable session and context APIs required by those two sides.

It does not launch the runtime or wire lexicon data into them.

Existing behavior to preserve

Preserve the existing:

* RuntimeInvocationEnvelopeV1 JSON representation;
* invocation argv layout;
* ProjectIdentity;
* SessionIdentity;
* RuntimeExecutionMode;
* RuntimeSupervisionMode;
* HTTP and processing admission order;
* source descriptor signatures;
* HTTP and processing handler invocation;
* runtime-information formats;
* managed runner identity constants;
* bundle verification, staging, admission, and publication;
* source workspace layout;
* CLI argument syntax.

Do not add project paths, source paths, or session paths to the invocation envelope.

Filesystem paths are local execution configuration, not portable invocation identity.

Core session module

Add:

lexicon-core/src/session/
├── mod.rs
├── model.rs
├── store.rs
├── lease.rs
├── transition.rs
├── context.rs
└── error.rs

Equivalent internal organization is acceptable.

Export the stable public API through:

lexicon_core::session

Do not place framework command policy inside Core.

Session schema version

Define an independent session format version:

pub const SESSION_SCHEMA_VERSION: u32 = 1;

This version is separate from:

* project schema version;
* source-manifest schema version;
* source contract versions;
* invocation protocol version;
* runner template version;
* raw-transaction schema version.

Operation identity

A session must unambiguously belong to one operation.

Define a typed operation scope equivalent to:

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionOperation {
    Acquisition,
    Processing,
}

Provide a stable identifier representation:

acquisition
processing

It may convert to and from RuntimeOperation, but session code must not accept an arbitrary protocol/operation combination where only HTTP acquisition or HTTP processing is supported.

Session state model

Define:

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Prepared,
    Running,
    Succeeded,
    Failed,
    Abandoned,
}

State meanings:

* Prepared: the parent created the durable session but the child has not entered normal execution.
* Running: the admitted child owns the session and handler execution has begun.
* Succeeded: the handler completed successfully.
* Failed: ordinary source failure, runtime failure, or supervisor-reconciled abnormal termination.
* Abandoned: a prior non-successful session was explicitly abandoned before a replacement run.

Succeeded, Failed, and Abandoned are terminal.

Failure classification

Define a bounded typed failure classification without storing arbitrary error output:

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionFailureKind {
    Source,
    Runtime,
    AbnormalTermination,
    StaleOwnership,
}

The durable record may include a concise sanitized failure summary.

It must not store:

* source arguments;
* invocation-envelope JSON;
* environment variables;
* request or response bodies;
* authorization values;
* cookies;
* arbitrary panic payloads;
* unbounded stderr.

Bound any persisted failure summary with an explicit constant.

Durable session record

Define a strict versioned representation equivalent to:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecordV1 {
    schema_version: u32,
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
    operation: SessionOperation,
    execution_mode: RuntimeExecutionMode,
    supervision_mode: RuntimeSupervisionMode,
    state: SessionState,
    revision: u64,
    created_at: SessionTimestamp,
    updated_at: SessionTimestamp,
    started_at: Option<SessionTimestamp>,
    finished_at: Option<SessionTimestamp>,
    failure: Option<SessionFailureV1>,
}

Equivalent field organization is acceptable.

Required invariants:

* schema_version is exactly SESSION_SCHEMA_VERSION;
* runtime operation agrees with operation;
* project, source, protocol, operation, contract version, and session identity are immutable after construction;
* revision starts at zero and increases on every committed transition;
* started_at exists only after entering Running;
* terminal states have finished_at;
* non-failed states do not contain failure information;
* Failed contains a typed failure classification;
* Succeeded and Abandoned do not contain failure information;
* timestamps never move backward within one record.

Use OwnedRuntimeIdentity for durable/framework-side identities. Do not use Box::leak.

Session timestamp

Define a serializable UTC timestamp representation with deterministic parsing and formatting.

Do not use locale-dependent output.

The timestamp abstraction must support injecting the current time through an internal clock boundary so callers are not forced to access wall-clock time throughout transition logic.

Do not introduce sleeps.

Operation-level status summary

The operation-root file remains:

session_status.json

Define a strict versioned summary equivalent to:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStatusV1 {
    schema_version: u32,
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    operation: SessionOperation,
    current_session: Option<SessionIdentity>,
    current_state: Option<SessionState>,
    revision: u64,
    updated_at: SessionTimestamp,
}

The summary is an index of current operation state.

Detailed durable history belongs in:

sessions/<session-id>/session.json

The summary must never be treated as the only durable session record.

Filesystem layout

Support the existing operation directories:

sources/<source>/http/get-raw-data/
├── sessions/
│   └── <session-id>/
│       ├── session.json
│       └── session.lock
└── session_status.json

and:

sources/<source>/http/process-data/
├── sessions/
│   └── <session-id>/
│       ├── session.json
│       └── session.lock
└── session_status.json

Define a validated SessionOperationRoot or equivalent type.

It must derive:

* the sessions directory;
* a particular session directory;
* the session record path;
* the lease path;
* the root status path.

Do not accept a session identity as an unchecked path component.

Use the already validated SessionIdentity.

Do not canonicalize paths by requiring every target file to exist before session creation.

Serialization behavior

Provide typed encoding and decoding APIs for:

* SessionRecordV1;
* SessionStatusV1.

Use strict decoding:

* reject unknown schema versions;
* reject unknown fields;
* reject unknown identifiers;
* reject invalid state-dependent field combinations;
* reject runtime/operation disagreement;
* reject identity disagreement;
* reject invalid timestamps;
* reject invalid revisions;
* reject malformed structural documents.

Do not return plain String errors internally.

Implement Display and Error, preserving nested errors through source().

Error formatting must not expose source arguments, environment values, envelope JSON, or sensitive filesystem contents.

Legal transitions

Centralize transition validation.

Allow:

Prepared → Running
Prepared → Failed
Prepared → Abandoned
Running  → Succeeded
Running  → Failed
Running  → Abandoned
Failed   → Abandoned

Reject all other transitions, including:

* transition to Prepared;
* Succeeded to any other state;
* Abandoned to any other state;
* Failed directly back to Running;
* repeated terminal transitions;
* revision rollback;
* mutation of immutable identity fields.

Resume does not reopen the previous record.

A resume invocation creates a new session record whose execution mode is Resume. The source-specific resume handler decides how to continue from previously committed source checkpoints in a later milestone.

This keeps individual session history immutable after terminal completion.

Atomic persistence

All writes to session.json and session_status.json must use atomic replacement or an equivalent transactional mechanism.

Required behavior:

1. Serialize the complete next document.
2. Write it to a unique temporary file in the same directory.
3. Flush the file contents.
4. Atomically replace the destination.
5. Flush the containing directory where supported.
6. Remove the temporary file after a failed pre-publication write when possible.

Do not:

* truncate the live JSON file before the replacement is ready;
* write through a shared fixed temporary filename;
* silently ignore partial writes;
* treat a corrupted existing record as an empty session;
* hold source arguments in persisted state.

Platform-specific directory-sync limitations may be represented through a narrow internal abstraction, but durability errors must remain typed.

Revision guard

Every update must use optimistic revision validation in addition to session ownership.

A transition API must require the expected current revision.

If the durable record has another revision, return a typed conflict error rather than overwriting it.

Updating the detailed record and root summary must follow a deterministic sequence that cannot make an older revision replace a newer revision.

If the first update succeeds and the second fails, return a typed partial-commit/reconciliation-required error. Do not pretend the combined operation succeeded.

A later supervisor call must be able to reconstruct the root summary from the authoritative detailed record.

Cross-process session lease

Implement a cross-process exclusive lease represented by:

sessions/<session-id>/session.lock

The lease must be owned by an RAII value equivalent to:

pub struct SessionLease {
    // private ownership state
}

Required properties:

* acquisition is exclusive across processes;
* acquisition does not block indefinitely;
* contention returns a typed AlreadyOwned result;
* dropping the value releases the active operating-system lock;
* ordinary errors do not call process::exit;
* the lock file’s mere existence is not treated as proof of active ownership;
* stale lock-file presence does not permanently block future execution.

Use an established operating-system file-locking facility available to the workspace rather than inventing a PID-file-only lock.

A PID may be recorded for diagnostics, but PID reuse must not be the ownership primitive.

Do not delete another live process’s lease.

Session store

Provide a Core-owned store equivalent to:

pub struct SessionStore {
    operation_root: SessionOperationRoot,
}

Representative public API:

impl SessionStore {
    pub fn open(
        operation_root: SessionOperationRoot,
    ) -> Result<Self, SessionStoreError>;
    pub fn create_prepared(
        &self,
        record: NewSessionRecord,
    ) -> Result<PreparedSession, SessionStoreError>;
    pub fn load(
        &self,
        session: &SessionIdentity,
    ) -> Result<SessionRecordV1, SessionStoreError>;
    pub fn load_status(
        &self,
    ) -> Result<Option<SessionStatusV1>, SessionStoreError>;
    pub fn acquire_lease(
        &self,
        session: &SessionIdentity,
    ) -> Result<SessionLease, SessionLeaseError>;
    pub fn transition(
        &self,
        session: &SessionIdentity,
        expected_revision: u64,
        transition: SessionTransition,
    ) -> Result<SessionRecordV1, SessionStoreError>;
    pub fn rebuild_status_from_record(
        &self,
        session: &SessionIdentity,
    ) -> Result<SessionStatusV1, SessionStoreError>;
}

Equivalent typed organization is acceptable.

Do not expose a method that writes an arbitrary caller-constructed session document without invariant validation.

Type-state lifecycle values

Use private fields and type-state wrappers to prevent ordinary callers from constructing invalid lifecycle states.

Provide representations equivalent to:

pub struct PreparedSession {
    record: SessionRecordV1,
}
pub struct RunningSession {
    record: SessionRecordV1,
    lease: SessionLease,
}

A successful transition into Running consumes the prepared/bound state and retains the lease.

Completion consumes RunningSession.

Ordinary source failure consumes RunningSession.

Do not implement Clone for lease-owning or running-session values.

Do not provide public unchecked constructors.

Parent-side session coordinator

Add a Framework-owned coordinator in a focused module, not in the existing monolithic command body.

Representative location:

lexicon-framework/src/session/
├── mod.rs
├── coordinator.rs
├── selection.rs
└── error.rs

Export only the API needed by later command execution.

The coordinator must operate on already validated project, source, protocol, operation, runtime, and filesystem identities.

Representative operations:

pub struct SessionCoordinator {
    // validated operation paths and expected identities
}
impl SessionCoordinator {
    pub fn prepare_run(
        &self,
        supervision: RuntimeSupervisionMode,
    ) -> Result<PreparedSessionLaunch, SessionCoordinationError>;
    pub fn prepare_resume(
        &self,
        supervision: RuntimeSupervisionMode,
    ) -> Result<PreparedSessionLaunch, SessionCoordinationError>;
    pub fn abandon_current_failure(
        &self,
    ) -> Result<SessionRecordV1, SessionCoordinationError>;
    pub fn reconcile_stale_current_session(
        &self,
    ) -> Result<Option<SessionRecordV1>, SessionCoordinationError>;
}

These operations prepare durable state only.

They must not launch a process.

Run selection

For a new run:

* reject an actively owned Prepared or Running current session;
* reject an unresolved Failed current session unless abandonment policy was applied;
* allow a new run after Succeeded or Abandoned;
* generate a new valid session identity;
* create its session directory;
* create session.json in Prepared;
* update session_status.json;
* acquire and retain the lease in the returned preparation value.

Do not overwrite or reuse a previous session directory.

Resume selection

For resume:

* require acquisition operation;
* reject processing resume;
* require a prior resumable failed or reconciled acquisition session;
* reject a currently live owned session;
* create a new session identity;
* preserve the previous session record;
* create a new Prepared record with execution mode Resume;
* retain an explicit predecessor-session identity in a typed resume linkage if needed by the durable format.

Do not infer resumption merely because a resume handler exists.

The source handler remains responsible for interpreting source-specific checkpoints later.

Abandon-past-failure behavior

The coordinator must provide the policy operation needed by the existing CLI flag:

--abandon-past-fail

It may abandon only a non-live Prepared, Running, or Failed current session after ownership has been checked.

It must not abandon:

* a live leased session;
* a succeeded session;
* an already abandoned session;
* a session belonging to another project, source, protocol, or operation.

Abandonment is durable and updates both the detailed record and the operation summary.

Do not wire the CLI flag to this API yet.

Stale ownership reconciliation

A durable Running state does not by itself prove a live runtime.

Reconciliation must:

1. load the current detailed record;
2. attempt non-blocking lease acquisition;
3. treat lease contention as live ownership;
4. treat successful acquisition of a record still marked Prepared or Running as stale durable state;
5. transition that record to Failed with StaleOwnership;
6. update the root summary;
7. release the temporary reconciliation lease.

Do not use only a stored PID to decide liveness.

Do not reconcile a live owner.

Do not silently delete stale history.

Trusted runtime-context paths

Replace the unstructured context-construction boundary with typed path data.

Define a value equivalent to:

pub struct RuntimeContextPaths {
    project_root: PathBuf,
    protocol_root: PathBuf,
    operation_root: PathBuf,
    session_directory: PathBuf,
    raw_data_directory: PathBuf,
    processed_data_directory: PathBuf,
}

Its constructor must validate the relationship among these paths instead of accepting unrelated arbitrary locations.

Required relationships for HTTP acquisition:

protocol_root          = sources/<source>/http
operation_root         = protocol_root/get-raw-data
session_directory      = operation_root/sessions/<session-id>
raw_data_directory     = protocol_root/data/raw
processed_data_directory = protocol_root/data/processed

Required relationships for processing:

protocol_root          = sources/<source>/http
operation_root         = protocol_root/process-data
session_directory      = operation_root/sessions/<session-id>
raw_data_directory     = protocol_root/data/raw
processed_data_directory = protocol_root/data/processed

Do not perform hostile-filesystem sandboxing.

Do reject:

* relative project roots;
* path traversal;
* session-directory identity disagreement;
* operation-root disagreement;
* protocol-root disagreement;
* acquisition/processing root substitution.

Runtime context configuration transport

Define one canonical, versioned, private-to-Lexicon environment transport for filesystem context.

Use a single UTF-8 JSON environment value rather than several independently mutable path variables.

Representative constant:

pub const RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE: &str =
    "LEXICON_RUNTIME_CONTEXT_V1";

Define a strict document containing:

* schema version;
* project identity;
* runtime identity;
* session identity;
* project root;
* protocol root;
* operation root;
* session directory;
* raw-data directory;
* processed-data directory.

The document must not contain source arguments or credentials.

Provide typed encode/decode APIs.

Child-side decoding must compare the configuration identities against the already admitted invocation before constructing a context.

Do not trust path configuration solely because the JSON is structurally valid.

Do not add filesystem paths to RuntimeInvocationEnvelopeV1.

The old LEXICON_SOURCE_DIRECTORY boundary must not remain the supported managed-runner path after this milestone. Remove or clearly quarantine it as unsupported legacy behavior if an unrelated API still requires it.

HTTP acquisition context

Refactor HttpAcquisitionContext so its fields are private and it is constructed from:

* an admitted HTTP invocation;
* validated RuntimeContextPaths;
* an owned running session handle.

Provide read-only path accessors required by future HTTP recording work.

Do not expose a public constructor accepting arbitrary independent paths.

The context must retain the running session ownership for the entire handler call.

On successful handler return:

* transition the session to Succeeded.

On ordinary AcquisitionError:

* transition the session to Failed with Source failure classification;
* preserve the original acquisition error as the primary typed nested cause where possible.

If terminal session persistence fails, return a typed combined runtime/session error. Do not report success.

Do not implement HTTP requests or raw transactions.

Processing context

Replace the empty ProcessingContext::default() production boundary.

Construct ProcessingContext from:

* an admitted processing invocation;
* validated runtime-context paths;
* an owned running session handle.

Its fields must be private.

Provide read-only accessors for:

* project root;
* protocol root;
* operation root;
* session directory;
* raw-data directory;
* processed-data directory;
* session record or identity where needed.

Do not add SQLite behavior.

On successful handler return, persist Succeeded.

On ordinary processing failure, persist Failed with Source classification.

Remove Default if it would create an unbound production context. Do not retain an unchecked default merely to keep old generated runner code compiling.

Core runner integration

Update the existing Core HTTP and processing normal-invocation runners so the supported order becomes:

parse invocation argv
→ admit invocation
→ decode runtime context configuration
→ compare context identities with admitted envelope
→ open session store
→ acquire/confirm session lease
→ transition Prepared to Running
→ construct bound operation context
→ invoke the selected handler
→ persist Succeeded or ordinary Failed
→ return typed result

Probe behavior remains before normal invocation parsing and must not require runtime context configuration or session files.

Information probes must not:

* acquire a lease;
* construct a session store;
* transition session state;
* create directories;
* invoke a handler.

Do not modify runtime-information JSON.

Lease handoff boundary

Because process launching is excluded, define but do not execute the parent-to-child lease handoff protocol.

The design must make ownership unambiguous:

* the parent preparation path owns the lease before launch;
* the child must not race another invocation for the same session;
* ownership transfer must not create an unlocked interval;
* a failed launch leaves the parent able to mark the prepared session failed;
* the parent supervisor can reconcile abnormal termination.

Use a typed launch-preparation value that retains the parent lease and produces the runtime-context environment document.

If a truly gapless cross-process lease transfer requires inherited handles or a later platform-specific launcher, keep the parent lease held through child startup and expose an explicit handoff method for that later launcher.

Do not fake transfer by dropping the parent lease before the child starts.

Do not launch a process in this milestone.

Typed errors

Add typed errors covering at least:

* session encoding and decoding;
* unknown session schema version;
* invalid session document invariants;
* invalid transition;
* immutable identity mismatch;
* revision conflict;
* missing session;
* corrupt session;
* session directory creation;
* atomic file persistence;
* root-summary update failure;
* partial record/summary commit;
* lease creation;
* lease contention;
* lease I/O;
* stale ownership reconciliation;
* invalid runtime-context document;
* runtime-context identity mismatch;
* runtime-context path mismatch;
* session preparation;
* resume unavailable;
* abandonment unavailable;
* Core runner session initialization;
* terminal-state persistence failure.

Implement Display and Error.

Preserve nested errors through source().

Do not convert errors to String inside Core or Framework session logic.

The eventual CLI boundary may stringify the final framework error later.

Security and diagnostic constraints

Diagnostics must not reveal:

* source arguments;
* invocation-envelope JSON;
* complete environment JSON;
* credentials;
* cookies;
* request or response bodies;
* arbitrary raw filesystem data.

Established non-secret identifiers may appear:

* project identifier;
* source identifier;
* protocol identifier;
* operation identifier;
* session identifier;
* state identifier;
* schema version;
* revision number.

Paths may appear in direct filesystem diagnostics where necessary, but do not include the full serialized runtime-context document.

Source-code-only execution constraint

This milestone is implementation-only.

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

Do not add or modify tests during this milestone. Comprehensive test creation and execution will be handled in the final validation phase.

Use static source inspection only.

Preserve existing behavior

Do not change:

* CLI command names or argument syntax;
* lexicon init;
* lexicon source create;
* lexicon source build;
* source.toml;
* managed workspace manifests;
* lockfile generation or immutability;
* managed runner package or binary names;
* invocation-envelope JSON;
* argv transport;
* source arguments;
* runtime-information JSON;
* probe output streams;
* probe limits or timeout;
* HTTP capability identifiers;
* handler signatures;
* runtime hashing;
* runtime manifests;
* bundle formats;
* verification;
* staging;
* bundle admission;
* paired publication;
* MZA;
* Protocol 1;
* installer behavior.

Explicit exclusions

Do not implement:

* CLI data --get execution;
* CLI data --process execution;
* lexicon build;
* child process launching;
* signal forwarding;
* foreground process supervision;
* background process supervision;
* __operator-host;
* HTTP client transport;
* redirects;
* retries;
* rate limiting;
* request construction;
* response decoding;
* redaction;
* raw transaction recording;
* checkpoint storage;
* source checkpoint APIs;
* SQLite creation or mutation;
* raw-transaction processing;
* automatic legacy-project migration;
* cross-compilation;
* MZA or installer changes.

Completion report

After implementation, replace current.md with a report containing:

* files created and changed;
* Core session module structure;
* session schema version;
* durable session record representation;
* operation-level status representation;
* exact state-transition table;
* failure classifications;
* revision-conflict behavior;
* atomic persistence behavior;
* detailed-record and root-summary consistency behavior;
* cross-process lease implementation;
* lease contention behavior;
* stale-ownership reconciliation;
* run-session preparation behavior;
* resume-session preparation behavior;
* abandonment behavior;
* owned identity behavior and confirmation that no new Box::leak was introduced;
* runtime-context path representation;
* runtime-context environment transport;
* identity checks between context configuration and admitted invocation;
* HTTP acquisition context changes;
* processing context changes;
* Core HTTP runner integration;
* Core processing runner integration;
* probe/session separation;
* lease-handoff boundary left for process launching;
* typed error hierarchy;
* legacy LEXICON_SOURCE_DIRECTORY behavior removed or intentionally quarantined;
* confirmation that no HTTP, raw recording, SQLite, process launching, or CLI data execution was implemented;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin foreground data-command execution or HTTP transport.