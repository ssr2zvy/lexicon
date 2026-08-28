# Completion report — Durable HTTP Acquisition Checkpoints and Historical Lookup

## Files created

- `lexicon-core/src/protocols/http/checkpoint/mod.rs`  
  Module root; re-exports all public types from `error` and `model`.

- `lexicon-core/src/protocols/http/checkpoint/error.rs`  
  All seven checkpoint error types (see below).

- `lexicon-core/src/protocols/http/checkpoint/model.rs`  
  Checkpoint document schema, `CommittedHttpCheckpoint` opaque type, admission
  logic, and atomic publication helper.

## Files changed

- `lexicon-core/src/protocols/http/transaction/mod.rs`  
  `HttpLogicalRequestKey` character rejection corrected (see below).

- `lexicon-core/src/protocols/http/context.rs`  
  `HttpAcquisitionContext` extended with transaction registry, `execute`
  wrapper/impl split, and four new public methods.

- `lexicon-core/src/protocols/http/error.rs`  
  `AcquisitionError` extended with `CheckpointCommit`, `CheckpointLookup`, and
  `TransactionAdmission` variants.

- `lexicon-core/src/protocols/http/mod.rs`  
  Added `pub mod checkpoint` and re-exported all public checkpoint types.

---

## Final checkpoint module structure

```
lexicon-core/src/protocols/http/checkpoint/
  mod.rs      — public re-exports
  error.rs    — all error enums
  model.rs    — document schema, CommittedHttpCheckpoint, admission, helpers
```

---

## Logical-request-key correction

`HttpLogicalRequestKey::new` previously rejected `/` and `\` as invalid
characters.  The rejection predicate now only rejects:

- empty string;
- strings longer than 512 bytes (UTF-8);
- strings containing NUL bytes (`\x00`);
- strings containing ASCII control characters (bytes `\x01`–`\x1f` and
  `\x7f`).

Path-separator characters `/` and `\` are now accepted as ordinary characters
in logical request keys.

---

## Checkpoint key representation

The checkpoint key is the `HttpLogicalRequestKey` string value stored verbatim
in the checkpoint document (`"key"` field).  The key is never used directly as
a filesystem path component.

---

## Checkpoint schema version

`HTTP_CHECKPOINT_SCHEMA_VERSION = 1` (exported constant from `checkpoint`
module).

---

## Checkpoint document size limit

`MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES = 65536` (64 KiB, exported constant).
Documents exceeding this size are rejected during admission.

---

## Checkpoint storage layout

```
<operation_root>/sessions/<session-id>/checkpoints/<key-sha256>.json
```

The `checkpoints/` directory is created lazily by `commit_checkpoint` before
first write.

---

## Checkpoint document fields

```json
{
  "schema_version":          1,
  "key":                     "<logical key string>",
  "key_sha256":              "<64-char lowercase hex SHA-256 of key UTF-8 bytes>",
  "project_name":            "<project name string>",
  "runtime_protocol":        "http",
  "runtime_operation":       "acquisition",
  "session_id":              "<session UUID string>",
  "transaction_id":          "<transaction UUID string>",
  "physical_attempt_index":  0,
  "redirect_index":          0,
  "retry_index":             0,
  "committed_at_unix_nanos": 1234567890000000000
}
```

Unknown fields are rejected on decode (`serde deny_unknown_fields`).

---

## Checkpoint filename derivation

The filename is the lowercase hex SHA-256 digest of the exact UTF-8 byte
representation of the logical key string, with a `.json` suffix:

```
<64-char hex SHA-256 of key bytes>.json
```

The filename stem must consist of exactly 64 lowercase hexadecimal digits.
Any other filename is rejected by `extract_layout_parts`.

---

## Context transaction registry behavior

`HttpAcquisitionContext` now holds an internal `HashMap<String,
TransactionRegistryEntry>` keyed by the logical request key string.

After every call to `execute`, if the result is `Ok(tx)` and the transaction
has a logical key and its outcome is `HttpRecordedOutcome::Response`, the
registry is updated (inserting or replacing) with a
`TransactionRegistryEntry { transaction_identity, attempt_identity,
transaction_path }`.

Transport failures and transactions without a logical key are never registered.
The registry survives across multiple `execute` calls; the entry for a key is
always replaced with the latest successful response for that key.

---

## `has_checkpoint(key: impl AsRef<str>) -> AcquisitionResult<bool>`

1. Requires a managed context (`session_identity` is `Some`); returns
   `AcquisitionError::CheckpointLookup(UnmanagedContext)` otherwise.
2. Validates and constructs `HttpLogicalRequestKey`; returns
   `InvalidKey` on failure.
3. Loads the current session record to obtain the `project_name` for
   cross-session identity filtering.
4. Computes `checkpoint_filename(key)`.
5. Reads `<operation_root>/sessions/` directory entries (non-symlink
   directories only).
6. For each session subdirectory, checks whether
   `<session_dir>/checkpoints/<filename>` exists.
7. If the file exists, calls `admit_http_checkpoint_from_disk` with
   `expected_session_id = None` (accepts any session).
   - On success: returns `Ok(true)`.
   - On admission error: returns `Err(AcquisitionError::CheckpointLookup(
     CorruptCandidate { session_id, source }))`.
8. If no session has a matching checkpoint, returns `Ok(false)`.

---

## `commit_checkpoint(key: impl AsRef<str>) -> AcquisitionResult<CommittedHttpCheckpoint>`

1. Requires managed context; returns `UnmanagedContext` otherwise.
2. Validates and constructs `HttpLogicalRequestKey`.
3. Loads the current session record; verifies state is `Running`, operation
   is `Acquisition`, runtime is HTTP/acquisition, session identity matches,
   and lease is `Owned`.
4. Looks up the transaction registry for the key; returns `NoTransactionForKey`
   if absent.
5. Re-admits the transaction from disk via `admit_transaction_from_disk`.
6. Reads `request/metadata.json` from the transaction directory; verifies the
   `session_id` field matches the current session.
7. Verifies the transaction's logical key matches the checkpoint key.
8. Verifies the transaction outcome is `HttpRecordedOutcome::Response` (not a
   transport failure).
9. Computes `key_sha256_hex(key)` and the target path
   `<session_directory>/checkpoints/<sha256>.json`.
10. Validates both `checkpoints/` directory and `target_path` as managed paths
    under `protocol_root`.
11. Creates `checkpoints/` directory if absent.
12. **Idempotency check**: if `target_path` already exists, calls
    `admit_http_checkpoint_from_disk` on it with `expected_session_id =
    Some(session_id)`; if the admitted checkpoint's key, key_sha256, session_id,
    transaction_identity, and attempt_identity all match the current commit
    request, returns `Ok(existing)` immediately.  If fields disagree, returns
    `ExistingIdentityMismatch`.
13. Acquires `committed_at_unix_nanos` from the monotonic wall clock.
14. Re-validates the session and lease immediately before publication.
15. Serializes `HttpCheckpointDocumentV1` to JSON; writes to a temp file in
    `checkpoints/`; syncs; publishes atomically with no-replace semantics (see
    below).
16. Syncs the `checkpoints/` directory; if directory sync fails, returns
    `PartialCommit(HttpCheckpointPartialCommitError)` (the checkpoint file
    itself was successfully written).
17. Constructs and returns `CommittedHttpCheckpoint`.

---

## Committed checkpoint representation

`CommittedHttpCheckpoint` is an opaque `pub struct` with read-only accessors:

- `key() -> &HttpLogicalRequestKey`
- `key_sha256() -> &str`
- `session_id() -> &str`
- `transaction_identity() -> &HttpTransactionIdentity`
- `attempt_identity() -> &HttpAttemptIdentity`
- `checkpoint_path() -> &Path`
- `committed_at_unix_nanos() -> u64`

There is no public constructor.  `CommittedHttpCheckpoint` can only be obtained
from `commit_checkpoint` or `admit_http_checkpoint_from_disk`.

---

## Exact checkpoint validation order (admission)

`admit_http_checkpoint_from_disk` performs checks in this order:

1. Validate `trusted_operation_root` as an existing managed directory.
2. Check no symlinks on any component of `checkpoint_path`.
3. Confirm `checkpoint_path` is a regular file (not a directory or symlink).
4. Extract layout parts: confirm path matches
   `…/sessions/<id>/checkpoints/<64-hex>.json`.
5. Read file; reject if exceeds `MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES`.
6. Deserialize JSON; verify `schema_version == 1`; reject unknown fields.
7. Verify `doc.key_sha256 == sha256_hex(doc.key)`.
8. Verify `doc.project_name == expected_project_name`.
9. Verify `doc.runtime_protocol` parses to `RuntimeProtocol::Http`.
10. Verify `doc.runtime_operation` parses to `RuntimeOperation::Acquisition`.
11. Verify `doc.session_id == layout_session_id` (path consistency).
12. If `expected_session_id` is `Some(s)`, verify `doc.session_id == s`.
13. Load the session record for `doc.session_id`; verify project, runtime, and
    that the session was started (`state != Prepared` and `started_at.is_some()`).
14. Find and admit the transaction from `trusted_raw_root` by scanning for a
    directory whose name ends with `doc.transaction_id`; verify session_id in
    request metadata.
15. Verify `transaction.logical_request_key() == doc.key`.
16. Verify transaction outcome is `Response`.
17. Verify `transaction.attempt_identity()` fields match
    `doc.physical_attempt_index`, `doc.redirect_index`, `doc.retry_index`.
18. Reconstruct and return `CommittedHttpCheckpoint`.

---

## Transaction provenance requirements

Before `commit_checkpoint` will proceed, the referenced transaction must:

- be recorded in the in-memory transaction registry for the current context
  (registered by a prior successful `execute` call in this session);
- admit successfully from disk;
- have its `request/metadata.json` `session_id` equal to the current session;
- have a logical key equal to the checkpoint key;
- have an outcome of `HttpRecordedOutcome::Response`.

---

## Checkpoint atomic no-replace publication behavior

On Unix: creates a hard link from the temp file to the target path, then
removes the temp file.  If `hard_link` fails with `AlreadyExists`, the error
is propagated as `AtomicPublication`.

On non-Unix: checks if the target file exists; if so, returns `AlreadyExists`
error; otherwise calls `TempPath::persist` (atomic rename).

---

## Checkpoint idempotency behavior

If the target path already exists when `commit_checkpoint` is called, the
existing checkpoint is admitted and its identity fields are compared to the
current request.  If all fields agree (key, key_sha256, session_id,
transaction_identity, attempt_identity), the existing committed checkpoint is
returned directly without re-writing.  Field disagreement returns
`ExistingIdentityMismatch`.

---

## Checkpoint partial-commit behavior

If the checkpoint file was successfully published (atomic no-replace succeeded)
but the directory sync step fails, `commit_checkpoint` returns
`Err(AcquisitionError::CheckpointCommit(PartialCommit(
HttpCheckpointPartialCommitError { directory_sync_error })))`.  

`HttpCheckpointPartialCommitError` is `pub` and exposes `directory_sync_error:
std::io::Error`.  A subsequent `has_checkpoint` or `commit_checkpoint` call may
succeed or observe the idempotency path if the file survived the partial
commit.

---

## Checkpoint admission behavior

See "Exact checkpoint validation order" above.  Any validation failure results
in a typed `HttpCheckpointAdmissionError` variant; no silent ignoring of
malformed checkpoints.

---

## Historical session-state behavior

A historical session record is considered valid for checkpoint lookup if:

- `state != SessionState::Prepared` (session was started at least once), and
- `started_at.is_some()`.

Sessions with state `Running`, `Succeeded`, `Failed`, or `Abandoned` that have
a `started_at` timestamp are all accepted.  `Prepared` sessions (never started)
are rejected with `SessionNotStarted`.

---

## Cross-session lookup behavior

`has_checkpoint` enumerates all entries in `<operation_root>/sessions/`,
skipping non-directory entries and symlinks.  For each session directory, it
checks for `checkpoints/<key-sha256>.json`.  If such a file exists, it is
admitted using `admit_http_checkpoint_from_disk` with `expected_session_id =
None`, which accepts checkpoints from any session so long as project name and
runtime metadata agree and the referenced transaction is valid.

---

## Corrupt checkpoint behavior

`has_checkpoint`: if admission of a found candidate file fails, the function
returns `Err(AcquisitionError::CheckpointLookup(CorruptCandidate {
session_id, source: Box<HttpCheckpointAdmissionError> }))`.  There is no
silent skipping of corrupt candidates.

`commit_checkpoint` (idempotency path): if the existing file at the target
path fails admission, `ExistingCorrupt(HttpCheckpointAdmissionError)` is
returned.

---

## Referenced transaction admission behavior

`admit_http_checkpoint_from_disk` re-admits the referenced transaction by
scanning `trusted_raw_root` for a directory name ending with the
`transaction_id` from the checkpoint document.  It then:

- calls `admit_transaction_from_disk` on the found path;
- reads `request/metadata.json` and verifies `session_id`;
- verifies `logical_request_key`, outcome, and attempt identity against the
  checkpoint document.

Any failure returns `ReferencedTransaction(HttpTransactionAdmissionError)` or
`TransactionSessionMismatch` / `TransactionKeyMismatch` /
`TransactionNotResponse` / `AttemptMismatch` as appropriate.

---

## `latest_transaction(key: impl AsRef<str>) -> AcquisitionResult<Option<RecordedTransaction>>`

Scans `raw_data_directory()` for finalized (non-`.partial-`) transaction
directories whose admitted `logical_request_key` equals `key` and whose
outcome is `HttpRecordedOutcome::Response` (transport failures skipped).

Selection rule (deterministic; most-recent wins):

1. Primary: numeric timestamp prefix of directory name (higher = newer).
2. Secondary: `physical_attempt_index` (higher = newer).
3. Tertiary: lexicographic comparison of transaction ID string (higher = newer).

Returns `Ok(None)` if no matching transaction exists.  Returns
`Err(AcquisitionError::TransactionAdmission(...))` if a candidate directory
fails admission.

Does not require a managed context.

---

## Deterministic latest-selection rule

As above: `(dir_timestamp_nanos DESC, physical_attempt_index DESC, transaction_id DESC)`.

---

## `latest_response_header(key, header_name) -> AcquisitionResult<Option<String>>`

1. Validates `header_name` with `reqwest::header::HeaderName::from_bytes`; 
   returns `source_message` error on invalid name.
2. Calls `latest_transaction(key)`; returns `Ok(None)` if no transaction.
3. Normalizes `header_name` to lowercase.
4. Silently returns `Ok(None)` (never returns a value) for headers:
   `set-cookie`, `authorization`, `proxy-authorization`, `cookie`.
5. Searches `RecordedHttpResponse::headers()` for the first matching
   (case-insensitive) header.
6. If the value is `RecordedHeaderValue::Utf8(text)` and `text == "<redacted>"`,
   returns `Ok(None)`.
7. If the value is `RecordedHeaderValue::Utf8(text)` and not redacted, returns
   `Ok(Some(text))`.
8. If the value is `RecordedHeaderValue::Base64(_)`, returns
   `Err(AcquisitionError::source_message(...))`.
9. If no matching header is found, returns `Ok(None)`.

---

## Non-UTF-8 header behavior

If the matching response header carries a `RecordedHeaderValue::Base64` value
(non-UTF-8 bytes encoded as base64), `latest_response_header` returns
`Err(AcquisitionError::source_message("response header value is non-UTF-8 encoded bytes"))`.

---

## Managed-redacted header behavior

The headers `set-cookie`, `authorization`, `proxy-authorization`, and `cookie`
are always suppressed: `latest_response_header` returns `Ok(None)` for these
regardless of the recorded value.

Any header whose recorded `Utf8` value equals the literal string `<redacted>`
is also suppressed (returns `Ok(None)`).

---

## Run-handler checkpoint behavior

An `HttpAcquireFn` (run handler) may call `ctx.has_checkpoint(key)` to
determine whether a prior session committed a checkpoint for a given key, and
`ctx.commit_checkpoint(key)` to durably record that the logical step
corresponding to `key` has been completed.  The handler receives an `Ok` result
containing a `CommittedHttpCheckpoint` on success.

---

## Resume-handler checkpoint behavior

An `HttpResumeFn` (resume handler) may use the same `has_checkpoint` and
`commit_checkpoint` APIs.  Checkpoint semantics are identical across run and
resume contexts; there is no separate resume-specific checkpoint path.

---

## Core does not interpret checkpoint meaning

`lexicon-core` records and retrieves checkpoints as opaque identity markers.
The semantic meaning of a checkpoint (what work it represents, how it affects
source behavior) is determined entirely by the source handler.  Core does not
inspect the logical key string for semantic content.

---

## No checkpoint payload API

No arbitrary payload (JSON body, metadata map, or any structured content beyond
the identity fields in `HttpCheckpointDocumentV1`) was added.  The
`CommittedHttpCheckpoint` exposes only identity and provenance fields.

---

## Session lifecycle files not mutated

`commit_checkpoint` creates files under
`<session_directory>/checkpoints/<sha256>.json`.  It does not read or write:

- `session.json` (session record);
- `status.json` (session status);
- the session lease file;
- any progress or metadata files.

Session lifecycle files are not mutated by checkpoint operations.

---

## Capability-set result

`HttpCapabilitySet::empty()` retained unchanged.  `ClientCertificateV1` was
not advertised.  No capability identifiers were modified.

---

## Explicitly not implemented

The following behaviors were explicitly not implemented in this milestone:

- arbitrary checkpoint payloads;
- automatic workflow resumption;
- automatic source-loop reconstruction;
- processing transaction discovery;
- processing SQLite behavior;
- decoded response readers;
- client certificates;
- proxy configuration;
- background operator host;
- signal forwarding;
- background supervision;
- lexicon build;
- automatic build-before-run;
- source migration;
- cross-compilation;
- MZA changes;
- installer changes.

---

## Test source adjustments

No existing test source was modified.  The logical-request-key correction
(allowing `/` and `\`) is backward-compatible; any existing key that was
previously valid remains valid.

---

## Confirmation: no tests, checks, builds, or tooling was run

No tests, checks, builds, formatting passes, linting passes, metadata
commands, CLI execution, runtime execution, HTTP execution, workspace
validation, or bundle/install pipeline steps were executed as part of this
implementation.
