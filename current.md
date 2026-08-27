Current implementation milestone: Core HTTP transaction engine and durable raw recording

Objective

Implement the Core-owned HTTP effect required by workspace/specs/contract.md:

let transaction = context.execute(request)?;

This milestone adds real HTTP acquisition execution, exact durable raw transaction recording, metadata redaction, explicit retry and redirect handling, and per-session acquisition progress.

The completed path must be:

admitted HTTP acquisition invocation
→ validated session-bound HttpAcquisitionContext
→ finalized HttpRequest
→ unique raw transaction staging directory
→ redacted request metadata + exact request body
→ one physical HTTP exchange
→ redacted response metadata or typed transport failure
→ undecoded entity-body streaming + hashing
→ durable transaction finalization
→ durable session progress update
→ RecordedTransaction

Every physical retry attempt and redirect exchange must pass through the recorder as a distinct transaction.

This is a Core HTTP milestone. Do not begin checkpoints, processing SQLite behavior, background supervision, or lexicon build.

Repository-grounded starting point

At commit:

7aecbcfd91e5112a9b2badc457e4dc25ae7495bc

the following foundations already exist:

* HttpAcquisitionContext is session-bound and exposes validated SessionDataPaths;
* acquisition handlers already receive &mut HttpAcquisitionContext;
* foreground execution and terminal reconciliation are implemented;
* session records, summaries, leases, and child lifecycle transitions exist;
* HTTP admission and handler selection exist;
* source arguments are preserved as native OsString values;
* AcquisitionResult<T> and AcquisitionError already form the handler error boundary.

The following are still absent:

* HttpAcquisitionContext::execute(...);
* a typed HTTP request model;
* a Core-owned HTTP transport;
* raw transaction identities and schemas;
* exact request/response body recording;
* metadata redaction;
* retry and redirect orchestration;
* durable acquisition progress.

Implement those missing pieces without redesigning the completed invocation, session, build, or foreground-supervision layers.

Contract authority

Follow:

workspace/specs/contract.md

especially:

* section 10, HTTP execution and raw-data contract;
* section 11, secret handling;
* the session ownership boundary in section 12.

If an implementation choice conflicts with this milestone text, preserve the contract’s raw-fidelity, redaction, and supervision guarantees.

Public execution API

Add:

impl HttpAcquisitionContext {
    pub fn execute(
        &mut self,
        request: HttpRequest,
    ) -> AcquisitionResult<RecordedTransaction>;
}

execute(...) must be the supported HTTP-effect boundary for acquisition source implementations.

A source handler must not receive a live unrecorded transport response. It receives a RecordedTransaction only after the final physical exchange has been recorded durably and session progress has been updated.

Preserve the existing handler signature:

fn(
    &mut HttpAcquisitionContext,
    &[OsString],
) -> AcquisitionResult<()>

Do not add an alternate handler ABI.

Module organization

Move HttpAcquisitionContext out of the crate-root implementation if useful, while preserving its existing public path.

Use a cohesive HTTP module structure equivalent to:

lexicon-core/src/protocols/http/
├── capability.rs
├── contract.rs
├── error.rs
├── invocation.rs
├── runner.rs
├── context.rs
├── request.rs
├── policy.rs
├── transport.rs
└── transaction/
    ├── mod.rs
    ├── identity.rs
    ├── metadata.rs
    ├── recorder.rs
    └── error.rs

Equivalent organization is acceptable.

Export the source-facing API through:

lexicon_core::http

Keep transport implementation details and unchecked recorder constructors private or pub(crate).

HTTP client dependency and configuration

Add a production blocking HTTP client suitable for Core’s synchronous handler API.

A configuration based on reqwest::blocking::Client is acceptable.

If using reqwest:

* disable default features;
* enable only the TLS and blocking functionality actually required;
* disable automatic redirect following;
* disable transparent gzip decoding;
* disable transparent Brotli decoding;
* disable transparent deflate decoding;
* disable transparent Zstandard decoding;
* do not enable an internal retry layer;
* do not expose a live reqwest::Response to source code.

The transport must yield HTTP entity-body bytes after transfer framing and before content decoding.

Stored response data must not be silently replaced by decompressed content.

Add only dependencies required for the implementation, such as:

* a blocking HTTP client;
* URL parsing;
* SHA-256 hashing;
* collision-resistant transaction identities.

Do not claim client-certificate support merely because the chosen client library could support it.

Available capability behavior

The only currently declared HTTP capability is:

HttpCapability::ClientCertificateV1

This milestone does not implement the complete contract for that capability.

Therefore:

HttpCapabilitySet::empty()

remains the managed runtime’s established available-capability set.

Do not advertise ClientCertificateV1.

Do not infer available capabilities from source requirements.

HTTP request model

Define an opaque typed request representation equivalent to:

pub struct HttpRequest {
    // private fields
}

Provide constructors for ordinary methods, including at least:

HttpRequest::get(...)
HttpRequest::post(...)
HttpRequest::put(...)
HttpRequest::patch(...)
HttpRequest::delete(...)
HttpRequest::head(...)

Provide typed builder operations equivalent to:

request.header(...)
request.sensitive_header(...)
request.sensitive_header_from_env(...)
request.query_parameter(...)
request.sensitive_query_parameter(...)
request.body_bytes(...)
request.text(...)
request.json(...)
request.logical_key(...)
request.retry_policy(...)
request.redirect_policy(...)

Equivalent naming is acceptable.

Builder operations must return typed errors.

Do not return plain String from request construction.

Request finalization

Before allocating or executing the first physical transaction, finalize the request into an immutable effective representation.

Finalization must:

* validate the method;
* validate and parse the URL;
* reject unsupported URL schemes;
* validate header names and values;
* preserve repeated headers;
* preserve the exact request-body bytes that will be supplied to transport;
* serialize JSON exactly once;
* retain sensitivity annotations;
* validate retry and redirect policies;
* establish the logical request key, if supplied.

The bytes written to:

request/body

must be the same bytes supplied to the transport for that physical attempt.

Do not reserialize JSON separately for persistence and transport.

Do not persist environment-variable names or secret values merely because a sensitive header was sourced from the environment.

URL and query handling

Support:

http
https

Reject unsupported schemes through a typed request-finalization error.

Explicitly sensitive query parameters must be marked at request construction.

The effective URL sent to transport may contain the real value, but persisted metadata must contain a deterministic redacted replacement.

Do not redact a query parameter solely because its name looks suspicious unless it is part of a documented mandatory managed rule.

At minimum, redact parameters explicitly marked sensitive.

Preserve duplicate query parameters and ordering in the effective request.

Redirect-generated requests must retain the sensitivity classification of existing query fields where applicable.

Header behavior

Header names are compared case-insensitively for redaction.

Persisted request metadata must redact at least:

* Authorization;
* Proxy-Authorization;
* Cookie;
* every explicitly marked sensitive request header.

Persisted response metadata must redact at least:

* Set-Cookie;
* every other response header covered by a documented managed-sensitive rule.

Preserve:

* repeated headers;
* header ordering where exposed by the transport;
* non-UTF-8-valid header values through a reversible safe metadata representation or a clearly typed encoded representation.

Do not use lossy UTF-8 conversion for exact header values.

The transport receives the unredacted request values. Only the persisted metadata representation is redacted.

Error Display implementations must not reveal sensitive header values.

Request bodies

Support at least:

* no body;
* exact owned bytes;
* UTF-8 text converted once to bytes;
* JSON serialized once to bytes.

The recorder must persist exact request bytes when a body exists.

Core cannot generically redact arbitrary body secrets while preserving exact bytes. Document this explicitly in the API.

Do not scan, rewrite, normalize, or semantically redact request or response bodies.

Transaction identity

Define an opaque collision-resistant transaction identity:

pub struct HttpTransactionIdentity {
    // private
}

Provide:

pub fn id(&self) -> &str;

The stable identifier must be safe as one path component.

Each physical exchange receives a new identity, including:

* the initial exchange;
* each retry attempt;
* each followed redirect.

Do not derive identity solely from process ID, thread ID, or a low-resolution timestamp.

Raw transaction schema version

Define a schema version distinct from:

* session schema version;
* invocation-envelope schema version;
* runtime-information schema version;
* manifest schema versions.

For example:

pub const HTTP_TRANSACTION_SCHEMA_VERSION: u32 = 1;

Apply strict decoding with unknown-field rejection to Core-owned transaction metadata.

Define explicit maximum metadata-document sizes.

Do not apply a Lexicon-defined size limit to recorded response bodies.

Raw transaction layout

Every finalized physical transaction must use:

data/raw/<timestamp>-<transaction-id>/
├── request/
│   ├── metadata.json
│   └── body
└── response/
    ├── metadata.json
    └── body

The directory name must contain a path-safe timestamp and transaction identifier.

The returned RecordedTransaction must identify the finalized directory.

Do not publish raw files elsewhere in the source workspace.

Do not place finalized acquisition transactions below the operation workspace.

Staging and partial records

Allocate a unique staging directory below the validated raw-data root before performing network I/O.

Use a recognizable Core-owned partial name, equivalent to:

data/raw/.partial-<timestamp>-<transaction-id>/

The staging directory must be on the same filesystem as the final transaction directory so final publication can use an atomic rename.

Required behavior:

1. Create the staging transaction directory.
2. Create the request and response subdirectories.
3. Persist request metadata.
4. Persist the exact request body.
5. Sync durable request files as appropriate.
6. Perform the physical exchange.
7. Persist response metadata or transport-failure metadata.
8. Stream response body bytes into the staging response body.
9. Finalize hashes and sizes.
10. Sync the completed transaction.
11. Atomically rename the staging directory to its final name.
12. Update session acquisition progress.
13. Return the recorded value.

If execution is interrupted, the partial directory must remain recognizable and must never be mistaken for a finalized transaction.

Do not delete diagnostically useful partial transaction data automatically after an I/O or transport failure.

Do not return a RecordedTransaction for an incomplete partial directory.

Request metadata

Define a strict request metadata document containing at least:

* schema version;
* transaction identity;
* session identity;
* physical attempt index;
* redirect index;
* retry index;
* optional parent transaction identity;
* optional logical request key;
* method;
* redacted effective URL;
* redacted request headers;
* whether a request body is present;
* exact request-body byte length;
* SHA-256 of the persisted request body when present;
* creation timestamp.

The document must not contain:

* source arguments;
* invocation-envelope JSON;
* runtime-context JSON;
* project filesystem paths;
* unredacted managed secrets.

The request-body hash must be calculated from the exact persisted bytes.

Response metadata

Define a strict response metadata document that distinguishes:

HttpRecordedOutcome::Response(...)
HttpRecordedOutcome::TransportFailure(...)

Equivalent tagged representation is acceptable.

A successful physical exchange response record must include at least:

* schema version;
* transaction identity;
* status code;
* HTTP version when available;
* redacted response headers;
* exact response-body byte length;
* SHA-256 of the stored response body;
* response-completion timestamp.

A transport-failure record must include at least:

* schema version;
* transaction identity;
* a stable typed failure class;
* whether retry policy considered it retryable;
* failure timestamp;
* no arbitrary library error dump.

Do not persist raw TLS diagnostics, request secrets, URLs containing unredacted sensitive queries, or arbitrary deeply nested client-library diagnostics.

Response streaming and exactness

Stream the response body directly into the staged:

response/body

while computing SHA-256.

Do not first accumulate the full response in memory.

The stored bytes must be:

HTTP entity-body bytes after transfer framing
and before content decoding

This means:

* chunk framing is not stored;
* TLS records are not stored;
* TCP packets are not stored;
* HTTP/2 frames are not stored;
* gzip/Brotli/deflate/Zstandard content remains encoded exactly as received.

If body reading fails after some bytes have been written:

* preserve the partial directory;
* preserve the bytes already written;
* write a typed incomplete-response marker or metadata document when safely possible;
* do not atomically publish the directory as a complete transaction;
* return a typed recording error.

Recorded transaction representation

Define an opaque value equivalent to:

pub struct RecordedTransaction {
    identity: HttpTransactionIdentity,
    directory: PathBuf,
    request: RecordedHttpRequest,
    response: RecordedHttpResponse,
}

Keep fields private.

Provide read-only accessors for at least:

pub fn identity(&self) -> &HttpTransactionIdentity;
pub fn directory(&self) -> &Path;
pub fn request(&self) -> &RecordedHttpRequest;
pub fn response(&self) -> &RecordedHttpResponse;

The response representation must provide at least:

pub fn status(&self) -> u16;
pub fn headers(&self) -> &RecordedHeaderCollection;
pub fn body_path(&self) -> &Path;
pub fn body_length(&self) -> u64;
pub fn body_sha256(&self) -> &str;
pub fn require_success(&self) -> AcquisitionResult<()>;

require_success() must use HTTP status semantics and return a typed acquisition error without printing the response body.

Do not expose an unchecked public constructor.

Transport boundary

Create a narrow internal transport seam so physical exchange behavior is separated from recording and orchestration.

The seam must represent:

* finalized method;
* finalized URL;
* exact headers;
* exact optional body bytes;
* status;
* HTTP version;
* response headers;
* a streaming response body reader;
* typed transport failure.

Production transport must perform exactly one physical exchange per seam call.

It must not automatically:

* retry;
* follow redirects;
* decompress content;
* log requests;
* log responses.

Retry and redirect orchestration belongs above this one-exchange seam so each exchange is independently recorded.

Do not make the transport seam a source-extensibility interface in this milestone.

Redirect policy

Default behavior must not follow redirects automatically.

Define an explicit typed redirect policy equivalent to:

pub enum HttpRedirectPolicy {
    None,
    Follow {
        maximum: u32,
    },
}

Use a bounded maximum and reject invalid policy values.

When following redirects:

* finalize the redirect target against the current effective URL;
* allocate a new transaction identity;
* record the redirect response as a complete transaction;
* create a new physical transaction for the next request;
* retain parent-transaction linkage;
* increment the redirect index;
* detect policy exhaustion;
* detect redirect loops;
* reject invalid or unsupported redirect targets;
* never let the underlying client follow invisibly.

For cross-origin redirects, remove at least:

* Authorization;
* Proxy-Authorization;
* Cookie;
* explicitly sensitive headers.

Never copy a Host header supplied for one origin to another origin.

Apply deterministic method/body rules:

* 303 becomes GET with no request body, except HEAD remains HEAD;
* 307 and 308 preserve method and body;
* preserve method and body for 301 and 302 rather than applying undocumented browser-style POST rewriting.

Document these rules in the public policy API.

A redirect-policy failure must retain the already finalized transaction history.

Retry policy

Default behavior must perform no retries.

Define a typed bounded retry policy. Equivalent organization is acceptable:

pub struct HttpRetryPolicy {
    maximum_attempts: u32,
    retryable_transport_failures: ...,
    retryable_statuses: ...,
}

Provide safe constructors such as:

HttpRetryPolicy::none()
HttpRetryPolicy::transient(maximum_attempts)

Do not add unbounded retry behavior.

Retry orchestration must:

* assign a distinct transaction identity to each attempt;
* persist every completed response or transport-failure record;
* increment the retry index;
* retain linkage to the original logical request;
* stop after the configured maximum;
* avoid retrying request-construction or recorder failures;
* avoid sleeping for arbitrary undocumented durations;
* use a bounded deterministic delay policy if delays are included.

Do not let the HTTP client library retry underneath Core.

A final exhausted-retry error must retain typed information about the final attempt and recorded transaction history without exposing secrets.

Retry and redirect interaction

Define a deterministic ordering.

Use this model:

* a redirect chain is composed of physical requests;
* each physical redirect target may use the request’s retry policy;
* every retry is recorded before proceeding;
* every redirect response is recorded before following;
* the returned RecordedTransaction is the final non-redirect response;
* all preceding transactions remain durable.

Do not collapse multiple physical attempts into one raw directory.

Durable acquisition progress

The contract requires Core to update session progress after physical transaction recording.

Do not overload the detailed session lifecycle record with repeated Running → Running transitions.

Add a distinct Core-owned acquisition progress document under the validated session directory, equivalent to:

<session-directory>/acquisition_progress.json

Define a separate schema version and strict document type.

The progress document must contain at least:

* schema version;
* session identity;
* completed physical transaction count;
* recorded transport-failure count;
* recorded redirect count;
* recorded retry count;
* optional last finalized transaction identity;
* optional last logical request key;
* update timestamp;
* monotonic revision.

Progress updates must:

* validate the session identity;
* validate that the durable session remains Running;
* use atomic replacement;
* increment revision exactly once;
* occur only after the transaction directory is finalized;
* never include source arguments, request URLs, header values, body contents, or arbitrary source error text.

A transaction-finalization success followed by a progress-write failure is a partial commit.

Return a typed error that preserves:

* the finalized transaction identity;
* the finalized transaction path;
* the progress persistence failure.

Do not delete the finalized transaction to disguise the partial commit.

The existing detailed session record and root summary remain authoritative for lifecycle state.

Context construction

Extend HttpAcquisitionContext with the private state required to execute and record HTTP effects.

Managed construction must continue to originate from:

HttpAcquisitionContext::from_session_data_paths(...)

It must bind:

* validated session data paths;
* session identity;
* operation root;
* raw-data directory;
* session directory;
* production transport configuration.

Do not accept arbitrary raw or session directories through a new public unchecked constructor.

The quarantined legacy constructor may remain for legacy API compatibility, but it must not silently acquire full managed session guarantees.

If legacy mode cannot safely write managed transaction progress, return a typed unsupported-context error from execute(...).

Do not synthesize a fake managed session identity.

Session-state validation

Before the first physical exchange, verify:

* the context has a managed session identity;
* the session record exists;
* the detailed session state is Running;
* the session operation is acquisition;
* the session identity matches the context;
* the runtime identity represents HTTP acquisition;
* the session data paths agree with the validated operation and session roots;
* an external supervisor lease remains owned.

Reuse the established session store and lease-inspection APIs.

Do not acquire the supervisor lease in the child.

Do not weaken the existing session binding model.

Acquisition error integration

Replace the current message-only acquisition error boundary with a typed hierarchy while preserving compatibility for source-authored failures.

An equivalent representation is acceptable:

#[derive(Debug)]
pub enum AcquisitionError {
    Source {
        message: String,
    },
    Request(
        HttpRequestError,
    ),
    Execution(
        HttpExecutionError,
    ),
    ResponseStatus(
        HttpResponseStatusError,
    ),
}

Preserve:

AcquisitionError::source_message(...)

for source implementations that need it.

Implement:

std::fmt::Display
std::error::Error

Use source() for nested errors.

Do not stringify nested Core HTTP, session, transport, or recording errors inside the engine.

Typed HTTP errors

Add typed errors for at least:

* request construction;
* unsupported scheme;
* invalid URL;
* invalid header name;
* invalid header value;
* environment-variable access;
* JSON body serialization;
* invalid retry policy;
* invalid redirect policy;
* unmanaged acquisition context;
* session validation;
* raw-root creation;
* transaction identity allocation;
* staging-directory creation;
* request metadata encoding;
* request metadata persistence;
* request-body persistence;
* transport failure;
* response metadata encoding;
* response metadata persistence;
* response-body streaming;
* response-body hashing;
* durable sync;
* atomic finalization;
* final path collision;
* progress loading;
* progress decoding;
* progress persistence;
* transaction/progress partial commit;
* redirect exhaustion;
* redirect loop;
* invalid redirect target;
* retry exhaustion;
* non-success HTTP status.

Equivalent nesting is acceptable.

All error types must implement:

std::fmt::Display
std::error::Error

Preserve typed nested causes through source().

Sensitive diagnostic behavior

HTTP errors and their Display implementations must not reveal:

* source arguments;
* invocation-envelope JSON;
* runtime-context JSON;
* project identity;
* session identity unless represented through its established non-secret identifier;
* unredacted URLs containing sensitive query values;
* request or response header values;
* request or response bodies;
* environment-variable values;
* cookies;
* authorization credentials;
* raw client-library diagnostics that may embed a URL or header.

Stable method names, status codes, transaction identifiers, retry indices, redirect indices, and sanitized failure classes may be displayed.

Do not print diagnostics directly from Core.

Atomic persistence

Use the repository’s established atomic-persistence principles:

1. serialize the complete metadata document;
2. create a unique temporary file in the destination directory;
3. write all bytes;
4. flush and sync_all;
5. atomically persist or replace;
6. best-effort sync the containing directory.

Do not use fixed shared temporary filenames.

Do not truncate an existing final metadata file in place.

Do not follow symlinks for Core-managed transaction roots or files.

Reject a raw root, staging entry, transaction entry, request directory, response directory, or managed file when it is a symlink.

Filesystem containment

All transaction and progress paths must derive from validated SessionDataPaths.

Before writing, validate that:

* the raw-data root is the expected absolute path;
* the session directory is the expected absolute path;
* the operation is HTTP acquisition;
* constructed paths remain descendants of their expected roots;
* no path component supplied by transaction identity can escape its root;
* existing managed paths are directories or regular files of the expected kind;
* managed paths are not symlinks.

Do not accept source-provided output paths for transaction recording.

Existing runner integration

The existing HTTP runner already constructs a session-bound HttpAcquisitionContext before invoking the selected handler.

Update it only as needed to initialize the new managed execution state.

Preserve this lifecycle:

parse invocation
→ admit HTTP invocation
→ decode runtime context
→ open session store
→ bind prepared session
→ enter Running
→ construct session-bound HTTP context
→ invoke selected source handler
→ persist Succeeded or Failed

Ordinary HTTP execution errors returned by the source handler continue through the existing safe source-failure lifecycle boundary.

Do not persist the full HTTP error or arbitrary source Display output into session.json.

The detailed transaction records are the diagnostic source for HTTP execution.

Source-facing response behavior

A source may:

* inspect the recorded status;
* inspect redacted-safe response metadata;
* open the exact recorded body file;
* call require_success().

A source must not:

* access a live socket or transport response through RecordedTransaction;
* mutate Core transaction metadata through public APIs;
* obtain unredacted managed-sensitive response metadata through the persisted representation;
* receive a finalized transaction before durability and progress publication succeed.

Do not implement content decoding helpers in this milestone.

Raw-fidelity boundary

Document clearly:

* request bodies are preserved exactly as supplied to transport;
* response bodies are preserved as undecoded HTTP entity bytes;
* metadata is redacted;
* bodies are not generically redacted;
* transport framing is not part of the stored body;
* TLS/TCP/HTTP2 framing is not part of the stored body.

Do not claim byte identity with network packets.

Source-level acceptance requirements

Implement source and API coverage for the following behavior. Do not execute tests in this milestone.

1. Managed HttpAcquisitionContext exposes execute(...).
2. Request construction is typed.
3. JSON is serialized once.
4. Exact request bytes are shared by recorder and transport.
5. Unsupported schemes are rejected.
6. Mandatory sensitive request headers are redacted in metadata.
7. Set-Cookie is redacted in response metadata.
8. Explicitly sensitive headers are redacted.
9. Explicitly sensitive query values are redacted.
10. Body bytes are not rewritten by metadata redaction.
11. Every physical exchange receives a unique transaction identity.
12. Request metadata is durable before transport begins.
13. Transport performs one physical exchange.
14. Automatic client redirects are disabled.
15. Automatic client retries are disabled.
16. Transparent response decompression is disabled.
17. Response bytes stream to disk rather than accumulating in memory.
18. SHA-256 is computed while response bytes stream.
19. Final metadata contains exact body length and hash.
20. Complete transactions finalize atomically.
21. Interrupted body reads leave recognizable partial records.
22. Transport failures leave typed durable failure records.
23. A redirect response is finalized before the next redirect request.
24. Each redirect has its own transaction.
25. Each retry has its own transaction.
26. Cross-origin redirects strip managed-sensitive request headers.
27. Redirect loops and limits are typed.
28. Retry exhaustion is typed.
29. The final returned transaction represents the final non-redirect response.
30. Session acquisition progress updates after finalization.
31. Progress revisions are monotonic.
32. Progress contains no request secrets.
33. Transaction/progress partial commits are preserved and typed.
34. RecordedTransaction exposes only finalized paths.
35. require_success() does not print or read the response body.
36. Error formatting does not reveal sensitive data.
37. Existing handler signatures remain unchanged.
38. Existing acquisition admission remains unchanged.
39. Existing session lifecycle ownership remains unchanged.
40. Existing foreground reconciliation remains unchanged.
41. ClientCertificateV1 remains unavailable.
42. No checkpoint, SQLite, background, or project-build behavior is introduced.

Add narrow internal seams and production-source fixtures where useful for later validation, but do not run them now.

Command-execution constraint

This is a source-only implementation milestone.

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
* generated source runners;
* HTTP test servers;
* real HTTP requests;
* workspace validation;
* bundle/install automation.

Do not wait on compilation or test execution.

Existing test source may be adjusted only where production API changes require alignment. Broad validation and the full HTTP test matrix are deferred to the final validation phase.

Preserve existing behavior

Do not change:

* CLI command names or argument syntax;
* lexicon init;
* source creation;
* source build;
* generated managed workspace structure;
* immutable Core revision pinning;
* runtime-information probe JSON;
* probe output streams;
* runtime verification;
* hashing of runtime executables;
* staging;
* bundle admission;
* paired publication;
* invocation-envelope JSON;
* argv transport;
* acquisition admission;
* processing admission;
* source arguments;
* session selection;
* session lease ownership;
* session lifecycle state transitions;
* foreground process launching;
* foreground terminal reconciliation;
* processing context;
* processing runner behavior;
* source descriptor signatures;
* capability identifiers;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior.

Explicit exclusions

Do not implement:

* HTTP client certificates;
* proxy configuration;
* content decoding helpers;
* checkpoint APIs;
* checkpoint persistence;
* source-defined safe argument summaries;
* processing raw-transaction discovery;
* processing SQLite output;
* processing query APIs;
* background operator host;
* signal forwarding;
* background supervision;
* lexicon build;
* automatic build-before-run;
* source workspace migration;
* cross-compilation;
* MZA changes;
* installer changes.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* final HTTP module structure;
* dependencies added and exact enabled features;
* HttpAcquisitionContext::execute(...) API;
* request model and finalization behavior;
* exact request-body preservation behavior;
* production transport configuration;
* confirmation that automatic redirects are disabled below Core;
* confirmation that automatic retries are disabled below Core;
* confirmation that transparent decompression is disabled;
* transport seam behavior;
* transaction identity representation;
* raw transaction schema version;
* final transaction directory structure;
* staging and partial-record behavior;
* request metadata fields;
* response metadata and transport-failure fields;
* mandatory redaction behavior;
* explicitly sensitive header/query behavior;
* response streaming and hash behavior;
* atomic finalization behavior;
* RecordedTransaction representation;
* redirect policy and per-exchange recording behavior;
* cross-origin redirect credential stripping;
* retry policy and per-attempt recording behavior;
* retry/redirect interaction;
* acquisition progress schema and path;
* progress revision behavior;
* transaction/progress partial-commit behavior;
* session-state validation before HTTP;
* acquisition error hierarchy;
* HTTP error sanitization behavior;
* capability set result;
* existing runner integration changes;
* confirmation that source arguments and bodies are never printed;
* confirmation that no checkpoint, SQLite, background-host, or build behavior was added;
* existing test source adjusted only for API alignment, if applicable;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, generated-runtime execution, HTTP execution, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin checkpoint or processing behavior.