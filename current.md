Current implementation milestone: durable acquisition checkpoints and historical transaction lookup

Objective

Implement the Core-owned durable checkpoint primitives required by workspace/specs/contract.md.

The completed acquisition path already provides:

session-bound HTTP context
→ Core-mediated HTTP execution
→ immutable finalized raw transaction
→ durable acquisition progress
→ source-visible RecordedTransaction

This milestone adds:

verified source-specific work
→ durable checkpoint commit
→ checkpoint discovery across acquisition sessions
→ historical transaction lookup by logical request key
→ source-defined resume behavior

The source decides what a checkpoint means.

Core owns:

* checkpoint identity validation;
* durable checkpoint persistence;
* checkpoint/session/transaction provenance;
* atomic publication;
* checkpoint admission;
* cross-session discovery;
* historical finalized-transaction lookup;
* safe response-header lookup.

Do not implement automatic source workflow resumption. The registered resume handler remains ordinary sequential Rust and decides what to skip, repeat, or continue.

Contract authority

Follow:

workspace/specs/contract.md

The intended source-facing pattern is:

let checkpoint = format!("item/{}", item.id);
if context.has_checkpoint(&checkpoint)? {
    continue;
}
let transaction = context.execute(
    HttpRequest::get(&item.url)?
        .logical_key(&checkpoint),
)?;
transaction.response().require_success()?;
// Source-specific verification occurs here.
context.commit_checkpoint(&checkpoint)?;

For conditional requests:

if let Some(etag) =
    context.latest_response_header(&checkpoint, "ETag")?
{
    request = request.header("If-None-Match", etag)?;
}

Preserve that ordinary-Rust model.

Repository-grounded starting point

At commit:

2d6b03b53a3e5731e7855f7db5b72b169863ec71

the following already exist:

* acquisition and resume handler registration;
* run/resume admission;
* session-bound HttpAcquisitionContext;
* finalized immutable HTTP transactions;
* typed transaction identities;
* typed attempt identities;
* typed logical request keys;
* strict transaction admission from disk;
* transaction body length and SHA-256 verification;
* acquisition progress persistence;
* foreground session creation and resume selection.

The following remain absent:

* has_checkpoint(...);
* commit_checkpoint(...);
* checkpoint schema and durable files;
* checkpoint admission;
* cross-session checkpoint discovery;
* historical transaction lookup by logical key;
* latest_response_header(...);
* an in-memory association between successful execute(...) results and logical request keys.

Implement those missing boundaries.

Logical request key correction

The current HttpLogicalRequestKey rejects:

/
\

because it was initially treated as if it might become a raw filesystem component.

The contract explicitly uses logical keys such as:

item/<id>
manifest/page/2
archive/2026-08

Logical request keys must never be used directly as filesystem paths.

Required correction

Allow / and \ as ordinary logical-key characters.

Continue rejecting:

* empty keys;
* control characters;
* NUL;
* values exceeding the configured UTF-8 byte maximum.

Do not trim or normalize the key.

Do not interpret:

/
\
.
..
:

as path syntax.

The exact logical key is metadata, not a path.

Checkpoint filenames must be derived from a cryptographic hash of the exact UTF-8 key bytes.

Canonical checkpoint key

Use the same validated logical identity for:

* HttpRequest::logical_key(...);
* checkpoint lookup;
* checkpoint commit;
* transaction lookup.

A checkpoint key may be represented by:

HttpLogicalRequestKey

or a narrow wrapper:

pub struct HttpCheckpointKey(
    HttpLogicalRequestKey,
);

Do not introduce two incompatible validation rules for logical request and checkpoint keys.

Provide source-facing APIs that continue accepting &str or impl AsRef<str> for convenience and return typed validation errors.

Checkpoint schema version

Define a schema version independent from:

* raw transaction schema;
* acquisition progress schema;
* session schema;
* invocation schema;
* runtime-information schema.

For example:

pub const HTTP_CHECKPOINT_SCHEMA_VERSION: u32 = 1;

Use strict decoding:

#[serde(deny_unknown_fields)]

Unknown schema versions must be typed.

Checkpoint storage layout

Store checkpoint records below the acquisition session that committed them:

get-raw-data/
└── sessions/
    └── <session-id>/
        └── checkpoints/
            └── <sha256-of-logical-key>.json

Requirements:

* checkpoint files remain part of detailed durable session history;
* checkpoint keys are never used directly as path components;
* the filename is lowercase hexadecimal SHA-256 of the exact logical-key UTF-8 bytes;
* each checkpoint file is immutable after successful publication;
* checkpoint files from failed, stale-reconciled, succeeded, or later abandoned sessions remain durable;
* no operation-root global mutable checkpoint index is required in this milestone;
* cross-session lookup derives state from admitted immutable checkpoint records.

Do not place checkpoint files inside:

data/raw/
data/processed/
runtime/

Do not add a mutable root-level checkpoint summary.

Checkpoint document

Define an opaque checkpoint document equivalent to:

pub struct HttpCheckpointV1 {
    schema_version: u32,
    key: HttpLogicalRequestKey,
    key_sha256: String,
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
    transaction: HttpTransactionIdentity,
    attempt: HttpAttemptIdentity,
    committed_at_unix_nanos: u64,
}

Equivalent naming is acceptable.

Include at least:

* schema version;
* exact logical key;
* SHA-256 of the key;
* project identity;
* runtime identity;
* committing session identity;
* finalized transaction identity;
* attempt identity;
* commit timestamp.

The runtime identity must represent:

HTTP acquisition

Do not persist:

* source arguments;
* invocation-envelope JSON;
* runtime-context JSON;
* URLs;
* headers;
* body contents;
* arbitrary source error messages;
* filesystem paths.

Checkpoint provenance

A checkpoint commit must refer to a finalized, progress-published transaction from the current managed context.

The transaction must:

* belong to the current session;
* have a logical request key;
* use the same exact logical key as the checkpoint;
* represent a finalized response transaction;
* have completed acquisition-progress publication;
* still pass strict transaction admission from disk;
* remain below the context’s trusted raw-data root.

Do not allow a source to commit a checkpoint for:

* an arbitrary transaction identity;
* a transaction from another session;
* a transport-failure transaction;
* a partial transaction;
* a transaction whose logical key differs;
* a transaction that failed strict admission;
* a transaction path supplied by source code.

The source remains responsible for interpreting the response and deciding whether its source-specific verification succeeded before calling commit_checkpoint(...).

Core proves only durable transaction provenance and checkpoint publication.

Context transaction registry

Extend HttpAcquisitionContext with private in-memory state tracking successfully returned transactions by logical request key.

After:

context.execute(request)

returns Ok(RecordedTransaction), register that finalized progress-published transaction when it has a logical key.

Requirements:

* store typed transaction identity and attempt identity;
* store or retain the admitted final transaction path internally;
* update the entry when a later successful execution uses the same key;
* never register transport-failure errors;
* never register retry/redirect attempts that were not returned as the final successful execute(...) result;
* never register a transaction before progress publication succeeds.

Do not expose a mutable registry.

Do not make checkpoint correctness depend only on the in-memory value; re-admit the transaction from disk during commit.

Source-facing checkpoint APIs

Add:

impl HttpAcquisitionContext {
    pub fn has_checkpoint(
        &self,
        key: impl AsRef<str>,
    ) -> AcquisitionResult<bool>;
    pub fn commit_checkpoint(
        &mut self,
        key: impl AsRef<str>,
    ) -> AcquisitionResult<CommittedHttpCheckpoint>;
}

Equivalent borrowing is acceptable if required by internal caching.

Provide an opaque returned value:

pub struct CommittedHttpCheckpoint {
    // private
}

Expose read-only accessors for:

* key;
* key hash;
* committing session identity;
* transaction identity;
* attempt identity;
* checkpoint file path;
* commit timestamp.

Do not provide a public unchecked constructor.

Ignoring the returned value with:

context.commit_checkpoint(&key)?;

must remain ergonomic.

Checkpoint commit validation order

Use this deterministic order:

1. Require a managed session context.
2. Validate the key.
3. Revalidate the current session record.
4. Require session state Running.
5. Require acquisition operation.
6. Require HTTP acquisition runtime identity.
7. Require matching session identity.
8. Require external supervisor lease ownership.
9. Locate the current context’s latest returned transaction for the key.
10. Re-admit that transaction from the trusted raw-data root.
11. Require transaction session identity matches the current session.
12. Require transaction logical key matches the checkpoint key.
13. Require the transaction outcome is an HTTP response.
14. Compute checkpoint filename from the exact key bytes.
15. Validate the checkpoint directory and target path.
16. Publish the immutable checkpoint atomically with no replacement.
17. Perform the platform-appropriate session/checkpoint directory durability step.
18. Return CommittedHttpCheckpoint.

Do not reorder filesystem publication ahead of provenance validation.

Checkpoint atomic publication

Checkpoint files are immutable.

Use this sequence:

create or validate checkpoints directory
→ serialize complete checkpoint document
→ create unique temporary file in checkpoints directory
→ write complete bytes
→ flush
→ sync file
→ atomically publish only if final checkpoint path does not exist
→ sync checkpoints directory
→ return committed checkpoint

Use the same cross-platform no-replace principles already established for transaction publication.

Do not use ordinary overwrite-capable rename.

Do not use NamedTempFile::persist(...) if it may replace an existing checkpoint.

A checkpoint target collision is not automatically an error; first admit the existing checkpoint.

Idempotent checkpoint behavior

Checkpoint commit must be idempotent.

If the current session already contains a checkpoint at the derived path:

1. strictly admit it;
2. require its exact logical key and key hash match;
3. require its project, runtime, and session identities match;
4. require its transaction and attempt identities match the current commit candidate;
5. return the existing admitted checkpoint.

If the derived path exists with incompatible contents, return a typed collision/corruption error.

Across different sessions, the same logical key may have one committed record per session.

Do not overwrite an existing checkpoint.

Cross-session checkpoint lookup

has_checkpoint(...) must search immutable checkpoint records across acquisition sessions below the operation root.

Required behavior:

1. Validate the current managed context and logical key.
2. Enumerate direct child session directories under:

get-raw-data/sessions/

3. Do not follow symlinks.
4. Derive the checkpoint filename from the key hash.
5. Inspect only that exact file within each session.
6. Strictly admit every existing candidate.
7. Require project identity agreement.
8. Require HTTP acquisition runtime identity agreement.
9. Require logical key and key hash agreement.
10. Require the referenced transaction still passes strict admission.
11. Require transaction/session/logical-key provenance agreement.
12. Return true when at least one valid committed checkpoint exists.
13. Return false when no candidate exists.

A malformed existing candidate is not equivalent to absence.

Return a typed corruption/admission error rather than silently skipping it.

Which historical session states count

A successfully committed checkpoint remains valid when its committing session later becomes:

* Succeeded;
* Failed;
* stale-reconciled Failed;
* Abandoned.

A checkpoint committed while the session was durably Running represents source-confirmed completed work and remains useful after ordinary or abnormal failure.

Do not require the historical session to have succeeded.

Reject a checkpoint whose referenced session record:

* is missing;
* cannot be decoded;
* belongs to another project/runtime/operation;
* never reached Running;
* disagrees with the checkpoint identity.

A checkpoint found in the current running session is also valid.

Checkpoint admission API

Provide a Core-owned admission API equivalent to:

pub fn admit_http_checkpoint_from_disk(
    trusted_operation_root: &Path,
    trusted_raw_root: &Path,
    checkpoint_path: &Path,
) -> Result<
    CommittedHttpCheckpoint,
    HttpCheckpointAdmissionError,
>;

Equivalent organization is acceptable.

Admission must validate:

* checkpoint path containment;
* exact sessions/<session-id>/checkpoints/<key-hash>.json layout;
* no symlinks;
* regular file type;
* document size limit;
* UTF-8 JSON;
* strict schema;
* supported schema version;
* logical-key validity;
* key hash correctness;
* filename/hash agreement;
* project identity;
* runtime identity;
* session identity;
* transaction identity;
* attempt identity;
* timestamp validity;
* session-record agreement;
* referenced transaction admission;
* transaction/session agreement;
* transaction/logical-key agreement;
* transaction/attempt agreement;
* response outcome requirement.

Do not trust a checkpoint merely because its JSON parses.

Checkpoint size limit

Define a bounded metadata limit, for example:

pub const MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES: usize =
    64 * 1024;

The limit applies only to checkpoint metadata.

Reject oversized documents before deserialization.

Checkpoint error hierarchy

Add typed errors equivalent to:

HttpCheckpointKeyError
HttpCheckpointEncodingError
HttpCheckpointDecodingError
HttpCheckpointAdmissionError
HttpCheckpointCommitError
HttpCheckpointLookupError

Equivalent nesting is acceptable.

Distinguish at least:

* unmanaged context;
* invalid key;
* session validation;
* supervisor lease unavailable;
* no current transaction for key;
* transaction admission;
* transaction session mismatch;
* transaction key mismatch;
* non-response transaction;
* managed-path validation;
* checkpoint-directory creation;
* encoding;
* temporary-file creation;
* write;
* file sync;
* atomic no-replace publication;
* directory sync;
* existing checkpoint corruption;
* identity mismatch;
* schema mismatch;
* key-hash mismatch;
* filename mismatch;
* oversized document;
* session-record admission;
* cross-session enumeration;
* referenced transaction missing or corrupt.

Implement:

std::fmt::Display
std::error::Error

Use source().

Integrate them into AcquisitionError without converting them to strings.

Historical finalized-transaction lookup

Add Core-owned lookup by logical request key.

Provide an API equivalent to:

impl HttpAcquisitionContext {
    pub fn latest_transaction(
        &self,
        key: impl AsRef<str>,
    ) -> AcquisitionResult<Option<RecordedTransaction>>;
}

Search only finalized transaction directories that pass:

admit_transaction_from_disk(...)

Ignore recognizable .partial-* directories.

Do not ignore malformed finalized-looking transaction directories.

Return a typed corruption/admission error if such a candidate must be considered but cannot be admitted.

Filter admitted transactions by:

* matching logical request key;
* matching project/protocol source scope through the trusted context;
* response outcome.

Do not return transport-failure transactions from this convenience API.

Deterministic latest-transaction selection

Select the latest matching response transaction using a deterministic tuple equivalent to:

request creation timestamp
physical attempt index
transaction identity

Do not rely on:

* directory enumeration order;
* filesystem modification time;
* lexical UUID ordering alone.

If two candidates have identical timestamp and attempt index, use canonical transaction identity as the final stable tie-breaker.

Latest response header API

Add:

impl HttpAcquisitionContext {
    pub fn latest_response_header(
        &self,
        key: impl AsRef<str>,
        header_name: impl AsRef<str>,
    ) -> AcquisitionResult<Option<String>>;
}

Required behavior:

1. Validate the logical key.
2. Validate the header name using the HTTP header-name parser.
3. Find the latest admitted response transaction for the key.
4. Search response headers case-insensitively.
5. Preserve repeated-header order.
6. Return the first matching value unless another exact policy is documented.
7. Return None if no transaction or header exists.
8. Return a typed non-UTF-8 error when the recorded value is encoded native bytes.
9. Reject access to managed-redacted headers.

At minimum, never return a usable value for:

* Set-Cookie;
* another header represented as the managed redaction marker.

Do not return the literal string:

<redacted>

as though it were a real historical header value.

Do not inspect partial transactions.

Historical metadata redaction boundary

Historical lookup uses the persisted admitted representation.

Therefore:

* source code can recover non-sensitive recorded headers such as ETag or Last-Modified;
* managed-sensitive headers remain unavailable;
* raw body files remain available through RecordedTransaction;
* no unredacted transport-only metadata is reconstructed;
* no live HTTP response is involved.

Do not weaken redaction to support conditional requests.

Resume behavior

The existing framework already:

* selects resume only for acquisition;
* requires a prior failed or stale-reconciled session;
* creates a new session with execution mode Resume;
* selects the registered resume handler;
* passes the same native source arguments.

Preserve that behavior.

The resume handler receives the same HttpAcquisitionContext APIs:

has_checkpoint
latest_transaction
latest_response_header
execute
commit_checkpoint

Core does not automatically invoke the acquisition handler from the resume handler.

Core does not interpret checkpoint keys.

Core does not automatically skip requests.

Core does not reconstruct source-local variables or loop state.

Run behavior

The normal acquisition handler may also use checkpoint and historical lookup APIs.

This supports incremental acquisition across successful runs.

Do not artificially restrict checkpoint lookup to resume mode.

Checkpoint commit remains restricted to the current running managed acquisition session.

Session lifecycle interaction

Checkpoint publication does not transition the session lifecycle state.

It requires the session to remain:

Running

Checkpoint commit must revalidate session and supervisor ownership immediately before final publication.

If the session or lease changes:

* do not publish the checkpoint;
* preserve the already finalized transaction;
* return a typed checkpoint commit failure.

Checkpoint publication does not update:

session.json
session_status.json
acquisition_progress.json

The immutable checkpoint file is the durable checkpoint record.

Do not introduce Running → Running session transitions.

Checkpoint visibility boundary

A checkpoint becomes visible to has_checkpoint(...) only after:

* its complete file is written;
* file sync succeeds;
* no-replace publication succeeds;
* checkpoint-directory durability step succeeds.

If publication succeeds but directory sync fails:

* preserve the published checkpoint;
* return a typed checkpoint partial commit;
* include the admitted or reconstructable checkpoint identity and path;
* do not delete it.

Later lookup may admit the checkpoint normally if it is present and valid.

Crash behavior

Required behavior:

* crash before checkpoint publication: no committed checkpoint is visible;
* crash after temp-file write: temporary file is ignored and cleaned when possible;
* crash after no-replace publication: valid checkpoint may be discovered later;
* checkpoint directory-sync uncertainty: typed partial commit;
* previously committed checkpoints remain immutable;
* finalized HTTP transactions remain unaffected.

Temporary checkpoint files must not be treated as committed records.

No checkpoint payload in this milestone

A checkpoint is a durable completion marker linked to one finalized transaction.

Do not add arbitrary checkpoint payload bytes or JSON in this milestone.

Sources may encode safe source-defined meaning in the logical key.

If arbitrary durable source state becomes necessary, it must be introduced later through an explicit size, redaction, and compatibility contract.

Source-facing exports

Export the supported checkpoint API through:

lexicon_core::http

At minimum export:

* committed checkpoint representation;
* checkpoint schema version;
* checkpoint size limit;
* checkpoint key/admission/commit/lookup errors;
* checkpoint admission function if intended for processing/framework use.

Do not expose internal unchecked document constructors.

Source-level acceptance requirements

Implement source and API coverage for the following behavior. Do not execute tests now.

1. Logical keys permit contract values such as item/123.
2. Logical keys are never used directly as paths.
3. Checkpoint filenames are SHA-256 of exact logical-key bytes.
4. Checkpoint documents are strict and versioned.
5. Checkpoint documents contain no URLs, headers, bodies, or source arguments.
6. Commit requires a managed running acquisition session.
7. Commit requires active supervisor ownership.
8. Commit requires a progress-published transaction from the current context.
9. Commit re-admits the transaction from disk.
10. Transaction and checkpoint session identities agree.
11. Transaction and checkpoint logical keys agree.
12. Transaction and checkpoint attempt identities agree.
13. Transport-failure transactions cannot be checkpointed.
14. Partial transactions cannot be checkpointed.
15. Checkpoint publication is atomic and no-replace.
16. Existing compatible checkpoint commit is idempotent.
17. Existing incompatible checkpoint is typed corruption.
18. Checkpoint directory sync failure is a typed partial commit.
19. has_checkpoint searches across session directories.
20. Directory enumeration order does not affect results.
21. Symlinked session/checkpoint paths are rejected.
22. Historical failed-session checkpoints remain valid.
23. Historical abandoned-session checkpoints remain valid.
24. Missing referenced transactions are typed.
25. Corrupt referenced transactions are typed.
26. latest_transaction admits files before returning them.
27. Partial transaction directories are ignored.
28. Malformed finalized transactions are not silently skipped.
29. Latest selection is deterministic.
30. latest_response_header supports ETag.
31. Header matching is case-insensitive.
32. Non-UTF-8 header values are typed.
33. Managed-redacted headers are never returned as secrets.
34. Run handlers may use checkpoints.
35. Resume handlers may use checkpoints.
36. Core does not interpret checkpoint meaning.
37. Checkpoint publication does not mutate session lifecycle state.
38. No checkpoint payload API is introduced.
39. Existing HTTP execution behavior remains unchanged.
40. Existing foreground and session ownership remains unchanged.

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

Do not add or execute the broad validation matrix now.

Preserve existing behavior

Do not change:

* raw transaction schema except where a narrowly required admitted accessor is needed;
* exact request-body recording;
* exact response-body recording;
* metadata redaction;
* HTTP retry behavior;
* HTTP redirect behavior;
* transaction publication;
* acquisition progress behavior;
* source handler signatures;
* resume registration;
* invocation-envelope JSON;
* argv transport;
* source arguments;
* HTTP admission;
* processing admission;
* session creation;
* session lease ownership;
* foreground launch;
* foreground reconciliation;
* runtime-information probes;
* capability identifiers;
* managed runner layout;
* source creation;
* source build;
* verification;
* staging;
* bundle admission;
* paired publication;
* CLI syntax;
* MZA;
* Protocol 1;
* installer behavior.

Keep:

HttpCapabilitySet::empty()

Do not advertise ClientCertificateV1.

Explicit exclusions

Do not implement:

* arbitrary checkpoint payloads;
* automatic workflow resumption;
* automatic source-loop reconstruction;
* processing transaction discovery;
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

* files created and changed;
* final checkpoint module structure;
* logical-request-key correction;
* checkpoint key representation;
* checkpoint schema version;
* checkpoint document size limit;
* checkpoint storage layout;
* checkpoint document fields;
* checkpoint filename derivation;
* context transaction registry behavior;
* has_checkpoint(...) API;
* commit_checkpoint(...) API;
* committed checkpoint representation;
* exact checkpoint validation order;
* transaction provenance requirements;
* checkpoint atomic no-replace publication behavior;
* checkpoint idempotency behavior;
* checkpoint partial-commit behavior;
* checkpoint admission behavior;
* historical session-state behavior;
* cross-session lookup behavior;
* corrupt checkpoint behavior;
* referenced transaction admission behavior;
* latest_transaction(...) API;
* deterministic latest-selection rule;
* latest_response_header(...) API;
* non-UTF-8 header behavior;
* managed-redacted header behavior;
* run-handler checkpoint behavior;
* resume-handler checkpoint behavior;
* confirmation that Core does not interpret checkpoint meaning;
* confirmation that no checkpoint payload API was added;
* confirmation that session lifecycle files are not mutated by checkpoint commit;
* capability-set result;
* confirmation that processing, SQLite, background supervision, and build behavior were not added;
* existing test source adjusted only for API alignment, if applicable;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, runtime execution, HTTP execution, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin processing behavior until durable checkpoints and historical lookup are complete.