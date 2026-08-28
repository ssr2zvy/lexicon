Current implementation milestone: HTTP transaction-engine correctness closure

Objective

Correct and complete the Core HTTP transaction engine introduced at commit:

c852581485c27638b349e33d2b090b4753efa927

The broad HTTP architecture is now present, but several correctness defects prevent the implementation from satisfying the durability, type-safety, retry, session-progress, and recorded-path guarantees in workspace/specs/contract.md.

This is a corrective milestone.

Do not begin:

* checkpoints;
* processing raw-transaction discovery;
* SQLite processing;
* background supervision;
* lexicon build;
* additional CLI behavior.

The corrected engine must preserve this boundary:

HttpAcquisitionContext::execute(request)
→ finalize immutable request
→ record every physical exchange
→ publish durable transaction
→ update durable acquisition progress
→ return final RecordedTransaction

Contract authority

Follow:

workspace/specs/contract.md

especially:

* section 10, HTTP execution and raw-data contract;
* section 11, secret handling;
* section 12, session and supervisor ownership.

Do not weaken raw fidelity, redaction, durable recording, or session ownership to preserve the current implementation.

Repository-grounded defects

Correct every defect below.

1. Recorded request body uses the obsolete staging path

record_transaction_attempt(...) currently constructs the source-facing request body path using:

.partial-<timestamp>-<transaction-id>/request/body

The staging directory is then renamed to:

<timestamp>-<transaction-id>/

The returned RecordedHttpRequest therefore retains a path inside a directory that no longer exists.

Required correction

Every path exposed by a successfully returned RecordedTransaction must point into the finalized transaction directory.

Use the final path when constructing both:

RecordedHttpRequest
RecordedHttpResponse

For example, the request body path must resolve to:

<final-transaction-directory>/request/body

The response body path must resolve to:

<final-transaction-directory>/response/body

Do not expose staging paths through any public recorded type.

Do not create a RecordedTransaction representing a completed transaction until atomic finalization succeeds.

2. Finalized recorded values are constructed too early

The current recorder constructs RecordedTransaction before the staging directory is atomically renamed.

That allows the in-memory type to represent a finalized path before publication has succeeded.

Required correction

Separate staged recording from finalized transaction construction.

Use an internal type-state or equivalent sequence:

StagedRecordedAttempt
→ sync completed staging contents
→ atomically rename staging directory
→ sync raw-data parent directory
→ construct RecordedTransaction

An equivalent private design is acceptable.

The public RecordedTransaction must prove:

* the transaction directory was finalized;
* the final directory exists at its represented path;
* required request and response metadata exist;
* required body files exist according to the recorded outcome;
* finalization did not return an error;
* the raw-data parent directory was synchronized after rename.

Do not provide a public unchecked constructor.

3. Raw-root sync is missing after final rename

The recorder currently synchronizes the staging directory and renames it, but it does not synchronize the raw-data parent directory after the rename.

A successful rename alone does not establish the intended durable-directory-entry guarantee.

Required correction

After:

fs::rename(staging_directory, final_directory)

open and synchronize the raw-data parent directory.

Required order:

1. persist and sync managed files;
2. sync the completed staging directory;
3. atomically rename staging to final;
4. sync the raw-data parent directory;
5. construct the finalized recorded value.

If the parent-directory sync fails after rename, return a typed post-rename durability failure that preserves:

* transaction identity;
* final transaction path;
* knowledge that the directory rename already occurred.

Do not delete the renamed transaction.

This is a partial commit and must be represented as such.

4. Transport failure classification is incorrect

The recorder currently marks every transport failure as:

retryable = true

This incorrectly treats the following as retryable physical transport failures:

* client configuration failure;
* request conversion/build failure;
* actual exchange I/O failure.

The context later discards the original failure classification and may convert it into generic Io.

Required correction

Define a stable typed transport-failure class that distinguishes at least:

Configuration
RequestBuild
Connect
Timeout
BodyWrite
ExchangeIo
Tls

Equivalent bounded organization is acceptable where the HTTP library cannot reliably expose every distinction.

At minimum:

* configuration failure is not retryable;
* request-build failure is not retryable;
* invalid method/header conversion is not retryable;
* only explicitly classified transient exchange failures may be retryable;
* unknown failures default to non-retryable.

The recorded transport-failure metadata must persist the stable sanitized failure class and the retryability decision.

The retry decision must use the same typed classification stored in the transaction metadata.

Do not replace a specific recorded failure with a newly constructed generic HttpTransportFailure::Io.

5. Transport configuration errors are discarded

HttpAcquisitionContext::from_session_data_paths(...) currently initializes the transport with:

ReqwestHttpTransport::new()
    .ok()

This discards the typed configuration error.

A later call to execute(...) reports only that no transport exists.

Required correction

Do not discard transport construction failure.

Prefer one of these designs:

1. Make managed context construction return a typed result; or
2. Store the typed initialization result inside the context and return the original typed cause from execute(...).

Choose the smallest change compatible with the established runner lifecycle.

If context construction becomes fallible, update the HTTP runner’s typed initialization error path without changing the handler ABI.

Do not stringify the configuration error.

Do not silently substitute another transport.

6. Reqwest feature configuration must be internally consistent

lexicon-core/Cargo.toml currently enables only:

features = ["blocking", "rustls-tls"]

while the client builder explicitly calls feature-dependent compression configuration methods.

The implementation must not depend on methods excluded by the selected feature set.

Required correction

Keep:

default-features = false

Enable only the features required for:

* blocking HTTP;
* the selected TLS implementation.

Do not enable compression features merely so calls such as .gzip(false) compile.

When compression features are absent, transparent decompression is already unavailable. Remove feature-dependent builder calls that cannot exist under the selected feature set.

If the selected Reqwest version provides unconditional no_* methods that remain available without enabling decoding features, those may be used.

The final source must have one coherent dependency/configuration strategy demonstrating that:

* gzip decoding is not compiled or enabled;
* Brotli decoding is not compiled or enabled;
* deflate decoding is not compiled or enabled;
* Zstandard decoding is not compiled or enabled;
* redirects remain disabled;
* no internal retry layer is introduced.

Do not run Cargo to validate this milestone.

7. Progress partial commits are incompletely represented

A finalized transaction followed by progress write failure currently returns PartialCommit.

However, after transaction finalization, the following progress failures are returned as ordinary errors:

* progress file load failure;
* progress decoding failure;
* progress schema failure;
* progress session-identity mismatch;
* progress invariant failure.

At that point the transaction is already durable, so every progress failure is a transaction/progress partial commit.

Required correction

Split progress work into:

load and validate progress
→ calculate next document
→ atomically persist

After transaction finalization, wrap every failure from that sequence in one typed partial-commit error containing:

* finalized transaction identity;
* finalized transaction path;
* typed progress failure as the nested source.

Define a nested progress error equivalent to:

pub enum AcquisitionProgressError {
    Load(...),
    Decode(...),
    UnknownSchemaVersion { ... },
    UnknownField { ... },
    InvalidInvariant { ... },
    SessionMismatch { ... },
    SessionLoad(...),
    SessionNotRunning,
    OperationMismatch,
    RuntimeMismatch,
    Persistence(...),
}

Equivalent organization is acceptable.

Do not collapse these cases into Load, Decode, or Persist unit variants.

8. Progress updates do not revalidate session state

The session is validated before the first exchange, but persist_progress(...) does not confirm that the durable session remains the same running acquisition session when progress is updated.

A request may take arbitrarily long, and progress publication is a separate durable operation.

Required correction

Before each progress update, validate:

* detailed session record exists;
* session identity matches the context;
* state remains Running;
* operation remains acquisition;
* runtime protocol remains HTTP;
* runtime operation remains acquisition;
* external supervisor lease remains owned.

Do not acquire the supervisor lease in the child.

If the session is no longer a matching running acquisition session:

* preserve the finalized transaction;
* do not overwrite progress;
* return a typed transaction/progress partial commit.

9. Progress document decoding is not sufficiently strict

The progress file is currently deserialized directly without a complete schema/invariant admission boundary.

Required correction

Make the progress document opaque outside its owning module.

Use:

#[serde(deny_unknown_fields)]

Require:

* exact supported schema version;
* nonempty valid session identity;
* revision consistent with whether a prior document exists;
* monotonic counters;
* last_transaction_id consistency with completed count;
* bounded logical request key;
* a valid update timestamp representation;
* no counter overflow.

Do not accept unknown schema versions as ordinary version-one documents.

Do not use unchecked arithmetic such as:

counter += 1

Use checked increments and return typed overflow errors.

10. Progress read-modify-write needs an ownership rule

The progress update currently performs an unlocked read-modify-write operation.

The current handler is synchronous, but the persistence API must make its single-writer assumption explicit and enforce the established supervisor/session ownership boundary.

Required correction

Perform progress updates only after confirming the active external supervisor lease.

Document that the linked Core child is the single acquisition-progress writer for the active running session.

Use a unique temporary file and atomic replacement.

Do not add a second independent progress lock unless the established supervisor lease is insufficient for the actual ownership design.

Do not introduce a global lock.

11. Progress temp-file cleanup and replacement behavior

The progress writer creates a named temporary path manually and may leave it behind after an error.

Its replacement behavior is also platform-sensitive when the destination already exists.

Required correction

Use the repository’s established unique temporary-file persistence pattern, preferably tempfile::NamedTempFile in the destination directory.

Required behavior:

1. serialize complete document;
2. create unique temporary file in the session directory;
3. write bytes;
4. flush;
5. sync_all;
6. atomically replace the destination using the repository’s supported replacement behavior;
7. best-effort sync the session directory;
8. automatically clean the temporary file on pre-publication failure.

Do not use a fixed or manually guessed temporary filename.

Preserve typed failures for:

* serialization;
* temporary-file creation;
* write;
* file sync;
* replacement;
* directory sync.

12. Managed-path symlink validation is incomplete

The current validation checks only the final Path value.

It does not reject a symlink in an existing ancestor component. create_dir_all(...) may therefore traverse a symlink.

Required correction

Add a shared Core-managed path validator that walks every existing component from the trusted validated root to the target.

For every existing component, use:

symlink_metadata

Reject:

* a symlink at the root;
* a symlink in any existing ancestor;
* a symlink at the target;
* a regular file where a directory is required;
* a directory where a regular managed file is required;
* traversal outside the validated root.

Apply this to:

* raw-data root;
* partial transaction directory;
* final transaction directory;
* request directory;
* response directory;
* request metadata file;
* request body file;
* response metadata file;
* response body file;
* session directory;
* acquisition progress file.

Do not canonicalize through an attacker-controlled symlink and then accept the result.

Do not expose arbitrary output paths to source code.

13. Request persistence order differs from the required sequence

The current recorder writes the request body before request metadata.

The established execution contract requires redacted request metadata to be durably persisted before the physical exchange and defines the sequence as metadata followed by exact request-body persistence.

Required correction

Use this order:

1. calculate exact request-body length and SHA-256;
2. construct redacted request metadata;
3. persist and sync request metadata;
4. persist and sync exact request-body bytes when present;
5. sync the request directory;
6. begin transport.

Both files must be durable before transport begins.

The body hash must be computed from the same immutable bytes passed to the transport.

Do not serialize or copy the request body into a different representation for transport.

14. Transport-failure response body creation errors are discarded

The current transport-failure path contains behavior equivalent to:

let _ = persist_body(&response_body_path, &[]);

This discards a correctness-relevant persistence error.

Required correction

Do not discard failure-record persistence errors.

Choose and document one exact transport-failure layout:

response/metadata.json
response/body

If response/body is required for every finalized transaction, failure to create and sync its empty representation must prevent finalization and return a typed recorder error.

If the schema explicitly permits no response body for a transport failure, do not attempt the ignored write, and make the absence explicit in metadata and invariant validation.

Preserve the stable transaction shape required by the contract. Prefer retaining an empty response/body file for finalized transport-failure transactions.

No correctness-relevant Result may be discarded.

15. Recorder error variants discard underlying causes

Several recorder operations currently map detailed I/O failures into unit-like errors.

Required correction

Make recorder failures retain their underlying typed causes.

At minimum preserve:

* affected operation;
* safe managed path category, without exposing arbitrary user-controlled path text;
* std::io::Error as the nested source;
* JSON serialization errors;
* body-stream read errors;
* hashing/write errors;
* rename errors;
* post-rename directory-sync errors.

Implement:

std::fmt::Display
std::error::Error

Use source().

Do not include request URLs, header values, body contents, source arguments, or environment values in diagnostics.

16. Retry exhaustion loses the final recorded attempt

Every retry attempt is recorded, but RetryExhausted currently carries no information about the final durable attempt.

Required correction

Define a typed retry-exhaustion error containing at least:

* final transaction identity;
* final transaction path;
* total physical attempt count;
* final stable response status or transport-failure class;
* optional logical request key only if it is explicitly constrained as safe metadata.

Do not include:

* request URL;
* header values;
* body data;
* arbitrary transport-library error text.

All completed attempts remain durable.

Do not delete or collapse retry history.

17. Transport failure returned to callers loses its recorded identity

When retries are disabled or exhausted after a transport failure, the current implementation constructs a generic transport error rather than preserving the recorded failure transaction.

Required correction

Return a typed execution error equivalent to:

HttpExecutionError::RecordedTransportFailure {
    transaction: RecordedFailedTransaction,
    failure: RecordedTransportFailure,
}

Equivalent organization is acceptable.

The error must provide programmatic access to:

* transaction identity;
* finalized transaction directory;
* stable failure class;
* retryability;
* attempt indices.

Do not expose a live response.

Do not create a new generic failure after the recorded failure already exists.

18. Recorded response status must not fabricate a value

A transport-failure transaction has no HTTP response status.

Ensure the source-facing API represents this honestly.

Required correction

Use an outcome-based API equivalent to:

pub enum HttpRecordedOutcome {
    Response(RecordedHttpResponse),
    TransportFailure(RecordedTransportFailure),
}

or retain the existing outcome enum with equivalent safe accessors.

Requirements:

* transport failure has no fabricated status code;
* redirect and retry decisions inspect status only for response outcomes;
* require_success() on a transport-failure outcome returns a typed transport-failure error;
* code must not use 0 as a synthetic HTTP status;
* response-only accessors must return Option, Result, or live only on a response-specific type.

Update HttpAcquisitionContext::execute(...) so it branches on the recorded outcome before reading status.

19. Redirect handling reads persisted redacted headers as control data

Redirect orchestration currently derives Location from the source-facing recorded response header collection.

Recorded metadata is a redacted persistence representation and should not be the authoritative control channel for protocol execution.

Required correction

The one-exchange recorder result must internally retain the sanitized control information required by the orchestrator separately from the public persisted representation.

For redirects, retain only the effective Location header value needed for redirect control.

Requirements:

* Location must come from the actual transport response;
* the response is fully recorded before redirect following;
* source-facing recorded headers remain derived from persisted-safe metadata;
* redirect control must not depend on rereading or decoding the persisted redacted representation;
* non-UTF-8 or invalid Location is a typed invalid-redirect failure;
* no sensitive response header is retained unnecessarily for orchestration.

Do not expose the internal transport response to the source.

20. Redirect-loop detection omits the initial effective URL

The redirect loop set starts empty and records only redirect targets.

This may allow an avoidable extra exchange before detecting a cycle returning to the original URL.

Required correction

Insert the initial finalized effective URL into the redirect-loop set before the first exchange.

Before following each redirect:

1. resolve the next effective URL;
2. normalize it using the same deterministic URL representation used for execution;
3. reject it if already present;
4. insert it before performing the next exchange.

Do not include URL values in error Display.

21. Sensitive-query classification does not safely cover original URL fields

Sensitivity tracking is currently derived from query parameters appended through the builder.

A sensitive value already embedded in the original URL cannot be marked without appending another parameter.

Required correction

Add an explicit source-facing method equivalent to:

request.sensitive_query_name("token")?

This marks every existing or appended query field with that decoded name as sensitive for persisted metadata.

Preserve:

* duplicate parameters;
* parameter ordering;
* the exact effective URL used by transport.

Sensitivity matching may be ASCII case-insensitive if documented consistently.

Do not require a source to append a duplicate secret parameter merely to mark an existing value sensitive.

On redirects, retain the sensitive-name classification for matching query names in the redirect target.

22. Sensitive environment-variable names are exposed

HttpRequestError::EnvironmentVariableMissing(String) retains and displays the requested environment-variable name.

The HTTP contract does not require that name to be exposed, and source-controlled names may themselves reveal sensitive configuration details.

Required correction

Use a sanitized typed variant such as:

EnvironmentVariableUnavailable
EnvironmentVariableNotUtf8

Do not include the variable name or value in Display.

If retaining the name internally is necessary for source-side branching, keep it private and provide no Debug or Display route that exposes it. Prefer not retaining it.

Support native environment values deliberately:

* either require valid UTF-8 and return EnvironmentVariableNotUtf8;
* or accept OsString only where the HTTP header-value conversion can be exact.

Do not use lossy conversion.

23. Untyped arbitrary execution-message escape hatch

The new typed acquisition hierarchy still exposes:

AcquisitionError::execution_message(...)
HttpExecutionError::Message(String)

This permits arbitrary text to bypass the typed HTTP engine and can expose secrets through diagnostics.

Required correction

Remove:

HttpExecutionError::Message(String)
AcquisitionError::execution_message(...)

unless an existing external supported API genuinely requires it.

If compatibility requires retaining a source-authored message route, keep it under:

AcquisitionError::Source

The Core-owned execution hierarchy must remain fully typed.

Do not convert internal HTTP failures into arbitrary strings.

24. Source-authored error diagnostics need an explicit boundary

AcquisitionError::Source { message } remains necessary for compatibility, but arbitrary source text is not Core-sanitized.

Required correction

Preserve the existing safe durable-session behavior:

* do not persist the source message in session.json;
* persist only the established Core-authored SourceReturnedError failure;
* do not include the source message in HTTP transaction metadata.

At the runner diagnostic boundary, distinguish:

* Core-owned sanitized errors, which may use their typed Display;
* arbitrary source-authored errors, which must be rendered as a generic message such as source handler returned an error.

Do not print arbitrary source error text from the managed runner.

The legacy compatibility API may preserve its existing direct-return semantics, but managed runners must use the sanitized boundary.

25. Timestamp representation is mislabeled

The implementation emits values equivalent to:

<unix-seconds>.<nanoseconds>Z

This is not an RFC 3339 timestamp even though helper naming implies it is.

Required correction

Use one exact representation:

1. a valid RFC 3339 UTC timestamp; or
2. a typed integer nanoseconds-since-Unix-epoch field.

Prefer reusing the established SessionTimestamp representation where appropriate.

Do not label a Unix epoch decimal as RFC 3339.

Use the same documented representation consistently in:

* request metadata;
* response metadata;
* transport-failure metadata;
* acquisition progress;
* transaction directory naming where applicable.

26. Counter overflow is silently possible

The execution and progress paths use saturating or unchecked increments.

Examples include:

saturating_add(1)
counter += 1
revision += 1

Silent saturation violates exact attempt and revision accounting.

Required correction

Use checked arithmetic for:

* physical attempt index;
* redirect index;
* retry index;
* completed transaction count;
* transport failure count;
* redirect count;
* retry count;
* progress revision.

Return typed overflow errors.

Do not silently saturate or wrap.

27. Finalization and progress types must distinguish three outcomes

The implementation needs an explicit type boundary between:

1. incomplete partial recording;
2. finalized transaction with failed progress publication;
3. finalized transaction with successful progress publication.

Required correction

Use private type-state or equivalent typed results:

PartialRecordedAttempt
FinalizedRecordedAttempt
ProgressPublishedRecordedAttempt

Equivalent naming is acceptable.

Only the third state may become the successful result of:

HttpAcquisitionContext::execute(...)

The second state must remain recoverable through a typed partial-commit error carrying the finalized transaction.

The first state must never be exposed as a RecordedTransaction.

28. Final path collision and staging allocation must be race-safe

The recorder checks:

final_directory.exists()

before creating and later renaming.

A check followed by creation is not an exclusive allocation guarantee.

Required correction

Allocate staging directories exclusively.

Requirements:

* transaction identity remains collision-resistant;
* staging creation fails if the exact staging path already exists;
* no existing staging directory is reused;
* final publication never overwrites an existing transaction;
* final-name collision is typed;
* collision handling does not delete the pre-existing entry;
* no exists() check is treated as the exclusive ownership mechanism.

Use filesystem operations with create-new/exclusive semantics where possible.

29. Atomic replacement behavior must not overwrite transactions

Metadata and progress files may use atomic replacement where versioned state is expected.

Final transaction directories are immutable.

Required correction

Distinguish:

* metadata construction inside a uniquely owned staging directory;
* acquisition-progress replacement at a stable path;
* final transaction-directory publication.

Final transaction publication must fail if the final path already exists.

It must never replace or merge with an existing transaction.

Finalized raw transactions are immutable after publication.

30. Response metadata must be finalized after body streaming

Ensure the response metadata containing body length and SHA-256 cannot be published as complete before body streaming succeeds.

Required correction

Required response sequence:

create response body
→ stream bytes and hash
→ flush and sync response body
→ construct complete response metadata
→ atomically persist response metadata
→ sync response directory
→ finalize transaction directory

If streaming fails:

* preserve the partial staging directory;
* preserve bytes already received;
* attempt to persist a bounded typed incomplete-response marker;
* never publish complete response metadata;
* never rename the partial directory as finalized;
* return a typed streaming/recording error.

Do not discard the original body-read failure if writing the incomplete marker also fails. Preserve both through a typed combined error.

HTTP execution ordering after correction

The authoritative physical-exchange order must be:

validate managed context and running session
→ finalize immutable request
→ allocate unique transaction identity and staging directory
→ persist redacted request metadata
→ persist exact request body
→ sync request state
→ perform exactly one physical HTTP exchange
→ persist transport failure or stream response body
→ persist final response metadata
→ sync completed staging transaction
→ atomically rename to final transaction
→ sync raw-data parent
→ construct finalized recorded attempt
→ revalidate running session and supervisor ownership
→ atomically update acquisition progress
→ return or continue retry/redirect orchestration

Every retry and redirect follows this same physical-exchange sequence.

Error hierarchy

Keep the HTTP engine fully typed.

Use nested errors equivalent to:

HttpRequestError
HttpTransportConfigurationError
HttpTransportFailure
HttpRecorderError
HttpTransactionFinalizationError
AcquisitionProgressError
HttpRetryExhaustionError
HttpRedirectError
HttpExecutionError
AcquisitionError

Equivalent organization is acceptable.

Every nested error must implement:

std::fmt::Display
std::error::Error

Use source().

Do not return plain String from the Core-owned HTTP engine.

Sensitive diagnostics

No Display or runner diagnostic may reveal:

* source arguments;
* invocation-envelope JSON;
* runtime-context JSON;
* request URL;
* sensitive query values;
* request header values;
* response header values;
* environment-variable names or values;
* request body;
* response body;
* cookies;
* authorization credentials;
* arbitrary source error messages;
* raw Reqwest error text that may contain URLs.

Safe diagnostics may include:

* stable failure class;
* HTTP status;
* retry count;
* redirect count;
* transaction identity;
* established non-secret session identifier;
* Core-owned managed path category.

Avoid printing full filesystem paths unless required to identify a durable partial commit at the command boundary. Prefer structured accessors over including paths in Display.

Capability behavior

Keep:

HttpCapabilitySet::empty()

as the managed runtime’s available set.

Do not advertise:

HttpCapability::ClientCertificateV1

No client-certificate behavior belongs in this correction.

Preserve existing architecture

Do not change:

* HttpAcquireFn;
* HttpResumeFn;
* HttpSourceContractV1;
* invocation-envelope JSON;
* argv transport;
* source-argument preservation;
* HTTP invocation admission;
* handler selection;
* runtime-information probes;
* processing admission;
* session lifecycle states;
* supervisor lease ownership;
* foreground process launch;
* foreground terminal reconciliation;
* generated managed runner structure;
* source creation;
* source build;
* verification;
* staging;
* bundle admission;
* paired publication;
* CLI command syntax;
* MZA;
* Protocol 1;
* installer behavior.

Source-level acceptance requirements

Correct the source so that:

1. Recorded request body paths point into the finalized directory.
2. Recorded response body paths point into the finalized directory.
3. No public recorded type retains a staging path.
4. RecordedTransaction is constructed only after final rename and parent sync.
5. Post-rename sync failure is a typed partial commit.
6. Transport configuration failures retain their typed cause.
7. Transport initialization errors are not discarded with .ok().
8. Reqwest configuration matches the enabled feature set.
9. Transparent content decoding remains unavailable.
10. Automatic redirects remain disabled.
11. Automatic retries remain disabled.
12. Configuration and request-build failures are non-retryable.
13. Only typed transient failures are retryable.
14. Recorded failure classification drives retry decisions.
15. Transport-failure body persistence errors are not discarded.
16. All transaction/progress failures after finalization are partial commits.
17. Progress decoding is strict and versioned.
18. Progress update revalidates the running acquisition session.
19. Progress update confirms supervisor ownership.
20. Progress counters and revision use checked arithmetic.
21. Progress temporary files clean themselves on failure.
22. Every managed path component is checked for symlinks.
23. Request metadata is persisted before request-body persistence.
24. Both request files are durable before transport begins.
25. Response metadata is published only after body streaming completes.
26. Response-stream failure leaves a recognizable partial transaction.
27. Raw-data parent is synced after transaction rename.
28. Retry exhaustion retains the final recorded attempt.
29. Recorded transport failure retains transaction identity and path.
30. Transport failure has no fabricated HTTP status.
31. Redirect control uses internal transport data, not persisted redacted metadata.
32. Initial URL participates in redirect-loop detection.
33. Existing URL query fields can be marked sensitive.
34. Sensitive environment-variable names are not displayed.
35. Arbitrary Core execution-message variants are removed.
36. Managed runners do not print arbitrary source error messages.
37. Timestamp representation is truthful and consistent.
38. Staging allocation and final publication are race-safe.
39. Finalized transactions are immutable.
40. HttpAcquisitionContext::execute(...) succeeds only after progress publication.

Command-execution constraint

This is a source-only correction milestone.

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
* real HTTP requests;
* test HTTP requests;
* workspace validation;
* bundle/install automation.

Do not wait on compilation or tests.

Existing test source may be adjusted only where production API alignment requires it. Do not add or execute the broad HTTP validation matrix in this milestone.

Explicit exclusions

Do not implement:

* checkpoints;
* checkpoint recovery;
* client certificates;
* proxy configuration;
* decoded response readers;
* content interpretation;
* processing transaction discovery;
* processing SQLite behavior;
* background operator host;
* signal forwarding;
* background supervision;
* lexicon build;
* automatic build-before-run;
* new CLI commands;
* source migration;
* cross-compilation;
* MZA changes;
* installer changes.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* corrected recorded request path behavior;
* corrected recorded response path behavior;
* staged/finalized/progress-published type boundary;
* exact finalization and sync order;
* post-rename durability-failure behavior;
* Reqwest dependency features;
* transparent-decompression configuration;
* typed transport initialization behavior;
* transport failure classification;
* retryability rules;
* recorded transport-failure representation;
* retry-exhaustion representation;
* progress partial-commit behavior;
* progress schema validation;
* progress session revalidation;
* progress atomic replacement behavior;
* checked counter behavior;
* managed-path component and symlink validation;
* request persistence order;
* response streaming and incomplete-response behavior;
* raw-parent directory sync behavior;
* redirect control-data boundary;
* redirect-loop correction;
* original-URL sensitive query marking;
* environment-variable diagnostic sanitization;
* removal of arbitrary Core execution messages;
* managed source-error diagnostic behavior;
* timestamp representation;
* collision-safe staging allocation;
* immutable transaction publication behavior;
* final HttpAcquisitionContext::execute(...) success guarantee;
* capability-set result;
* confirmation that checkpoints, SQLite, background supervision, and build behavior were not added;
* existing test source adjusted only for API alignment, if applicable;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, runtime execution, HTTP execution, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin checkpoint or processing behavior until this HTTP correctness closure is complete.