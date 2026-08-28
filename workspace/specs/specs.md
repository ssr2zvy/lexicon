Lexicon Technical Specification

Status: Normative implementation specification
Implements: contract.md, contract version 1
Source-manifest schema: 2

1. Scope

This specification defines the concrete project layout, command behavior, Rust interfaces, persistence boundaries, build process, runtime admission, HTTP recording, session behavior, durable source state, processing, and release construction required by the Lexicon architecture contract.

Examples are normative when they use must or must not. Otherwise, names and convenience methods may be refined without weakening the described invariant.

2. Supported identities

The initial supported protocol is:

http

The supported operations are:

get-raw-data
process-data

A source identity consists of:

project
+ source name
+ protocol

A runtime identity additionally includes:

operation
+ source contract
+ Core contract
+ runner-template version
+ runtime protocol

Names must be validated before joining filesystem paths.

Source names must not:

* be empty;
* be absolute paths;
* contain path separators;
* contain . or .. path components;
* escape the configured source directory;
* collide with reserved Lexicon names.

3. Public CLI grammar

The required command grammar is:

lexicon init <parent-path> <project-name>
lexicon source create <source> --protocol http
lexicon source build <source> --protocol http
lexicon build
lexicon data --get <source> --protocol http \
    [--bg] \
    [--abandon-past-fail] \
    -- [source-arguments...]
lexicon data --process <source> --protocol http \
    [--bg] \
    [--abandon-past-fail] \
    -- [source-arguments...]

Framework arguments and source arguments must not be conflated.

Source arguments must be forwarded as OsString values without a lossy UTF-8 round trip.

A source-specific phase therefore appears after the separator:

lexicon data --get video-source --protocol http -- \
    --phase download

Lexicon must not treat that phase as framework state.

Reserved internal modes, including operator-host and runtime-information modes, are not part of the ordinary public CLI.

4. Project manifest

A project contains:

lexicon.toml

Representative form:

schema_version = 1
[project]
name = "example-project"
[paths]
sources_directory = "sources"

Project discovery starts at the supplied or current path and walks toward the filesystem root until a valid lexicon.toml is found.

The configured source directory must resolve within the project according to validated path rules.

5. Source manifest

New sources use source-manifest schema 2:

schema_version = 2
[source]
name = "video-source"
protocol = "http"
[acquisition]
contract = "native-rust-http-source-v1"
runner_template = 1
core_contract = 1
runtime_protocol = 1
[processing]
contract = "native-rust-processing-v1"
runner_template = 1
core_contract = 1
runtime_protocol = 1

The distinct version fields must not be replaced by one generic version.

The implementation may add fields for required capabilities, schemas, or compatibility metadata, but it must reject unknown incompatible major versions.

6. Source filesystem layout

A newly created HTTP source has:

sources/<source>/http/
├── source.toml
├── discovery.md
├── data/
│   ├── raw/
│   └── processed/
├── get-raw-data/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── get-raw-data-impl/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   ├── lexicon-runner/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   ├── runtime/
│   ├── state/
│   ├── sessions/
│   └── session_status.json
└── process-data/
    ├── Cargo.toml
    ├── Cargo.lock
    ├── process-data-impl/
    │   ├── Cargo.toml
    │   └── src/
    │       └── lib.rs
    ├── lexicon-runner/
    │   ├── Cargo.toml
    │   └── src/
    │       └── main.rs
    ├── runtime/
    ├── sessions/
    └── session_status.json

get-raw-data/state/ is durable across sessions.

Temporary build outputs must not be placed in this source tree except inside temporary staging paths that are removed or atomically published.

7. lexicon source create

For:

lexicon source create video-source --protocol http

Lexicon must:

1. discover and validate the containing project;
2. resolve the configured source directory;
3. validate the source name;
4. validate the explicitly selected protocol;
5. reject an existing destination;
6. allocate a temporary sibling staging directory;
7. generate source.toml;
8. generate discovery.md;
9. create raw and processed data directories;
10. create acquisition sessions and root status;
11. create processing sessions and root status;
12. create get-raw-data/state/;
13. create the acquisition Cargo workspace;
14. create the acquisition implementation library;
15. create the managed acquisition runner;
16. create the processing Cargo workspace;
17. create the processing implementation library;
18. create the managed processing runner;
19. create empty runtime directories;
20. create or resolve committed Cargo lockfiles without compiling;
21. flush required staged files;
22. atomically publish the complete source directory.

The command must not compile or execute source code.

Failure before publication must not leave a partially created source at the final destination.

8. Embedded Core dependency identity

Generated source workspaces must depend on the exact compatible Lexicon Core revision or release selected by the installed Lexicon executable.

The identity required for scaffold generation must be embedded at Lexicon build time.

The installed executable must not:

* inspect its original CARGO_MANIFEST_DIR;
* assume its original Git checkout still exists;
* run git rev-parse against a build-machine path;
* generate an unpinned Core dependency.

Valid distribution policies include an embedded Git revision, an exact package version with integrity metadata, or another reproducible immutable reference.

9. Acquisition implementation contract

The acquisition implementation is a Rust library.

Representative scaffold:

use std::ffi::OsString;
use lexicon_core::http::{
    AcquisitionResult,
    HttpAcquisitionContext,
    HttpSourceContractV1,
};
pub const SOURCE: HttpSourceContractV1 =
    HttpSourceContractV1::new(acquire);
pub fn acquire(
    context: &mut HttpAcquisitionContext,
    args: &[OsString],
) -> AcquisitionResult<()> {
    let _ = context;
    let _ = args;
    Ok(())
}

Conceptual Core types:

pub type HttpAcquireFn =
    fn(
        &mut HttpAcquisitionContext,
        &[OsString],
    ) -> AcquisitionResult<()>;
pub type HttpResumeFn =
    fn(
        &mut HttpAcquisitionContext,
        &[OsString],
    ) -> AcquisitionResult<()>;
pub struct HttpSourceContractV1 {
    acquire: HttpAcquireFn,
    resume: Option<HttpResumeFn>,
    capabilities: CapabilitySet,
}

Required builder behavior:

HttpSourceContractV1::new(acquire)
    .with_resume(resume)
    .require_capability(
        HttpCapability::ClientCertificateV1,
    )

Exact method names may be refined, but:

* acquire is mandatory;
* resume is optional;
* handlers are exact typed function pointers;
* capability requirements are declared in the descriptor.

10. Managed acquisition runner

The managed runner package contains the supported binary target.

Its main() must delegate immediately to Core with the compiled descriptor and identity:

fn main() -> ExitCode {
    lexicon_core::runner::run_http_source(
        COMPILED_RUNTIME_IDENTITY,
        &get_raw_data_impl::SOURCE,
    )
}

The runner must handle:

* --lexicon-runtime-info;
* reserved invocation-envelope parsing;
* identity validation;
* capability validation;
* context construction;
* session attachment;
* source invocation;
* error-to-exit-code conversion.

Source-authored main.rs is not a supported acquisition contract.

Build validation must reject:

* a missing managed runner;
* changed managed runner source;
* unexpected binary targets;
* a source-owned replacement runner;
* incompatible Core dependencies;
* missing lockfiles.

11. Runtime context paths

Core constructs validated runtime paths rather than supplying arbitrary untyped directories.

Conceptual form:

pub struct RuntimeContextPaths {
    project_directory: PathBuf,
    source_directory: PathBuf,
    operation_directory: PathBuf,
    raw_directory: PathBuf,
    processed_directory: PathBuf,
    source_state_directory: PathBuf,
    session_directory: PathBuf,
}

The acquisition context exposes:

impl HttpAcquisitionContext {
    pub fn source_state_directory(
        &self,
    ) -> &Path;
}

The returned path must resolve to:

sources/<source>/<protocol>/get-raw-data/state/

Core must create and validate the directory before calling source code.

The path must not be derived from untrusted source arguments.

12. Durable source state

The supported source-state boundary is:

get-raw-data/state/

Properties:

* it survives session completion and failure;
* it is not removed by temporary build cleanup;
* it is not replaced during runtime publication;
* it belongs to one source/protocol acquisition operation;
* it may be accessed by both acquire and resume handlers;
* it is backed up or migrated according to project-level data policy;
* Core does not interpret arbitrary source-owned contents.

Source state must not be stored in:

* a runtime bundle;
* a temporary Cargo target;
* an individual session directory when it must span sessions;
* data/raw/;
* data/processed/;
* the project root.

13. Initial work-ledger design

The first fan-out sources should use source-owned SQLite under the validated state root:

get-raw-data/state/work.sqlite

The filename is conventional, not globally mandatory.

The source owns its schema. A representative schema is:

CREATE TABLE work_items (
    kind TEXT NOT NULL,
    stable_key TEXT NOT NULL,
    payload_version INTEGER NOT NULL,
    payload BLOB NOT NULL,
    status TEXT NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    origin_transaction_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (kind, stable_key)
);

This schema is illustrative rather than a Core-owned persistence contract.

A source schema should provide equivalent concepts where applicable:

* stable deduplication identity;
* work kind;
* versioned payload;
* pending, active, complete, and failed state;
* attempt information;
* last diagnostic;
* discovery provenance;
* schema-version migration.

The source must perform SQLite updates transactionally.

Initial operation assumes one active supervised acquisition runtime. Multiple concurrent Lexicon processes must not mutate the same work database without an explicitly introduced lease or concurrency design.

14. Discovery and fan-out

A discovery stage follows this pattern:

fn discover(
    context: &mut HttpAcquisitionContext,
    work: &WorkLedger,
    query: &str,
) -> AcquisitionResult<()> {
    let logical_key =
        format!("discover/{query}");
    if context.has_checkpoint(
        &logical_key,
    )? {
        reconcile_discovery(
            context,
            work,
            &logical_key,
        )?;
        return Ok(());
    }
    let transaction =
        context.execute(
            HttpRequest::get(
                search_url(query),
            )?
            .logical_key(
                logical_key.clone(),
            ),
        )?;
    transaction
        .response()
        .require_success()?;
    let video_ids =
        parse_video_ids(
            transaction
                .response()
                .body_path(),
        )?;
    work.insert_if_absent(
        &video_ids,
        transaction.id(),
    )?;
    context.commit_checkpoint(
        &logical_key,
    )?;
    Ok(())
}

Stable work keys make repeated discovery safe:

(kind = "video-download", stable_key = video-id)

If termination occurs after insertion but before checkpoint commit, discovery may repeat. Repeated insertion must converge without duplicating intended work.

15. Work execution and checkpoint composition

A work item that causes an HTTP operation follows:

fn execute_work_item(
    context: &mut HttpAcquisitionContext,
    work: &WorkLedger,
    item: &WorkItem,
) -> AcquisitionResult<()> {
    let logical_key =
        format!(
            "work/{}/{}",
            item.kind,
            item.stable_key,
        );
    if context.has_checkpoint(
        &logical_key,
    )? {
        work.mark_complete(
            &item.stable_key,
        )?;
        return Ok(());
    }
    work.mark_active(
        &item.stable_key,
    )?;
    let transaction =
        context.execute(
            build_request(item)?
                .logical_key(
                    logical_key.clone(),
                ),
        )?;
    verify_response(
        item,
        &transaction,
    )?;
    context.commit_checkpoint(
        &logical_key,
    )?;
    work.mark_complete(
        &item.stable_key,
    )?;
    Ok(())
}

The ordering is intentional:

record response
→ verify source semantics
→ commit checkpoint
→ mark work complete

Crash behavior:

Interruption point	Recovery
Before request dispatch	Work remains pending or active and is retried
During HTTP exchange	Partial or failed transaction remains; work is retried according to source policy
After response recording, before verification	Recorded evidence remains; source may repeat or inspect it
After verification, before checkpoint	Request may repeat
After checkpoint, before work completion	Checkpoint reconciliation marks work complete without repeating
After work completion	Item remains complete

The guarantee is at-least-once around the request/checkpoint boundary.

16. Source phases

A source may parse:

--phase discover
--phase download
--phase all

Representative behavior:

match args.phase {
    Phase::Discover => {
        discover(
            context,
            &work,
            &args.query,
        )?;
    }
    Phase::Download => {
        drain_pending_work(
            context,
            &work,
        )?;
    }
    Phase::All => {
        discover(
            context,
            &work,
            &args.query,
        )?;
        drain_pending_work(
            context,
            &work,
        )?;
    }
}

Lexicon must not:

* add these phases to source.toml;
* schedule them independently;
* infer prerequisites;
* claim discovery freshness;
* interpret their source-specific payloads.

A future framework-level phase contract requires a separate architectural decision.

17. Checkpoint representation

Acquisition checkpoints reside under:

get-raw-data/sessions/<session-id>/checkpoints/

A checkpoint record must contain enough information to validate:

* checkpoint schema;
* project identity;
* source and protocol identity;
* operation identity;
* session identity;
* logical-key digest;
* transaction identity;
* physical attempt identity;
* redirect or retry position where applicable;
* commit timestamp.

Checkpoint filenames should use a safe digest of the logical key rather than embedding arbitrary source strings.

commit_checkpoint(key) must fail unless the current context has observed a completed, durable transaction for the same logical key.

has_checkpoint(key) may inspect compatible prior sessions, but it must validate that the referenced transaction still exists and is complete.

Checkpoints are receipts. They must not grow into an untyped general-purpose payload database.

18. Acquisition workspace validation

Before building, Lexicon validates:

* workspace manifest;
* committed lockfile;
* source implementation library target;
* managed runner package;
* managed runner source;
* exact compatible Core dependency;
* declared capabilities;
* expected package names;
* expected binary target;
* manifest identity;
* absence of prohibited replacement entrypoints.

The corresponding processing workspace receives equivalent validation.

The build pipeline should represent validation stages using opaque types:

SourceLocation
→ DiscoveredSource
→ ValidatedSource
→ ValidatedOperationWorkspace
→ ReproducibleBuildPlan
→ StagedArtifact
→ VerifiedRuntime
→ PublishedRuntime

Publication functions must accept validated objects rather than arbitrary paths whenever possible.

19. lexicon source build

For:

lexicon source build video-source --protocol http

Lexicon must:

1. discover the project;
2. resolve the source;
3. validate the source manifest;
4. validate acquisition workspace and lockfile;
5. validate processing workspace and lockfile;
6. allocate isolated temporary targets and staging directories;
7. run the locked acquisition release build;
8. select the exact acquisition executable from Cargo JSON;
9. run the locked processing release build;
10. select the exact processing executable;
11. stage both runtime bundles;
12. hash both executables;
13. probe both runtimes in information mode;
14. validate both runtime identities and capabilities;
15. generate both runtime.json documents;
16. transactionally publish the pair;
17. restore both previous bundles if either publication fails;
18. remove only temporary build material.

Conceptual Cargo invocation:

cargo build
  --manifest-path <get-raw-data/Cargo.toml>
  --package <expected-runner-package>
  --bin <expected-runner-binary>
  --release
  --locked
  --message-format=json-render-diagnostics
  --target-dir <temporary-target>

Processing uses its own isolated target directory.

A permanent project-level target/ directory must not be required.

20. Cargo artifact selection

Lexicon parses Cargo JSON output and accepts exactly one executable artifact matching:

* expected package ID;
* expected binary target name;
* target kind bin;
* release profile;
* expected runner package;
* current machine target.

It must not:

* guess target/release/<name>;
* select the first executable reported;
* select a library artifact;
* publish a debug artifact;
* accept multiple ambiguous matches.

21. Runtime bundle

A published acquisition bundle contains:

get-raw-data/runtime/
├── <source>-get-raw-data[.exe]
└── runtime.json

A published processing bundle contains:

process-data/runtime/
├── <source>-process-data[.exe]
└── runtime.json

Representative runtime metadata:

{
  "schema_version": 1,
  "source": "video-source",
  "protocol": "http",
  "operation": "get-raw-data",
  "executable": "video-source-get-raw-data",
  "executable_sha256": "<hex>",
  "runtime_protocol": 1,
  "source_contract": "native-rust-http-source-v1",
  "core_contract": 1,
  "runner_template": 1,
  "capabilities": []
}

Publication is paired. A new acquisition runtime must not remain paired with an old processing runtime because a second rename or replacement failed.

Executable-lock failures on Windows are publication failures and trigger rollback.

22. Runtime-information mode

The managed runtime supports:

<runtime> --lexicon-runtime-info

Core handles this before source state is constructed and before source code is invoked.

Representative response:

{
  "runtime_protocol": 1,
  "source": "video-source",
  "protocol": "http",
  "operation": "get-raw-data",
  "source_contract": "native-rust-http-source-v1",
  "core_contract": 1,
  "runner_template": 1,
  "capabilities": [
    "recorded-http-v1",
    "checkpoints-v1",
    "source-state-directory-v1"
  ]
}

The probe must be bounded by a timeout and output-size limit.

Probe output must be machine-readable and must not be contaminated by source-authored startup output.

23. Runtime invocation envelope

The parent launches a runtime using a private, versioned envelope containing at least:

* envelope version;
* project identity;
* source identity;
* protocol;
* operation;
* session identity;
* session directory;
* session lease identity;
* execution mode;
* source arguments.

Source arguments remain operating-system strings. The envelope encoding must support:

* non-UTF-8 Unix arguments;
* Windows Unicode arguments;
* empty strings;
* arguments beginning with -;
* arguments containing the public -- token.

Sensitive values should not be placed in the envelope when doing so exposes them in process listings.

24. Parent-side data --get

For:

lexicon data --get video-source --protocol http -- <source-args>

the parent must:

1. discover the project;
2. resolve source and protocol;
3. validate source.toml;
4. load runtime metadata;
5. verify the executable hash;
6. run the bounded runtime-information probe;
7. validate source, protocol, operation, contracts, and capabilities;
8. inspect previous acquisition sessions;
9. apply prior-failure policy;
10. apply --abandon-past-fail;
11. create or select a session;
12. acquire the session lease;
13. durably persist initial session state;
14. construct the invocation envelope;
15. launch the child runtime;
16. supervise exit or signal;
17. reconcile abnormal termination;
18. return a meaningful CLI exit code.

25. Child-side data --get

The child’s Core runner must:

1. detect and answer information mode;
2. parse the reserved envelope;
3. validate envelope version;
4. validate compiled runtime identity;
5. validate project, source, protocol, and operation;
6. validate required capabilities;
7. validate session identity and lease;
8. validate runtime paths;
9. create HttpAcquisitionContext;
10. expose the validated source-state directory;
11. transition the session from prepared to running;
12. call acquire or registered resume;
13. record ordinary source errors;
14. record successful completion;
15. return an appropriate process exit code.

The parent, not the child source implementation, owns reconciliation of abnormal process termination.

26. Resume selection

If the latest compatible session failed or was interrupted:

* a registered resume handler may be selected;
* the new execution receives a new supervised session;
* prior raw transactions and checkpoints remain available;
* source-owned state remains available;
* the source reconstructs its continuation from durable information.

If no resume handler exists, Lexicon must not pretend arbitrary local computation can be resumed.

The exact prior-failure policy must produce a clear operator error or require --abandon-past-fail according to the selected rule.

Abandonment changes session policy. It must not delete raw transaction evidence or source-owned state.

27. Session states

The initial durable session states are:

prepared
running
succeeded
failed
abandoned

Valid ordinary transitions:

From	To	Owner
none	prepared	Parent
prepared	running	Child Core
running	succeeded	Child Core
running	failed	Child Core for ordinary source failure
prepared	failed	Parent for launch/admission failure
running	failed	Parent for abnormal child termination
prepared or failed	abandoned	Parent under explicit policy

A stale prepared or running session must be reconciled using durable lease ownership and process observations.

Session documents must never claim success merely because a process disappeared.

28. Session persistence

Detailed history belongs under:

get-raw-data/sessions/<session-id>/

The root summary is:

get-raw-data/session_status.json

The root file is a current summary, not the sole durable history.

Representative session metadata includes:

{
  "schema_version": 1,
  "session_id": "<uuid>",
  "source": "video-source",
  "protocol": "http",
  "operation": "get-raw-data",
  "mode": "acquire",
  "state": "running",
  "created_at": "<timestamp>",
  "started_at": "<timestamp>",
  "finished_at": null,
  "transaction_count": 42,
  "last_transaction_id": "<id>",
  "error": null
}

Writes must use durable staging and atomic replacement where supported.

29. Foreground supervision

In foreground mode, the installed lexicon process remains the supervisor.

It must:

* retain the session lease or a defined supervisory ownership token;
* observe child exit;
* handle operator cancellation;
* use bounded graceful termination before forced termination where supported;
* reconcile the final session state;
* return a suitable shell status.

Platform-specific Unix and Windows termination behavior must be tested independently.

30. Background operator host

For --bg, the original process must transfer supervision to:

lexicon __operator-host <reserved-envelope>

The handoff must avoid a lease gap in which another invocation can falsely classify the session as stale.

A valid design must provide one of:

* inherited lease ownership;
* an atomic durable handoff token;
* parent-to-operator-host acknowledgement before release;
* another mechanism proving continuous ownership.

The public command reports successful background start only after the operator host has acknowledged durable supervision.

The operator host must perform terminal reconciliation when the source child exits.

31. HTTP request API

Representative request construction:

let request =
    HttpRequest::get(url)?
        .header(
            "Accept",
            "application/json",
        )?
        .sensitive_header(
            "Authorization",
            token,
        )?
        .query(
            "page",
            page,
        )?
        .sensitive_query(
            "api_key",
            api_key,
        )?
        .logical_key(
            logical_key,
        );
let transaction =
    context.execute(request)?;

Core owns transport configuration required to uphold raw recording.

A source receives a RecordedTransaction, not an unrecorded live response.

32. Transaction layout

Every physical attempt receives a directory:

data/raw/<timestamp>-<transaction-id>/
├── request/
│   ├── metadata.json
│   └── body
└── response/
    ├── metadata.json
    └── body

Request or response bodies may be absent only when the protocol semantics or failure state make them absent.

Core must distinguish at least:

request persisted
request dispatched
response headers received
response body incomplete
transport failed
interrupted
complete

The exact staging filenames are private implementation details.

Metadata must never falsely claim a complete response exists.

33. Record-before-return algorithm

For one physical attempt, execute() must:

1. allocate an identity;
2. create temporary transaction storage;
3. persist effective redacted request metadata;
4. persist exact transport request-body bytes;
5. flush required request records;
6. dispatch one transport exchange;
7. persist response headers or failure details;
8. stream undecoded entity bytes to temporary body storage;
9. compute the body hash while streaming;
10. detect truncation or interruption;
11. flush metadata and body;
12. atomically finalize the transaction when complete;
13. update the session progress record;
14. update the in-process logical-key registry;
15. return RecordedTransaction.

If any step fails, the caller receives an error and diagnostic partial state remains where meaningful.

34. Redirect behavior

Automatic redirects below Core are disabled.

Core may implement an explicit redirect loop.

Each response in that loop is independently recorded.

Redirect policy must include bounded limits and protection against malformed or cyclic redirects.

Sensitive headers must not be forwarded across origins unless an explicit safe policy allows it.

35. Retry behavior

Automatic transport retries below Core are disabled.

Core may implement a typed retry policy with:

* maximum attempts;
* permitted methods;
* retryable transport failures;
* retryable status codes;
* delay policy;
* Retry-After handling;
* total elapsed-time bound.

Every attempt is independently recorded.

Non-idempotent requests are not retried by default.

36. Body encoding

Raw response storage captures entity-body bytes before content decoding.

The HTTP transport must disable transparent content decompression on the raw-capture path.

A convenience decoder may be exposed:

transaction
    .response()
    .decoded_body_reader()?

Decoded data must not replace raw body bytes.

Request bodies record the exact bytes supplied by Core to transport.

37. Redaction

Mandatory case-insensitive sensitive headers:

Authorization
Proxy-Authorization
Cookie
Set-Cookie

Persisted metadata represents a redacted value structurally:

{
  "headers": {
    "Authorization": {
      "redacted": true
    }
  }
}

Sensitive query values must not remain in persisted URLs, error messages, or diagnostic strings.

Core should persist a normalized representation showing that a field existed without retaining its value.

Raw bodies are not generically redacted.

Source-owned state databases and logs are not automatically redacted by Core.

38. Transport failures

A failure before response headers still produces an attempted transaction record containing:

* finalized redacted request metadata;
* request-body bytes where applicable;
* dispatch state;
* transport-failure category;
* safe diagnostic information;
* attempt identity;
* completion state indicating no complete response.

A truncated response preserves the partial body and marks it incomplete.

39. Processing contract

The processing implementation is a Rust library:

use std::ffi::OsString;
use lexicon_core::processing::{
    ProcessingContext,
    ProcessingResult,
    ProcessingSourceContractV1,
};
pub const PROCESSOR:
    ProcessingSourceContractV1 =
        ProcessingSourceContractV1::new(
            process,
        );
pub fn process(
    context: &mut ProcessingContext,
    args: &[OsString],
) -> ProcessingResult<()> {
    let transactions =
        context.raw_transactions()?;
    let database =
        context.create_staged_database()?;
    for transaction in transactions {
        process_transaction(
            &database,
            transaction,
        )?;
    }
    context.publish_database(
        database,
    )?;
    Ok(())
}

Processing must:

* enumerate validated raw transactions;
* distinguish complete and incomplete transactions;
* stage processed output;
* publish transactionally;
* retain previous processed output if processing fails.

Processing must not mutate raw records or acquisition checkpoints.

40. lexicon build

lexicon build must:

1. discover the containing project;
2. resolve the configured source directory;
3. deterministically discover supported source/protocol combinations;
4. reject ambiguous or invalid layouts;
5. invoke the same validated source-build pipeline for each source;
6. report failures with exact source and protocol identities.

It must not implement a weaker second build architecture.

Per-source pairing of acquisition and processing publication is mandatory.

A project-wide all-or-nothing publication transaction may remain deferred until supported by implementation evidence.

41. MZA Protocol 1 release construction

Source builds and complete-product release construction are separate.

Source build:

lexicon source build
→ current-machine native source runtimes
→ no MZA invocation

Product release construction:

build lexicon executable
→ provide MZA Protocol 1 inputs
→ MZA constructs target bundle or installer

The outer release package may contain:

lexicon-bundle/
├── Cargo.toml
├── build.rs
└── src/main.rs

The adapter must use the actual types and entrypoints exported by the selected MZA Protocol 1 dependency.

If MZA provides generated Rust through MZA_BUNDLE_INPUTS, the consumer uses:

include!(
    env!("MZA_BUNDLE_INPUTS")
);

Generated material belongs in Cargo’s OUT_DIR.

The bundle must install:

* the lexicon control executable;
* required Lexicon resources;
* command registration metadata.

It must not install a removed standalone framework executable.

Lexicon must not reimplement MZA installer behavior.

42. Publication durability

Runtime publication must use staging and recoverable replacement.

Conceptual algorithm:

1. create acquisition and processing staging bundles;
2. validate both;
3. preserve references to both existing bundles;
4. replace acquisition bundle;
5. replace processing bundle;
6. finalize success;
7. on failure, restore both previous bundles;
8. retain diagnostics if rollback is incomplete.

The implementation must account for Windows executable locks and cross-platform rename behavior.

43. Security boundaries

Opaque validated build-state types prevent accidental internal misuse. They do not make native code safe against malice.

The supported guarantees do not include:

* prevention of source filesystem access;
* prevention of direct network access;
* prevention of subprocess creation;
* prevention of FFI;
* prevention of dynamic library loading;
* arbitrary body-secret removal;
* safe execution of untrusted source packages.

If hostile third-party sources become a requirement, the architecture must be reopened to include operating-system enforcement.

44. Required tests

Source contract

* valid descriptor compile-pass;
* missing descriptor compile-fail;
* private handler compile-fail;
* wrong acquisition signature compile-fail;
* wrong resume signature compile-fail;
* unsupported capability rejection.

Scaffold and validation

* atomic source creation;
* exact source layout;
* schema-2 manifest;
* managed runner integrity;
* source-owned main.rs rejection;
* lockfile requirement;
* installed scaffold generation without original Git checkout.

Build and publication

* locked release build;
* isolated target directory;
* exact Cargo JSON artifact selection;
* acquisition build failure preserves runtimes;
* processing build failure preserves runtimes;
* runtime probe mismatch;
* executable hash mismatch;
* paired publication rollback;
* Windows executable-lock rollback behavior.

HTTP recording

* one GET;
* POST request-body preservation;
* compressed response preservation;
* redirect chain;
* retry attempts;
* connection failure;
* truncated response;
* request metadata;
* response metadata;
* mandatory header redaction;
* sensitive-query redaction;
* record-before-return.

Checkpoints

* commit after durable keyed transaction;
* reject commit without matching transaction;
* lookup across compatible sessions;
* missing backing transaction;
* crash after response before checkpoint;
* checkpoint-backed resume.

Durable source state

* validated state path;
* state survives sessions;
* state survives runtime rebuild and publication;
* work insertion deduplication;
* repeated discovery convergence;
* crash after checkpoint before work completion;
* recovery marks checkpointed work complete;
* SQLite schema migration;
* simultaneous unsupported writer rejection.

Sessions and supervision

* source success;
* ordinary source error;
* source panic;
* abnormal child exit;
* foreground interruption;
* stale lease recovery;
* abandon policy;
* non-UTF-8 Unix arguments;
* Windows Unicode arguments;
* background operator-host acknowledgement;
* continuous lease ownership during handoff;
* operator-host terminal reconciliation.

Processing

* raw transaction enumeration;
* incomplete transaction handling;
* staged database publication;
* failed processing preserves prior output;
* paired runtime compatibility.

Environment handling

An unsupported environment may produce an explicit skipped test only when:

* the test framework records it as skipped rather than passed;
* the reason is specific and diagnostic;
* the skipped invariant is covered in a supported environment;
* no broad error category converts arbitrary failures into skips.

An ETXTBSY race or similar failure must not cause an unconditional successful return that hides the missing assertion.

45. Compatibility and migration

Existing schema-1 sources may remain on an explicitly identified legacy contract during a transition period.

Migration to schema 2 consists of:

1. converting source-owned executable implementations to libraries;
2. exposing typed acquisition and processing descriptors;
3. generating managed runners;
4. adding the acquisition durable-state directory;
5. adding distinct contract and runtime versions to source.toml;
6. pinning the exact compatible Core dependency;
7. regenerating lockfiles;
8. rebuilding and probing both runtimes;
9. publishing the pair transactionally.

Existing source-owned executables must not be silently rewritten.

Legacy support must be removed only through an explicit compatibility decision.

46. Deferred Core-owned work capability

The source-owned SQLite model is intentional for the first implementations.

Core should collect evidence from at least two or three materially different fan-out sources before defining a shared capability such as:

durable-work-v1

Promotion should occur only if sources converge on stable concepts such as:

* work kinds;
* stable identities;
* versioned payloads;
* readiness;
* attempts;
* terminal state;
* retry policy;
* provenance;
* reconciliation;
* migration.

Until then, Core exposes the validated durable-state location and retains ownership of transactions, checkpoints, sessions, and supervision.

47. Conformance documentation

The repository should maintain a separate implementation-status document containing:

* contract requirement;
* source location;
* test location;
* conformance status;
* known gap;
* planned milestone.

That document must distinguish:

implemented and tested
implemented but insufficiently tested
partially implemented
not implemented
intentionally deferred

It must not describe planned behavior as already guaranteed.

contract.md remains the architectural authority. This specification remains the behavioral authority. Implementation status must be updated whenever code or tests materially change.