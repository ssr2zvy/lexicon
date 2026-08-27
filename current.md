Current implementation milestone: foreground data-command execution and supervision

Objective

Implement the first complete normal runtime execution path from the installed CLI to a managed acquisition or processing runtime.

The supported foreground path must become:

lexicon data --get <source> -- <source-arguments...>
or
lexicon data --process <source> -- <source-arguments...>
→ locate and validate the Lexicon project
→ resolve the configured source and HTTP protocol layout
→ admit the published runtime bundle
→ reconcile prior durable session state
→ select run, resume, or abandon-and-run policy
→ prepare the durable session and supervisor lease
→ construct the invocation envelope
→ encode native argv
→ provide the runtime-context environment document
→ launch the exact admitted runtime executable
→ retain the supervisor lease for the complete child lifecycle
→ wait for child termination
→ reconcile normal, failed, and abnormal termination
→ return a sanitized typed result to the CLI

This milestone implements foreground execution only.

Do not implement background execution, HTTP transport, raw transaction recording, checkpoints, or SQLite processing.

Repository-grounded starting point

The repository currently has:

* parsed data --get and data --process CLI modes;
* --bg;
* --abandon-past-fail;
* passthrough source arguments;
* managed acquisition and processing runtime bundles;
* bundle manifests and executable hashes;
* HTTP and processing bundle admission;
* invocation-envelope JSON;
* native argv transport;
* child-side runtime admission;
* child-side handler selection and execution;
* durable session records;
* operation-level session status;
* supervisor leases;
* run and resume session preparation;
* stale-session reconciliation;
* runtime-context environment transport;
* child-side session binding;
* child-side normal completion and ordinary failure persistence.

The CLI currently only prints a parsed-data-command diagnostic.

It does not execute a runtime.

Source-level correction before integration

Correct the current malformed Error::source() match in:

lexicon-framework/src/session/error.rs

Ensure the no-source variants are terminated correctly before matching:

SessionCoordinationError::ContextEncoding(...)
SessionCoordinationError::InvalidOperationRoot(...)

Preserve nested typed errors.

Do not use a validation command to discover or verify this correction.

Required modules

Add focused Framework modules equivalent to:

lexicon-framework/src/data/
├── mod.rs
├── request.rs
├── project.rs
├── runtime.rs
├── session.rs
├── foreground.rs
├── outcome.rs
└── error.rs

Equivalent organization is acceptable.

Do not put the complete foreground execution pipeline into the existing large lexicon-framework/src/lib.rs.

Export the public command-facing API through:

lexicon_framework::data

The CLI should call this Framework API directly.

Public Framework API

Provide a typed request representation equivalent to:

pub struct ForegroundDataRequest {
    operation: DataOperation,
    source_name: String,
    abandon_past_failure: bool,
    source_arguments: Vec<OsString>,
}

Define:

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataOperation {
    Acquisition,
    Processing,
}

Provide one public foreground entrypoint:

pub fn execute_foreground_data(
    request: ForegroundDataRequest,
) -> Result<
    ForegroundDataOutcome,
    ForegroundDataExecutionError,
>;

The framework operation must return a typed result.

Do not return String internally.

The existing CLI boundary may convert the final error to its current public representation if changing the complete CLI error architecture is outside scope.

Native source arguments

Change data-command passthrough storage from:

Vec<String>

to:

Vec<OsString>

Source arguments must remain native operating-system values from CLI parsing through child launch.

Preserve:

* order;
* duplicates;
* empty values;
* Unicode values;
* non-UTF-8 Unix values;
* Windows Unicode values;
* values beginning with -;
* values equal to --;
* values equal to --lexicon-invocation-v1;
* values equal to --lexicon-runtime-information-v1.

Do not:

* print source arguments;
* convert them to UTF-8;
* serialize them into the envelope;
* store them in session records;
* log them;
* normalize them;
* inspect reserved-looking values after the CLI delimiter.

Remove the current CLI diagnostic that formats:

command.passthrough

with Debug.

Background flag behavior

If:

--bg

is supplied, return a clear typed unsupported-mode error.

Do not silently run the request in foreground mode.

Do not implement __operator-host in this milestone.

Do not prepare or modify a session before returning the unsupported-background error.

Project discovery

Reuse the existing Framework project-discovery and project-configuration logic used by source commands.

Do not introduce a second parser for lexicon.toml.

Resolve:

* absolute project root;
* project identity;
* configured sources_directory;
* absolute configured sources root.

Reject:

* no project found;
* malformed lexicon.toml;
* unsupported project schema;
* invalid project identity;
* relative traversal escaping the project root;
* missing configured sources directory;
* configured sources path that is not a directory.

Do not silently assume:

sources/

when lexicon.toml configures another directory.

Source and protocol resolution

HTTP remains the only supported protocol.

For a requested source, derive and validate:

<project-root>/<configured-sources-directory>/<source>/http

Validate the source name through the established safe source-identifier rules.

Require:

source.toml
data/raw/
data/processed/
get-raw-data/
process-data/

as appropriate to the selected operation.

Do not automatically create a missing source or operation workspace during data execution.

Do not invoke Cargo.

Do not build a missing runtime automatically.

Return an actionable typed error instructing the user to run the established source build command when the runtime bundle is missing.

Complete trusted path binding

Close the remaining runtime-context path gap.

Add a session-independent validated project/source layout equivalent to:

pub struct RuntimeProjectLayout {
    project_root: PathBuf,
    sources_root: PathBuf,
    source_name: String,
    protocol_root: PathBuf,
}

Construct it from:

* validated project configuration;
* validated source identity;
* RuntimeProtocol::Http.

It must prove:

sources_root =
    project_root/<configured-sources-directory>
protocol_root =
    sources_root/<runtime-source>/http

It must derive, rather than accept independently:

acquisition operation root
processing operation root
raw-data directory
processed-data directory
runtime-bundle directory
session directory for a generated session identity

Required derived paths:

protocol_root/data/raw
protocol_root/data/processed
protocol_root/get-raw-data
protocol_root/get-raw-data/runtime
protocol_root/get-raw-data/sessions/<session-id>
protocol_root/process-data
protocol_root/process-data/runtime
protocol_root/process-data/sessions/<session-id>

Reject:

* runtime source identity disagreement;
* protocol disagreement;
* operation-root substitution;
* unrelated absolute path substitution;
* configured sources root outside the project root;
* .. traversal;
* acquisition/processing substitution.

Lexical containment is sufficient.

Do not implement hostile symlink sandboxing.

Refactor SessionCoordinator to accept this validated session-independent layout or an operation-specific derivative.

Do not require callers to manufacture a RuntimeContextPaths containing a placeholder session directory before the session identity exists.

After SessionStore generates the real identity, derive the session-specific RuntimeContextPaths.

Published runtime-bundle selection

For acquisition, use the bundle rooted at:

<protocol-root>/get-raw-data/runtime

For processing, use:

<protocol-root>/process-data/runtime

Use the existing admission APIs:

admit_http_runtime_bundle(...)
admit_processing_runtime_bundle(...)

Do not duplicate:

* manifest parsing;
* manifest size limits;
* directory-shape validation;
* symlink rejection;
* executable selection;
* executable hashing;
* runtime compatibility validation.

Use only:

AdmittedHttpRuntimeBundle::executable_path()
AdmittedProcessingRuntimeBundle::executable_path()

as the child executable.

Do not guess a filename.

Do not select the first executable in a directory.

Do not execute an implementation-library artifact.

Expected runtime identity

Construct the expected owned identity from:

* requested source name;
* HTTP protocol;
* selected operation;
* established source contract version.

For acquisition:

OwnedRuntimeIdentity::http_acquisition(
    source_name,
    HttpSourceContractV1::CONTRACT_VERSION,
)

For processing:

OwnedRuntimeIdentity::http_processing(
    source_name,
    ProcessingSourceContractV1::CONTRACT_VERSION,
)

Use the owned bundle-admission compatibility boundary where needed.

Require exact agreement for:

* source;
* protocol;
* operation;
* source contract version.

Do not use Box::leak.

Do not accept a processing bundle for acquisition or an acquisition bundle for processing.

Bundle integrity immediately before launch

Bundle admission verifies the executable hash.

Minimize the gap between admission and process creation.

Immediately before spawning, verify that:

* the admitted executable path still identifies a regular file;
* it is not a symlink;
* its size and SHA-256 still match the admitted artifact.

Reuse the existing hashing implementation.

If the executable changed after admission:

* do not launch it;
* fail the prepared session safely if preparation already occurred;
* return a typed runtime-integrity error.

Do not introduce a second manifest format.

A future platform-specific open-handle execution design is outside scope.

Session reconciliation before selection

Before preparing a new session:

load current session status
→ reconcile stale ownership if applicable
→ reload resulting current status
→ apply operation-specific selection policy

Do not decide policy from a stale pre-reconciliation record.

A live current session must reject another foreground invocation.

Do not wait indefinitely for another session.

Acquisition selection policy

For acquisition:

No current session

Prepare:

RuntimeExecutionMode::Run

Current session succeeded

Prepare a new:

Run

Current session abandoned

Prepare a new:

Run

Current session failed

If:

--abandon-past-fail

then:

abandon failed session
→ prepare new Run session

Otherwise:

prepare Resume session

The runtime bundle must report a registered resume handler before a resume session is prepared.

Use the admitted HTTP runtime information to determine resume availability.

If resume is required but unavailable, return an actionable typed error explaining that the previous failure must be abandoned before a fresh run.

Do not prepare a Resume invocation that the runtime cannot admit.

Current session prepared or running with live ownership

Reject as already active.

Stale prepared or running session

Reconcile it to failed first, then apply the failed-session policy.

Processing selection policy

Processing does not support resume.

No current session, succeeded, or abandoned

Prepare:

Run

Current session failed

Without:

--abandon-past-fail

return the established unresolved-failure error.

With the flag:

abandon failed session
→ prepare new Run

Current session prepared or running with live ownership

Reject as already active.

Stale prepared or running session

Reconcile to failed, then require explicit abandonment before a new processing run.

Do not construct a processing resume envelope.

Abandonment ordering

When abandonment is requested:

1. Reconcile stale ownership.
2. Confirm the current record is failed and not live.
3. Transition it to Abandoned.
4. Update root status.
5. Only then create the new prepared session.

If abandonment succeeds but later preparation fails, preserve the abandoned record.

Do not restore a failed state after a committed abandonment.

Do not delete prior session history, raw data, or processed data.

Session preparation

Use SessionCoordinator.

A successful preparation must return an owning PreparedSessionLaunch containing:

* the generated session identity;
* the durable prepared record;
* the active supervisor lease;
* the encoded runtime-context document;
* the operation root needed for later reconciliation.

The lease must remain alive throughout:

invocation encoding
→ pre-launch integrity verification
→ spawn
→ child execution
→ wait
→ terminal reconciliation

Do not drop the lease after spawning.

Do not transfer or reacquire it in the child.

Invocation envelope

Create:

RuntimeInvocationEnvelopeV1

from:

* validated project identity;
* exact compiled runtime identity from the admitted bundle;
* session identity from PreparedSessionLaunch;
* selected execution mode;
* RuntimeSupervisionMode::Foreground.

Do not use an identity guessed independently from the admitted bundle after admission.

Require the bundle identity to equal the expected owned identity before converting it into the invocation envelope’s compiled identity representation.

Do not change invocation-envelope JSON.

Invocation argv encoding

Use:

encode_runtime_invocation(...)

The child argv after argv[0] must be:

--lexicon-invocation-v1
<envelope-json>
--
<untouched-source-arguments...>

Do not construct the reserved prefix manually.

Do not use environment variables or files for source arguments.

Do not print the encoded envelope.

Child environment

Set exactly the canonical runtime-context variable:

RUNTIME_CONTEXT_ENVIRONMENT_VARIABLE

to:

PreparedSessionLaunch::context_document()

Do not add paths to the invocation envelope.

Do not restore the legacy:

LEXICON_SOURCE_DIRECTORY

for managed runtimes.

Do not print the context document.

Inherit the ordinary parent environment unless an established Lexicon rule already specifies otherwise.

Override any inherited value for LEXICON_RUNTIME_CONTEXT_V1 with the newly prepared document.

Remove any inherited legacy LEXICON_SOURCE_DIRECTORY value from the managed child environment so it cannot accidentally affect execution.

Foreground process launch

Introduce a focused launcher seam equivalent to:

pub trait ForegroundRuntimeLauncher {
    fn launch_and_wait(
        &self,
        executable: &Path,
        arguments: &[OsString],
        context_environment: &OsStr,
    ) -> Result<ObservedChildTermination, ForegroundLaunchError>;
}

Production uses std::process::Command.

Keep the seam narrow enough for future validation without turning it into a generic process framework.

Command behavior:

* executable is the exact admitted bundle executable;
* argv is the exact encoded invocation;
* runtime context is set through the canonical environment variable;
* current directory is the validated protocol root or project root, chosen once and documented;
* stdin is inherited;
* stdout is inherited;
* stderr is inherited;
* the parent waits for completion;
* no shell is used;
* no argument string is reconstructed;
* no std::process::exit is called by Framework.

Do not run through:

sh -c
cmd /C
powershell

Do not search PATH.

Spawn failure

If Command::spawn() fails:

1. Keep the supervisor lease held.
2. Transition the prepared session to Failed.
3. Use:
    * SessionFailureKind::Runtime;
    * stable code LaunchFailed;
    * Core-authored bounded diagnostic only.
4. Update root status.
5. Release the lease when the owning launch value is consumed.
6. Return a typed launch error preserving the std::io::Error.

If failure persistence also fails, return a typed combined error preserving both causes.

Do not leave a launch failure recorded as Prepared when reconciliation is possible.

Child exit observation

Represent child termination without reducing it immediately to a Boolean.

Equivalent type:

pub enum ObservedChildTermination {
    ExitCode(i32),
    Signaled {
        signal: Option<i32>,
    },
    UnknownAbnormalTermination,
}

Use platform-appropriate information.

Do not invent a Unix signal on Windows.

A zero exit code alone is not sufficient proof of successful durable execution.

The session record is authoritative for ordinary Core completion.

Successful child reconciliation

After a zero exit:

1. Load the detailed session record.
2. Require exact identity agreement with the prepared launch.
3. Require terminal state:

Succeeded

4. Confirm root status identifies the same session and state.
5. Return ForegroundDataOutcome::Succeeded.

If the child exits zero while the record remains Prepared or Running:

* treat this as abnormal runtime behavior;
* transition to Failed while retaining the lease;
* return a typed reconciliation error.

If the child exits zero while the session is Failed:

* return a typed failed outcome;
* do not rewrite it to succeeded.

If the record is corrupt or missing:

* return a typed reconciliation error;
* do not fabricate success.

Ordinary nonzero exit reconciliation

After a nonzero exit:

Durable state is Failed

Return a typed foreground failure outcome using only:

* operation;
* source identifier;
* session identifier;
* stable failure kind;
* stable failure code;
* exit status.

Do not expose arbitrary source error text.

Durable state is Succeeded

Return a typed inconsistency error because process status and durable state disagree.

Do not rewrite succeeded state to failed automatically.

Durable state is Prepared or Running

Treat it as abnormal termination:

1. retain supervisor lease;
2. transition to Failed;
3. use SessionFailureKind::AbnormalTermination;
4. use a stable abnormal-exit code;
5. update root summary;
6. return a typed abnormal-termination result.

Durable record is missing or corrupt

Return a typed reconciliation failure preserving the decoding or I/O cause.

Signal and abnormal termination reconciliation

If the child is terminated by a signal, abort, forced exit, or an exit status without a usable code:

* inspect the durable record;
* preserve an already committed Succeeded or Failed terminal record;
* if still Prepared or Running, transition it to Failed(AbnormalTermination);
* record a stable platform-neutral failure code;
* optionally persist a bounded Core-authored signal/code field where available;
* do not persist stderr;
* do not persist panic payloads;
* do not persist source arguments.

The supervisor lease remains held until reconciliation completes.

Parent interruption during wait

Do not implement complete signal forwarding or cancellation in this milestone.

However, structure ownership so dropping the foreground execution scope cannot silently mark the session succeeded.

Do not install broad global signal handlers.

Signal forwarding and cancellation belong to a later supervision milestone.

Foreground outcome

Define a typed outcome equivalent to:

pub struct ForegroundDataOutcome {
    project: String,
    source: String,
    operation: DataOperation,
    session: SessionIdentity,
    execution_mode: RuntimeExecutionMode,
}

A successful outcome represents:

* child exited successfully;
* detailed session state is Succeeded;
* root summary agrees.

Do not include source arguments or envelope JSON.

The CLI may print a concise success message containing:

* source;
* operation;
* session identifier.

Typed errors

Define a typed Framework hierarchy covering at least:

* unsupported background mode;
* project discovery;
* project configuration;
* configured sources-root validation;
* invalid source identity;
* missing source;
* missing protocol layout;
* missing operation layout;
* missing runtime bundle;
* HTTP bundle admission;
* processing bundle admission;
* expected runtime identity mismatch;
* trusted runtime-path construction;
* stale-session reconciliation;
* session selection;
* resume handler unavailable;
* abandonment;
* session preparation;
* invocation construction;
* invocation transport encoding;
* executable integrity recheck;
* process spawn;
* process wait;
* launch-failure persistence;
* child exit/session disagreement;
* abnormal termination persistence;
* missing terminal session;
* corrupt terminal session;
* root-summary disagreement;
* combined execution and reconciliation failure.

Implement:

std::fmt::Display
std::error::Error

Preserve nested causes through source().

Do not stringify nested errors inside Framework.

The CLI boundary may convert the final top-level error once if necessary.

CLI integration

Replace the current parsed-command print path.

For:

RootCommand::Data(command)

construct ForegroundDataRequest and call:

lexicon_framework::data::execute_foreground_data(...)

If command.bg is true, the Framework returns the typed unsupported-background error.

Do not duplicate project discovery, runtime admission, session policy, or launch logic in the CLI.

Do not print:

* passthrough arguments;
* invocation JSON;
* runtime-context JSON;
* child environment;
* session record JSON.

On success, print a concise result.

On failure, use the CLI’s existing top-level diagnostic boundary.

Do not call std::process::exit inside Framework.

The existing CLI main behavior may remain until a later CLI exit-code cleanup if changing it is not necessary for this milestone.

Existing build command

Do not implement:

lexicon build

The existing build-command placeholder remains outside scope.

Do not change source build behavior.

Source-level test policy

Do not run tests or validation commands.

Production source implementation is the priority.

Existing test source may be adjusted only where necessary to keep it aligned with changed public or crate-private APIs. Do not create a broad new test suite in this milestone.

Do not spend execution time validating test behavior.

All comprehensive test creation, correction, and execution remains deferred to the final validation phase.

Commands that must not be run

Do not run:

cargo test
cargo check
cargo build
cargo fmt
cargo clippy
cargo metadata
rustc

Do not execute generated runtimes.

Do not invoke the newly implemented data command.

Do not run workspace validation.

Do not run the bundle/install pipeline.

Use static source inspection only.

Preserve existing behavior

Do not change:

* CLI data-command argument syntax;
* lexicon init;
* lexicon source create;
* lexicon source build;
* project configuration schema;
* source-manifest schema;
* managed workspace layout;
* managed runner templates except API-alignment changes strictly required by this milestone;
* managed package or binary names;
* immutable Core revision pin;
* invocation-envelope JSON;
* invocation argv contract;
* runtime-context environment variable name;
* runtime-information probe protocol;
* probe stdout/stderr behavior;
* probe limits and timeout;
* child admission order;
* source handler signatures;
* HTTP capability identifiers;
* executable hashing algorithm;
* runtime manifest formats;
* bundle directory formats;
* verification;
* staging;
* bundle admission rules;
* paired publication;
* MZA;
* Protocol 1;
* installer behavior.

Explicit exclusions

Do not implement:

* background execution;
* __operator-host;
* daemonization;
* detached processes;
* process groups;
* full signal forwarding;
* cancellation;
* HTTP client transport;
* request construction;
* retries;
* redirects;
* rate limiting;
* authentication handling;
* redaction;
* raw transaction recording;
* transaction metadata;
* checkpoints;
* source resume semantics beyond selecting the registered resume handler;
* SQLite behavior;
* processing raw transactions;
* lexicon build;
* automatic source migration;
* cross-compilation;
* MZA or installer changes.

Completion report

After implementation, replace current.md with a report containing:

* files created and changed;
* foreground data module structure;
* malformed session error match correction;
* CLI passthrough native-argument representation;
* confirmation that source arguments are not printed;
* foreground public API;
* project discovery behavior;
* configured sources-directory behavior;
* final RuntimeProjectLayout or equivalent;
* complete project/source/protocol/operation path binding;
* acquisition bundle path;
* processing bundle path;
* exact bundle-admission APIs reused;
* expected runtime identity behavior;
* pre-launch executable integrity behavior;
* stale-session reconciliation order;
* acquisition run/resume/abandon selection behavior;
* processing run/abandon selection behavior;
* resume-handler availability behavior;
* session preparation behavior;
* supervisor lease lifetime;
* invocation-envelope construction;
* exact argv construction;
* runtime-context environment behavior;
* exact child executable selection;
* process stdin/stdout/stderr behavior;
* confirmation that no shell or PATH lookup is used;
* spawn-failure session behavior;
* zero-exit reconciliation behavior;
* nonzero-exit reconciliation behavior;
* signaled/abnormal termination behavior;
* child exit/session disagreement behavior;
* foreground success result;
* typed foreground error hierarchy;
* CLI integration behavior;
* confirmation that --bg returns an unsupported-mode error without creating a session;
* confirmation that no HTTP transport, raw recording, checkpoints, SQLite, background host, or lexicon build was implemented;
* any existing test source adjusted solely for API alignment;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, runtime execution, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin HTTP transport or background supervision.