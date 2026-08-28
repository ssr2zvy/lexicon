Lexicon Architecture Contract

Status: Normative
Contract version: 1
Source-manifest schema: 2

1. Authority

This document defines Lexicon’s supported architecture and guarantees.

Normative terms such as must, must not, should, and may are used deliberately.

specs.md defines the detailed behavior required to satisfy this contract. Implementation-status documents may report temporary gaps, but they do not weaken this contract.

When source code, tests, documentation, and this contract disagree, the disagreement must be resolved explicitly. Tests must not be silently deleted, weakened, or skipped merely to conceal a conformance failure.

2. Purpose

Lexicon is a framework for building, installing, and operating trusted native data-source implementations.

A source implementation may:

* perform arbitrary source-specific Rust computation;
* parse source-specific arguments;
* branch, iterate, paginate, and poll;
* authenticate according to source-specific rules;
* submit HTTP operations through Lexicon Core;
* preserve source-specific durable state;
* resume partially completed work;
* transform recorded raw transactions into processed data.

Lexicon controls the supported execution boundary:

* project and source discovery;
* source-contract validation;
* managed runtime entrypoints;
* native runtime construction;
* artifact selection and publication;
* runtime admission;
* Core-mediated HTTP;
* raw transaction recording;
* checkpoints;
* session lifecycle;
* foreground and background supervision;
* complete-product release construction through MZA.

3. Trust model

Source implementations are trusted native code.

Lexicon does not claim that a Rust source library is sandboxed. A source may use the standard library, native dependencies, filesystem APIs, sockets, subprocesses, or FFI unless the operating system independently prevents it.

The precise HTTP guarantee is:

Every physical HTTP attempt submitted through Lexicon Core is durably recorded before its result is returned to source code.

Lexicon does not guarantee that trusted native source code is incapable of opening an independent socket or producing an unrecorded external effect.

A hostile-source security boundary would require operating-system confinement and is outside this contract.

4. Installed and linked components

The supported architecture contains:

* one installed control executable named lexicon;
* one reusable lexicon-framework library;
* one narrow reusable lexicon-core library;
* one release-construction package that integrates MZA Protocol 1;
* one managed acquisition runtime per source and protocol;
* one managed processing runtime per source and protocol.

There is no separately installed framework executable.

Foreground framework operations run in process:

lexicon
→ lexicon-framework
→ lexicon-core

Source operations run through supervised native child runtimes:

lexicon supervisor
→ published managed runtime
→ linked lexicon-core runner
→ source-authored library

Background execution re-executes the installed lexicon binary in a reserved operator-host mode. It does not install or invoke a second framework program.

5. Command boundary

The public command surface includes:

lexicon init <parent-path> <project-name>
lexicon source create <source> --protocol http
lexicon source build <source> --protocol http
lexicon build
lexicon data --get <source> --protocol http [framework-options] -- [source-arguments...]
lexicon data --process <source> --protocol http [framework-options] -- [source-arguments...]

The separator has architectural meaning:

* arguments before -- belong to Lexicon;
* arguments after -- are preserved as operating-system strings and belong to the source.

For example:

lexicon data --get video-source --protocol http -- \
    --phase discover \
    --topic history

--phase, --topic, and their meanings are source-owned. Lexicon forwards them without interpreting them.

The protocol selector is explicit at the public CLI boundary, even while HTTP is the only supported protocol.

6. Source structure

A protocol-scoped source has this conceptual structure:

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
│   │   └── src/lib.rs
│   ├── lexicon-runner/
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── runtime/
│   ├── state/
│   ├── sessions/
│   └── session_status.json
└── process-data/
    ├── Cargo.toml
    ├── Cargo.lock
    ├── process-data-impl/
    │   ├── Cargo.toml
    │   └── src/lib.rs
    ├── lexicon-runner/
    │   ├── Cargo.toml
    │   └── src/main.rs
    ├── runtime/
    ├── sessions/
    └── session_status.json

The source implementation is a library. The supported executable entrypoint is generated and managed by Lexicon.

7. Source contract

An HTTP acquisition source exposes a versioned descriptor:

pub const SOURCE: HttpSourceContractV1 =
    HttpSourceContractV1::new(acquire);

Its mandatory operation has this shape:

pub fn acquire(
    context: &mut HttpAcquisitionContext,
    args: &[OsString],
) -> AcquisitionResult<()>;

A source may register an optional resume function with the same input boundary.

The descriptor is the source’s compiled declaration of:

* its mandatory acquisition function;
* its optional resume function;
* the capabilities it requires.

The managed runner cannot compile against an incorrectly typed descriptor or handler.

Lexicon owns no universal source argument schema. A source may use clap, another parser, or direct OsString inspection.

8. Ordinary Rust, not a workflow language

Source-specific behavior remains ordinary Rust.

Lexicon does not require a source author to express acquisition as:

* a workflow DSL;
* TOML control flow;
* a serialized acquisition IR;
* registered job-handler callbacks;
* a universal pagination abstraction;
* a DAG;
* a distributed task graph.

For example, discovering video identifiers and later downloading videos may remain one source implementation:

discover identifiers
→ durably register intended downloads
→ download outstanding videos
→ checkpoint verified completions

A source may expose --phase discover|download|all, but those phases remain source-specific arguments unless a future contract explicitly promotes phases into a framework concept.

9. Durable source state and work ledgers

Lexicon reserves:

get-raw-data/state/

as the supported durable state boundary for an acquisition source.

Core must expose this location through a validated context API. Source code must not reconstruct it from arbitrary parent-directory traversal.

State under this directory is:

* scoped to one project, source, protocol, and operation;
* durable across acquisition sessions;
* distinct from raw transaction evidence;
* distinct from session history;
* managed semantically by the source;
* available to both acquisition and resume handlers.

The initial contract deliberately does not impose a universal Core-owned job schema. A fan-out source may own a SQLite work ledger under this directory.

Lexicon owns the validated location. The source owns:

* table definitions;
* work kinds;
* stable work keys;
* payload schemas;
* deduplication policy;
* readiness and completion rules;
* source-specific recovery rules;
* schema migrations.

A work ledger must not be confused with a background scheduler. In the initial contract:

* one supervised acquisition runtime advances the source’s work;
* Lexicon does not run a cross-source daemon;
* Lexicon does not distribute work between machines;
* Lexicon does not dispatch registered job handlers;
* Lexicon does not promise exactly-once external effects.

A future DurableWorkV1 Core capability may be introduced only after multiple real sources demonstrate a stable common lifecycle.

10. Four distinct durable concepts

Lexicon distinguishes:

Concept	Meaning
Raw transaction	Evidence of one physical Core-mediated HTTP attempt
Checkpoint	Transaction-backed evidence that a source-defined logical operation completed
Source state or work item	Durable source-owned intended work and continuation data
Session	One supervised attempt to advance an acquisition or processing operation

These concepts must not be collapsed into one file or one status field.

Raw transactions establish provenance. Checkpoints establish logical completion. Source state represents intended work. Sessions describe execution attempts.

11. Checkpoints

A checkpoint is not arbitrary key-value storage.

A committed acquisition checkpoint must refer to a durable Core-recorded transaction associated with the same logical key.

The supported pattern is:

if context.has_checkpoint(&logical_key)? {
    return Ok(());
}
let transaction =
    context.execute(
        request.logical_key(
            logical_key.clone(),
        ),
    )?;
verify(
    transaction.response().body_path(),
)?;
context.commit_checkpoint(
    &logical_key,
)?;

Checkpoint persistence is session-specific, but checkpoint discovery may span compatible prior sessions.

If the process terminates after the transaction is recorded but before the checkpoint is committed, the source may repeat the logical request. Lexicon therefore provides at-least-once recovery around this boundary, not exactly-once HTTP execution.

12. Work-ledger recovery

A source-owned work ledger and Core checkpoints compose as follows:

work item pending
→ execute keyed HTTP operation
→ verify recorded response
→ commit Core checkpoint
→ mark work item complete

If termination occurs after checkpoint commit but before the work database is updated, a later run must be able to reconcile the work item from the checkpoint.

If discovery repeats after interruption, stable source-owned work keys and idempotent insertion should prevent duplicate intended work.

The contract does not claim automatic resumption of arbitrary program counters or local variables. Durable continuation must be represented explicitly through:

* checkpoints;
* source state;
* recorded raw transactions;
* source-defined reconstruction logic.

13. Managed runners

Lexicon owns the supported runtime main().

The managed runner:

* parses the reserved invocation envelope;
* handles runtime-information mode before source initialization;
* validates compiled source identity;
* validates protocol and operation identity;
* validates contract and runtime protocol versions;
* validates required capabilities;
* attaches to the selected session;
* validates session ownership;
* constructs the Core context;
* invokes the source descriptor;
* records normal completion or ordinary source failure;
* returns a meaningful process exit status.

A source may not replace the supported runner entrypoint.

Managed runner sources and manifests are validated before build. A modified managed runner is rejected rather than silently overwritten.

14. Native builds

Source runtimes are native release executables for the current machine.

A supported build must use:

* a committed Cargo workspace;
* a committed Cargo lockfile;
* cargo build --release --locked;
* isolated temporary target directories;
* Cargo JSON artifact selection;
* exact package and binary target identities;
* staged runtime bundles;
* runtime identity probing;
* executable hashing;
* paired acquisition and processing publication;
* rollback on publication failure.

Lexicon must not guess an executable path or publish an .rlib.

A failed build must leave existing published runtimes intact.

The Lexicon executable must embed the Core source identity or revision needed when generating source workspaces. An installed Lexicon binary must not depend on its original compilation checkout or invoke git there at runtime.

lexicon source build does not invoke MZA.

15. Runtime admission

Before launching a source runtime, the parent must validate:

* project and source identity;
* requested protocol and operation;
* source manifest;
* runtime metadata;
* executable hash;
* runtime-information response;
* runtime protocol;
* Core contract;
* runner-template version;
* required capabilities.

The child validates the invocation independently.

Parent and child validation protect against accidental mismatch. They do not create a security boundary against a trusted user manually replacing files outside supported commands.

16. Core-mediated HTTP

The supported HTTP effect is:

let transaction =
    context.execute(request)?;

For every physical HTTP attempt, Core must:

1. allocate a unique transaction identity;
2. allocate transaction staging storage;
3. finalize the effective request;
4. persist redacted request metadata;
5. persist exact request-body bytes supplied to transport, when present;
6. dispatch exactly one physical exchange;
7. persist response metadata or a transport-failure record;
8. stream the undecoded HTTP entity body to raw storage;
9. hash the stored body while streaming;
10. flush required durable files;
11. atomically finalize the transaction or leave recognizable partial state;
12. update session transaction progress;
13. return only after required recording is durable.

Parsing and validation performed after execute() do not erase the transaction if they fail.

17. Raw-byte meaning

“Exact response bytes” means:

HTTP entity-body octets after transfer framing and before content decoding.

It does not mean TCP packets, TLS records, HTTP/2 frames, HTTP/3 frames, or transfer-chunk framing.

If a response has Content-Encoding: gzip, the compressed entity bytes are stored.

Any decoded-body convenience API is secondary. It does not replace raw storage.

18. Redirects and retries

Every physical exchange receives a distinct transaction record.

A redirect sequence:

request → 301 → 302 → 200

produces three transactions.

A retry sequence:

connection reset → 429 → 503 → 200

produces four transactions.

The underlying HTTP library must not invisibly follow redirects or dispatch retries below Core’s recording layer.

Retries for non-idempotent operations must not be silently enabled.

19. Metadata redaction and secrets

Core redacts managed sensitive metadata before persistence.

Mandatory sensitive fields include at least:

Authorization
Proxy-Authorization
Cookie
Set-Cookie

Core also supports explicitly sensitive headers, query parameters, metadata fields, and secret references.

Raw request and response bodies remain byte-preserved. Core cannot both preserve arbitrary body bytes exactly and generically remove secrets embedded in those bytes.

Source-authored files, databases, logs, and state are outside Core’s generic redaction guarantee. Sources must not persist credentials in their state ledger unless an explicit protected-secret design permits it.

Secrets should be supplied through environment variables, protected files, or future Core secret references. Secrets should not be placed in process arguments.

20. Sessions

Acquisition and processing have separate session histories.

The supervising lexicon process owns:

* new-versus-resume selection;
* prior-failure policy;
* --abandon-past-fail;
* initial session creation;
* session lease acquisition;
* runtime launch;
* foreground cancellation supervision;
* background operator-host handoff;
* abnormal termination reconciliation;
* stale ownership reconciliation.

Core inside the child runtime owns:

* invocation validation;
* attachment to the selected session;
* transition to running;
* transaction progress;
* ordinary source errors;
* checkpoint persistence;
* normal completion.

Source code owns:

* source-specific progress;
* checkpoint selection;
* source-state mutation;
* work-ledger reconciliation;
* pagination and continuation semantics.

The source must not directly edit Lexicon session documents.

A machine power loss cannot be atomically converted into a clean terminal status at the moment it occurs. A later invocation must detect and reconcile stale durable ownership.

21. Processing

Processing is separate from acquisition.

A processing implementation:

* is a source-authored library;
* runs through a separate managed runner;
* has separate sessions and runtime publication;
* reads protocol-scoped raw transactions;
* semantically interprets recorded data;
* stages source-specific processed output;
* publishes processed output transactionally.

Processing does not alter raw acquisition evidence.

Acquisition source state is operational continuation state, not a substitute for raw provenance and not an implicit processing input.

22. Versioning

The following compatibility surfaces are distinct:

* project schema;
* source-manifest schema;
* acquisition source contract;
* processing source contract;
* managed runner template;
* Core crate version;
* Core contract;
* runtime invocation protocol;
* runtime metadata;
* raw-data schema;
* session schema;
* checkpoint schema;
* individual capabilities;
* MZA bundling protocol;
* MZA library version.

One number must not represent every surface.

The source library and managed runner compile together. Lexicon does not promise a stable Rust ABI between them.

23. MZA Protocol 1

MZA owns construction and installation of the complete Lexicon product.

MZA is used for:

* target-specific application bundling;
* installer construction;
* application placement;
* command registration;
* bundle metadata;
* installation and uninstall behavior.

Lexicon supplies MZA Protocol 1 inputs through the outer release-construction package.

MZA must not be linked into:

* lexicon-core;
* generated acquisition runtimes;
* generated processing runtimes;
* ordinary source-development builds.

The installed Lexicon executable must not retain installer authority merely because an MZA-produced installer installed it.

24. Enforcement

Property	Enforcement
Correct source descriptor and function signatures	Compiler
Core linked into managed runtime	Compiler
Managed runner is selected and unmodified	Build validation
Workspace, lockfile, target, and dependency compatibility	Build validation
Correct executable artifact selected	Build validation
Published runtime identity and hash	Parent and child runtime admission
Every Core-mediated physical HTTP attempt is recorded	Runtime
Core-managed metadata is redacted	Runtime
Checkpoint references a durable keyed transaction	Runtime
Source state stays within its validated supported root	API and implementation discipline
Existing runtimes survive failed builds	Staging and transactional publication
Arbitrary native network or filesystem effects are prevented	Not enforced
Exactly-once external HTTP effects	Not guaranteed
Hostile source confinement	Not provided

25. Required verification

The project must maintain meaningful tests for:

* compile-pass and compile-fail source contracts;
* managed-runner integrity;
* source and processing workspace validation;
* one-request acquisition;
* request-body preservation;
* compressed-response preservation;
* redirect and retry recording;
* metadata redaction;
* connection and truncation failures;
* checkpoints and resume;
* source errors and panics;
* abnormal child exit;
* stale session recovery;
* argument preservation;
* runtime identity and hash mismatch;
* isolated locked builds;
* paired publication rollback;
* foreground supervision;
* background operator-host handoff;
* durable source-state access;
* fan-out ledger deduplication;
* recovery between checkpoint commit and work-ledger completion;
* installed execution without the original source checkout.

Environmental limitations may be reported as explicit unsupported test environments, but an environmental race must not be converted into an unconditional successful test result.

26. Explicit non-goals

This contract does not initially standardize:

* a universal job queue;
* a distributed scheduler;
* cross-source work stealing;
* a workflow language;
* a durable program counter;
* a stable acquisition IR;
* a dynamic plugin ABI;
* hostile-source sandboxing;
* a universal pagination model;
* a universal polling model;
* a broad retry-policy language;
* framework-owned source phases;
* a broker protocol;
* exactly-once HTTP effects.

27. Final resolution

Lexicon’s selected boundary is:

Lexicon controls:
    supported entrypoints
    contract validation
    managed runners
    locked native builds
    artifact selection
    runtime admission
    Core-mediated HTTP
    raw transaction recording
    checkpoints
    validated durable-state locations
    sessions
    supervision
    runtime publication
Source implementations control:
    trusted Rust computation
    source arguments
    parsing
    branching and iteration
    authentication decisions
    pagination and continuation
    durable work schema and semantics
    source-specific recovery
    checksum and semantic validation
MZA controls:
    complete release bundling
    installation artifacts
    application placement
    command registration
    installer behavior

This boundary must remain honest: Lexicon provides strong guarantees for effects submitted through its supported APIs without claiming to sandbox unrestricted trusted native code.