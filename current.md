Current implementation milestone: foreground reconciliation closure

Objective

Complete the remaining foreground supervision and reconciliation corrections in the implementation currently pushed to main.

The source now has:

* explicit prepared and running foreground owners;
* a narrow process-launcher seam;
* supervisor lease retention during the ordinary wait path;
* typed pre-launch failure handling;
* detailed-session identity validation;
* root-summary validation and rebuilding;
* typed child failure kinds and codes.

However, several error paths still discard durable-state failures or release ownership without proving the child has terminated.

This milestone closes those remaining defects.

workspace/specs/contract.md remains the source of truth.

Do not begin the Core HTTP transaction engine until this closure is complete.

Contract boundary

The contract requires:

supervising Lexicon process
├── select, create, or resume a session
├── acquire session locks
├── apply --abandon-past-fail
├── launch the source runtime
├── observe process exit and signals
└── reconcile abnormal termination

For foreground execution, the original Lexicon process owns this complete sequence.

The required invariant is:

child may still be alive
⇒ supervisor retains child ownership
⇒ supervisor retains session lease

After confirmed child termination:

supervisor retains session lease
⇒ durable session reconciliation completes
⇒ root summary is validated
⇒ final result is constructed
⇒ lease is released

No error path may silently bypass this boundary.

Repository-grounded defects to correct

1. Nonzero child failure discards root-summary reconciliation errors

The current nonzero-exit/failed-session path calls:

let _ = validate_or_rebuild_summary_if_needed(...);

It then returns ChildFailed even if:

* session_status.json is missing;
* the summary is corrupt;
* the summary identifies another session;
* the summary has the wrong state or revision;
* rebuilding fails;
* revalidation fails.

Remove this discarded result.

A child failure is not fully reconciled until the detailed record and root summary agree.

2. Signal reconciliation discards detailed-record load failures

The current signal path uses:

if let Ok(record) =
    load_terminal_session(...)

If the detailed record is missing, corrupt, or unreadable, execution falls through and returns only AbnormalTermination.

Preserve and return the typed session-loading error.

3. Signal reconciliation validates an obsolete record

When a signaled child leaves the session Prepared or Running, the supervisor transitions it to Failed.

The current code then validates the root summary against the old pre-transition record.

Use the SessionRecordV1 returned by the successful transition.

Never validate a post-transition summary against a stale record revision or state.

4. Root-summary helper erases all errors

The current helper returns:

Result<(), ()>

and discards:

* operation-root construction errors;
* session-store opening errors;
* status loading errors;
* status decoding errors;
* validation disagreements;
* rebuild failures;
* post-rebuild validation failures.

Replace it with a fully typed result.

5. Wait recovery can drop ownership without confirming termination

After a non-interrupted Child::wait() error, the current code:

1. calls Child::kill();
2. calls Child::wait() once;
3. stores any errors;
4. drops the child and lease;
5. returns.

If killing or reaping fails, the child may still be alive when ownership is released.

Correct this recovery path.

6. Wait recovery ignores missing or corrupt session state

The current wait-recovery path reconciles only when:

load_terminal_session(...)

returns Ok.

A load failure is discarded instead of being retained in WaitRecoveryFailure.

7. Integrity-error dispatch contains unreachable!

The current adapter assumes the integrity function can return exactly two variants:

Err(_) => unreachable!(...)

A later variant can therefore turn an ordinary typed failure into a panic.

Remove this assumption.

8. Root-summary validation remains string-based

The current API is:

validate_root_summary_against_record(...)
    -> Result<(), String>

Replace free-form mismatch strings with typed validation errors.

9. Detailed-record identity disagreement remains string-based

SessionIdentityDisagreement currently stores formatted expected and actual strings.

Preserve typed identity values or use a typed mismatch enum.

Do not use debug-formatted strings as an internal data model.

10. Project discovery and configuration remain partly string-based

The current wrappers still contain:

ProjectDiscoveryError::FindRoot(String)
ProjectConfigurationError::Other(String)

because the shared project helpers return String.

Move the shared project discovery and configuration helpers to typed errors so the foreground path does not stringify and re-wrap them.

Authoritative reconciliation API

Create one authoritative operation equivalent to:

pub fn reconcile_terminal_execution(
    owner: RunningForegroundExecution,
    termination: ObservedChildTermination,
) -> Result<
    ForegroundDataOutcome,
    ForegroundDataExecutionError,
>;

It must own the supervisor lease throughout:

* detailed-record loading;
* identity validation;
* terminal transition where required;
* root-summary loading;
* root-summary rebuilding;
* root-summary revalidation;
* final result construction.

Do not maintain separate best-effort reconciliation paths that discard different errors.

Typed root-summary validation

Define an error equivalent to:

#[derive(Debug)]
pub enum RootSummaryValidationError {
    Missing,
    Load(SessionStoreError),
    SchemaVersionMismatch {
        expected: u32,
        actual: u32,
    },
    ProjectMismatch,
    RuntimeMismatch,
    OperationMismatch,
    MissingCurrentSession,
    SessionMismatch,
    MissingCurrentState,
    StateMismatch {
        expected: SessionState,
        actual: SessionState,
    },
    RevisionMismatch {
        expected: u64,
        actual: u64,
    },
}

Equivalent typed organization is acceptable.

Provide:

pub fn validate_root_summary_against_record(
    store: &SessionStore,
    record: &SessionRecordV1,
) -> Result<(), RootSummaryValidationError>;

Do not return a plain String.

Do not format entire records into errors.

Established non-secret identity values may be represented through their stable identifiers where required.

Typed root-summary reconciliation

Define:

#[derive(Debug)]
pub enum RootSummaryReconciliationError {
    Validation(
        RootSummaryValidationError,
    ),
    Rebuild(
        SessionStoreError,
    ),
    ValidationAfterRebuild(
        RootSummaryValidationError,
    ),
}

Equivalent naming is acceptable.

Provide one helper:

pub fn validate_or_rebuild_root_summary(
    store: &SessionStore,
    record: &SessionRecordV1,
) -> Result<(), RootSummaryReconciliationError>;

Required order:

validate current summary
→ if valid: success
→ if invalid: rebuild from exact detailed record
→ reload and revalidate
→ success only if revalidation passes

Do not return Result<(), ()>.

Do not discard a rebuild error.

Do not report success merely because rebuild_status_from_record(...) returned successfully; revalidation remains mandatory.

Terminal record and summary result

Provide one typed validated result equivalent to:

pub struct ReconciledTerminalSession {
    record: SessionRecordV1,
}

It may be constructed only after:

* detailed record identity agrees with the prepared record;
* record is in the expected terminal state for the selected result;
* root summary agrees after any required rebuild.

Keep its field private.

Do not provide an unchecked public constructor.

Detailed identity mismatch

Replace string-based mismatch fields with a typed representation such as:

pub enum TerminalSessionIdentityMismatch {
    Project,
    Runtime,
    Session,
    Operation,
    ExecutionMode {
        expected: RuntimeExecutionMode,
        actual: RuntimeExecutionMode,
    },
    SupervisionMode {
        expected: RuntimeSupervisionMode,
        actual: RuntimeSupervisionMode,
    },
}

For project, runtime, and session mismatches, retain typed expected and actual values only when those types are established non-secret identifiers.

Do not use:

format!("{:?}", ...)

to produce stored error fields.

Successful zero exit

For:

exit code 0

and detailed state:

Succeeded

require:

1. exact detailed-record identity agreement;
2. valid root summary or successful rebuild;
3. successful post-rebuild validation;
4. lease still held through all checks.

Only then return ForegroundDataOutcome.

If root-summary reconciliation fails, return the typed reconciliation error.

Do not discard it.

Zero exit with Failed

If the child exits zero but Core recorded Failed:

1. validate detailed-record identity;
2. validate or rebuild root summary;
3. return typed ChildFailed.

Do not rewrite the session to succeeded.

Do not return ChildFailed if summary reconciliation failed; return the reconciliation error instead.

Zero exit with Prepared or Running

Transition to:

Failed
AbnormalTermination
ZeroExitWithoutCompletion

Use the SessionRecordV1 returned by the transition.

Then:

1. validate the returned failed record’s identity;
2. validate or rebuild root summary against the returned record;
3. return ZeroExitSessionIncomplete.

If transition or summary reconciliation fails, preserve the typed nested error.

Nonzero exit with Failed

For an ordinary child failure already recorded by Core:

1. validate detailed-record identity;
2. validate or rebuild the root summary;
3. return typed ChildFailed.

Remove:

let _ = validate_or_rebuild_summary_if_needed(...)

The returned child failure must retain:

* DataOperation;
* source identifier;
* SessionIdentity;
* SessionFailureKind;
* SessionFailureCode;
* exit code.

Do not convert these fields to strings before the CLI boundary.

Nonzero exit with Prepared or Running

Transition to:

Failed
AbnormalTermination
NonzeroExitWithoutFailureRecord

Use the returned failed record for root-summary reconciliation.

Do not validate the old prepared or running record afterward.

Return AbnormalTermination only after the resulting failed detailed record and root summary agree.

Signaled or unknown abnormal termination

Remove the current:

if let Ok(record)

behavior.

Required order:

load exact detailed record
→ preserve typed load failure
→ validate identity
→ inspect state

For Prepared or Running:

transition to Failed
→ use returned failed record
→ validate/rebuild root summary
→ return typed abnormal termination

For Failed:

preserve record
→ validate/rebuild root summary
→ return typed child/abnormal failure

For Succeeded:

preserve record
→ validate/rebuild root summary
→ return typed exit/session disagreement

For Abandoned:

do not mutate
→ return typed exit/session disagreement

Do not silently ignore record or summary errors.

Wait recovery state machine

Replace the current one-shot recovery with an explicit state machine.

Representative states:

enum WaitRecoveryState {
    WaitFailed,
    ChildAlreadyExited,
    TerminationRequested,
    TerminationObserved,
    Reaped,
    OwnershipUncertain,
}

Equivalent private organization is acceptable.

Initial wait error

For ErrorKind::Interrupted, retry ordinary waiting.

For another error:

1. retain the child;
2. retain the lease;
3. call try_wait() to determine whether the child already exited;
4. if terminated, reconcile the observed status normally;
5. if still running, request termination;
6. wait for termination;
7. retry on Interrupted;
8. reconcile the durable state;
9. release ownership only after reconciliation.

Do not immediately call kill() before checking whether the child already exited.

Kill behavior

Child::kill() failure does not by itself prove the child remains alive.

After kill failure:

* call try_wait();
* if termination is observed, continue to reap and reconcile;
* if still running or state is unknown, preserve the typed uncertainty.

Do not discard the kill error.

Reap behavior

After a successful termination request, continue waiting until:

* termination is observed; or
* a nonrecoverable operating-system error leaves ownership genuinely uncertain.

Retry Interrupted.

Do not add arbitrary sleeps.

Do not perform a single reap attempt and immediately release ownership.

Ownership-uncertain result

If operating-system errors make it impossible to determine whether the child remains alive, return a distinct typed result:

ForegroundDataExecutionError::ChildOwnershipUncertain(
    ChildOwnershipUncertainError,
)

It must retain:

* original wait error;
* try_wait error, if any;
* kill error, if any;
* reap error, if any;
* session-loading error, if any;
* session-reconciliation error, if any.

Before returning, perform every safe available child-state query and reconciliation action.

Do not claim the child terminated.

Do not claim the session was reconciled.

Do not silently downgrade this to ProcessWaitRecovery.

Because the public foreground API cannot safely return an owned live child to the CLI, document this as a fatal supervision failure requiring the next invocation’s stale-ownership reconciliation.

Do not intentionally leak the child or lease.

Wait recovery and durable session state

After termination is confirmed:

* load the exact session record;
* preserve load or decoding failures;
* validate identity;
* transition Prepared or Running to failed;
* preserve existing terminal state;
* reconcile root summary;
* return a typed wait-recovery error containing the final durable state.

Add the session-load error to WaitRecoveryFailure.

Do not use:

if let Ok(record)

for recovery.

Remove unit-error helper

Delete:

validate_or_rebuild_summary_if_needed(...)
    -> Result<(), ()>

Replace all callers with the typed authoritative helper.

Do not retain it as a compatibility wrapper.

Remove discarded reconciliation results

Remove every production pattern equivalent to:

let _ = reconcile(...);
let _ = validate(...);
if let Ok(...) { ... }

when failure affects session correctness.

Best-effort behavior is acceptable only for explicitly nonessential cleanup, not for:

* detailed session loading;
* session transitions;
* root-summary validation;
* root-summary rebuilding;
* child termination observation;
* child reaping.

Remove unreachable! from integrity adaptation

Change the executable-integrity API so it returns a dedicated typed error directly.

Preferred structure:

pub enum ExecutableIntegrityError {
    Changed {
        path: PathBuf,
        expected: ExecutableIdentity,
        actual: ExecutableIdentity,
    },
    Inspection(
        RuntimeArtifactHashError,
    ),
}

Then:

recheck_executable_integrity(...)
    -> Result<(), ExecutableIntegrityError>

The foreground preparation layer should wrap this type directly.

Do not translate from a broad top-level execution error.

Do not use unreachable!.

Typed shared project discovery

Refactor the shared helper:

find_project_root(...)

to return a typed error.

Representative variants:

pub enum ProjectRootDiscoveryError {
    CurrentDirectoryMetadata {
        path: PathBuf,
        source: io::Error,
    },
    ParentTraversal,
    ProjectNotFound,
    NestedProjectConflict,
}

Equivalent organization is acceptable.

Update existing source-create, source-build, and foreground callers to convert this typed error only at their public command boundary.

Do not change discovery behavior.

Typed shared project configuration

Refactor:

load_project_config(...)

to return a typed error.

Representative variants:

pub enum ProjectConfigLoadError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    DecodeToml {
        path: PathBuf,
        source: toml::de::Error,
    },
    UnsupportedSchemaVersion {
        actual: u32,
    },
    InvalidProjectIdentity(
        RuntimeInvocationValueError,
    ),
    InvalidSourcesDirectory,
    SourcesDirectoryTraversal,
}

Equivalent organization is acceptable.

Remove:

ProjectConfigurationError::Other(String)

when no supported caller requires it.

Do not change lexicon.toml syntax or schema.

Error formatting constraints

Diagnostics must not reveal:

* source arguments;
* invocation-envelope JSON;
* runtime-context JSON;
* environment contents;
* arbitrary source errors;
* request or response contents;
* encoded native path bytes.

Established non-secret identifiers may appear:

* project;
* source;
* protocol;
* operation;
* session;
* state;
* revision;
* failure kind;
* failure code;
* exit code;
* Unix signal number.

Source-only execution policy

Do not run tests or validation commands.

Existing test source may be adjusted only where required by changed production APIs.

Do not create a broad new test suite in this milestone.

Prohibited commands

Do not run:

cargo test
cargo check
cargo build
cargo fmt
cargo clippy
cargo metadata
rustc

Do not execute:

* the CLI data command;
* generated runtimes;
* source build;
* workspace validation;
* bundle/install automation.

Use static source inspection only.

Preserve existing behavior

Do not change:

* CLI commands or arguments;
* source-argument OsString preservation;
* project schema;
* source schema;
* managed runner templates;
* managed runner package or binary names;
* source handler signatures;
* invocation-envelope JSON;
* invocation argv layout;
* runtime-context environment format;
* runtime-information probe behavior;
* probe limits or timeout;
* HTTP or processing admission;
* session schema except narrowly typed error additions;
* bundle manifests;
* executable hashing;
* runtime verification;
* staging;
* bundle admission;
* paired publication;
* lexicon init;
* lexicon source create;
* lexicon source build;
* MZA;
* Protocol 1;
* installer behavior.

Explicit exclusions

Do not implement:

* HTTP request execution;
* HttpAcquisitionContext::execute;
* HTTP transport configuration;
* redirects;
* retries;
* rate limiting;
* authentication;
* redaction;
* raw transaction recording;
* checkpoints;
* SQLite processing;
* background execution;
* __operator-host;
* process groups;
* signal forwarding;
* user cancellation;
* lexicon build;
* automatic migration;
* cross-compilation;
* MZA or installer changes.

Completion report

After implementation, replace current.md with a report containing:

* files changed;
* contract supervision boundary;
* exact child/lease ownership invariant;
* nonzero failed-session summary handling;
* signaled-session load-error handling;
* post-transition record usage;
* typed root-summary validation error;
* typed root-summary reconciliation error;
* root-summary rebuild and mandatory revalidation;
* detailed-record identity mismatch representation;
* zero-exit reconciliation;
* nonzero-exit reconciliation;
* signaled reconciliation;
* unknown abnormal reconciliation;
* wait-recovery state machine;
* try_wait behavior;
* kill behavior;
* reap behavior;
* interrupted-wait behavior;
* ownership-uncertain behavior;
* wait-recovery session-load behavior;
* confirmation that no session load, transition, summary validation, or summary rebuild error is discarded;
* unit-error helper removal;
* let _ = reconciliation removal;
* integrity unreachable! removal;
* executable-integrity typed error;
* typed shared project-root discovery;
* typed shared project-configuration loading;
* free-form project/configuration fallback strings removed;
* final foreground success guarantee;
* confirmation that the supervisor lease remains held through all normal terminal reconciliation;
* confirmation that no HTTP transport, raw recording, checkpoints, SQLite, background host, signal forwarding, or lexicon build was implemented;
* existing test source adjusted only for API alignment, if applicable;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, generated-runtime execution, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin the Core HTTP transaction engine until this reconciliation closure is complete.