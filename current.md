The next step should be a processing correctness closure, not background supervision yet. The implementation is substantial, but the source has concrete lifecycle and provenance gaps that should be closed first.

Current implementation milestone: processing correctness, durability, and error-preservation closure

Objective

Correct and complete the processing raw-transaction and SQLite implementation at commit:

26932a749d096d7abffb235473af39fac8cb20ed

The processing architecture now exists:

processing invocation
→ session Running
→ raw transaction discovery
→ acquisition provenance validation
→ SQLite BEGIN IMMEDIATE
→ source handler
→ commit or rollback
→ terminal session transition

However, source inspection identified correctness gaps in:

* per-transaction provenance validation;
* ordinary-path ownership handling;
* setup-error preservation;
* SQLite transaction-state enforcement;
* database durability;
* processing context invariants;
* supported source error construction;
* processing scaffold behavior.

This is a corrective milestone.

Do not begin background supervision, __operator-host, lexicon build, or automatic build-before-run until this closure is complete.

Contract authority

Follow:

workspace/specs/contract.md

Processing must remain separate from acquisition, read only admitted protocol-scoped raw transactions, and create the source-specific SQLite database without altering the acquisition raw-data contract.

The source remains trusted native Rust. This milestone strengthens the supported Core route against accidental invariant violations; it does not introduce hostile-code confinement.

Repository-grounded defects

Correct every defect below.

1. Provenance is validated only for the first transaction in each session

The current discovery loop validates provenance only while inserting a session into acquisition_records.

Equivalent current behavior:

if !acquisition_records.contains_key(&session_key) {
    let record = acquisition_store.load(...)?;
    validate_provenance(
        project,
        processing_runtime,
        &record,
        &transaction,
    )?;
    acquisition_records.insert(...);
}

Subsequent transactions from the same acquisition session reuse the cached record without validating that transaction’s timestamps and identity agreement.

This means only the first transaction encountered for a session receives full transaction-specific provenance validation.

Required correction

Separate:

1. session-record admission, which may be cached once per typed session identity;
2. transaction-to-session provenance validation, which must run for every transaction.

Required sequence:

load or retrieve typed acquisition session record
→ validate session-level project/runtime/state invariants
→ validate this transaction against that record
→ construct ProcessingHttpTransaction

Every transaction must independently validate:

* transaction session equals the durable session;
* transaction creation timestamp;
* transaction completion timestamp;
* transaction ordering;
* session start bound;
* terminal session finish bound, when present.

Do not let cache presence bypass transaction validation.

2. Provenance cache is keyed by String

The current cache uses:

HashMap<String, SessionRecordV1>

and derives the key using:

acquisition_session.id().to_string()

Required correction

Use:

HashMap<SessionIdentity, SessionRecordV1>

or an equivalent typed map.

Add Hash to SessionIdentity if necessary and semantically correct.

Do not convert typed session identity into an arbitrary string merely to use it as a cache key.

3. Provenance cache lookup uses expect

The current discovery path contains:

.expect("session provenance cache must contain loaded record")

The processing runner also contains ordinary-path expect(...) calls while extracting its running lifecycle owner.

Ordinary typed failures must not panic.

Required correction

Remove every ordinary-path expect, unwrap, or assertion from production processing discovery and execution.

Prefer ownership structures that make the value statically present.

Where absence remains representable, return a typed internal-state error such as:

ProcessingLifecycleError::RunningSessionUnavailable
ProcessingTransactionDiscoveryError::ProvenanceCacheInvariant

Do not panic merely because an internal state invariant was violated.

Test-only expect calls may remain in fixture construction.

4. Running-session ownership is unnecessarily represented as Option

The runner currently changes the valid running-session owner into:

let mut running = Some(running);

and later repeatedly calls:

running.take().expect(...)

This weakens an already proven type state.

Required correction

Introduce a processing execution owner that retains the running session and consumes it exactly once.

Representative structure:

struct RunningProcessingExecution<'store> {
    running: RunningRuntimeSession<'store>,
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
}

Equivalent organization is acceptable.

It should provide consuming operations for:

* setup failure;
* source failure;
* runtime failure;
* successful completion;
* committed-database partial completion.

Do not model a mandatory owner as Option throughout the ordinary path.

5. Setup failures discard the original error when failure persistence also fails

Current setup branches perform behavior equivalent to:

if let Some(session_error) =
    persist_runtime_failure(&mut running)
{
    return Err(
        ProcessingRuntimeInvocationExecutionError::
            TerminalPersistence {
                handler_error: None,
                session_error,
            },
    );
}
return Err(original_setup_error);

If terminal persistence fails, the original discovery, database-path, database-open, or context-construction error is lost.

Required correction

Preserve both failures in typed combined errors.

Representative shape:

pub enum ProcessingSetupError {
    TransactionDiscovery(
        ProcessingTransactionDiscoveryError,
    ),
    DatabasePath(
        ProcessingDatabasePathError,
    ),
    DatabaseOpen(
        ProcessingDatabaseOpenError,
    ),
    ContextConstruction(
        ProcessingContextConstructionError,
    ),
}
pub struct ProcessingSetupAndPersistenceFailure {
    setup_error: ProcessingSetupError,
    persistence_error: SessionStoreError,
}

Expose read-only accessors.

Use source() for the primary error and preserve the secondary typed failure through an accessor.

Do not reduce either error to String.

6. Setup failure codes are too broad

All processing setup failures currently use:

SessionFailureCode::RuntimeInitializationFailed

with a generic diagnostic.

This loses stable failure classification between:

* raw discovery;
* transaction provenance;
* database path admission;
* database open/configuration;
* context construction;
* database commit.

Required correction

Add stable processing-specific session failure codes, for example:

ProcessingTransactionDiscoveryFailed
ProcessingTransactionProvenanceFailed
ProcessingDatabasePathInvalid
ProcessingDatabaseOpenFailed
ProcessingDatabaseTransactionFailed
ProcessingContextConstructionFailed

Equivalent stable organization is acceptable.

Update identifier() and strict session decoding accordingly.

Persist only Core-authored bounded diagnostics.

Do not persist URLs, headers, bodies, SQL, source arguments, or arbitrary source error text.

7. Raw-root relationship is not asserted inside discovery

Discovery validates that raw_root is beneath protocol_root, but does not explicitly require:

raw_root = protocol_root/data/raw

The caller currently supplies the validated runtime-context path, but the discovery type itself should establish its own invariant.

Required correction

Require exact equality with:

protocol_root.join("data").join("raw")

Return a typed root-disagreement error otherwise.

Likewise require the acquisition operation root to be exactly:

protocol_root.join("get-raw-data")

Validate it as a managed existing directory before opening the session store.

Do not accept merely any descendant of the protocol root.

8. Partial-directory recognition is too broad

The current classifier ignores every directory beginning with:

.partial-

That can silently hide arbitrary directories that happen to use the prefix.

Required correction

Recognize only the exact staging-name grammar created by the HTTP transaction recorder.

Share the recorder’s staging-name parser or introduce one authoritative internal parser.

Distinguish:

* valid Core partial transaction directory: ignore;
* malformed partial-looking directory: typed failure;
* finalized candidate: strictly admit;
* unrelated directory: typed failure.

Do not treat any arbitrary .partial-* name as valid staging.

Do not delete partial directories.

9. Processing context does not prove all field relationships

ProcessingContext::new(...) currently checks only:

* HTTP protocol;
* processing operation.

It does not independently prove that:

* database path is beneath processed_data_directory;
* database filename matches the runtime source;
* project/runtime/session agree with the validated paths;
* transaction catalog belongs to the same project and source;
* processing session directory ends in the supplied session identity.

Required correction

Make context construction validate all relationships necessary for the admitted type.

At minimum require:

database_path
    = processed_data_directory/<runtime-source>.sqlite3
session_directory
    = operation_root/sessions/<session-id>
operation_root
    = protocol_root/process-data
raw_data_directory
    = protocol_root/data/raw
processed_data_directory
    = protocol_root/data/processed

Require every catalog entry to agree with:

* processing project;
* HTTP protocol;
* processing source.

Do not create a context that combines separately valid but mutually inconsistent components.

10. Database transaction methods silently succeed in invalid states

Current methods return Ok(()) whenever the database is no longer in OpenTransaction:

if self.database_state
    != ProcessingDatabaseState::OpenTransaction
{
    return Ok(());
}

A second commit, commit-after-rollback, or rollback-after-commit is therefore silently accepted.

Required correction

Reject invalid database state transitions with typed errors.

Representative variants:

ProcessingDatabaseTransactionError::AlreadyCommitted
ProcessingDatabaseTransactionError::AlreadyRolledBack
ProcessingDatabaseTransactionError::TransactionNotActive

Required legal transitions:

Open → Committed
Open → RolledBack

No other transition succeeds.

11. Source SQL can accidentally end the Core-owned transaction

ProcessingContext::database() returns:

&mut rusqlite::Connection

This is necessary for source-owned arbitrary SQL, but it also permits source code to execute:

COMMIT
ROLLBACK
END

before returning to Core.

The trusted-code model means deliberate bypass cannot be prohibited globally, but the supported path must detect accidental transaction-boundary loss.

Required correction

Before invoking the handler, require:

!connection.is_autocommit()

After the handler returns, require the same before Core commit or rollback.

If the source prematurely ended the transaction, return a typed boundary violation.

If source code committed changes before the violation was detected, represent this as a possible database partial commit. Do not claim rollback succeeded.

Do not return processing success.

Document this as enforcement of the supported Core route, not hostile-code confinement.

12. The source API makes simultaneous transaction iteration and database use awkward

A source commonly needs:

for each admitted transaction
→ parse recorded body
→ insert/update SQLite

With separate:

context.transactions()
context.database()

a borrowed transaction iterator may prevent a mutable borrow of the database from the same context inside the loop.

Required correction

Provide a disjoint borrowing API equivalent to:

pub fn resources(
    &mut self,
) -> (
    &ProcessingHttpTransactionCatalog,
    &mut rusqlite::Connection,
);

Equivalent naming is acceptable.

The API must permit:

let (transactions, database) = context.resources();
for transaction in transactions.iter() {
    database.execute(...)?;
}

Keep the individual accessors if useful.

Do not clone the full transaction catalog merely to work around borrowing.

13. ProcessingError is not useful for real source implementations

The current type is:

pub struct ProcessingError;

It cannot retain source parsing, I/O, or SQLite failure causes.

The source-specific processing implementation needs a practical way to return errors while the runner still persists only safe Core-authored failure data.

Required correction

Replace the unit error with a typed source boundary equivalent to:

pub enum ProcessingError {
    Source {
        operation: &'static str,
        source: Box<
            dyn std::error::Error
                + Send
                + Sync
                + 'static,
        >,
    },
    SourceMessage,
}

Equivalent organization is acceptable.

Provide constructors such as:

ProcessingError::source(
    operation,
    error,
)
ProcessingError::source_message(...)

Requirements:

* Display remains sanitized;
* source() returns typed nested errors where present;
* arbitrary source error text is not persisted into session records;
* Core diagnostics do not print SQL, row data, bodies, headers, or arguments;
* the handler signature remains unchanged.

Do not force sources to discard every underlying error into a unit value.

14. SQLite baseline configuration is not verified

The current runner executes:

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = DELETE;
BEGIN IMMEDIATE;

but does not read back and verify the effective configuration.

SQLite pragmas can be ignored or return a different effective value.

Required correction

After configuration and before handler invocation, verify:

PRAGMA foreign_keys = 1
PRAGMA journal_mode = delete
connection is not autocommit

Return typed configuration-disagreement errors otherwise.

Keep WAL disabled.

Do not silently continue with an unexpected journal mode.

15. Database open flags are implicit

The current implementation calls:

rusqlite::Connection::open(database_path)

Required correction

Use explicit open flags appropriate to the supported path:

SQLITE_OPEN_READ_WRITE
| SQLITE_OPEN_CREATE
| SQLITE_OPEN_NO_MUTEX

Equivalent safe flags are acceptable.

Do not enable SQLite URI filename interpretation.

Do not open the database read-only.

Do not accept an alternate filename embedded in a URI.

16. Database creation and commit durability are incomplete

SQLite protects its logical transaction, but a newly created database directory entry and its containing processed-data directory require an explicit durability policy.

The current path performs no post-creation or post-commit managed filesystem synchronization.

Required correction

After opening a newly created database:

* validate the resulting regular file again;
* sync the database file where appropriate;
* sync the processed-data directory using the repository’s cross-platform durability boundary.

After a successful SQLite commit:

* perform the required database durability operation;
* perform any required parent-directory durability operation;
* only then mark the processing session Succeeded.

Do not mark the session successful while required database durability remains unresolved.

If the SQLite commit succeeded but subsequent file or directory durability fails, return a typed database partial commit retaining:

* project;
* runtime;
* session;
* database path;
* failure phase;
* typed durability error.

Do not claim that the database was rolled back after SQLite already committed it.

17. New SQLite sidecar paths are not modeled

DELETE journaling can create a temporary:

<source>.sqlite3-journal

The managed path policy currently validates only the main database file.

Required correction

Define the allowed SQLite sidecar policy explicitly.

For this milestone:

* allow only the transient rollback-journal file associated with the canonical database;
* reject pre-existing symlinks at the journal path;
* reject pre-existing wrong file types;
* reject persistent -wal and -shm files;
* validate cleanup after the transaction finishes;
* return typed cleanup or unexpected-sidecar errors.

Do not recursively accept arbitrary files in data/processed.

Do not delete an unexpected user file merely because its name resembles a SQLite sidecar.

18. Commit failure handling does not distinguish uncertain commit state

A SQLite COMMIT error does not always prove that no database changes became durable.

The current code treats every commit error as an ordinary database transaction failure and then attempts runtime-session failure persistence.

Required correction

Model commit outcome conservatively.

Distinguish where possible:

* definitely not committed;
* committed;
* commit outcome uncertain.

For an uncertain result, return a typed uncertain database outcome containing:

* project;
* runtime;
* session;
* database path;
* SQLite error.

Do not claim rollback or no-change guarantees that SQLite cannot prove.

The processing session must not become Succeeded when commit outcome is uncertain.

19. Combined processing errors need complete typed access

Some current combined variants preserve multiple fields but expose only one through source() and provide no uniform accessors.

Required correction

For every combined failure, provide read-only typed accessors for all retained errors.

This includes:

* setup plus terminal persistence;
* handler plus rollback;
* handler plus terminal persistence;
* commit plus failure persistence;
* committed database plus success-persistence failure;
* commit durability partial failure;
* uncertain commit result.

source() may return the primary error, but secondary errors must remain inspectable without parsing Display.

20. Generated processing scaffold silently reports success

The completion report says the generated processing placeholder no longer uses todo!() and demonstrates transaction iteration and database access.

A newly scaffolded source must not silently return success while performing no source-specific processing.

Required correction

The generated processing implementation should compile while making incompleteness explicit.

Use a sanitized placeholder failure such as:

Err(ProcessingError::source_message(
    "processing implementation is not configured",
))

or an equivalent non-secret source-authored placeholder.

It may include commented examples showing:

let (transactions, database) =
    context.resources();

Do not let an untouched generated source mark a real processing session Succeeded with an empty database.

Do not put processing mechanics into managed runner main.rs.

21. Debug output contains managed filesystem paths

ProcessingContext::Debug currently includes:

database_path

Other new error structures retain entry paths.

Managed paths are not equivalent to source arguments or credentials, but Core diagnostics should remain deliberately bounded and consistent.

Required correction

Review new processing Debug and Display implementations.

Display must not reveal:

* URLs;
* headers;
* bodies;
* SQL;
* row data;
* source arguments;
* envelope JSON;
* runtime-context JSON;
* environment values.

Prefer stable path categories over arbitrary raw paths in Display.

Typed error fields may retain paths for programmatic recovery.

ProcessingContext::Debug should use identities and managed path categories or remain non-exhaustive without printing the full database path.

Final corrected processing sequence

After this closure, the authoritative runner sequence must be:

parse invocation
→ admit processing invocation
→ decode managed context
→ open processing SessionStore
→ bind processing session
→ enter Running with non-optional owner
→ validate exact raw/acquisition/processed roots
→ enumerate raw entries
→ strictly admit finalized transactions
→ load typed acquisition-session cache
→ validate every transaction against its session
→ build deterministic catalog
→ derive exact database path
→ validate main and sidecar paths
→ open SQLite with explicit flags
→ configure and verify baseline pragmas
→ BEGIN IMMEDIATE
→ construct fully checked ProcessingContext
→ verify transaction is active
→ invoke source handler
→ verify transaction remains active
→ success:
     SQLite COMMIT
     database/file/directory durability
     sidecar validation
     processing session Succeeded
→ source failure:
     SQLite ROLLBACK
     sidecar validation
     processing session Failed
→ setup/runtime failure:
     preserve primary typed error
     persist stable processing failure code
→ partial or uncertain commit:
     preserve database provenance
     do not report success

Error hierarchy

Use a coherent hierarchy equivalent to:

ProcessingTransactionDiscoveryError
ProcessingTransactionProvenanceError
ProcessingContextConstructionError
ProcessingDatabasePathError
ProcessingDatabaseOpenError
ProcessingDatabaseConfigurationError
ProcessingDatabaseTransactionError
ProcessingDatabaseDurabilityError
ProcessingDatabaseSidecarError
ProcessingDatabasePartialCommit
ProcessingDatabaseCommitOutcomeUncertain
ProcessingSetupError
ProcessingSetupAndPersistenceFailure
ProcessingLifecycleError
ProcessingRuntimeInvocationExecutionError

Equivalent nesting is acceptable.

All implement:

std::fmt::Display
std::error::Error

Use typed fields and source().

Do not stringify nested Core, filesystem, session, transaction-admission, or SQLite errors.

Public API boundary

Expose through:

lexicon_core::processing

Only source-useful types:

* ProcessingContext;
* ProcessingHttpTransaction;
* ProcessingHttpTransactionCatalog;
* ProcessingError;
* ProcessingResult;
* compatible rusqlite;
* existing descriptor, admission, probe, and runner APIs;
* errors that genuinely cross the source boundary.

Keep internal:

* raw-directory classifiers;
* transaction admission helpers;
* provenance caches;
* lifecycle ownership wrappers;
* database-state transitions;
* sidecar validation helpers;
* commit/durability helpers;
* unchecked constructors.

Source-level acceptance requirements

Correct the source so that:

1. Every transaction receives transaction-specific provenance validation.
2. Session-record caching never bypasses timestamp checks.
3. Provenance caches use typed session identities.
4. Production processing paths contain no ordinary expect or unwrap.
5. Running session ownership is non-optional and consumed once.
6. Setup errors remain available when failure persistence also fails.
7. Stable processing failure codes distinguish major failure phases.
8. Raw root equals the exact protocol-scoped raw root.
9. Acquisition root equals the exact acquisition operation root.
10. Only valid Core partial-directory names are ignored.
11. Malformed partial-looking directories are typed failures.
12. Context construction proves all identity and path relationships.
13. Catalog project/source agreement is validated.
14. Invalid database state transitions are rejected.
15. Premature source transaction completion is detected.
16. Transaction-boundary loss never produces success.
17. Source code can borrow catalog and database together.
18. ProcessingError can retain useful typed source causes.
19. Arbitrary source failure text is not persisted.
20. SQLite baseline pragmas are read back and verified.
21. SQLite open flags are explicit.
22. URI filename interpretation remains disabled.
23. New database creation has a durability boundary.
24. Successful commit is made durable before session success.
25. Post-commit durability failure is a typed partial commit.
26. SQLite rollback-journal policy is explicit.
27. Unexpected WAL/SHM sidecars are rejected.
28. Commit-outcome uncertainty is represented honestly.
29. Combined errors expose every retained typed cause.
30. Untouched generated processing scaffolds fail clearly instead of succeeding.
31. Processing never mutates acquisition raw data, checkpoints, progress, or sessions.
32. Foreground supervision and session ownership remain unchanged.

Preserve existing behavior

Do not change:

* processing handler signature;
* acquisition or resume handler signatures;
* invocation-envelope JSON;
* argv transport;
* source argument preservation;
* acquisition admission;
* processing admission;
* runtime-information probes;
* session schema except adding stable failure-code variants;
* supervisor lease ownership;
* foreground launching;
* foreground reconciliation;
* HTTP transport;
* retries;
* redirects;
* raw transaction formats;
* raw-byte fidelity;
* header redaction;
* acquisition progress;
* checkpoints;
* managed runner entrypoints;
* source build;
* runtime verification;
* bundle staging;
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

* lexicon CLI commands;
* generated runners;
* processing runtimes;
* SQLite tools;
* HTTP servers;
* real or test HTTP requests;
* workspace validation;
* bundle/install automation.

Do not attempt a CLI command merely to confirm whether it is installed.

Existing test source may be adjusted only when necessary to align with changed production APIs.

Full validation remains deferred to the final project-wide validation milestone.

Explicit exclusions

Do not implement:

* background operator host;
* background handoff;
* signal forwarding;
* cancellation;
* processing checkpoints;
* automatic incremental-processing policy;
* fixed source schemas;
* ORM behavior;
* decoded response readers;
* new HTTP capabilities;
* client certificates;
* proxies;
* lexicon build;
* automatic build-before-run;
* source migration;
* cross-compilation;
* MZA changes;
* installer changes.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* per-transaction provenance correction;
* typed provenance-cache behavior;
* removal of production processing expect and unwrap;
* final running-session ownership representation;
* setup plus persistence error preservation;
* stable processing failure codes;
* exact raw/acquisition/processed root validation;
* exact partial-directory classification;
* final processing-context invariants;
* catalog/context identity agreement;
* database state-transition behavior;
* source transaction-boundary detection;
* simultaneous catalog/database borrowing API;
* final ProcessingError representation;
* SQLite open flags;
* SQLite pragma verification;
* database creation durability;
* post-commit durability;
* rollback-journal policy;
* WAL/SHM rejection;
* commit-outcome uncertainty behavior;
* combined typed-error accessors;
* generated processing placeholder behavior;
* sensitive Debug and Display behavior;
* final processing runner sequence;
* public/internal API boundary;
* acquisition raw-data immutability confirmation;
* confirmation that foreground supervision remained unchanged;
* confirmation that background supervision and lexicon build were not added;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, runtime execution, SQLite execution, HTTP execution, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin background supervision until this processing closure is complete.