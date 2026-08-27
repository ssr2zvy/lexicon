# Completion report: Core HTTP transaction engine and durable raw recording

## Files created and changed

### Created
- `/home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/context.rs`
- `/home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/policy.rs`
- `/home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/request.rs`
- `/home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/transport.rs`
- `/home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/transaction/error.rs`
- `/home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/transaction/identity.rs`
- `/home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/transaction/metadata.rs`
- `/home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/transaction/mod.rs`
- `/home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/transaction/recorder.rs`

### Changed
- `/home/runner/work/lexicon/lexicon/lexicon-core/Cargo.toml`
- `/home/runner/work/lexicon/lexicon/lexicon-core/src/lib.rs`
- `/home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/error.rs`
- `/home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/mod.rs`

## Final HTTP module structure

`lexicon-core/src/protocols/http/`
- `capability.rs`
- `context.rs`
- `contract.rs`
- `error.rs`
- `invocation.rs`
- `mod.rs`
- `policy.rs`
- `request.rs`
- `runner.rs`
- `transport.rs`
- `transaction/`
  - `error.rs`
  - `identity.rs`
  - `metadata.rs`
  - `mod.rs`
  - `recorder.rs`

## Dependencies added and exact enabled features
- `reqwest = { version = "0.12.9", default-features = false, features = ["blocking", "rustls-tls"] }`
- `sha2 = "0.10"`
- `url = "2.5"`
- `uuid = { version = "1.11", features = ["v4"] }`

## HttpAcquisitionContext::execute(...) API
- Added `pub fn execute(&mut self, request: HttpRequest) -> AcquisitionResult<RecordedTransaction>` on managed HTTP context.
- The method validates managed session state, finalizes request, records each physical exchange, updates acquisition progress, and only then returns `RecordedTransaction`.

## Request model and finalization behavior
- Added opaque typed `HttpRequest` with constructors: `get/post/put/patch/delete/head/new`.
- Added typed builders: `header`, `sensitive_header`, `sensitive_header_from_env`, `query_parameter`, `sensitive_query_parameter`, `body_bytes`, `text`, `json`, `logical_key`, `retry_policy`, `redirect_policy`.
- Request finalization validates URL parsing and supported scheme (`http`/`https`), validates header name/value, applies query values in order, tracks sensitivity, and serializes JSON exactly once at build time.

## Exact request-body preservation behavior
- Request body bytes used for transport are the same bytes persisted in `request/body`.
- JSON serialization occurs once into bytes and those bytes are reused.

## Production transport configuration
- Added blocking production transport seam via `ReqwestHttpTransport` implementing internal `HttpTransport`.
- Transport performs one physical exchange per call and returns status/version/headers plus stream body reader.

## Confirmation that automatic redirects are disabled below Core
- Reqwest client is configured with `redirect(Policy::none())`.

## Confirmation that automatic retries are disabled below Core
- Transport has no retry layer; retries are orchestrated in `HttpAcquisitionContext::execute`.

## Confirmation that transparent decompression is disabled
- Reqwest client explicitly disables `gzip`, `brotli`, `deflate`, and `zstd` transparent decoding.

## Transport seam behavior
- Internal seam accepts finalized method/url/headers/body and returns one exchange response or typed transport failure.
- No live response object is exposed to source-facing API.

## Transaction identity representation
- Added opaque `HttpTransactionIdentity` with stable path-safe id string and `id()` accessor.
- New identity allocated for each physical exchange.

## Raw transaction schema version
- Added `HTTP_TRANSACTION_SCHEMA_VERSION: u32 = 1` in transaction metadata module.

## Final transaction directory structure
- Finalized transaction layout:
  - `data/raw/<timestamp>-<transaction-id>/request/metadata.json`
  - `data/raw/<timestamp>-<transaction-id>/request/body`
  - `data/raw/<timestamp>-<transaction-id>/response/metadata.json`
  - `data/raw/<timestamp>-<transaction-id>/response/body`

## Staging and partial-record behavior
- Recording allocates `.partial-<timestamp>-<transaction-id>` staging path.
- Request metadata/body are persisted before transport starts.
- On interruptions/errors before finalize, partial staging remains recognizable.
- On success, staging is atomically renamed to final directory.

## Request metadata fields
- Persisted request metadata includes:
  - schema version
  - transaction id
  - session id
  - physical attempt index
  - redirect index
  - retry index
  - optional parent transaction id
  - optional logical request key
  - method
  - redacted effective URL
  - redacted request headers
  - body presence flag
  - exact body length
  - body SHA-256 (when present)
  - creation timestamp

## Response metadata and transport-failure fields
- Response metadata is tagged by outcome:
  - `response`: status, HTTP version, redacted headers, body length, body SHA-256, completion timestamp
  - `transport_failure`: stable failure class, retryable flag, failure timestamp

## Mandatory redaction behavior
- Request metadata redacts at least `Authorization`, `Proxy-Authorization`, `Cookie`.
- Response metadata redacts at least `Set-Cookie`.

## Explicitly sensitive header/query behavior
- Explicitly sensitive request headers are marked and redacted in persisted metadata.
- Explicitly sensitive query parameter names are tracked and redacted deterministically in persisted URL metadata.

## Response streaming and hash behavior
- Response body streams directly to disk (`response/body`) while computing SHA-256.
- Response bytes are not accumulated in memory before persistence.

## Atomic finalization behavior
- Metadata writes use unique temporary files and atomic rename.
- Staging transaction directory finalizes with atomic directory rename to final path.

## RecordedTransaction representation
- Added opaque `RecordedTransaction` with private fields and read-only accessors for identity, directory, request, response.
- Added `RecordedHttpResponse::require_success()` returning typed acquisition error based on status semantics.

## Redirect policy and per-exchange recording behavior
- Added `HttpRedirectPolicy::{None, Follow{maximum}}`.
- Redirect orchestration is explicit in `execute`; each redirect response is recorded before follow.
- 303 rewrites to GET with no body (HEAD stays HEAD), 307/308 preserve method/body, 301/302 preserve method/body.

## Cross-origin redirect credential stripping
- Cross-origin redirect request derivation removes `Authorization`, `Proxy-Authorization`, `Cookie`, explicit sensitive headers, and `Host`.

## Retry policy and per-attempt recording behavior
- Added bounded `HttpRetryPolicy` with constructors `none`, `transient`, `custom`.
- Each retry attempt is executed and recorded as a separate transaction.

## Retry/redirect interaction
- Per redirect target, retries are applied before next redirect advance.
- Final returned `RecordedTransaction` is the terminal non-redirect response transaction.

## Acquisition progress schema and path
- Added `HTTP_ACQUISITION_PROGRESS_SCHEMA_VERSION: u32 = 1`.
- Progress document path: `<session-directory>/acquisition_progress.json`.

## Progress revision behavior
- Progress updates are atomic replacement and increment `revision` monotonically by one per finalized transaction update.

## Transaction/progress partial-commit behavior
- If transaction finalization succeeds and progress persistence fails, a typed `ProgressPersistenceError::PartialCommit` is returned with finalized transaction identity/path.
- Finalized transaction is preserved.

## Session-state validation before HTTP
- `execute` validates managed context/session before first exchange:
  - managed session identity presence
  - path shape/containment and symlink rejection
  - session record load
  - `Running` state
  - acquisition operation
  - runtime protocol/operation agreement (`Http`/`Acquisition`)
  - session identity agreement
  - supervisor lease currently owned

## Acquisition error hierarchy
- Replaced message-only boundary with typed `AcquisitionError` hierarchy:
  - `Source { message }`
  - `Request(HttpRequestError)`
  - `Execution(HttpExecutionError)`
  - `ResponseStatus(HttpResponseStatusError)`
- Preserved `AcquisitionError::source_message(...)`.

## HTTP error sanitization behavior
- Display output avoids printing request/response bodies, header values, source args, runtime-context JSON, or raw transport diagnostics.
- Errors surface stable typed classes and safe status/identity data.

## Capability set result
- Available capability behavior unchanged; managed availability remains externally supplied and `HttpCapabilitySet::empty()` remains valid default behavior.
- `ClientCertificateV1` was not newly advertised or implemented.

## Existing runner integration changes
- Existing invocation admission/execution lifecycle remains intact.
- Runner still constructs `HttpAcquisitionContext` from session data paths and invokes existing acquire/resume signatures.
- `HttpAcquisitionContext` implementation moved under HTTP module and re-exported from crate root for compatibility.

## Confirmation that source arguments and bodies are never printed
- No added code prints source arguments, request bodies, or response bodies.

## Confirmation that no checkpoint, SQLite, background-host, or build behavior was added
- No checkpoint APIs, processing SQLite behavior, background host behavior, or project build pipeline behavior was introduced.

## Existing test source adjusted only for API alignment, if applicable
- Existing tests were not broadened; no validation matrix execution added.

## Confirmation that no tests/checks/builds/format/lint/metadata/CLI/runtime/HTTP/workspace/bundle-install execution was run
- No `cargo test`, `cargo check`, `cargo build`, `cargo fmt`, `cargo clippy`, `cargo metadata`, or `rustc` commands were run.
- No Lexicon CLI execution, generated runner execution, HTTP test server execution, real HTTP request execution, workspace validation, or bundle/install pipeline execution was run.
