The commit confirms the checkpoint closure landed, while processing is still only a thin context plus handler invocation. So the next milestone remains the processing raw-transaction and SQLite path.

Current implementation milestone: processing raw-transaction discovery and transactional SQLite output

Objective

Implement the complete Core processing data path from the source state at commit:

6615606770e03d0ced8ddb3b839371ec0a6deb39

The repository already contains:

* processing runtime information and admission;
* managed processing runners;
* processing session binding and lifecycle transitions;
* foreground processing supervision;
* durable HTTP raw transactions;
* strict finalized-transaction admission;
* acquisition-session provenance;
* durable acquisition checkpoints.

However, processing is not yet functional.

ProcessingContext currently exposes only validated paths and a session identity. The processing runner constructs that minimal context, invokes the handler, and marks the session successful or failed. It does not discover raw transactions or provide the source-specific SQLite database required by workspace/specs/contract.md.

This milestone implements:

admitted processing invocation
→ validated processing session
→ deterministic raw-transaction discovery
→ strict transaction and acquisition-session provenance admission
→ managed source-specific SQLite transaction
→ source processing handler
→ commit database on success
→ roll back database on failure
→ persist the processing session result

Do not begin background supervision, __operator-host, lexicon build, or automatic build-before-run.

Contract authority

Follow:

workspace/specs/contract.md

The controlling processing requirement is:

Processing remains separate from acquisition. It has its own implementation, managed runner, runtime, sessions, and status.

Processing reads protocol-scoped raw transactions and creates the source-specific SQLite database. It does not alter the acquisition raw-data contract.

Processing remains ordinary trusted Rust.

Core owns:

* trusted raw-root discovery;
* transaction admission;
* acquisition-session provenance validation;
* the managed database location;
* SQLite transaction lifecycle;
* processing session integration.

The source implementation owns:

* selection of relevant admitted transactions;
* body parsing and content decoding;
* its SQLite schema;
* migrations;
* SQL statements;
* transformation logic;
* source-specific arguments.

Do not introduce a declarative processing language, fixed schema, ORM, or callback state machine.

Repository-grounded starting point

At the target commit:

Processing context

lexicon-core/src/processing/context.rs contains:

pub struct ProcessingContext {
    paths: SessionDataPaths,
    session_identity: SessionIdentity,
}

It exposes path accessors but no admitted raw transactions and no database.

Processing runner

run_processing_runtime_invocation(...) currently performs:

parse
→ admit
→ decode runtime context
→ open session store
→ bind session
→ enter Running
→ construct minimal ProcessingContext
→ invoke handler
→ complete or fail session

Extend this existing path. Do not add another processing execution route.

Raw transactions

The HTTP transaction layer already provides:

* RecordedTransaction;
* HttpTransactionIdentity;
* HttpAttemptIdentity;
* HttpLogicalRequestKey;
* typed request and response records;
* typed recorded headers and redaction;
* response-body paths;
* strict finalized transaction admission through the existing internal admit_transaction_from_disk(...).

Reuse that admission implementation.

Processing database

lexicon-core currently has no SQLite dependency and no processing database engine.

Add the narrow SQLite boundary required by the contract.

Canonical processing database

The canonical database is:

<protocol-root>/data/processed/<source-name>.sqlite3

The source name must come from the admitted processing runtime identity.

Do not accept:

* a database filename from source arguments;
* an environment-selected database;
* an arbitrary source-supplied path;
* a per-session final database;
* multiple named output databases.

The processing session may update the existing canonical database transactionally.

The validated source identity must form exactly one safe filename component. Do not sanitize an invalid identity into another name.

SQLite dependency

Add a pinned rusqlite dependency to lexicon-core.

Use bundled SQLite so supported generated processing runtimes do not depend on an ambient system SQLite installation.

Expose the compatible SQLite API through:

lexicon_core::processing

A suitable public boundary is:

pub use rusqlite;

and:

impl ProcessingContext {
    pub fn database(
        &mut self,
    ) -> &mut rusqlite::Connection;
}

Equivalent typed wrapping is acceptable if it still permits arbitrary source-owned schemas and SQL.

Do not design a Lexicon-specific query API.

Processing context

Replace the path-only context with a fully admitted context equivalent to:

pub struct ProcessingContext {
    paths: SessionDataPaths,
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
    transactions: ProcessingHttpTransactionCatalog,
    database_path: PathBuf,
    database: rusqlite::Connection,
    database_state: ProcessingDatabaseState,
}

Keep every field private.

Do not provide:

* Default;
* a public unchecked constructor;
* a constructor accepting arbitrary roots;
* public database commit or rollback methods.

Provide source-useful accessors:

impl ProcessingContext {
    pub fn project(
        &self,
    ) -> &ProjectIdentity;
    pub fn runtime(
        &self,
    ) -> &OwnedRuntimeIdentity;
    pub fn session_identity(
        &self,
    ) -> &SessionIdentity;
    pub fn transactions(
        &self,
    ) -> &ProcessingHttpTransactionCatalog;
    pub fn database_path(
        &self,
    ) -> &Path;
    pub fn database(
        &mut self,
    ) -> &mut rusqlite::Connection;
}

Existing validated path accessors may remain.

Construction must require the admitted project, complete processing runtime identity, processing session identity, and SessionDataPaths.

Raw-transaction discovery module

Create a processing-owned module such as:

lexicon-core/src/processing/transactions.rs

Its internal discovery boundary should accept typed trusted values equivalent to:

pub(crate) fn discover_http_transactions_for_processing(
    project: &ProjectIdentity,
    processing_runtime: &OwnedRuntimeIdentity,
    protocol_root: &Path,
    raw_root: &Path,
) -> Result<
    ProcessingHttpTransactionCatalog,
    ProcessingTransactionDiscoveryError,
>;

Require:

processing_runtime.protocol = http
processing_runtime.operation = processing

Derive the acquisition operation root as:

<protocol-root>/get-raw-data

Do not accept the acquisition root or expected source as independent arbitrary strings.

Raw-root discovery rules

Scan only immediate children of:

<protocol-root>/data/raw

Do not recursively search.

Use native filesystem names. Do not classify entries through to_string_lossy().

For every immediate entry:

* recognized Core partial transaction directory: preserve and ignore;
* finalized transaction directory: strictly admit;
* symlink: typed rejection;
* regular file: typed unexpected-entry rejection;
* unrecognized directory: typed rejection;
* device, socket, or unsupported type: typed rejection;
* unreadable entry: typed filesystem failure.

Do not silently skip malformed entries.

A directory that looks finalized but fails transaction admission must fail discovery. It must not be reclassified as partial.

Filesystem enumeration order must not affect the catalog.

Reuse strict transaction admission

Reuse the existing transaction admission implementation for every finalized candidate.

Do not independently parse:

* request metadata;
* response metadata;
* transaction identities;
* attempt indices;
* redirect indices;
* retry indices;
* parent identities;
* logical request keys;
* stored headers;
* transport failures;
* body lengths;
* body hashes;
* timestamps;
* redaction markers.

Change internal module visibility only as necessary.

Do not expose unchecked metadata constructors publicly.

Do not turn admit_transaction_from_disk(...) into an unrestricted public filesystem API.

Acquisition-session provenance

Every admitted transaction has a typed acquisition session identity.

Open the acquisition SessionStore at:

<protocol-root>/get-raw-data

For each distinct acquisition session referenced by the catalog, load its detailed durable session record exactly once and validate:

1. The record is structurally valid.
2. Its project equals the admitted processing project.
3. Its session identity equals the transaction session.
4. Its runtime protocol is HTTP.
5. Its runtime operation is acquisition.
6. Its runtime source equals the processing runtime source.
7. Its state proves that the acquisition session entered execution.
8. Transaction creation and completion timestamps agree with the session’s durable temporal bounds.

Finalized transactions from these acquisition states may remain processable:

* Running, when visible during a concurrent or recovered history;
* Succeeded;
* Failed;
* Abandoned, if the transaction itself was finalized before abandonment.

A Prepared acquisition session must not be accepted as provenance for a finalized transaction.

Do not require the acquisition contract version to equal the processing contract version. They are distinct version surfaces.

Retain the admitted acquisition runtime identity, including its acquisition contract version, in processing-visible provenance.

A missing, corrupt, or mismatched acquisition session record is a typed discovery failure.

Processing-visible transaction

Define an opaque source-facing representation equivalent to:

pub struct ProcessingHttpTransaction {
    project: ProjectIdentity,
    acquisition_runtime: OwnedRuntimeIdentity,
    acquisition_session: SessionIdentity,
    acquisition_session_state: SessionState,
    transaction: RecordedTransaction,
}

Provide read-only accessors.

The contained RecordedTransaction remains authoritative for:

* transaction identity;
* attempt identity;
* parent transaction;
* logical request key;
* acquisition session;
* creation timestamp;
* completion timestamp;
* request body metadata;
* response status;
* response headers;
* response body path;
* transport outcome.

Do not copy recorded bodies into memory while discovering transactions.

Do not automatically deserialize, decompress, or transform response bodies.

Transaction catalog

Define:

pub struct ProcessingHttpTransactionCatalog {
    transactions: Vec<ProcessingHttpTransaction>,
}

Keep construction internal.

Provide:

impl ProcessingHttpTransactionCatalog {
    pub fn as_slice(
        &self,
    ) -> &[ProcessingHttpTransaction];
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<
        Item = &ProcessingHttpTransaction,
    >;
    pub fn len(
        &self,
    ) -> usize;
    pub fn is_empty(
        &self,
    ) -> bool;
}

Sort deterministically by:

1. transaction creation timestamp;
2. full transaction identity.

Reject duplicate transaction identities.

Do not group, collapse, or hide:

* redirects;
* retries;
* transport failures;
* repeated logical keys;
* multiple physical attempts.

Every finalized physical exchange remains separately visible.

Raw-data immutability

Processing must treat the entire raw-data tree as read-only.

It must not:

* rewrite transaction metadata;
* rewrite recorded bodies;
* delete partial transactions;
* repair malformed transactions;
* move transaction directories;
* add processing markers to raw directories;
* add SQLite files beneath data/raw;
* commit or alter acquisition checkpoints;
* update acquisition progress;
* change acquisition session records.

Any processing bookkeeping belongs in the processing session tree or the SQLite database.

Header and body behavior

Preserve:

RecordedHeaderValue::Utf8(...)
RecordedHeaderValue::Base64(...)
RecordedHeaderValue::Redacted

Do not expose redacted values as strings.

Preserve repeated response-header order.

Do not convert headers to a lossy map.

Response body paths point to already admitted exact raw bytes. Processing decides whether and how to perform content decoding.

Core must not transparently replace compressed response bytes with decoded data.

Managed database-path validation

Before opening SQLite:

1. Validate the protocol root.
2. Validate the processed-data root as exactly:

<protocol-root>/data/processed

3. Reject symlink ancestors.
4. Derive the database filename from the admitted source identity.
5. If the database exists, require an existing regular file.
6. If missing, require a valid missing leaf directly under the processed-data root.
7. Reject a database symlink.
8. Reject directories and unsupported filesystem types.
9. Revalidate the database path after opening and before source invocation.

Reuse or generalize the shared typed managed-path validator.

Do not use fs::metadata(...) as the sole symlink authority.

SQLite connection policy

Open the canonical database read-write, creating it when missing.

Apply the Core-owned baseline:

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = DELETE;
BEGIN IMMEDIATE;

Do not enable WAL during this milestone.

Set a bounded busy timeout or fail immediately with a typed locked/busy error. Do not wait indefinitely.

Do not:

* delete an existing database before processing;
* truncate the database;
* run VACUUM;
* create a replacement database automatically;
* infer or enforce a source-specific schema.

The source owns application tables, indexes, schema migrations, queries, and transformations.

One processing transaction per invocation

One processing handler invocation runs inside one SQLite write transaction.

Before calling the source:

open canonical database
→ apply Core connection policy
→ BEGIN IMMEDIATE

During the handler, source database changes remain uncommitted.

Handler success

Required order:

source handler returns Ok
→ COMMIT SQLite
→ persist processing session Succeeded
→ return success

The processing session must not become Succeeded before SQLite commits.

Handler failure

Required order:

source handler returns Err
→ ROLLBACK SQLite
→ persist safe processing session Failed
→ return typed handler failure

Do not persist arbitrary source error text.

Panic or unwind

ProcessingContext::Drop must never commit.

Dropping a context with an open transaction must allow SQLite to roll it back or perform an explicit best-effort rollback.

Do not hide or replace the supervisor’s abnormal-termination reconciliation responsibilities.

Discovery or setup failure

If transaction discovery, provenance validation, database path validation, database opening, configuration, or BEGIN IMMEDIATE fails:

* do not invoke the source handler;
* persist a Core-authored runtime failure;
* retain the typed underlying error;
* leave acquisition raw data unchanged.

SQLite/session partial commit

SQLite commit and session persistence cannot be one atomic filesystem operation.

Represent the boundary honestly.

If SQLite COMMIT succeeds but persisting the processing session as Succeeded fails, return a typed partial-commit error retaining:

* processing project identity;
* processing runtime identity;
* processing session identity;
* canonical database identity or safe database path;
* typed session persistence failure.

Do not attempt to roll back an already committed SQLite transaction.

Do not return success.

If SQLite commit fails:

* do not transition the session to Succeeded;
* attempt to persist a safe runtime failure;
* retain both the SQLite error and any terminal-persistence error.

If rollback fails after a handler error, preserve:

* the source-handler failure;
* rollback failure;
* terminal-session persistence failure, if any.

Do not discard one failure by converting another to String.

Processing runner integration

Update the existing:

run_processing_runtime_invocation(...)

The authoritative sequence becomes:

parse invocation
→ admit processing invocation
→ decode runtime context
→ open processing SessionStore
→ bind session
→ enter Running
→ derive typed processing identities and paths
→ discover and admit raw transactions
→ validate acquisition-session provenance
→ derive and validate canonical database
→ open database
→ BEGIN IMMEDIATE
→ construct ProcessingContext
→ invoke exact admitted handler
→ commit or roll back SQLite
→ complete or fail processing session

Do not add a second normal-invocation function.

Do not change the processing handler signature:

fn(
    context: &mut ProcessingContext,
    args: &[OsString],
) -> ProcessingResult<()>

Source arguments must remain in exact native order.

Processing execution errors

Extend the processing execution error hierarchy with typed variants equivalent to:

ProcessingRuntimeInvocationExecutionError {
    Transport(...),
    Admission(...),
    Session(...),
    TransactionDiscovery(...),
    ContextConstruction(...),
    DatabaseOpen(...),
    DatabaseTransaction(...),
    Handler(...),
    HandlerRollbackFailure { ... },
    TerminalPersistence { ... },
    DatabaseCommitAndPersistenceFailure { ... },
    DatabaseCommittedSessionPersistenceFailed(
        ProcessingDatabasePartialCommit,
    ),
}

Equivalent nesting is acceptable.

Add coherent nested errors such as:

ProcessingTransactionDiscoveryError
ProcessingTransactionProvenanceError
ProcessingDatabasePathError
ProcessingDatabaseOpenError
ProcessingDatabaseTransactionError
ProcessingContextConstructionError
ProcessingDatabasePartialCommit

All errors must implement:

std::fmt::Display
std::error::Error

Use source().

Do not stringify:

* session-store errors;
* transaction-admission errors;
* managed-path errors;
* filesystem errors;
* SQLite errors;
* context-construction errors;
* terminal-persistence errors.

Sensitive diagnostics

Processing errors must not reveal:

* URLs or query parameters;
* header values;
* request bodies;
* response bodies;
* SQLite row values;
* arbitrary source SQL;
* source arguments;
* runtime-context JSON;
* invocation-envelope JSON;
* environment-variable contents;
* arbitrary source error messages.

Safe diagnostics may include:

* stable project identity;
* stable runtime identity;
* session identity;
* transaction identity;
* schema version;
* stable failure category;
* canonical managed database path.

Core must not print diagnostics.

Public processing API

Export through:

lexicon_core::processing

Only source-useful types:

* ProcessingContext;
* ProcessingHttpTransaction;
* ProcessingHttpTransactionCatalog;
* appropriate typed public processing errors;
* the compatible SQLite API;
* existing processing descriptor, result, admission, probe, and runner APIs.

Keep internal:

* raw-directory enumeration;
* acquisition-session caches;
* unchecked constructors;
* database state enum;
* runner-only commit and rollback methods;
* canonical database derivation;
* low-level transaction document decoding.

Generated processing scaffold

Update the generated processing implementation template to reflect the supported API.

The placeholder may demonstrate, through concise comments or minimal non-destructive code, that a source can:

for transaction in context.transactions().iter() {
    // Inspect admitted metadata and recorded body paths.
}
let database = context.database();
// Source owns schema and SQL.

Do not generate a mandatory schema.

Do not put discovery or SQLite transaction mechanics into managed runner main.rs. The generated runner must continue entering the established Core processing runner.

If managed runner source itself does not change, do not increment its template version unnecessarily.

Existing stale test source

The inspected processing/runner.rs still contains test-only calls using the removed ProcessingContext::default() and an obsolete extra argument to run_processing_runtime_invocation(...).

Because tests are not being executed in this milestone, do not expand into a validation campaign. However, update directly affected test source where necessary so it aligns structurally with the final production API.

Do not restore ProcessingContext::default() merely to satisfy obsolete test code.

Do not add an unchecked context constructor to support tests.

Source-level acceptance requirements

The completed source must establish:

1. Processing context retains typed project, runtime, and session identities.
2. Raw and processed roots derive from validated runtime context.
3. Only immediate raw-root entries are examined.
4. Native names are not lossily decoded.
5. Recognized partial transactions are ignored.
6. Malformed finalized transactions are typed failures.
7. Symlink and unexpected raw entries are typed failures.
8. Strict existing transaction admission is reused.
9. Acquisition session records validate transaction provenance.
10. Transactions from another project are rejected.
11. Transactions from another source are rejected.
12. Non-acquisition session provenance is rejected.
13. Prepared-only acquisition provenance is rejected.
14. Finalized transactions from failed sessions remain available.
15. Acquisition and processing contract versions remain distinct.
16. Transaction timestamps agree with acquisition-session bounds.
17. Duplicate transaction identities are rejected.
18. Catalog ordering is deterministic.
19. Redirects, retries, and transport failures remain separate.
20. Recorded bodies remain exact and read-only.
21. Header ordering and typed redaction remain intact.
22. Database path is exactly data/processed/<source>.sqlite3.
23. Database path derives from the admitted runtime.
24. Symlink and wrong-type database paths are rejected.
25. SQLite foreign keys are enabled.
26. WAL is not used.
27. One immediate SQLite transaction wraps one handler invocation.
28. Handler success commits SQLite before session success.
29. Handler failure rolls back before session failure.
30. Panic or drop never commits SQLite.
31. Discovery and setup failures do not invoke the handler.
32. SQLite commit failures never produce a successful session.
33. SQLite-success/session-failure is a typed partial commit.
34. Processing never mutates acquisition raw data, checkpoints, progress, or sessions.
35. Source arguments and handler signatures remain unchanged.
36. The existing processing admission and foreground-supervision paths remain authoritative.

Preserve existing behavior

Do not change:

* acquisition handler signatures;
* resume registration;
* processing handler signature;
* invocation-envelope JSON;
* argv transport;
* source argument handling;
* acquisition admission;
* processing admission;
* runtime-information JSON;
* probe behavior;
* session schema;
* session state identifiers;
* supervisor lease ownership;
* foreground launch behavior;
* foreground reconciliation;
* HTTP request execution;
* retry or redirect behavior;
* raw transaction formats;
* raw-body fidelity;
* acquisition progress;
* checkpoint formats and meaning;
* checkpoint publication;
* runtime manifests;
* executable hashing;
* source-build artifact selection;
* bundle staging;
* bundle admission;
* paired publication;
* CLI syntax;
* MZA;
* Protocol 1;
* installer behavior.

Keep:

HttpCapabilitySet::empty()

Do not advertise ClientCertificateV1.

Command-execution constraint

This is a source-only milestone.

Do not run:

cargo test
cargo check
cargo build
cargo fmt
cargo clippy
cargo metadata
rustc

Do not execute:

* Lexicon CLI commands;
* generated runners;
* processing runtimes;
* SQLite tools;
* HTTP servers;
* real or test HTTP requests;
* workspace validation;
* bundle/install automation.

Existing test source may be adjusted only where production API alignment requires it.

Full validation remains deferred to the final project-wide validation milestone.

Explicit exclusions

Do not implement:

* background operator host;
* background session handoff;
* signal forwarding;
* cancellation;
* automatic incremental-processing policy;
* processing checkpoints;
* automatic schema generation;
* Lexicon-owned source tables;
* ORM behavior;
* decoded HTTP response readers;
* client certificates;
* proxy configuration;
* lexicon build;
* automatic build-before-run;
* source migration;
* cross-compilation;
* MZA changes;
* installer changes.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* final processing module structure;
* final ProcessingContext representation;
* typed processing identity accessors;
* raw-root discovery behavior;
* native entry-name handling;
* partial transaction handling;
* malformed finalized transaction handling;
* strict transaction-admission reuse;
* acquisition-session-store derivation;
* transaction provenance validation;
* project/source/runtime/session filtering;
* acquisition-session state behavior;
* timestamp validation;
* processing-visible transaction representation;
* deterministic catalog ordering;
* duplicate transaction behavior;
* redirect, retry, and transport-failure visibility;
* raw-body immutability behavior;
* header and redaction behavior;
* canonical SQLite database path;
* SQLite dependency and exposure boundary;
* managed database-path validation;
* SQLite connection configuration;
* handler transaction lifecycle;
* handler-success sequence;
* handler-failure sequence;
* panic/drop behavior;
* discovery and setup failure behavior;
* SQLite commit failure behavior;
* SQLite/session partial-commit representation;
* final processing runner sequence;
* generated processing scaffold changes;
* stale processing test-source alignment;
* public/internal API boundary;
* typed error hierarchy;
* sensitive diagnostic behavior;
* confirmation that acquisition data was not modified;
* confirmation that foreground ownership remained unchanged;
* confirmation that background supervision and lexicon build were not added;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, runtime execution, HTTP execution, SQLite execution, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin background supervision or project-wide build orchestration until the processing raw-transaction and SQLite boundary is complete.