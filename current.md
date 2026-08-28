Completion report

1. Replaced unit retry exhaustion with `HttpRetryExhaustionError`, retaining the finalized transaction, total physical attempts, and typed final outcome.
2. Added `HttpExecutionError::RecordedTransportFailure(RecordedHttpTransportFailure)` so durable transport failures preserve transaction provenance and attempt indices.
3. Removed synthetic `status.unwrap_or(0)` flow and made execution branch on `HttpRecordedOutcome`.
4. Collapsed redirect unit errors into `HttpRedirectFailure` with typed failure kinds and retained finalized redirect transactions.
5. Replaced saturating timestamp helpers with fallible `HttpClockError` conversion in recorder and progress paths.
6. Switched response streaming byte counting to `checked_add` and added `HttpBodyStreamingError::LengthOverflow`.
7. Persisted incomplete-response metadata markers with failure class, recorded byte count, partial SHA-256, and failure timestamp.
8. Replaced final publication with platform-specific no-replace directory publication and an unsupported-platform typed failure.
9. Added bounded staging-identity allocation retry and typed exhaustion behavior after eight collisions.
10. Centralized managed-path validation with `validate_managed_path` and applied it across execution, recording, publication, progress, and admission paths.
11. Tightened acquisition progress validation to exact revision matching with `RevisionMismatch`.
12. Added progress counter, revision, timestamp, last-transaction, logical-key, and overflow invariants.
13. Moved progress mutation into `AcquisitionProgressDocument::advance`.
14. Made progress file replacement report session-directory sync failure through typed progress errors.
15. Changed progress publication to consume `FinalizedRecordedAttempt` and yield `ProgressPublishedRecordedAttempt`.
16. Introduced typed `HttpAttemptIdentity` and threaded it through request metadata, recorded attempts, and failure reporting.
17. Changed parent transaction linkage to typed `Option<HttpTransactionIdentity>` with string conversion only at serialization boundaries.
18. Added `admit_transaction_from_disk` with metadata decoding, identity validation, header admission, body integrity checks, and incomplete-response rejection.
19. Added stable serialized `StoredTransportFailureClass` with conversions and retryability validation.
20. Replaced debug-formatted HTTP versions with stable `StoredHttpVersion`.
21. Added recorded-header admission checks for valid names, strict Base64 decoding, and mandatory redaction of managed-sensitive headers.
22. Added opaque `HttpLogicalRequestKey` and `HttpLogicalRequestKeyError`, then applied the type through requests, finalized requests, progress, and recorded attempts.
23. Replaced raw transaction-id admission with canonical validated `HttpTransactionIdentity::from_validated`.
24. Added `RecordedTransaction::transport_failure()` and `RecordedTransaction::response_status()` convenience APIs to avoid fabricating response semantics.
25. Extended recorder errors with clock, body-streaming, managed-path inspection, unsupported publication, and allocation exhaustion coverage.
26. Updated response transport-failure metadata to use stable typed failure classes instead of freeform strings.
27. Added redirect-location encoding tracking so redirect failures distinguish missing versus invalid Location headers without exposing values.
28. Preserved finalized transaction ownership and metadata consistency through retry, redirect, and progress publication sequencing.
29. Expanded top-level HTTP re-exports to include the new typed request key, attempt identity, admission, streaming, clock, retry, redirect, and transport-failure errors.
30. Left CLI/runtime behavior, capability advertising, session lifecycle, and non-HTTP protocol surfaces unchanged.
