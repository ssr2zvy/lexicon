Current implementation milestone: foreground supervision closure and lifecycle correctness

Objective

Correct and complete the foreground runtime-supervision path currently pushed to main.

The existing implementation can discover a project, admit a runtime bundle, prepare a session, construct an invocation, launch the runtime, and inspect its termination. However, it currently releases session ownership before reconciliation and can leave sessions in nonterminal states after several failure paths.

This milestone must establish the exact foreground supervision contract required by:

workspace/specs/contract.md

The contract remains the source of truth.

Do not begin HTTP transport, raw transaction recording, processing SQLite behavior, or background supervision.

Contract-owned responsibility boundary

The contract assigns foreground ownership as follows:

supervising Lexicon process
├── select, create, or resume a session
├── acquire session locks
├── apply --abandon-past-fail
├── launch the source runtime
├── observe process exit and signals
└── reconcile abnormal termination
linked Core inside the source runtime
├── validate the invocation
├── enter running state
├── record ordinary source failure
└── record normal completion

For foreground execution, the original Lexicon process is the supervisor.

It must retain durable session ownership for the complete lifecycle:

session preparation
→ pre-launch preparation
→ process creation
→ child execution
→ child termination observation
→ durable terminal reconciliation
→ final outcome
→ lease release

The supervisor lease must not be released between child termination and reconciliation.

Repository-grounded defects

Correct the following current-source defects.

1. Supervisor lease is released before reconciliation

launch_and_wait(...) currently drops PreparedSessionLaunch immediately after child.wait().

reconcile_termination(...) is called only afterward.

This means terminal state is inspected or modified without the supervisor lease.

Retain the owning lease value through the complete reconciliation operation.

2. Wait failure can release ownership while the child may remain alive

A child.wait() error currently returns through ?.

That drops:

* the child handle;
* the prepared launch owner;
* the supervisor lease.

It does not prove the child terminated and does not reconcile the session.

A wait failure must enter a typed recovery path that preserves ownership while determining or forcing child termination.

3. Invocation-construction failure leaves Prepared

The session is prepared before constructing RuntimeInvocationEnvelopeV1.

If envelope construction fails, the prepared owner is dropped without recording a terminal failure.

4. Invocation-encoding failure leaves Prepared

If encode_runtime_invocation(...) fails, the same prepared session can remain durable as Prepared after its lease is released.

5. Integrity-failure persistence is discarded

The pre-launch integrity branch currently uses:

let _ = prepared.fail_launch(...);

If terminal persistence fails, that error is lost.

6. Successful reconciliation does not verify the root summary

The zero-exit path loads the detailed session record but does not prove that:

session_status.json

identifies the same session, state, runtime, project, operation, and revision.

7. Filesystem layout validation accepts wrong file types

Several layout checks use only:

Path::exists()

A regular file can therefore satisfy a required directory check.

8. Process execution lacks an owning launcher abstraction

std::process::Command usage is embedded directly inside the main foreground pipeline.

Introduce a narrow ownership-oriented launcher seam.

Do not create a generic subprocess framework.

9. Internal errors are still stringified

Foreground execution stores broad String variants for project discovery, project configuration, path validation, invocation construction, invocation encoding, and reconciliation details.

Preserve typed nested errors across the Framework boundary.

10. Failure identity is rendered through debug strings

Failure kind and failure code are converted using:

format!("{:?}", ...)

Use established stable identifiers and typed values.

Do not make debug formatting part of a public or durable compatibility surface.

Required architectural result

The foreground execution flow must become:

resolve project/source/layout
→ admit runtime bundle
→ reconcile stale prior session
→ select run/resume/abandon policy
→ prepare session and acquire supervisor lease
→ construct invocation
→ encode argv
→ recheck executable integrity
→ spawn exact executable
→ retain child handle and lease in one owning value
→ wait for termination
→ reconcile detailed session and root summary
→ release ownership
→ return typed outcome

No post-preparation error path may simply drop the lease and return while the session remains Prepared or Running.

Foreground execution owner

Introduce a single internal owner equivalent to:

pub struct PreparedForegroundExecution {
    prepared: PreparedSessionLaunch,
    operation: DataOperation,
    project: ProjectIdentity,
    source: String,
}

After successful spawn, consume it into:

pub struct RunningForegroundExecution {
    child: std::process::Child,
    prepared: PreparedSessionLaunch,
    operation: DataOperation,
    project: ProjectIdentity,
    source: String,
}

Equivalent naming and organization are acceptable.

Required ownership properties:

* neither type is Clone;
* fields remain private;
* the prepared owner retains the session lease;
* the running owner retains both child handle and session lease;
* conversion from prepared to running occurs only after successful spawn;
* terminal reconciliation consumes the running owner;
* the lease is released only after reconciliation completes or produces its final structured failure.

Do not store source arguments in either long-lived owner after the child has been spawned unless needed for process construction.

Do not implement Drop by silently mutating durable state.

Narrow launcher seam

Define a focused launcher abstraction equivalent to:

pub trait ForegroundRuntimeLauncher {
    fn spawn(
        &self,
        executable: &Path,
        arguments: &[OsString],
        context_environment: &OsStr,
        working_directory: &Path,
    ) -> Result<std::process::Child, ForegroundRuntimeSpawnError>;
}

Production implementation uses:

std::process::Command

The seam exists to separate:

* preparation;
* spawning;
* child ownership;
* waiting;
* reconciliation.

It must not expose:

* shell commands;
* arbitrary environment maps;
* arbitrary executable search;
* alternate entrypoints.

Production launch behavior remains:

* exact admitted executable path;
* exact encoded invocation arguments;
* no shell;
* no PATH lookup;
* protocol root as the documented working directory;
* inherited stdin;
* inherited stdout;
* inherited stderr;
* canonical runtime-context environment variable set;
* inherited LEXICON_RUNTIME_CONTEXT_V1 overwritten;
* inherited LEXICON_SOURCE_DIRECTORY removed.

Do not capture or persist child stdout or stderr in this milestone.

Prepared-phase failure handling

After PreparedSessionLaunch exists, every error before successful spawn must transition the session to Failed.

Provide one centralized operation equivalent to:

fn fail_prepared_execution(
    prepared: PreparedSessionLaunch,
    failure_code: SessionFailureCode,
    cause: ForegroundPreparationError,
) -> ForegroundDataExecutionError;

Required order:

retain lease
→ transition Prepared to Failed
→ update root summary
→ release lease
→ return error

Apply it to:

* invocation project-identity construction;
* invocation session-identity construction;
* invocation-envelope construction;
* invocation transport encoding;
* executable metadata recheck;
* executable hash recheck;
* executable mutation detection;
* command construction if it can fail before spawn;
* process spawn failure.

Use distinct stable failure codes where the existing model permits:

InvocationConstructionFailed
InvocationEncodingFailed
ExecutableIntegrityFailed
LaunchFailed

Add stable codes if necessary.

Do not persist the nested error’s arbitrary Display text.

The returned Framework error must retain the original typed cause.

Combined preparation and persistence errors

If a post-preparation operation fails and transitioning the session to Failed also fails, return one typed combined error equivalent to:

ForegroundDataExecutionError::PreparationFailureAndPersistenceFailure {
    preparation: ForegroundPreparationError,
    persistence: SessionCoordinationError,
}

Both nested errors must remain available.

std::error::Error::source() can return only one source. Provide typed accessors for both where needed.

Do not discard either error.

Do not collapse them into one formatted String.

Spawn transition

On successful spawn:

* do not release the prepared owner;
* move the Child and PreparedSessionLaunch into RunningForegroundExecution;
* do not assume the child has already entered Running;
* do not alter the child’s session record from the parent merely because spawn succeeded.

The linked Core child remains responsible for:

Prepared → Running

The supervisor reconciles only when startup or termination fails abnormally.

Wait behavior

Implement a wait loop that handles interruption correctly.

For:

std::io::ErrorKind::Interrupted

retry waiting without releasing ownership.

Do not add arbitrary sleeps.

For another wait error:

1. retain the child handle;
2. retain the supervisor lease;
3. attempt to determine whether the child already terminated;
4. if still running, attempt controlled termination using Child::kill();
5. reap the child with wait();
6. inspect durable session state;
7. reconcile a nonterminal session to failed;
8. return a typed combined wait/recovery result.

Do not return from a wait error while knowingly leaving a live child unsupervised.

Wait-recovery failure

If the supervisor cannot:

* determine child state;
* terminate the child;
* or reap it;

return a typed supervision failure containing:

* the original wait error;
* any termination error;
* any reap error;
* any session reconciliation error.

Do not include source arguments or environment contents.

Continue holding ownership during every recovery attempt.

The owner may be released only when:

* the child is known to have terminated; or
* no further safe action is available and the final typed error explicitly reports incomplete supervision.

Do not intentionally leak memory, files, child handles, or lease handles.

Terminal reconciliation owner

Move reconciliation onto the owning running value.

Representative API:

impl RunningForegroundExecution {
    pub fn wait_and_reconcile(
        self,
    ) -> Result<
        ForegroundDataOutcome,
        ForegroundDataExecutionError,
    >;
}

Internally, keep the lease alive while:

* waiting;
* loading the detailed session record;
* loading root status;
* applying abnormal transitions;
* checking consistency;
* constructing the final outcome.

Do not return ObservedChildTermination to a caller that no longer holds the execution owner.

ObservedChildTermination may remain as an internal typed observation.

Detailed session validation

After child termination, load the exact prepared session record.

Require agreement with the prepared launch for:

* project identity;
* runtime identity;
* session identity;
* operation;
* execution mode;
* supervision mode;
* expected immutable fields.

Do not trust a record merely because it exists at the expected path.

Return a typed identity-disagreement error.

Do not overwrite a mismatched record.

Root-summary validation

Load:

session_status.json

after loading the detailed record.

For a successfully reconciled invocation, require exact agreement for:

* schema version;
* project identity;
* runtime identity;
* operation;
* current session identity;
* current session state;
* revision;
* updated timestamp where the schema requires agreement.

The root summary must identify the same current session.

A zero child exit plus a Succeeded detailed record is not a successful outcome unless the root summary agrees.

If the detailed record is authoritative and the summary is missing or stale:

* attempt the established rebuild_status_from_record(...) recovery while the supervisor lease remains held;
* reload the summary;
* require exact agreement.

If recovery fails, return a typed reconciliation error.

Do not silently return success with inconsistent root status.

Zero-exit behavior

For:

exit code 0

Detailed state is Succeeded

Validate or rebuild root summary.

Return success only after both agree.

Detailed state is Failed

Preserve the failed record.

Return a typed child-failure result.

Do not rewrite it to succeeded.

Detailed state is Prepared or Running

While retaining the lease:

1. transition it to Failed;
2. use SessionFailureKind::AbnormalTermination;
3. use a stable ZeroExitWithoutCompletion failure code;
4. update root status;
5. return a typed zero-exit/session-incomplete error.

Detailed state is Abandoned

Return a typed state disagreement.

Do not mutate it.

Nonzero-exit behavior

For a nonzero exit code:

Detailed state is Failed

Validate or rebuild root status.

Return a typed child failure using:

* typed SessionFailureKind;
* typed SessionFailureCode;
* exit code;
* source identifier;
* operation;
* session identity.

Detailed state is Prepared or Running

Transition it to failed while holding the lease.

Use:

SessionFailureKind::AbnormalTermination
SessionFailureCode::NonzeroExitWithoutFailureRecord

Persist the exit code only through a bounded Core-authored structured field if supported.

Do not persist stderr.

Detailed state is Succeeded

Return a typed exit/session disagreement.

Do not rewrite it to failed.

Detailed state is Abandoned

Return a typed exit/session disagreement.

Signaled or unknown abnormal termination

For a signaled Unix termination or an abnormal termination without a usable exit code:

* retain the lease;
* load the detailed record;
* preserve an existing Succeeded or Failed terminal record;
* transition Prepared or Running to Failed;
* use SessionFailureKind::AbnormalTermination;
* use a stable failure code;
* update root status;
* validate the resulting summary;
* return a typed abnormal-termination result.

A Unix signal number may be retained as bounded structured diagnostic data.

Do not invent a signal value on Windows.

Do not persist panic payloads.

Process-wait ownership on success and failure

The following invariant must hold:

child may be alive
⇒ foreground execution owner exists
⇒ supervisor lease remains held

After the child is known dead:

lease remains held
⇒ terminal reconciliation runs
⇒ final result is constructed
⇒ lease is released

Document this invariant next to the owner types.

Integrity failure correction

Replace:

let _ = prepared.fail_launch(...)

with the centralized typed prepared-failure path.

If the executable changed after bundle admission:

* preserve the original integrity error;
* transition the session to Failed(ExecutableIntegrityFailed);
* preserve a persistence error if that transition fails;
* do not launch the executable.

Do not include executable contents in diagnostics.

Typed invocation errors

Replace:

InvocationConstruction(String)
InvocationEncoding(String)

with nested typed variants.

Preserve:

RuntimeInvocationConstructionError
RuntimeInvocationTransportEncodingError
RuntimeInvocationValueError

as applicable.

Do not convert them with .to_string() inside Framework.

Typed project and layout errors

Introduce focused typed errors equivalent to:

ProjectDiscoveryError
ProjectConfigurationError
RuntimeProjectLayoutError

Preserve nested:

* current-directory I/O errors;
* project-file read errors;
* TOML decoding errors;
* project-schema errors;
* source-identity errors;
* filesystem metadata errors;
* path-containment errors.

Replace broad variants such as:

ProjectDiscovery(String)
ProjectConfiguration(String)
ConfiguredSourcesRoot(String)
TrustedPathConstruction(String)
InvalidSourceIdentity(String)

Do not stringify these errors inside the data pipeline.

Filesystem type validation

Replace exists()-only validation with symlink_metadata() or the established appropriate metadata API.

Require directories for:

* configured sources root;
* source root;
* HTTP protocol root;
* selected operation root;
* data/raw;
* data/processed;
* runtime bundle root before admission.

Return distinct typed errors for:

* missing path;
* symlink where prohibited by the established path policy;
* regular file where a directory is required;
* metadata I/O failure.

Do not attempt hostile filesystem sandboxing.

Continue relying on bundle admission for exact runtime-bundle contents.

Typed failure values in outcomes

Do not convert:

SessionFailureKind
SessionFailureCode

through debug formatting.

Define stable identifier accessors if they do not already exist:

pub const fn identifier(&self) -> &'static str;

Prefer retaining typed values in errors:

ForegroundDataExecutionError::ChildFailed {
    operation: DataOperation,
    source: String,
    session: SessionIdentity,
    failure_kind: SessionFailureKind,
    failure_code: SessionFailureCode,
    exit_code: i32,
}

Use Display only at the CLI boundary.

Remove free-form disagreement strings

Replace variants such as:

ExitSessionDisagreement {
    detail: String,
}

with typed fields:

ExitSessionDisagreement {
    termination: ObservedChildTermination,
    durable_state: SessionState,
}

Replace combined free-form detail strings with nested typed errors.

Do not make error text a data model.

Root status helpers

Add focused Framework or Core helpers for:

load_and_validate_terminal_session(...)
validate_root_summary_against_record(...)
rebuild_and_validate_root_summary(...)

Equivalent organization is acceptable.

Do not duplicate session-record decoding or status decoding.

Use existing SessionStore operations.

Do not add a second session persistence implementation.

Foreground outcome guarantee

ForegroundDataOutcome may be returned only when all are true:

* the exact admitted executable was launched;
* the child exited with code zero;
* detailed session state is Succeeded;
* detailed record identities match the prepared invocation;
* root summary identifies the same session;
* root summary state and revision agree;
* no reconciliation error remains;
* the supervisor retained its lease through these checks.

Document this guarantee on the type.

CLI behavior

Preserve the current CLI syntax.

On success, print the concise source, operation, and session result.

On failure, render the typed Framework error once at the CLI boundary.

Do not print:

* source arguments;
* invocation-envelope JSON;
* runtime-context JSON;
* child environment;
* session-record JSON;
* arbitrary source error messages.

Do not add background behavior.

Launcher visibility

Keep the launcher seam internal or narrowly public for future Framework use.

Do not expose it as a user extension point.

Source implementations must not select launch behavior.

Lexicon owns the supported runtime entrypoint and process lifecycle.

Source-code test policy

Do not execute tests.

Existing test source may be adjusted only where API changes make it structurally obsolete.

Do not create a broad new test suite in this milestone.

Comprehensive testing and execution remain deferred to the final project validation phase.

Commands that must not be run

Do not run:

cargo test
cargo check
cargo build
cargo fmt
cargo clippy
cargo metadata
rustc

Do not execute the CLI data command.

Do not execute generated runtimes.

Do not run workspace validation.

Do not run the bundle/install pipeline.

Use static source inspection only.

Preserve existing behavior

Do not change:

* CLI commands or argument syntax;
* native source-argument preservation;
* lexicon init;
* lexicon source create;
* lexicon source build;
* project configuration schema;
* source-manifest schema;
* managed workspace layout;
* managed runner templates except necessary API-alignment corrections;
* managed package and binary names;
* immutable Core revision pin;
* invocation-envelope JSON;
* invocation argv layout;
* runtime-context environment variable name;
* runtime-information probe protocol;
* probe stdout/stderr behavior;
* probe limits or timeout;
* HTTP and processing admission;
* source handler signatures;
* HTTP capability identifiers;
* executable hashing algorithm;
* runtime manifest formats;
* bundle directory formats;
* runtime verification;
* staging;
* bundle admission;
* paired publication;
* session schema except narrowly adding stable failure codes or identifiers;
* MZA;
* Protocol 1;
* installer behavior.

Explicit exclusions

Do not implement:

* HTTP request execution;
* HTTP client configuration;
* redirects;
* retries;
* rate limiting;
* authorization handling;
* secret redaction;
* raw transaction recording;
* raw transaction metadata;
* request or response body storage;
* checkpoints;
* source resume semantics beyond selecting the existing resume handler;
* processing SQLite behavior;
* raw transaction discovery for processing;
* background execution;
* __operator-host;
* daemonization;
* detached processes;
* process groups;
* complete signal forwarding;
* user cancellation;
* lexicon build;
* automatic source migration;
* cross-compilation;
* MZA or installer changes.

Completion report

After implementation, replace current.md with a report containing:

* files created and changed;
* contract responsibility boundary preserved;
* foreground owner types;
* prepared-to-running ownership transition;
* exact supervisor lease lifetime;
* confirmation that reconciliation occurs before lease release;
* launcher seam;
* exact production command behavior;
* invocation-construction failure behavior;
* invocation-encoding failure behavior;
* integrity-failure behavior;
* combined preparation/persistence error behavior;
* spawn-failure behavior;
* interrupted-wait behavior;
* non-interrupted wait-failure recovery;
* child termination and reap behavior;
* wait-recovery failure behavior;
* detailed session identity validation;
* root-summary validation;
* root-summary rebuild behavior;
* zero-exit reconciliation;
* nonzero-exit reconciliation;
* signaled termination reconciliation;
* unknown abnormal termination reconciliation;
* exit/session disagreement behavior;
* filesystem metadata and type validation;
* typed project-discovery errors;
* typed project-configuration errors;
* typed runtime-layout errors;
* typed invocation errors;
* typed child failure kind and code behavior;
* free-form disagreement strings removed;
* final ForegroundDataOutcome guarantee;
* CLI diagnostic behavior;
* confirmation that source arguments, envelope JSON, context JSON, and arbitrary source errors are not printed or persisted;
* confirmation that no HTTP transport, raw recording, checkpoints, SQLite, background host, or lexicon build was implemented;
* any existing test source adjusted only for API alignment;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, generated-runtime execution, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin the HTTP transaction engine until foreground supervision closure is complete.