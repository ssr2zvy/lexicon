Current implementation milestone: HTTP durability and failure-provenance closure

Objective

Correct the remaining source-level defects in the Core HTTP transaction engine at commit:

Ac1aa65

Do not begin checkpoints or processing yet.

The previous corrective milestone improved the implementation substantially, but the source still does not fully satisfy its own completion claims or the HTTP guarantees in:

workspace/specs/contract.md

This closure must establish that:

every completed physical exchange
→ has one immutable finalized transaction
→ retains its exact recorded outcome
→ cannot overwrite an existing transaction
→ is represented in every retry/redirect failure
→ updates strictly validated session progress

This is a corrective source-only milestone.

Repository-grounded remaining defects

Correct every defect below.

1. Retry exhaustion still discards the final transaction

The current error remains:

HttpExecutionError::RetryExhausted

It carries no information about the durable final attempt.

This contradicts the prior milestone’s required retry-exhaustion representation.

Required correction

Define a typed exhaustion error equivalent to:

pub struct HttpRetryExhaustionError {
    final_transaction: RecordedTransaction,
    total_physical_attempts: u32,
    final_outcome: HttpRetryFinalOutcome,
}

Equivalent organization is acceptable.

The final outcome must distinguish:

ResponseStatus(u16)
TransportFailure(HttpTransportFailure)

Provide read-only accessors for:

* final transaction;
* final transaction identity;
* final transaction directory;
* total physical attempts;
* final status or stable transport-failure class.

Change:

HttpExecutionError::RetryExhausted

to carry this typed value.

Do not include request URLs, headers, body data, or arbitrary Reqwest diagnostics in Display.

2. Non-retryable transport failures discard recorded transaction provenance

After recording and finalizing a transport-failure transaction, the current code returns:

HttpExecutionError::Transport(failure)

This retains the failure class but loses the finalized transaction carrying the durable failure record.

Required correction

Introduce an error equivalent to:

pub struct RecordedHttpTransportFailure {
    transaction: RecordedTransaction,
    failure: HttpTransportFailure,
}

and use:

HttpExecutionError::RecordedTransportFailure(
    RecordedHttpTransportFailure,
)

The error must retain:

* finalized transaction identity;
* finalized transaction directory;
* recorded failure classification;
* retryability;
* physical attempt index;
* redirect index;
* retry index.

Do not construct a new unrecorded generic transport error after the physical failure has already been recorded.

HttpExecutionError::Transport(...) may remain only for a failure that occurs before a physical transaction can be durably finalized.

3. Status handling still uses fabricated zero values

The current orchestration contains calls equivalent to:

status.unwrap_or(0)

including redirect construction and retry-status evaluation.

A transport failure has no HTTP status. Zero is not a valid substitute.

Required correction

Branch on the typed recorded outcome before any status-dependent behavior.

Use logic equivalent to:

match transaction.response().outcome() {
    HttpRecordedOutcome::Response(response) => {
        // Status, redirect, and status-retry handling.
    }
    HttpRecordedOutcome::TransportFailure(failure) => {
        // Transport retry or recorded transport failure.
    }
}

Equivalent APIs are acceptable.

Requirements:

* no synthetic status code;
* no unwrap_or(0);
* redirect handling receives a real response status;
* status-based retry receives a real response status;
* transport-failure handling never enters status logic;
* require_success() on transport failure remains typed.

4. Redirect-policy failures lose the last finalized transaction

The following errors are currently unit variants:

RedirectExhausted
RedirectLoop
InvalidRedirectTarget

However, the redirect response that caused the decision has already been recorded and progress-published.

Required correction

Define a typed redirect failure containing:

* last finalized transaction;
* redirect failure kind;
* redirect count;
* total physical attempt count.

Use a stable kind equivalent to:

pub enum HttpRedirectFailureKind {
    MaximumExceeded,
    LoopDetected,
    MissingLocation,
    InvalidLocationEncoding,
    InvalidTarget,
    UnsupportedScheme,
}

Equivalent organization is acceptable.

Do not include the effective URL or Location value in Display.

Preserve the already finalized redirect transaction.

5. Timestamp conversion still silently saturates

The current helper converts SystemTime to u64 nanoseconds and uses:

unwrap_or(u64::MAX)

This silently fabricates the maximum timestamp on overflow.

Required correction

Make timestamp acquisition fallible.

Define a typed error for:

* system time before Unix epoch;
* nanosecond value outside the persisted representation.

Use one of these approaches:

1. Store nanoseconds as u128 using a deliberate serialized representation; or
2. Retain u64 and reject an out-of-range timestamp.

Do not silently saturate, truncate, or substitute zero.

Apply this to:

* transaction directory timestamp;
* request creation time;
* response completion time;
* transport-failure time;
* incomplete-response time;
* acquisition-progress update time.

Propagate the typed clock failure through recorder or progress errors.

6. Response-body byte counting still uses unchecked addition

The streaming loop currently performs behavior equivalent to:

total += n as u64;

Required correction

Use:

checked_add

Return a typed response-body-length overflow error.

Preserve the recognizable partial transaction and bytes already written.

Do not publish complete response metadata or finalize the transaction after overflow.

7. Incomplete-response marker behavior is declared but not completed

The error hierarchy includes:

IncompleteResponseMarkerFailed

but the response streaming path returns directly from stream_body(...) and does not visibly persist the required incomplete-response marker.

Required correction

When response streaming fails after the physical response begins:

1. retain the staging directory;
2. retain every body byte already persisted;
3. flush and sync those bytes when possible;
4. persist response metadata with:

ResponseOutcomeDocument::IncompleteResponse

5. include a stable sanitized failure class;
6. include the number of body bytes successfully recorded;
7. include the partial body SHA-256 when available;
8. sync the incomplete metadata;
9. leave the directory under its recognizable .partial-* name;
10. return the original typed stream failure.

Do not rename the partial directory to a finalized transaction.

If incomplete metadata persistence also fails, return a combined typed error preserving:

* original streaming failure;
* marker persistence failure.

Do not discard either cause.

8. Final publication still uses a check-then-rename sequence

The current recorder:

1. checks whether the final directory exists;
2. later calls ordinary fs::rename(...).

The existence check is not an exclusive publication guarantee. On platforms where rename may replace an existing target, this can violate finalized-transaction immutability.

Required correction

Implement a private no-replace directory publication primitive.

Required semantics:

rename staging directory to final directory
only if final path does not already exist

It must be atomic at the directory-entry boundary.

Use platform-specific primitives where required:

* Linux: a no-replace rename operation such as renameat2(..., RENAME_NOREPLACE);
* macOS: the supported no-replace rename primitive;
* Windows: a move operation that fails when the destination exists.

Equivalent safe platform implementations are acceptable.

For unsupported platforms, return a typed unsupported-publication error rather than falling back to an overwrite-capable rename.

Requirements:

* no pre-existing transaction is replaced;
* no pre-existing transaction is merged;
* final collision is typed;
* collision leaves staging recognizable;
* collision leaves the existing final transaction untouched;
* parent-directory sync still follows successful publication.

The preliminary existence check may remain as an optimization, but it must not provide the correctness guarantee.

9. Staging allocation does not retry identity collisions

Exclusive staging creation correctly rejects an existing path, but a UUID collision currently becomes an ordinary staging failure.

Required correction

Use a small bounded transaction-identity allocation loop.

For each allocation attempt:

1. generate a new collision-resistant identity;
2. derive staging and final names;
3. reject an existing final path;
4. exclusively create the staging directory;
5. retry only when the exact staging or final name already exists.

Do not retry:

* permission failures;
* invalid roots;
* symlink rejection;
* arbitrary I/O failures.

Use a small fixed maximum and return a typed identity-allocation-exhausted error if every candidate collides.

Do not use an unbounded loop.

10. Managed-path validation remains split and incomplete

The recorder walks components of the raw-data root, but context validation still checks only endpoint paths using:

path.exists() && path.is_symlink()

Progress persistence also needs the same complete managed-path protection.

Required correction

Create one shared Core-managed path-validation implementation.

It must validate every existing component using:

symlink_metadata

Apply it to:

* protocol root;
* operation root;
* session directory;
* acquisition progress path;
* raw-data root;
* partial transaction path;
* final transaction path;
* request directory;
* response directory;
* every managed metadata and body path.

Validate expected file type:

* roots and managed directories must be directories;
* progress and metadata destinations must be regular files when present;
* transaction body destinations must be regular files when present;
* symlinks are always rejected;
* other filesystem object types are rejected.

Do not maintain separate weaker endpoint-only validation in context.rs.

Do not follow an untrusted symlink and then treat the canonicalized destination as valid.

11. Progress validation does not establish exact revision semantics

AcquisitionProgressDocument::validate_existing(...) accepts a minimum revision:

doc.revision < expected_revision_min

The caller cannot establish exact optimistic revision ownership through a minimum check.

Required correction

Define exact progress revision behavior:

* a missing document begins at revision 0;
* the first published transaction produces revision 1;
* every later successful progress publication increments exactly once;
* the loaded revision is the exact expected prior revision;
* no minimum-range acceptance;
* no revision rollback;
* no skipped revision;
* no silent saturation.

Because one active managed child is the established writer, the update may use the loaded exact revision as its expected value after confirming session and supervisor ownership.

Keep the revision inside the complete atomically replaced document.

12. Progress counter invariants are incomplete

Current validation checks only limited relationships.

Required correction

Validate at least:

transport_failure_count <= completed_transaction_count
redirect_count <= completed_transaction_count
retry_count <= completed_transaction_count
revision == completed_transaction_count

Also require:

* completed count zero implies no last transaction;
* completed count nonzero implies a last transaction;
* revision zero implies no last transaction;
* session identity is valid;
* last transaction identity is structurally valid;
* logical request key respects its byte limit;
* timestamp is valid and nonzero under the chosen representation;
* all counters can be incremented without overflow.

If the chosen retry/redirect counting semantics make revision == completed_transaction_count invalid, document and encode the exact alternative invariant. Do not leave the relationship unspecified.

13. Progress document fields remain directly mutable inside the module

The document is described as opaque, but its fields are publicly mutable within its visibility boundary and context code manually updates them.

Required correction

Move progress mutation into methods owned by the progress module.

Provide an operation equivalent to:

pub(crate) fn advance(
    self,
    finalized: &FinalizedRecordedAttempt,
    logical_key: Option<&str>,
    now: Timestamp,
) -> Result<Self, AcquisitionProgressError>;

The method must:

* validate the existing document first;
* use checked arithmetic;
* update all relevant counters consistently;
* increment revision exactly once;
* update final transaction identity;
* apply logical-key bounds;
* return a completely valid next document.

Do not let context.rs mutate counters field by field.

14. Progress replacement must report directory-sync failure

The completion report describes session-directory sync as best effort.

For transaction progress, returning success before the replacement directory entry is durably synchronized weakens the stated successful-execution guarantee.

Required correction

After atomically replacing:

acquisition_progress.json

synchronize the session directory.

If directory sync fails after replacement:

* return a typed progress partial-commit error;
* preserve the updated progress file;
* preserve the finalized transaction;
* report that replacement occurred but directory durability could not be confirmed.

Do not silently discard the directory-sync error.

15. Finalized transactions are unnecessarily cloned before progress publication

The context currently clones the finalized transaction before updating progress.

That weakens the intended ownership progression:

finalized
→ progress-published
→ source-visible

Required correction

Use ownership rather than cloning for the state transition.

Prefer:

FinalizedRecordedAttempt
→ publish_progress(self, ...)
→ ProgressPublishedRecordedAttempt
→ RecordedTransaction

Equivalent consuming APIs are acceptable.

The source-visible transaction should emerge from the successfully progress-published owner.

Do not require RecordedTransaction: Clone merely to work around ownership.

If Clone is not part of a deliberate source-facing contract, remove it.

16. Attempt indices are not exposed through the recorded transaction API

Retry- and redirect-failure types need typed access to the attempt indices, but those values currently live primarily in serialized request metadata.

Required correction

Include an opaque attempt representation in the finalized transaction:

pub struct HttpAttemptIdentity {
    physical_attempt_index: u32,
    redirect_index: u32,
    retry_index: u32,
}

Provide read-only accessors.

The in-memory values and persisted request metadata must be created from the same typed attempt identity.

Do not reparse metadata merely to construct execution errors.

17. Parent transaction linkage is only a string

Use typed transaction identity for in-memory orchestration.

Required correction

Represent parent linkage internally as:

Option<HttpTransactionIdentity>

Convert to the stable identifier only during metadata serialization.

Do not pass free-form transaction-ID strings between recorder and orchestration.

Validate deserialized parent transaction identifiers in transaction metadata admission.

18. Persisted transaction documents lack a complete decoding/admission API

The source writes strict documents, but processing and later recovery will need Core-owned admission of finalized raw transactions.

Do not implement processing discovery yet, but complete the Core transaction document boundary now.

Required correction

Provide private or public Core-owned decoding APIs for:

* request metadata;
* response metadata;
* finalized transaction reconstruction.

Require:

* supported schema version;
* valid transaction identity;
* matching transaction identities between request and response;
* valid session identity;
* valid attempt indices;
* valid parent transaction identity;
* consistent request body presence, length, and hash fields;
* valid response outcome invariants;
* response status only for response outcomes;
* failure class only for transport failures;
* incomplete response never admitted as finalized;
* expected managed directory layout;
* actual body lengths match metadata;
* actual SHA-256 values match metadata.

Define typed decoding/admission errors.

Do not scan the raw-data root or implement processing selection in this milestone.

This boundary exists so later processing does not deserialize internal structs directly and trust unverified files.

19. Stored transport-failure classes are free-form strings

Response metadata currently persists:

failure_class: String

Required correction

Define a stable serialized enum corresponding to supported transport-failure classes.

Use strict identifiers such as:

configuration
request_build
connect
timeout
body_write
exchange_io
tls

Unknown identifiers must fail strict transaction admission.

Do not deserialize an arbitrary string and treat it as a known failure class.

Ensure the persisted retryable flag agrees with the known failure class. Reject disagreement.

20. HTTP-version metadata uses debug formatting

The transport currently derives HTTP version using behavior equivalent to:

format!("{:?}", response.version())

Debug output is not a stable serialized protocol identifier.

Required correction

Define a stable HTTP-version enum or identifier boundary covering the versions exposed by the selected client, for example:

http_0_9
http_1_0
http_1_1
http_2
http_3
unknown

Prefer rejecting unsupported/unknown versions during strict admission rather than persisting unstable debug text.

Do not use Rust Debug output as a persistence format.

21. Recorded-header admission must validate encoded values

Stored non-UTF-8 header values use Base64, but strict admission must validate them.

Required correction

When reading persisted header metadata:

* validate header names;
* decode Base64 values strictly;
* reject malformed Base64;
* preserve repeated header ordering;
* reject managed-sensitive headers persisted without the redacted marker;
* reject Set-Cookie persisted without redaction;
* reject malformed redaction representations.

Do not use lossy UTF-8 conversion.

22. Logical request keys need one canonical validator

Logical request keys are used by:

* request construction;
* transaction metadata;
* progress metadata;
* later checkpoints and lookup.

Required correction

Define one opaque type:

pub struct HttpLogicalRequestKey {
    // private
}

Provide a checked constructor and stable accessor.

Require:

* nonempty;
* bounded UTF-8 byte length;
* no control characters;
* no path interpretation;
* no use as a raw filesystem component.

Use this type in:

* HttpRequest;
* finalized request;
* transaction attempt context;
* request metadata conversion;
* acquisition progress;
* retry and redirect state.

Do not pass logical keys as free-form String internally.

This prepares the boundary for the next checkpoint milestone without implementing checkpoints now.

23. Transaction identity fields must use one canonical validator

Create one strict identity parser for:

HttpTransactionIdentity

Use it for:

* generated identities;
* parent linkage;
* transaction metadata decoding;
* progress last-transaction identity;
* finalized transaction admission.

Do not separately validate IDs as arbitrary nonempty strings.

The stable identifier must remain path-component safe.

24. Source-facing response API must reflect outcome types

Ensure the public API cannot accidentally treat a transport failure as an HTTP response.

Prefer a model equivalent to:

pub enum RecordedHttpOutcome {
    Response(RecordedHttpResponse),
    TransportFailure(RecordedTransportFailure),
}

Then provide:

RecordedTransaction::outcome()
RecordedTransaction::response() -> Option<&RecordedHttpResponse>
RecordedTransaction::transport_failure() -> Option<&RecordedTransportFailure>

Equivalent APIs are acceptable.

If compatibility requires retaining transaction.response(), it must return a typed result rather than an object containing optional status fields for incompatible outcomes.

Do not fabricate response fields for transport failures.

Corrected ownership sequence

The authoritative sequence after this milestone must be:

Validated finalized request
→ exclusively allocated staged attempt
→ durably recorded physical outcome
→ atomic no-replace transaction publication
→ raw-parent directory sync
→ FinalizedRecordedAttempt
→ running-session and supervisor revalidation
→ exact acquisition-progress replacement
→ session-directory sync
→ ProgressPublishedRecordedAttempt
→ RecordedTransaction or typed recorded failure

The finalized attempt must remain attached to every failure after transaction publication.

Error hierarchy

Use typed errors equivalent to:

HttpClockError
HttpBodyStreamingError
HttpTransactionIdentityAllocationError
HttpTransactionPublicationError
HttpTransactionAdmissionError
HttpRetryExhaustionError
HttpRedirectFailure
RecordedHttpTransportFailure
AcquisitionProgressValidationError
AcquisitionProgressPersistenceError
HttpExecutionError
AcquisitionError

Equivalent nesting is acceptable.

Implement:

std::fmt::Display
std::error::Error

Use source().

Do not introduce new plain-String errors inside the Core-owned HTTP path.

Sensitive diagnostics

No diagnostic may reveal:

* effective request URL;
* redirect Location;
* sensitive query values;
* request or response header values;
* request or response bodies;
* source arguments;
* runtime-context JSON;
* invocation-envelope JSON;
* environment-variable names or values;
* arbitrary source errors;
* raw Reqwest diagnostics containing request information.

Safe diagnostics may include:

* stable transaction identity;
* attempt indices;
* stable failure class;
* HTTP status;
* retry count;
* redirect count;
* managed path category.

Prefer typed path accessors over printing full filesystem paths.

Preserve existing behavior

Do not change:

* source handler signatures;
* acquisition/resume registration;
* invocation-envelope JSON;
* argv transport;
* source arguments;
* HTTP admission;
* processing admission;
* session lifecycle states;
* supervisor lease ownership;
* foreground process launch;
* foreground terminal reconciliation;
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

1. Retry exhaustion retains the final transaction.
2. Non-retryable transport failure retains its transaction.
3. Redirect failure retains the last redirect transaction.
4. No synthetic status code is used.
5. Transport failures never enter status logic.
6. Timestamp overflow is typed.
7. Response-body length overflow is typed.
8. Partial response bytes remain recognizable.
9. Incomplete-response metadata is actually persisted.
10. Streaming and marker failures are both preserved.
11. Final transaction publication is atomic and no-replace.
12. Existing finalized transactions cannot be overwritten.
13. Staging identity collisions are retried only through a bounded collision loop.
14. Every relevant managed path component rejects symlinks.
15. Progress revision increments exactly once.
16. Progress counters have complete invariants.
17. Progress mutation belongs to its owning type.
18. Progress directory-sync failure is not discarded.
19. Finalized transaction ownership is consumed into progress publication.
20. Attempt indices use one typed representation.
21. Parent transaction linkage is typed.
22. Transaction metadata has strict decoding and admission.
23. Body lengths and hashes are verified during admission.
24. Transport-failure identifiers are strict enums.
25. Retryable metadata agrees with failure class.
26. HTTP version uses a stable identifier.
27. Stored header values are strictly admitted.
28. Logical request keys use one opaque validated type.
29. Transaction identities use one parser and validator.
30. Source-facing transaction outcome cannot fabricate a response.

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

Explicit exclusions

Do not implement:

* checkpoints;
* checkpoint persistence;
* checkpoint recovery;
* latest-transaction lookup;
* processing transaction discovery;
* SQLite processing;
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
* retry-exhaustion representation;
* recorded non-retryable transport-failure representation;
* redirect-failure representation;
* removal of synthetic status handling;
* timestamp overflow behavior;
* response-body counter overflow behavior;
* incomplete-response persistence behavior;
* combined stream/marker failure behavior;
* atomic no-replace publication implementation by platform;
* bounded staging collision behavior;
* shared managed-path validation behavior;
* exact progress revision behavior;
* complete progress invariants;
* progress mutation API;
* progress directory-sync failure behavior;
* finalized-to-progress-published ownership sequence;
* typed attempt identity;
* typed parent transaction linkage;
* transaction decoding and admission API;
* finalized body length/hash verification;
* stable transport-failure serialization;
* stable HTTP-version serialization;
* recorded-header admission behavior;
* logical request-key representation;
* transaction-identity parsing behavior;
* final source-facing recorded-outcome API;
* final HttpAcquisitionContext::execute(...) success and failure guarantees;
* capability-set result;
* confirmation that checkpoints, processing, background supervision, and build behavior were not added;
* existing test source adjusted only for API alignment, if applicable;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, runtime execution, HTTP execution, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin checkpoints until this closure is complete.