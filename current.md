Current implementation milestone: durable session model and supervisor lease foundation

Objective

Implement the durable session-state foundation shared by acquisition and processing.

This milestone defines:

* session schemas;
* operation-specific session stores;
* validated session paths;
* atomic durable state updates;
* supervisor-held session leases;
* legal state transitions;
* stale-running-session reconciliation;
* root session_status.json summaries;
* typed Core APIs used later by managed runners and framework process launching.

This milestone changes source code only.

Do not add or run tests in this request. All testing and validation are deferred to the final project-wide validation phase.

Do not modify generated managed runners, their immutable Core revision pin, or source build.

Architectural ownership

lexicon-core

Core owns the shared durable session contract because both:

* the supervising lexicon process;
* the linked Core inside a managed runtime;

must read and update the same session representation.

Core owns:

* session identifiers and schema;
* session states;
* detailed session records;
* root summaries;
* safe path derivation below an explicitly supplied operation root;
* encoding and decoding;
* atomic persistence;
* legal transition validation;
* opening and validating an existing session;
* child-side running, success, and ordinary-failure transitions.

lexicon-framework

Framework owns supervisor behavior:

* selecting or creating a session;
* acquiring and retaining the exclusive session lease;
* choosing run versus resume;
* applying abandonment policy;
* reconciling stale running sessions;
* preparing the invocation envelope;
* eventually launching and supervising the managed runtime.

This milestone implements reusable supervisor session APIs but does not launch a child process.

Required Core module

Create:

lexicon-core/src/session/
├── mod.rs
├── identity.rs
├── state.rs
├── record.rs
├── store.rs
├── transition.rs
├── lease.rs
└── error.rs

Equivalent subdivision is acceptable.

Export the intended public API through:

lexicon_core::session

Do not place the session implementation in the root lib.rs.

Session schema version

Define:

pub const SESSION_SCHEMA_VERSION: u32 = 1;

This is distinct from:

* project schema version;
* source schema version;
* source contract version;
* runtime invocation version;
* runner template version;
* raw transaction schema version.

Do not reuse another version constant.

Session operation root

Every session store is explicitly rooted at one operation directory:

<protocol-root>/get-raw-data/

or:

<protocol-root>/process-data/

The operation root contains:

session_status.json
sessions/

Core receives this operation root explicitly.

Core must not:

* search upward for lexicon.toml;
* infer the operation root from the executable path;
* read the current working directory;
* read environment variables;
* accept a raw session-relative path from a source implementation.

Define an opaque validated root equivalent to:

pub struct SessionOperationRoot {
    path: PathBuf,
    operation: RuntimeOperation,
}

Construction must require:

* an absolute path;
* an existing directory;
* RuntimeOperation::Acquisition or RuntimeOperation::Processing;
* no symlink escape when resolving existing components.

Provide read-only accessors.

Session identity

Reuse:

SessionInvocationIdentity

as the invocation-facing session identity.

Do not create a conflicting second validation rule.

Where durable storage needs ownership, clone the validated identifier into an owned session identifier type or use an established owned representation.

A session identifier must remain a single safe path component.

It must reject:

* empty values;
* .;
* ..;
* separators;
* absolute paths;
* NUL;
* traversal;
* platform prefixes.

Do not use timestamps alone as proof of uniqueness.

Provide a session-ID generator using:

* UTC time for human readability;
* a cryptographically random or operating-system-random suffix for uniqueness.

Do not use process ID alone.

Durable directory layout

For session S, use:

<operation-root>/
├── session_status.json
└── sessions/
    └── S/
        ├── session.json
        └── session.lock

Do not put sessions below data/raw or data/processed.

Do not create transaction directories in this milestone.

Do not add checkpoint files yet.

Session states

Define a closed typed state model equivalent to:

pub enum SessionState {
    Prepared,
    Running,
    Succeeded,
    Failed,
    Abandoned,
}

Do not represent state internally as arbitrary strings.

Stable serialized identifiers:

prepared
running
succeeded
failed
abandoned

Failed must retain a typed failure classification.

At minimum distinguish:

pub enum SessionFailureKind {
    Source,
    Runtime,
    AbnormalTermination,
    StaleOwnership,
}

A sanitized optional diagnostic message may be retained.

It must not contain:

* source arguments;
* envelope JSON;
* credentials;
* raw request or response bodies.

Detailed session record

Define an opaque record equivalent to:

pub struct SessionRecordV1 {
    schema_version: u32,
    project: ProjectInvocationIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionInvocationIdentity,
    execution_mode: RuntimeExecutionMode,
    supervision_mode: RuntimeSupervisionMode,
    state: SessionState,
    created_at: SessionTimestamp,
    updated_at: SessionTimestamp,
    revision: u64,
    failure: Option<SessionFailure>,
}

Use exact existing identity types where possible.

Requirements:

* revision begins at 1;
* every durable transition increments revision exactly once;
* created_at never changes;
* updated_at never moves backward;
* failure is absent outside Failed;
* the record’s operation matches the operation root;
* processing cannot use resume mode;
* source, protocol, operation, version, project, and session are preserved exactly.

Do not store source arguments.

Root session summary

Replace the scaffold-only meaning of session_status.json with a real typed summary.

Define a representation equivalent to:

pub struct SessionStatusSummaryV1 {
    schema_version: u32,
    source: String,
    protocol: RuntimeProtocol,
    operation: RuntimeOperation,
    latest_session: Option<SessionInvocationIdentity>,
    state: Option<SessionState>,
    updated_at: Option<SessionTimestamp>,
    revision: u64,
}

The root file is only the current summary.

Detailed durable history remains under:

sessions/<session-id>/session.json

Updating a detailed record and summary must not leave the summary claiming a state newer than the durable detailed record.

The detailed session record is authoritative if recovery finds disagreement.

Stable timestamp representation

Define one typed timestamp representation and one stable serialized form.

Use UTC RFC 3339 with sufficient precision to preserve ordering, or an equivalent explicitly versioned UTC representation.

Do not use locale-dependent formatting.

Do not expose raw SystemTime directly in the serialized schema.

Inject a clock behind an internal seam so later validation can exercise deterministic transitions.

Do not add a general time framework.

Legal state transitions

Allow only:

new → prepared
prepared → running
running → succeeded
running → failed
prepared → abandoned
failed → abandoned
running(stale) → failed(stale-ownership)

Resume creates a new session invocation only if that is the selected contract, or re-enters an existing failed session only through an explicit supervisor API.

Do not silently reinterpret run as resume.

Reject:

* succeeded → running;
* succeeded → failed;
* abandoned → running;
* failed → succeeded;
* repeated succeeded;
* repeated abandoned;
* any state regression;
* transitions with the wrong expected revision.

Use typed transition errors containing the current and requested states but no sensitive invocation data.

Optimistic revision guard

Every update API must require the caller’s expected revision.

Equivalent API:

pub fn transition(
    &mut self,
    expected_revision: u64,
    transition: SessionTransition,
) -> Result<
    SessionRecordV1,
    SessionTransitionError,
>;

Reject stale writers rather than silently overwriting a newer record.

The exclusive lease is the primary writer guard. Revision checking protects against programming mistakes and recovery races.

Supervisor lease

Implement a cross-process exclusive lease over:

sessions/<session-id>/session.lock

Define an owning guard equivalent to:

pub struct SessionLease {
    file: File,
    session: SessionInvocationIdentity,
}

The lock remains held until the guard is dropped.

Requirements:

* exclusive;
* nonblocking acquisition option;
* typed already-locked result;
* released on guard drop;
* no global process mutex as the production lock;
* no deletion of the lock file to represent unlocking;
* supported on Linux and Windows through one explicit cross-platform dependency or well-contained platform implementations.

Do not rely solely on a PID file.

A PID may be recorded for diagnostics, but lock ownership is authoritative.

Atomic persistence

All session JSON writes must use same-directory atomic replacement or an equivalent transactional technique.

Required sequence:

serialize complete document
→ write unique temporary file in destination directory
→ flush file contents
→ atomically replace destination
→ durably flush containing directory where supported

Requirements:

* no in-place truncation;
* no partially written visible JSON;
* no fixed shared temporary filename;
* temporary files cleaned after failure where possible;
* exact final newline boundary;
* reject oversized input before allocation when reading.

Define explicit limits:

pub const MAX_SESSION_RECORD_BYTES: usize = 128 * 1024;
pub const MAX_SESSION_STATUS_BYTES: usize = 64 * 1024;

Equivalent conservative limits are acceptable.

Do not silently ignore directory-sync failures on platforms where the operation is supported and meaningful. Represent unsupported platform behavior deliberately.

Strict decoding

Use strict schema decoding:

* reject unknown schema versions;
* reject unknown fields;
* reject duplicate JSON keys;
* reject invalid state identifiers;
* reject invalid operation and protocol identifiers;
* reject zero revisions;
* reject invalid timestamps;
* reject failure/state inconsistency;
* reject identity/path mismatch;
* reject documents above their size limits;
* reject NUL bytes;
* require exactly one JSON document and final newline.

Do not repair malformed durable state automatically.

Return typed corruption errors.

Session store API

Provide an opaque store equivalent to:

pub struct SessionStore {
    root: SessionOperationRoot,
}

Provide operations equivalent to:

pub fn open(
    root: SessionOperationRoot,
) -> Result<Self, SessionStoreError>;
pub fn create_prepared(
    &self,
    request: NewSessionRequest,
) -> Result<PreparedSession, SessionStoreError>;
pub fn load(
    &self,
    session: &SessionInvocationIdentity,
) -> Result<SessionRecordV1, SessionStoreError>;
pub fn acquire_lease(
    &self,
    session: &SessionInvocationIdentity,
) -> Result<SessionLease, SessionLeaseError>;
pub fn transition(
    &self,
    lease: &SessionLease,
    expected_revision: u64,
    transition: SessionTransition,
) -> Result<SessionRecordV1, SessionStoreError>;
pub fn read_summary(
    &self,
) -> Result<SessionStatusSummaryV1, SessionStoreError>;

Do not expose an unchecked constructor.

A lease must be proven to belong to the same store and session before mutation.

Prepared session capability

Return a typed prepared-session value after creation.

Equivalent:

pub struct PreparedSession {
    record: SessionRecordV1,
}

Do not expose a public constructor.

This value proves:

* the session directory exists;
* the initial detailed record is durable;
* the root summary has been updated consistently;
* the state is Prepared.

Child-side session binding

Define a Core API that binds an already-admitted runtime invocation to an existing prepared session.

Equivalent:

pub fn bind_runtime_session(
    store: &SessionStore,
    envelope: &RuntimeInvocationEnvelopeV1,
) -> Result<
    BoundRuntimeSession,
    RuntimeSessionBindingError,
>;

Validate exact agreement for:

* project;
* runtime source;
* protocol;
* operation;
* source contract version;
* session ID;
* execution mode;
* supervision mode.

Do not invoke a handler.

Do not construct acquisition or processing contexts yet.

The bound value must have no public unchecked constructor.

Child transition API

Provide source-code foundations for the later managed runner:

impl BoundRuntimeSession {
    pub fn enter_running(
        self,
        lease: &SessionLease,
    ) -> Result<
        RunningRuntimeSession,
        SessionStoreError,
    >;
}
impl RunningRuntimeSession {
    pub fn complete(
        self,
        lease: &SessionLease,
    ) -> Result<
        SessionRecordV1,
        SessionStoreError,
    >;
    pub fn fail_source(
        self,
        lease: &SessionLease,
        error: &dyn std::error::Error,
    ) -> Result<
        SessionRecordV1,
        SessionStoreError,
    >;
    pub fn fail_runtime(
        self,
        lease: &SessionLease,
        error: &dyn std::error::Error,
    ) -> Result<
        SessionRecordV1,
        SessionStoreError,
    >;
}

Equivalent consuming type-state APIs are preferred.

Do not let a caller call complete before entering running.

Sanitize stored error messages.

Do not catch panics in this milestone.

Framework supervisor session module

Create:

lexicon-framework/src/session/
├── mod.rs
├── coordinator.rs
├── selection.rs
└── reconciliation.rs

Equivalent structure is acceptable.

Export a typed coordinator through the framework library, not the CLI parser.

Representative API:

pub struct SessionCoordinator {
    store: SessionStore,
}

Provide source-level operations for:

prepare_run(...)
prepare_resume(...)
abandon_failed(...)
reconcile_stale(...)

Each prepared supervisor operation must return an owning value containing:

* the durable prepared record;
* the held SessionLease;
* the invocation envelope to be transported later.

Equivalent:

pub struct PreparedSupervisedInvocation {
    envelope: RuntimeInvocationEnvelopeV1,
    lease: SessionLease,
    record: SessionRecordV1,
}

Keep fields private and provide accessors.

Dropping this value releases the lease but must not delete durable session state.

Run selection

prepare_run must:

1. validate the operation root;
2. create a new unique session ID;
3. acquire the session lease;
4. write the detailed prepared record;
5. update the root summary;
6. construct the exact invocation envelope;
7. return the prepared invocation with its lease held.

Do not launch a runtime.

Resume selection

For HTTP acquisition resume:

* require RuntimeExecutionMode::Resume;
* require a selected resumable prior session according to explicit state rules;
* require the runtime identity to match;
* require the source descriptor’s resume availability to be checked later during child admission;
* acquire the lease before mutation;
* reject succeeded or abandoned sessions.

Processing resume remains unsupported.

Do not infer resume-handler registration from filesystem state.

Abandonment

Provide an explicit abandonment operation for a prepared or failed session.

It must:

* acquire the lease;
* transition durably to Abandoned;
* update the root summary;
* preserve detailed history;
* not delete raw or processed data;
* not delete the session directory.

Do not implement CLI --abandon-past-fail yet.

Stale reconciliation

A session is stale when:

* its durable state is Running;
* no process holds its session lease;
* a supervisor successfully acquires that lease.

Do not decide staleness from elapsed wall-clock time alone.

Reconciliation must transition:

running → failed(stale-ownership)

and update the root summary.

If the lease cannot be acquired, the session is still owned and must not be reconciled.

Do not kill processes in this milestone.

Context preparation boundary

Add typed path information needed by future context construction:

pub struct SessionDataPaths {
    protocol_root: PathBuf,
    raw_data_directory: PathBuf,
    processed_data_directory: PathBuf,
    operation_root: PathBuf,
    session_directory: PathBuf,
}

Construct it only from validated roots and identities.

Do not yet:

* add HTTP clients;
* allocate raw transactions;
* open SQLite;
* modify HttpAcquisitionContext;
* modify ProcessingContext;
* change generated runner templates.

This milestone supplies the validated session/path foundation those contexts will consume next.

Typed errors

Define typed errors for:

* invalid operation root;
* unsafe path or symlink escape;
* session not found;
* session already exists;
* session already locked;
* lock I/O;
* malformed session record;
* malformed root summary;
* oversized state;
* atomic write failure;
* directory durability failure;
* identity mismatch;
* operation mismatch;
* illegal execution mode;
* illegal transition;
* revision conflict;
* lease/store mismatch;
* stale reconciliation conflict;
* timestamp failure;
* session-ID generation failure.

Implement:

std::fmt::Display
std::error::Error

Use source() for nested I/O and decoding failures.

Do not return plain String inside Core or framework session engines.

The eventual CLI boundary may format typed framework errors later.

Source-code-only instruction

Do not create tests in this milestone.

Do not run:

cargo test
cargo check
cargo build
cargo fmt
cargo clippy

Do not launch generated runtimes.

Do not run workspace validation or the bundle/install pipeline.

Validation will occur once during the final project-wide validation phase.

The completion report must describe implementation structure and explicitly state that validation was deferred by instruction.

Preserve existing behavior

Do not change:

* managed runner templates;
* managed runner template version;
* immutable Core Git revision pin;
* source scaffolding;
* source build;
* runtime probe behavior;
* runtime verification;
* staging;
* paired publication;
* invocation JSON;
* argv transport;
* handler signatures;
* acquisition or processing admission;
* normal invocation execution;
* CLI commands;
* MZA;
* Protocol 1;
* installer behavior.

Explicit exclusions

Do not implement:

* project-path transport to a child;
* managed runner session integration;
* process launching;
* data --get;
* data --process;
* lexicon build;
* HTTP transport;
* HTTP requests;
* redirects;
* retries;
* redaction;
* raw transaction recording;
* checkpoints;
* SQLite processing;
* foreground supervision;
* background supervision;
* __operator-host;
* signals or process termination;
* automatic user-source migration;
* cross-compilation;
* tests or validation commands.

Completion report

After implementation, replace current.md with a source-code report containing:

* files created and changed;
* Core session module structure;
* framework session module structure;
* session schema;
* detailed session record;
* root summary schema;
* stable state identifiers;
* legal transition table;
* revision behavior;
* operation-root validation;
* session path derivation;
* atomic persistence implementation;
* session size limits;
* strict decoding behavior;
* session lease implementation and platform behavior;
* prepared-session type state;
* bound/running session type states;
* child transition APIs;
* framework coordinator APIs;
* run preparation;
* resume preparation;
* abandonment behavior;
* stale reconciliation behavior;
* session data-path representation;
* typed error hierarchy;
* confirmation that generated runners and build integration were unchanged;
* confirmation that no tests or validation commands were run.

Then stop.