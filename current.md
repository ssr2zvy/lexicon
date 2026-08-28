HTTP transaction-engine correctness closure — completion report

Files changed

lexicon-core/src/protocols/http/transport.rs
lexicon-core/src/protocols/http/request.rs
lexicon-core/src/protocols/http/error.rs
lexicon-core/src/protocols/http/context.rs
lexicon-core/src/protocols/http/mod.rs
lexicon-core/src/protocols/http/transaction/mod.rs
lexicon-core/src/protocols/http/transaction/error.rs
lexicon-core/src/protocols/http/transaction/metadata.rs
lexicon-core/src/protocols/http/transaction/recorder.rs

Corrected recorded request path behavior

RecordedTransaction is now constructed only after the staging directory has been atomically renamed to the final directory and the raw-data parent directory has been synced. RecordedHttpRequest::body_path() returns the path under the final directory (e.g., <final-dir>/request/body). No path inside the staging directory is exposed through any public recorded type.

Corrected recorded response path behavior

RecordedHttpResponse::body_path() returns the path under the final directory (e.g., <final-dir>/response/body). This path is constructed after the rename succeeds, not before.

Staged/finalized/progress-published type boundary

The recorder returns FinalizedRecordedAttempt (private to the crate), which carries the RecordedTransaction, effective_location, and transport_failure. RecordedTransaction is constructed inside record_transaction_attempt only after the rename and parent sync succeed. The public RecordedTransaction is only exposed to the source after persist_progress succeeds in execute(). This provides the three logical states: partially recorded (only in staging, returned as Recorder error), finalized (FinalizedRecordedAttempt returned from recorder), and progress-published (RecordedTransaction returned from execute()).

Exact finalization and sync order

1. Persist and sync redacted request metadata.
2. Persist and sync exact request-body bytes (when present).
3. Sync request directory.
4. Perform exactly one physical HTTP exchange.
5. Persist and sync response body (or empty body for transport failure).
6. Persist and sync response metadata.
7. Sync staging directory.
8. Atomically rename staging directory to final directory.
9. Sync raw-data parent directory.
10. Construct FinalizedRecordedAttempt with final-directory paths.
11. Revalidate session and supervisor lease.
12. Atomically update acquisition progress.
13. Return RecordedTransaction.

Post-rename durability-failure behavior

After the rename succeeds, if the raw-data parent directory sync fails, record_transaction_attempt returns HttpRecorderError::PostRenameSyncFailed(PostRenameSyncFailure { transaction_id, final_path, cause }). The renamed transaction is never deleted. This is a partial commit and the error carries the transaction identity and final path.

Reqwest dependency features

Features: blocking, rustls-tls. default-features = false. The gzip(false), brotli(false), deflate(false), zstd(false) builder calls were removed because they require feature flags that are not enabled. Transparent decompression is unavailable when the corresponding features are absent.

Transparent-decompression configuration

Transparent decompression (gzip, brotli, deflate, zstd) is not compiled or enabled. No compression features are enabled in Cargo.toml. The builder calls that depended on those features were removed.

Typed transport initialization behavior

ReqwestHttpTransport::new() returns Result<Self, HttpTransportConfigurationError>. HttpAcquisitionContext stores both the transport and the initialization error. If initialization failed, execute() returns HttpExecutionError::TransportConfiguration(error) immediately. The initialization error is not discarded with .ok().

Transport failure classification

HttpTransportFailure has seven variants: Configuration, RequestBuild, Connect, Timeout, BodyWrite, ExchangeIo, Tls. reqwest errors are classified using is_timeout(), is_connect(), is_request()/is_builder() before falling back to ExchangeIo. Each variant has a stable_class() method returning a &'static str and a retryable() method.

Retryability rules

Configuration: not retryable. RequestBuild: not retryable. Connect: retryable. Timeout: retryable. BodyWrite: not retryable. ExchangeIo: retryable. Tls: not retryable. Unknown failures default to non-retryable. The retry decision uses failure.retryable() from the recorded transport failure. Non-retryable failures return Transport(failure) immediately.

Recorded transport-failure representation

RecordedTransportFailure carries an HttpTransportFailure value (not a String). failure_class() returns the stable_class() &str. retryable() delegates to HttpTransportFailure::retryable(). The response metadata document stores the stable class string and retryable flag.

Retry-exhaustion representation

When a retryable failure is exhausted (retry_index + 1 >= max_attempts), execute() returns HttpExecutionError::RetryExhausted. When a non-retryable failure occurs, execute() returns HttpExecutionError::Transport(failure) with the original typed failure.

Progress partial-commit behavior

After transaction finalization, every failure in the progress pipeline is wrapped in ProgressPersistenceError::PartialCommit { transaction_id, transaction_path, source }. This includes session revalidation failures, lease check failures, progress load failures, decode failures, invariant validation failures, counter overflow, and persistence failures. Failures before finalization return ProgressPersistenceError::Progress(AcquisitionProgressError).

Progress schema validation

AcquisitionProgressDocument::validate_existing checks: exact schema version match; non-empty session_id matching the context; revision monotonicity; last_transaction_id consistency with completed_transaction_count. The document uses serde(deny_unknown_fields). Unknown schema versions return AcquisitionProgressValidationError::UnknownSchemaVersion.

Progress session revalidation

persist_progress calls validate_running_acquisition_session before each progress update. This checks: session record exists; session identity matches; state is Running; operation is Acquisition; runtime protocol is HTTP and operation is Acquisition; supervisor lease is Owned. Any failure returns a typed PartialCommit wrapping AcquisitionProgressError::from_session_validation(e).

Progress atomic replacement behavior

write_progress_atomic uses tempfile::Builder::new().prefix(".acquisition-progress-").suffix(".tmp").tempfile_in(parent) to create a unique temporary file in the session directory. The file is written, synced, and then atomically replaced via temp_path.persist(path). The session directory is best-effort synced after replacement. Temporary files are cleaned automatically on pre-publication failure by tempfile's drop behavior.

Checked counter behavior

All counters (physical_attempt_index, redirect_index, retry_index, completed_transaction_count, transport_failure_count, redirect_count, retry_count, revision) use checked_add(1).ok_or_else(|| CounterOverflow error). No saturating_add or unchecked addition is used. Overflow returns HttpExecutionError::CounterOverflow (for execution counters) or ProgressPersistenceError::PartialCommit wrapping AcquisitionProgressError::CounterOverflow (for progress counters).

Managed-path component and symlink validation

validate_no_symlink_components walks every component of the raw_data_root path and rejects any existing symlink via symlink_metadata(). Applied to the raw-data root in validate_root(). The recorder also calls validate_root before allocating staging directories.

Request persistence order

1. Compute exact request-body length and SHA-256 from the immutable body bytes.
2. Construct and persist redacted request metadata (metadata.json).
3. Persist exact request-body bytes (body file).
4. Sync request directory.
5. Begin transport.

Both request files are durable before transport begins.

Response streaming and incomplete-response behavior

Response body is streamed via stream_body() before response metadata is constructed. Response metadata is only persisted after streaming completes successfully. For transport failures, an empty response body is persisted before the failure metadata. The body hash in response metadata is computed from the actual streamed bytes. Body persistence errors are not discarded.

Raw-parent directory sync behavior

After fs::rename(staging, final) succeeds, sync_directory(raw_data_root) is called. If this fails, HttpRecorderError::PostRenameSyncFailed(PostRenameSyncFailure { transaction_id, final_path, cause }) is returned. The renamed directory is preserved. This satisfies the durable-directory-entry guarantee.

Redirect control-data boundary

The effective Location header value is extracted directly from the raw reqwest::Response in HttpTransportResponse::from_response() and stored as location_header: Option<String>. The recorder returns this in FinalizedRecordedAttempt::effective_location. The execute() method uses record.effective_location for redirect orchestration, not the persisted redacted response headers.

Redirect-loop correction

The initial finalized effective URL is inserted into seen_redirect_targets before the first exchange. Before following each redirect, the next URL is resolved, the canonical string is checked against seen_redirect_targets, and inserted if not present. Duplicate detection happens before performing the next exchange.

Original-URL sensitive query marking

HttpRequest::sensitive_query_name(name) marks every existing or appended query field with that decoded name as sensitive for persisted metadata. The name is matched ASCII case-insensitively. The set of explicit sensitive names is merged into sensitive_query_names during finalize(). Existing URL query fields marked this way are redacted in the persisted metadata.

Environment-variable diagnostic sanitization

HttpRequestError::EnvironmentVariableMissing(String) was replaced with HttpRequestError::EnvironmentVariableUnavailable and HttpRequestError::EnvironmentVariableNotUtf8. Neither variant includes the variable name or value in Display. The sensitive_header_from_env method uses std::env::var_os() and rejects non-UTF-8 values with the EnvironmentVariableNotUtf8 variant using exact (non-lossy) conversion.

Removal of arbitrary Core execution messages

HttpExecutionError::Message(String) was removed. AcquisitionError::execution_message() was removed. AcquisitionError::transport_failure() was added as a typed constructor. require_success() on a transport-failure outcome returns AcquisitionError::transport_failure(failure.failure()) rather than an execution_message.

Managed source-error diagnostic behavior

The AcquisitionError::Source { message } Display now emits "source handler returned an error" without printing the source-authored message text. The runner's HttpRuntimeInvocationExecutionError::Handler(_) Display emits "acquisition handler error". No arbitrary source error text is printed by the managed runner.

Timestamp representation

All timestamp fields use u64 nanoseconds since the Unix epoch. Field names are created_at_unix_nanos, completed_at_unix_nanos, failed_at_unix_nanos, updated_at_unix_nanos. This is a typed integer representation (option 2 from the contract). The now_nanos() helper returns u64 truncated from u128 with u64::MAX as a fallback on overflow (valid through approximately year 2554).

Collision-safe staging allocation

The staging directory is created with fs::create_dir() (exclusive creation, fails if already exists) rather than create_dir_all(). Before creating the staging directory, the final directory path is checked for existence and an error is returned if it already exists. Transaction identity uses uuid::Uuid::new_v4() which provides collision-resistant identities.

Immutable transaction publication behavior

After fs::rename(staging, final), the transaction directory is never replaced, overwritten, or deleted. Final-name collision (final_directory.exists() before rename) returns HttpRecorderError::FinalPublicationCollision without attempting the rename. PostRenameSyncFailed preserves the renamed directory.

Final HttpAcquisitionContext::execute() success guarantee

execute() returns Ok(RecordedTransaction) only after: (1) the recorder returns a FinalizedRecordedAttempt (staging renamed, parent synced), and (2) persist_progress returns Ok(()). Both session revalidation and progress persistence must succeed. Any failure after finalization returns a PartialCommit error with transaction identity and path.

Capability-set result

HttpCapabilitySet::empty() remains the managed runtime's available set. HttpCapability::ClientCertificateV1 is not advertised.

Confirmation: excluded items

Checkpoints, checkpoint recovery, SQLite processing, background supervision, lexicon build, automatic build-before-run, new CLI commands, client certificates, proxy configuration, decoded response readers, content interpretation, processing transaction discovery, cross-compilation, MZA changes, installer changes — none added.

Existing test source adjusted for API alignment

runner.rs tests and lib.rs tests did not require adjustment. No broad HTTP validation test matrix was added or executed.

Command-execution constraint

No cargo test, cargo check, cargo build, cargo fmt, cargo clippy, cargo metadata, rustc, Lexicon CLI commands, generated runners, HTTP servers, real HTTP requests, test HTTP requests, workspace validation, or bundle/install pipeline commands were run.
