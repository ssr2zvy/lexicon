Current implementation milestone: checkpoint identity, admission, and cross-platform publication closure

Objective

Correct and complete the durable HTTP checkpoint and historical-lookup implementation at commit:

d973c845d766ed7c7206795a527d3fcd25cb5499

The checkpoint architecture is present, but several source-level defects prevent it from satisfying the identity, path-containment, cross-platform publication, transaction-provenance, and typed-error guarantees required by workspace/specs/contract.md.

This is a corrective milestone.

Do not begin processing or SQLite behavior until this closure is complete.

Contract authority

Follow:

workspace/specs/contract.md

The source decides checkpoint meaning.

Core must prove:

* the checkpoint belongs to the exact project and compiled source runtime;
* the committing session reached Running;
* the referenced transaction is finalized and admitted;
* the transaction belongs to the checkpoint’s session;
* the transaction logical key and attempt identity agree;
* the checkpoint file occupies the exact managed path;
* publication is immutable and no-replace on Linux, macOS, and Windows;
* lookup never silently accepts malformed or mismatched records.

Repository-grounded defects

Correct every defect below.

1. Checkpoint runtime identity is incomplete

The current checkpoint document stores only:

runtime_protocol
runtime_operation

That does not identify the compiled source runtime.

Two different HTTP acquisition sources can therefore satisfy the persisted runtime fields.

The checkpoint must preserve the complete runtime identity:

* source identity;
* protocol;
* operation;
* source contract version.

Required correction

Replace the separate protocol/operation strings with a complete strict runtime identity document equivalent to:

pub struct CheckpointRuntimeIdentityDocument {
    source: String,
    protocol: String,
    operation: String,
    source_contract_version: u32,
}

Equivalent nesting is acceptable.

Admission must reconstruct and compare an:

OwnedRuntimeIdentity

covering all four fields.

The checkpoint must agree with:

* the expected runtime supplied by the managed context;
* the committing session record;
* the referenced transaction’s session record.

Do not admit a checkpoint belonging to another source merely because both use HTTP acquisition.

2. Public checkpoint admission accepts untyped expected identity strings

The current admission boundary accepts:

expected_project_name: &str
expected_session_id: Option<&str>

and independently validates protocol and operation.

Required correction

Use typed expected values.

Representative API:

pub fn admit_http_checkpoint_from_disk(
    trusted_operation_root: &Path,
    trusted_raw_root: &Path,
    checkpoint_path: &Path,
    expected_project: &ProjectIdentity,
    expected_runtime: &OwnedRuntimeIdentity,
    expected_session: Option<&SessionIdentity>,
) -> Result<
    CommittedHttpCheckpoint,
    HttpCheckpointAdmissionError,
>;

Equivalent organization is acceptable.

Require the expected runtime to be HTTP acquisition before admission proceeds.

Do not compare identities through separately supplied arbitrary strings.

3. Committed checkpoint omits project and runtime identity

CommittedHttpCheckpoint currently exposes key, session, transaction, attempt, path, and timestamp, but does not retain the admitted project or runtime identity.

Required correction

Add private typed fields:

project: ProjectIdentity
runtime: OwnedRuntimeIdentity
session: SessionIdentity

Provide read-only accessors.

Replace:

session_id() -> &str

with a typed accessor:

session() -> &SessionIdentity

A compatibility identifier accessor may remain if necessary.

Do not reconstruct identities later from arbitrary strings.

4. Checkpoint path admission does not prove exact operation-root containment

The current admission validates the operation root itself, then separately inspects the checkpoint path and recognizes any suffix shaped like:

sessions/<session>/checkpoints/<hash>.json

It does not first prove that the supplied checkpoint is exactly beneath:

<trusted-operation-root>/sessions/

Required correction

Derive the expected checkpoint path from trusted components:

trusted_operation_root
→ sessions
→ validated session identity
→ checkpoints
→ validated key-hash filename

Require the supplied path to equal that exact derived path.

Use the shared managed-path containment validator with:

trusted root = trusted_operation_root
target = checkpoint_path

Reject:

* another operation root;
* a nested duplicate sessions/ tree;
* additional parent directories;
* ..;
* alternate platform prefixes;
* symlinks;
* non-regular target files.

Suffix shape alone is insufficient.

5. Symlink errors are collapsed into a unit variant

The current helper:

check_no_symlinks_on_path(...)

returns:

Result<(), ()>

It also converts every metadata error into a generic symlink rejection.

Required correction

Remove this duplicate helper.

Use the shared typed managed-path validator.

Preserve distinctions between:

* symlink;
* missing component;
* metadata inspection failure;
* non-directory ancestor;
* wrong target type;
* path outside trusted root.

Do not use fs::metadata(...) as the authority for rejecting symlinks because it follows symlinks.

Use symlink_metadata(...).

6. Session-record errors are stringified into synthetic decoding errors

The current checkpoint session loader converts SessionStoreError through behavior equivalent to:

SessionDecodingError::StructuralDocument(
    error.to_string(),
)

This destroys the typed store error and may misclassify:

* missing session;
* filesystem I/O;
* document corruption;
* revision/state issues.

Required correction

Let HttpCheckpointAdmissionError retain the typed session-store failure:

SessionStore(SessionStoreError)

or an equivalent nested hierarchy.

Use source().

Do not synthesize a session decoding error from a display string.

Do not stringify session errors inside checkpoint lookup or commit helpers.

7. has_checkpoint stringifies session errors

The current helper used by has_checkpoint(...) converts session-store errors into newly constructed std::io::Error values containing to_string() output.

Required correction

Add typed lookup variants for:

* operation-root construction;
* session-store open;
* current session load;
* session enumeration;
* candidate admission.

Preserve the underlying typed errors through source().

Do not convert session errors into io::ErrorKind::Other.

8. has_checkpoint does not fully validate the active context

has_checkpoint(...) checks for a session identity and loads a session record, but it does not use the authoritative managed-context validation path.

Required correction

Before cross-session lookup, require:

* managed context;
* valid protocol root;
* valid operation root;
* valid current session directory;
* valid raw-data root;
* current session record exists;
* current session is Running;
* operation is acquisition;
* runtime is the expected HTTP acquisition runtime;
* current session identity agrees;
* external supervisor lease remains owned.

Reuse one authoritative validation helper.

Do not require resume mode specifically. Both run and resume handlers may look up checkpoints.

9. Symlinked session entries are silently skipped

Cross-session lookup currently skips symlink session entries and continues.

A malformed managed entry inside the Core-owned sessions directory is not equivalent to absence.

Required correction

When enumerating direct children of:

<operation-root>/sessions/

require every candidate that could represent a session entry to have an admitted managed type.

Return a typed managed-layout error for:

* symlinked session entry;
* non-UTF-8 session name where a Lexicon session identity is required;
* invalid session identifier;
* unexpected file occupying a session-identity name;
* metadata inspection failure.

Temporary or explicitly documented non-session entries may be ignored only through a narrow named rule.

Do not silently skip symlinked session directories.

10. Session names use lossy UTF-8 conversion

The lookup path currently uses:

to_string_lossy()

for session directory names included in typed errors.

Required correction

Parse session directory names through the established SessionIdentity constructor.

If the native name is not valid UTF-8 or is not a valid session identity, return a typed layout error.

Do not use lossy conversion.

11. Referenced transaction discovery uses suffix matching

Checkpoint admission scans the raw-data root for a directory whose name ends with the checkpoint transaction ID.

This is not exact transaction identity resolution.

It can:

* match malformed directory names;
* match unrelated suffixes;
* choose the first filesystem enumeration result;
* fail to detect multiple matches;
* depend on directory order.

Required correction

For every finalized-looking raw transaction directory:

1. parse its exact <timestamp>-<transaction-id> name;
2. reject malformed finalized-looking entries;
3. compare the parsed HttpTransactionIdentity;
4. collect exact identity matches;
5. require exactly one match;
6. admit that exact directory using admit_transaction_from_disk(...).

Return typed errors for:

* no exact match;
* multiple exact matches;
* malformed managed transaction entry;
* candidate admission failure.

Do not use:

contains
ends_with
starts_with

to identify a transaction.

12. Transaction session identity is not retained by RecordedTransaction

Checkpoint commit and admission reopen and deserialize:

request/metadata.json

outside the authoritative transaction admission boundary merely to recover the session ID.

Required correction

Extend the admitted transaction representation with typed provenance:

session: SessionIdentity
created_at_unix_nanos: u64

and, if useful:

runtime/project provenance supplied by the admitting context

At minimum provide:

RecordedTransaction::session()
RecordedTransaction::created_at_unix_nanos()

Construct these only through transaction recording or strict disk admission.

Checkpoint code must not independently deserialize transaction metadata after admit_transaction_from_disk(...) succeeds.

13. Commit validates only part of an existing attempt identity

The idempotency path compares:

physical_attempt_index

but does not compare:

* redirect index;
* retry index.

Required correction

Compare the complete typed:

HttpAttemptIdentity

Require equality across all fields.

Do not compare attempt identity field-by-field incompletely.

Implement PartialEq/Eq on the typed value and compare the complete type.

14. Checkpoint timestamp relationship is not validated

Admission requires a nonzero checkpoint timestamp but does not establish its relationship to:

* transaction creation;
* transaction completion;
* session start;
* session finish.

Required correction

Require:

checkpoint committed_at >= transaction response/failure completion timestamp
checkpoint committed_at >= session started_at

When the committing session is terminal, also require:

checkpoint committed_at <= session finished_at

Use the same timestamp representation and checked conversions.

Expose the transaction completion timestamp through the admitted transaction outcome rather than rereading raw JSON in checkpoint code.

Do not use filesystem modification times.

15. Attempt identity decoding is incomplete

Checkpoint decoding currently requires only:

physical_attempt_index >= 1

Required correction

Construct the attempt identity through one checked constructor.

Require invariants equivalent to:

* physical attempt index starts at one;
* redirect index is less than physical attempt index;
* retry index is less than physical attempt index;
* the first physical attempt has redirect index zero and retry index zero.

Use the same constructor for:

* transaction metadata admission;
* checkpoint admission;
* recorded transaction construction.

Do not duplicate partial attempt validation.

16. Checkpoint publication is not race-safe on Windows

The current non-Unix publication path:

1. checks whether the destination exists;
2. calls TempPath::persist(...).

On Windows this is a check-then-persist race and does not supply the required no-replace guarantee.

Required correction

Use one atomic no-replace checkpoint-file publication abstraction with platform-specific implementations:

* Linux;
* macOS;
* Windows.

Windows must use native wide paths and a no-replace API.

Do not use:

exists check
→ overwrite-capable persist

as the correctness boundary.

The source temporary file and final checkpoint must remain on the same filesystem.

Return typed:

* collision;
* I/O;
* unsupported platform;
* path encoding/argument errors.

17. Unix hard-link publication needs one explicit durability contract

The Unix implementation uses a hard link from the temporary file to the final checkpoint path and then removes the temporary name.

This can provide no-replace behavior, but its durability and cleanup semantics must be explicit.

Required correction

Ensure this exact sequence:

1. temporary file is fully written and synced;
2. create final hard link with no-replace behavior;
3. synchronize checkpoint directory;
4. remove temporary name;
5. synchronize checkpoint directory again if removal durability is required;
6. return committed checkpoint.

If final link succeeds but later cleanup or directory sync fails:

* retain the published checkpoint;
* return a typed partial commit;
* do not report ordinary pre-publication failure.

Do not delete the final checkpoint during recovery.

Equivalent native no-replace rename primitives may replace the hard-link approach.

18. Checkpoint partial commit lacks checkpoint provenance

HttpCheckpointPartialCommitError currently carries only:

directory_sync_error

It does not identify the checkpoint that was already published.

Required correction

Make the partial-commit error own or retain a reconstructable committed checkpoint representation.

Equivalent design:

pub struct HttpCheckpointPartialCommit {
    checkpoint: CommittedHttpCheckpoint,
    source: HttpCheckpointPostPublicationError,
}

Provide accessors for:

* checkpoint;
* checkpoint path;
* key;
* session identity;
* transaction identity;
* attempt identity.

Every failure after no-replace publication must use this type.

Do not discard the published checkpoint owner.

19. Checkpoint document size is checked only during admission

Before publication, serialized checkpoint JSON must also respect:

MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES

Required correction

After serialization and before temporary-file creation:

* measure exact serialized bytes;
* reject oversized output through a typed encoding/commit error.

Do not rely on the current schema merely being small.

Admission retains its independent size check.

20. Commit directory validation trusts the protocol root too broadly

Checkpoint commit validates its target beneath:

protocol_root

The actual trusted checkpoint root is the current session directory.

Required correction

Use:

trusted root = current session directory
target = current session directory/checkpoints/<hash>.json

Validate the session directory itself through the operation-root relationship first.

Do not use the broader protocol root as the immediate checkpoint containment root.

21. Directory creation and validation are vulnerable to replacement between steps

The current path performs:

validate creatable path
→ create_dir_all
→ use directory

without revalidating the created directory immediately before temporary-file creation.

Required correction

After creating the checkpoint directory:

* revalidate it as an existing managed directory;
* reject symlink or wrong type;
* validate the final target beneath that directory;
* create the temporary file directly inside the revalidated directory.

Immediately before final publication:

* revalidate the checkpoint directory;
* revalidate the target as creatable or existing-idempotent;
* revalidate the running session and supervisor lease.

The trusted-source model does not require hostile sandboxing, but Core must not knowingly use stale path validation.

22. Commit registry uses string keys instead of the canonical type

The transaction registry is:

HashMap<String, TransactionRegistryEntry>

although HttpLogicalRequestKey is hashable/equatable in concept.

Required correction

Implement:

Hash

for HttpLogicalRequestKey and use:

HashMap<HttpLogicalRequestKey, TransactionRegistryEntry>

Do not repeatedly convert the canonical key to arbitrary strings for internal lookup.

23. Registry entries duplicate transaction identity data

The registry stores:

* transaction identity;
* attempt identity;
* transaction path.

Checkpoint commit then re-admits the transaction and separately compares parts.

Required correction

Use one narrow registry entry that identifies the returned progress-published transaction without pretending to remain authoritative.

For example:

struct TransactionRegistryEntry {
    identity: HttpTransactionIdentity,
    final_path: PathBuf,
}

After disk admission, use the admitted transaction as the authority for:

* attempt identity;
* logical key;
* session identity;
* outcome.

Compare the admitted transaction identity with the registry identity.

Do not trust duplicated attempt fields in the registry over admitted disk state.

24. Historical transaction lookup does not require managed context

The completion report states:

latest_transaction does not require a managed context

That contradicts the milestone’s managed historical-lookup boundary.

Required correction

Require the same managed running acquisition context validation used by:

* execute;
* has_checkpoint;
* commit_checkpoint.

Historical lookup must derive:

* trusted raw root;
* expected project;
* expected runtime;
* current session ownership

from the validated context.

The quarantined legacy context must not gain managed historical lookup guarantees.

Return a typed unmanaged-context error.

25. Historical transaction filtering lacks source/session provenance

latest_transaction(...) filters primarily by logical key and response outcome.

Required correction

For each candidate:

* strictly admit the transaction;
* load and validate its referenced session record;
* require project identity agreement;
* require complete runtime identity agreement;
* require acquisition operation;
* require transaction session identity agrees with the session record;
* require logical key agreement;
* require response outcome.

Historical transactions from another source runtime must not be returned.

26. Latest transaction scan needs exact managed-entry classification

Define which raw-root entries are:

* finalized transaction;
* recognizable .partial-* transaction;
* explicitly supported temporary entry;
* invalid unexpected managed entry.

Required behavior:

* ignore recognizable partial transactions;
* admit every finalized-looking transaction;
* return typed corruption for malformed finalized-looking entries;
* reject symlink entries;
* reject duplicate admitted transaction identities;
* do not depend on enumeration order.

Do not silently skip an entry solely because its type is inconvenient.

27. Latest response header uses source-message errors

The current implementation reports:

* invalid header name;
* non-UTF-8 recorded value

through:

AcquisitionError::source_message(...)

These are Core-owned lookup failures, not arbitrary source failures.

Required correction

Define:

HttpHistoricalLookupError

or equivalent typed errors for:

* unmanaged context;
* invalid logical key;
* invalid header name;
* raw-root enumeration;
* managed-entry corruption;
* transaction admission;
* session provenance;
* non-UTF-8 header value;
* redacted header unavailable.

Integrate through a typed AcquisitionError variant.

Do not use source_message for Core-owned lookup failures.

28. Redaction is represented by a magic string

Historical lookup treats:

<redacted>

as a magic marker.

A legitimate non-sensitive header whose literal value is <redacted> becomes indistinguishable from a managed-redacted value.

Required correction

Represent persisted header state explicitly:

pub enum StoredHeaderValue {
    Utf8(String),
    Base64(String),
    Redacted,
}

Expose the corresponding source-facing state:

RecordedHeaderValue::Redacted

Update:

* request metadata writing;
* response metadata writing;
* transaction admission;
* mandatory-sensitive-header validation;
* historical-header lookup.

Do not identify redaction by comparing an ordinary string value.

This changes the raw metadata representation. Because the raw transaction schema remains version 1, either:

1. update schema version deliberately and admit the previous form explicitly; or
2. confirm that version one has not reached a compatibility boundary and document the intentional pre-release schema correction.

Do not silently reinterpret existing version-one values without a migration/admission rule.

29. Redacted header lookup should be typed

For managed-sensitive headers, choose one explicit result:

Ok(None)

or:

Err(HttpHistoricalLookupError::HeaderRedacted)

Prefer a typed HeaderRedacted error when the header exists but cannot be returned, and Ok(None) only when no matching header exists.

Document the exact behavior.

Do not return redacted marker text.

30. Response header duplicate behavior must be explicit

The current implementation returns the first matching header.

Required correction

Keep that behavior only if documented:

first matching header in persisted transport order

Alternatively expose:

latest_response_headers(...)

returning all admitted matching values.

Do not concatenate repeated headers using commas because not every HTTP header is safely comma-joinable.

At minimum, retain the first-value API required by the contract and preserve recorded order.

31. Checkpoint project/runtime identity must come from the session record

During commit, construct the checkpoint identity fields from the admitted current session record.

Do not independently hardcode:

RuntimeProtocol::Http
RuntimeOperation::Acquisition

as a substitute for persisting the full runtime identity.

The validation step may require HTTP acquisition, but the document must record the actual admitted identity.

32. Checkpoint discovery should avoid first-result dependence

has_checkpoint(...) returns immediately when it finds the first valid checkpoint.

That can hide a later corrupt duplicate candidate for the same logical key.

Required correction

Enumerate every exact candidate for the key.

Required result:

* no candidates: false;
* one or more valid candidates and no corrupt candidates: true;
* any corrupt or mismatched candidate: typed error.

Sort candidate paths or session identities before admission to make error selection deterministic.

Do not depend on filesystem enumeration order.

33. Checkpoint admission must detect duplicate referenced transactions

If more than one finalized transaction directory claims the same transaction identity, checkpoint admission must return typed ambiguity.

Do not select the first admitted match.

34. Commit idempotency must compare complete identity

An existing checkpoint is idempotently equivalent only when all of these agree:

* schema version;
* exact key;
* key hash;
* project identity;
* complete runtime identity;
* session identity;
* transaction identity;
* complete attempt identity.

Timestamp may differ only if the existing admitted checkpoint is returned as the authoritative prior commit. It must not be rewritten.

35. Checkpoint API duplication and visibility

Keep checkpoint document internals private.

Review exports and expose only source-useful types:

* CommittedHttpCheckpoint;
* checkpoint schema/size constants if needed;
* typed public errors;
* public admission only if framework/processing genuinely needs it.

Do not publicly expose internal serialization helpers such as:

* filename builders;
* key-hash helpers;
* document constructors;
* atomic writers.

Final checkpoint sequence

After this correction, checkpoint commit must be:

validate managed running acquisition context
→ validate canonical logical key
→ locate current progress-published transaction
→ strictly admit transaction
→ verify current session and full identity provenance
→ derive SHA-256 checkpoint path under current session
→ admit existing checkpoint for idempotency, if present
→ construct strict full-identity document
→ enforce encoded size limit
→ revalidate session, lease, and managed directory
→ write and sync unique temporary file
→ atomically publish no-replace on Linux/macOS/Windows
→ perform platform-appropriate directory durability step
→ return CommittedHttpCheckpoint

Every post-publication failure must retain the committed checkpoint identity.

Error hierarchy

Use a coherent typed hierarchy equivalent to:

HttpCheckpointKeyError
HttpCheckpointEncodingError
HttpCheckpointDecodingError
HttpCheckpointAdmissionError
HttpCheckpointPublicationError
HttpCheckpointPartialCommit
HttpCheckpointCommitError
HttpCheckpointLookupError
HttpHistoricalLookupError

All implement:

std::fmt::Display
std::error::Error

Use source().

Do not convert session, transaction, path, JSON, or filesystem errors into display strings inside the checkpoint engine.

Sensitive diagnostics

Checkpoint and historical-lookup diagnostics must not reveal:

* logical key contents;
* URLs;
* header values;
* bodies;
* source arguments;
* runtime-context JSON;
* invocation-envelope JSON;
* environment-variable names or values;
* arbitrary source error text.

Safe diagnostics may include:

* checkpoint hash;
* transaction identity;
* session identity;
* stable runtime identifiers;
* managed path category;
* schema version.

Do not print diagnostics from Core.

Preserve existing behavior

Do not change:

* source handler signatures;
* acquisition/resume registration;
* HTTP request execution;
* retry behavior;
* redirect behavior;
* raw body fidelity;
* acquisition progress;
* invocation-envelope JSON;
* argv transport;
* source arguments;
* HTTP admission;
* processing admission;
* session lifecycle transitions;
* supervisor lease ownership;
* foreground launch;
* foreground reconciliation;
* runtime-information probes;
* capability identifiers;
* managed runner layout;
* source creation;
* source build;
* runtime verification;
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

Source-level acceptance requirements

Correct the source so that:

1. Checkpoints persist complete runtime identity.
2. Typed expected identities enter checkpoint admission.
3. Committed checkpoints retain typed project/runtime/session identities.
4. Checkpoint paths are exact descendants of the operation root.
5. Suffix-only path admission is removed.
6. Symlink and path errors remain typed.
7. Session-store errors are not stringified.
8. has_checkpoint validates the managed running context.
9. Symlink session entries are not silently skipped.
10. Session names are never lossily decoded.
11. Referenced transaction matching is exact.
12. Missing and ambiguous referenced transactions are typed.
13. Recorded transactions retain typed session provenance.
14. Idempotency compares the complete attempt identity.
15. Checkpoint timestamps obey session/transaction ordering.
16. Attempt identities use one checked constructor.
17. Checkpoint publication is atomic no-replace on Windows.
18. Unix post-publication cleanup failures are partial commits.
19. Checkpoint partial commits retain checkpoint provenance.
20. Serialized checkpoint size is checked before publication.
21. Commit containment is rooted at the current session directory.
22. Paths are revalidated after directory creation.
23. Transaction registry uses the canonical logical-key type.
24. Re-admitted transaction state is authoritative over registry duplicates.
25. latest_transaction requires managed context.
26. Historical transaction source/runtime/session provenance is verified.
27. Partial transactions remain ignored.
28. Corrupt finalized transactions are typed.
29. Historical header failures use Core-owned typed errors.
30. Redaction no longer depends on a magic string.
31. Managed-redacted values are never returned.
32. Duplicate response-header ordering remains explicit.
33. Checkpoint document identities come from the admitted session.
34. has_checkpoint examines all matching candidates deterministically.
35. Checkpoint internals remain private.

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
* HTTP servers;
* real or test HTTP requests;
* workspace validation;
* bundle/install automation.

Existing test source may be adjusted only where production API alignment requires it.

Do not add or execute the broad validation matrix.

Explicit exclusions

Do not implement:

* arbitrary checkpoint payloads;
* automatic workflow resumption;
* processing raw-transaction discovery;
* processing SQLite behavior;
* decoded response readers;
* client certificates;
* proxy configuration;
* background operator host;
* signal forwarding;
* background supervision;
* lexicon build;
* automatic build-before-run;
* source migration;
* cross-compilation;
* MZA changes;
* installer changes.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* complete checkpoint runtime identity representation;
* typed checkpoint admission inputs;
* committed checkpoint identity accessors;
* exact checkpoint path-containment behavior;
* managed symlink/path error behavior;
* typed session-record failure behavior;
* active-context validation for checkpoint lookup;
* session-directory enumeration behavior;
* native session-name behavior;
* exact referenced-transaction resolution;
* ambiguous referenced-transaction behavior;
* recorded transaction session/timestamp provenance;
* complete attempt-identity validation;
* checkpoint timestamp ordering;
* Linux checkpoint publication behavior;
* macOS checkpoint publication behavior;
* Windows checkpoint publication behavior;
* post-publication partial-commit behavior;
* committed checkpoint ownership in partial errors;
* pre-publication encoded-size enforcement;
* checkpoint path revalidation behavior;
* canonical registry-key behavior;
* managed historical-transaction validation;
* historical transaction source/runtime/session filtering;
* typed latest-header errors;
* explicit redaction representation;
* managed-redacted header lookup behavior;
* repeated-header selection behavior;
* deterministic all-candidate checkpoint lookup;
* checkpoint public/internal API boundary;
* capability-set result;
* confirmation that processing, SQLite, background supervision, and build behavior were not added;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, runtime execution, HTTP execution, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin processing until checkpoint identity, admission, and cross-platform publication are complete.