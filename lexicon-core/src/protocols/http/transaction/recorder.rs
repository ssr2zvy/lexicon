use std::collections::HashSet;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use sha2::{Digest, Sha256};

use super::error::{
    HttpBodyStreamingError, HttpClockError, HttpIncompleteMarkerError, HttpManagedPathValidationMode,
    HttpMetadataPersistenceError, HttpRecorderError, HttpTransactionIdentityAllocationError,
    HttpTransactionPublicationError, IncompleteHttpResponseFailure, PostRenameSyncFailure,
    validate_managed_path,
};
use super::identity::HttpTransactionIdentity;
use super::metadata::{
    HTTP_TRANSACTION_SCHEMA_VERSION, RequestMetadataDocument, ResponseMetadataDocument,
    ResponseOutcomeDocument, StoredHeader, StoredHeaderValue, StoredTransportFailureClass,
};
use super::{
    FinalizedRecordedAttempt, HttpAttemptIdentity, HttpLogicalRequestKey, HttpRecordedOutcome,
    RecordedHeader, RecordedHeaderValue, RecordedHttpRequest, RecordedHttpResponse,
    RecordedTransaction, RecordedTransportFailure,
};
use crate::protocols::http::request::{FinalizedHeader, FinalizedHttpRequest, redact_url};
use crate::protocols::http::transport::{HttpLocationHeader, HttpTransport};

const MAX_STAGING_IDENTITY_ATTEMPTS: usize = 8;

/// Test hook storage for the HTTP-03 durability observer. Tests install a
/// `DurabilityPublisherHooks` to receive a sequence of side-effect events
/// the recorder emits inside `record_transaction_attempt`. Production
/// callers never install a hook, so the cell stays empty and notifications
/// short-circuit to nothing.
#[cfg(test)]
static DURABILITY_HOOKS: OnceLock<
    crate::protocols::http::test_support::DurabilityPublisherHooks,
> = OnceLock::new();

/// Test-only install entrypoint. Calling this with a non-default hooks
/// value drives the side-effect observer in the recorder.
#[cfg(test)]
pub(crate) fn install_durability_hooks(
    hooks: crate::protocols::http::test_support::DurabilityPublisherHooks,
) {
    let _ = DURABILITY_HOOKS.set(hooks);
}

#[cfg(test)]
fn notify_durability(
    kind: crate::protocols::http::test_support::DurabilityEventKind,
) {
    if let Some(hooks) = DURABILITY_HOOKS.get() {
        hooks.notify(kind);
    }
}

/// Prefix used by the recorder for in-progress (staging) transaction directories.
///
/// This module owns the staging-name grammar. Every other Core component that has
/// to recognize a staging directory must use
/// [`classify_staging_transaction_directory_name`] rather than re-deriving the rule.
pub(crate) const PARTIAL_TRANSACTION_DIRECTORY_PREFIX: &str = ".partial-";

/// Build the Core-authored staging directory name for a transaction attempt.
pub(crate) fn staging_transaction_directory_name(timestamp: u64, transaction_id: &str) -> String {
    format!("{PARTIAL_TRANSACTION_DIRECTORY_PREFIX}{timestamp}-{transaction_id}")
}

/// Build the Core-authored finalized directory name for a transaction attempt.
pub(crate) fn finalized_transaction_directory_name(timestamp: u64, transaction_id: &str) -> String {
    format!("{timestamp}-{transaction_id}")
}

/// Classification of a raw-root directory name against the staging grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StagingDirectoryNameClass<'a> {
    /// The name does not use the Core staging prefix at all.
    NotStaging,
    /// The name is exactly a Core-authored staging directory name.
    Valid {
        timestamp: u64,
        transaction_id: &'a str,
    },
    /// The name uses the Core staging prefix but violates the recorder grammar.
    Malformed,
}

/// Classify a raw-root directory name against the exact recorder staging grammar.
///
/// The grammar is `.partial-<timestamp>-<transaction-id>` where `<timestamp>` is a
/// nonzero ASCII-decimal `u64` and `<transaction-id>` is a valid HTTP transaction
/// identity. Anything that carries the prefix but violates the grammar is reported
/// as [`StagingDirectoryNameClass::Malformed`] so callers can reject it instead of
/// silently ignoring an arbitrary directory.
pub(crate) fn classify_staging_transaction_directory_name(
    name: &str,
) -> StagingDirectoryNameClass<'_> {
    let Some(remainder) = name.strip_prefix(PARTIAL_TRANSACTION_DIRECTORY_PREFIX) else {
        return StagingDirectoryNameClass::NotStaging;
    };

    let Some((timestamp, transaction_id)) = remainder.split_once('-') else {
        return StagingDirectoryNameClass::Malformed;
    };

    if timestamp.is_empty() || !timestamp.bytes().all(|byte| byte.is_ascii_digit()) {
        return StagingDirectoryNameClass::Malformed;
    }

    let Ok(parsed_timestamp) = timestamp.parse::<u64>() else {
        return StagingDirectoryNameClass::Malformed;
    };

    if parsed_timestamp == 0 {
        return StagingDirectoryNameClass::Malformed;
    }

    if HttpTransactionIdentity::from_validated(transaction_id).is_err() {
        return StagingDirectoryNameClass::Malformed;
    }

    StagingDirectoryNameClass::Valid {
        timestamp: parsed_timestamp,
        transaction_id,
    }
}

pub(crate) struct RecordedAttemptContext {
    pub(crate) session_id: String,
    pub(crate) raw_data_root: PathBuf,
    pub(crate) logical_request_key: Option<HttpLogicalRequestKey>,
    pub(crate) parent_transaction_id: Option<HttpTransactionIdentity>,
    pub(crate) attempt_identity: HttpAttemptIdentity,
    pub(crate) sensitive_query_names: HashSet<String>,
    /// Header names the source marked as explicitly sensitive at request
    /// construction. Propagates into both the request and response decisions
    /// in [`crate::protocols::http::sensitivity::must_redact_header`].
    pub(crate) sensitive_header_names: HashSet<String>,
}

pub(crate) fn record_transaction_attempt(
    attempt: RecordedAttemptContext,
    request: &FinalizedHttpRequest,
    transport: &dyn HttpTransport,
) -> Result<FinalizedRecordedAttempt, HttpRecorderError> {
    ensure_directory(&attempt.raw_data_root)?;

    let mut allocated = None;
    for _ in 0..MAX_STAGING_IDENTITY_ATTEMPTS {
        let timestamp = now_nanos()
            .map_err(|error| HttpRecorderError::IdentityAllocation(HttpTransactionIdentityAllocationError::Clock(error)))?;
        let identity = HttpTransactionIdentity::new().map_err(|error| {
            HttpRecorderError::IdentityAllocation(HttpTransactionIdentityAllocationError::Identity(
                error,
            ))
        })?;
        let staging_dir = attempt
            .raw_data_root
            .join(staging_transaction_directory_name(timestamp, identity.id()));
        let final_dir = attempt
            .raw_data_root
            .join(finalized_transaction_directory_name(timestamp, identity.id()));

        validate_managed_path(
            &attempt.raw_data_root,
            &staging_dir,
            HttpManagedPathValidationMode::CreatableDirectory,
        )
        .map_err(HttpRecorderError::ManagedPath)?;
        validate_managed_path(
            &attempt.raw_data_root,
            &final_dir,
            HttpManagedPathValidationMode::CreatableDirectory,
        )
        .map_err(HttpRecorderError::ManagedPath)?;
        match fs::create_dir(&staging_dir) {
            Ok(()) => {
                validate_managed_path(
                    &attempt.raw_data_root,
                    &staging_dir,
                    HttpManagedPathValidationMode::ExistingDirectory,
                )
                .map_err(HttpRecorderError::ManagedPath)?;
                allocated = Some((timestamp, identity, staging_dir, final_dir));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(HttpRecorderError::ExclusiveStagingCreation(error)),
        }
    }

    let (timestamp, identity, staging_dir, final_dir) = allocated.ok_or_else(|| {
        HttpRecorderError::IdentityAllocation(HttpTransactionIdentityAllocationError::Exhausted)
    })?;

    let request_dir = staging_dir.join("request");
    let response_dir = staging_dir.join("response");
    fs::create_dir_all(&request_dir).map_err(HttpRecorderError::DirectoryCreation)?;
    fs::create_dir_all(&response_dir).map_err(HttpRecorderError::DirectoryCreation)?;
    validate_managed_path(
        &attempt.raw_data_root,
        &request_dir,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(HttpRecorderError::ManagedPath)?;
    validate_managed_path(
        &attempt.raw_data_root,
        &response_dir,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(HttpRecorderError::ManagedPath)?;

    let request_metadata_path = request_dir.join("metadata.json");
    let request_body_path = request_dir.join("body");
    let response_metadata_path = response_dir.join("metadata.json");
    let response_body_path = response_dir.join("body");
    for path in [
        &request_metadata_path,
        &response_metadata_path,
        &request_body_path,
        &response_body_path,
    ] {
        validate_managed_path(
            &attempt.raw_data_root,
            path,
            HttpManagedPathValidationMode::CreatableRegularFile,
        )
        .map_err(HttpRecorderError::ManagedPath)?;
    }

    let request_body_bytes = request.body.clone().unwrap_or_default();
    let has_body = request.body.is_some();
    let request_body_sha = has_body.then(|| hex_sha256(&request_body_bytes));

    let request_metadata = RequestMetadataDocument {
        schema_version: HTTP_TRANSACTION_SCHEMA_VERSION,
        transaction_id: identity.id().to_string(),
        session_id: attempt.session_id.clone(),
        physical_attempt_index: attempt.attempt_identity.physical_attempt_index(),
        redirect_index: attempt.attempt_identity.redirect_index(),
        retry_index: attempt.attempt_identity.retry_index(),
        parent_transaction_id: attempt
            .parent_transaction_id
            .as_ref()
            .map(|parent| parent.id().to_string()),
        logical_request_key: attempt
            .logical_request_key
            .as_ref()
            .map(|key| key.as_str().to_string()),
        method: request.method.clone(),
        url: redact_url(&request.url, &attempt.sensitive_query_names),
        headers: redact_request_headers(&request.headers, &attempt.sensitive_header_names),
        has_body,
        body_length: request_body_bytes.len() as u64,
        body_sha256: request_body_sha.clone(),
        created_at_unix_nanos: timestamp,
    };
    write_json_atomic(&attempt.raw_data_root, &request_metadata_path, &request_metadata)?;
    #[cfg(test)]
    notify_durability(
        crate::protocols::http::test_support::DurabilityEventKind::RequestMetadataSynced,
    );

    if has_body {
        persist_body(&request_body_path, &request_body_bytes).map_err(HttpRecorderError::BodyPersistence)?;
        #[cfg(test)]
        notify_durability(
            crate::protocols::http::test_support::DurabilityEventKind::RequestBodySynced,
        );
    }

    sync_directory(&request_dir).map_err(HttpRecorderError::DurableSync)?;

    let transport_result = transport.execute(request);
    let (
        response_status,
        response_headers,
        response_body_length,
        response_body_sha256,
        response_completed_at_unix_nanos,
        outcome,
        location_text,
        invalid_location_encoding,
        transport_failure,
    ) = match transport_result {
        Ok(mut response) => {
            let location_state = response.location_header;
            let body_stream = stream_body(&mut response.body, &response_body_path);
            let (body_length, body_sha256) = match body_stream {
                Ok(ok) => (ok.total, ok.sha256),
                Err(stream_failure) => {
                    let StreamBodyFailure {
                        error,
                        bytes_recorded,
                        partial_body_sha256,
                        partial_body_sync_error,
                    } = stream_failure;
                    let marker_error = persist_incomplete_response_marker(
                        &attempt.raw_data_root,
                        &response_metadata_path,
                        identity.id(),
                        bytes_recorded,
                        partial_body_sha256.as_deref(),
                        &error,
                    )
                    .err();
                    return Err(HttpRecorderError::IncompleteResponseStreamingFailed(
                        IncompleteHttpResponseFailure {
                            stream_error: error,
                            partial_body_sync_error,
                            marker_error,
                            bytes_recorded,
                            partial_body_sha256,
                        },
                    ));
                }
            };

            let headers = redact_response_headers(&response.headers, &attempt.sensitive_header_names);
            let completed_at_unix_nanos = now_nanos().map_err(HttpRecorderError::Clock)?;
            let response_metadata = ResponseMetadataDocument {
                schema_version: HTTP_TRANSACTION_SCHEMA_VERSION,
                transaction_id: identity.id().to_string(),
                outcome: ResponseOutcomeDocument::Response {
                    status: response.status,
                    http_version: response.version,
                    headers: headers.clone(),
                    body_length,
                    body_sha256: body_sha256.clone(),
                    completed_at_unix_nanos,
                },
            };
            write_json_atomic(&attempt.raw_data_root, &response_metadata_path, &response_metadata)?;
            #[cfg(test)]
            notify_durability(
                crate::protocols::http::test_support::DurabilityEventKind::ResponseBodySynced,
            );
            notify_durability(
                crate::protocols::http::test_support::DurabilityEventKind::ResponseMetadataSynced,
            );

            let (location_text, invalid_location_encoding) = match location_state {
                HttpLocationHeader::Missing => (None, false),
                HttpLocationHeader::InvalidEncoding => (None, true),
                HttpLocationHeader::Present(text) => (Some(text), false),
            };

            (
                Some(response.status),
                headers,
                body_length,
                Some(body_sha256),
                completed_at_unix_nanos,
                HttpRecordedOutcome::Response,
                location_text,
                invalid_location_encoding,
                None,
            )
        }
        Err(failure) => {
            persist_body(&response_body_path, &[])
                .map_err(HttpRecorderError::BodyPersistence)?;

            let failed_at_unix_nanos = now_nanos().map_err(HttpRecorderError::Clock)?;
            let response_metadata = ResponseMetadataDocument {
                schema_version: HTTP_TRANSACTION_SCHEMA_VERSION,
                transaction_id: identity.id().to_string(),
                outcome: ResponseOutcomeDocument::TransportFailure {
                    failure_class: StoredTransportFailureClass::from(failure),
                    retryable: failure.retryable(),
                    failed_at_unix_nanos,
                },
            };
            write_json_atomic(&attempt.raw_data_root, &response_metadata_path, &response_metadata)?;

            (
                None,
                Vec::new(),
                0,
                None,
                failed_at_unix_nanos,
                HttpRecordedOutcome::TransportFailure(RecordedTransportFailure::new(failure)),
                None,
                false,
                Some(failure),
            )
        }
    };

    sync_directory(&staging_dir).map_err(HttpRecorderError::DurableSync)?;
    publish_transaction_directory_no_replace(&staging_dir, &final_dir)
        .map_err(HttpRecorderError::Publication)?;
    #[cfg(test)]
    notify_durability(
        crate::protocols::http::test_support::DurabilityEventKind::DirectoryAtomicallyReplaced,
    );
    let final_request_dir = final_dir.join("request");
    let final_response_dir = final_dir.join("response");
    let final_request_metadata_path = final_request_dir.join("metadata.json");
    let final_request_body_path = final_request_dir.join("body");
    let final_response_metadata_path = final_response_dir.join("metadata.json");
    let final_response_body_path = final_response_dir.join("body");
    validate_managed_path(
        &attempt.raw_data_root,
        &final_dir,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(HttpRecorderError::ManagedPath)?;
    validate_managed_path(
        &attempt.raw_data_root,
        &final_request_dir,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(HttpRecorderError::ManagedPath)?;
    validate_managed_path(
        &attempt.raw_data_root,
        &final_response_dir,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(HttpRecorderError::ManagedPath)?;
    validate_managed_path(
        &attempt.raw_data_root,
        &final_request_metadata_path,
        HttpManagedPathValidationMode::ExistingRegularFile,
    )
    .map_err(HttpRecorderError::ManagedPath)?;
    validate_managed_path(
        &attempt.raw_data_root,
        &final_response_metadata_path,
        HttpManagedPathValidationMode::ExistingRegularFile,
    )
    .map_err(HttpRecorderError::ManagedPath)?;
    if has_body {
        validate_managed_path(
            &attempt.raw_data_root,
            &final_request_body_path,
            HttpManagedPathValidationMode::ExistingRegularFile,
        )
        .map_err(HttpRecorderError::ManagedPath)?;
    }
    validate_managed_path(
        &attempt.raw_data_root,
        &final_response_body_path,
        HttpManagedPathValidationMode::ExistingRegularFile,
    )
    .map_err(HttpRecorderError::ManagedPath)?;
    sync_published_parent(&attempt.raw_data_root).map_err(|cause| {
        HttpRecorderError::PostRenameSyncFailed(PostRenameSyncFailure {
            transaction_id: identity.id().to_string(),
            final_path: final_dir.clone(),
            cause,
        })
    })?;
    #[cfg(test)]
    notify_durability(
        crate::protocols::http::test_support::DurabilityEventKind::ParentDirectorySynced,
    );

    let transaction = RecordedTransaction::new(
        identity,
        attempt.attempt_identity,
        attempt.parent_transaction_id,
        attempt.logical_request_key,
        crate::session::SessionIdentity::new(&attempt.session_id)
            .expect("RecordedAttemptContext must contain a valid session identity"),
        timestamp,
        final_dir.clone(),
        RecordedHttpRequest::new(
            has_body.then(|| final_request_body_path),
            request_body_bytes.len() as u64,
            request_body_sha,
        ),
        RecordedHttpResponse::new(
            response_status,
            super::RecordedHeaderCollection::new(
                response_headers
                    .into_iter()
                    .map(stored_to_recorded_header)
                    .collect(),
            ),
            final_response_body_path,
            response_body_length,
            response_body_sha256,
            response_completed_at_unix_nanos,
            outcome,
        ),
    );

    Ok(FinalizedRecordedAttempt {
        transaction,
        attempt_identity: attempt.attempt_identity,
        effective_location: location_text,
        invalid_location_encoding,
        transport_failure,
    })
}

fn ensure_directory(path: &Path) -> Result<(), HttpRecorderError> {
    if path.is_relative() {
        return Err(HttpRecorderError::ManagedPath(
            super::error::HttpManagedPathError::RelativePath {
                path: path.to_path_buf(),
            },
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        HttpRecorderError::ManagedPath(super::error::HttpManagedPathError::RelativePath {
            path: path.to_path_buf(),
        })
    })?;
    validate_managed_path(parent, parent, HttpManagedPathValidationMode::ExistingDirectory)
        .map_err(HttpRecorderError::ManagedPath)?;
    validate_managed_path(parent, path, HttpManagedPathValidationMode::CreatableDirectory)
        .map_err(HttpRecorderError::ManagedPath)?;
    fs::create_dir_all(path).map_err(HttpRecorderError::DirectoryCreation)?;
    validate_managed_path(path, path, HttpManagedPathValidationMode::ExistingDirectory)
        .map_err(HttpRecorderError::ManagedPath)
}

fn redact_request_headers(
    headers: &[FinalizedHeader],
    explicit_sensitive: &HashSet<String>,
) -> Vec<StoredHeader> {
    headers
        .iter()
        .map(|header| {
            // The single shared policy: a header is redacted iff its name falls
            // inside the mandatory set, or the source marked the same name as
            // sensitive. `header.sensitive` is already captured by the
            // explicit set that came from `FinalizedHttpRequest`; the explicit
            // set keeps the decision in one place for both directions.
            let redact =
                crate::protocols::http::sensitivity::must_redact_header(&header.name, explicit_sensitive);
            StoredHeader {
                name: header.name.clone(),
                value: if redact {
                    StoredHeaderValue::Redacted
                } else {
                    bytes_to_stored_value(&header.value)
                },
            }
        })
        .collect()
}

fn redact_response_headers(
    headers: &[(String, Vec<u8>)],
    explicit_sensitive: &HashSet<String>,
) -> Vec<StoredHeader> {
    // Same shared policy as request direction. Carrying the explicit set
    // forward means a source that marks `X-Api-Key` as sensitive on the
    // request sees the same name redacted in the response.
    headers
        .iter()
        .map(|(name, value)| {
            let redact =
                crate::protocols::http::sensitivity::must_redact_header(name, explicit_sensitive);
            StoredHeader {
                name: name.clone(),
                value: if redact {
                    StoredHeaderValue::Redacted
                } else {
                    bytes_to_stored_value(value)
                },
            }
        })
        .collect()
}

fn bytes_to_stored_value(value: &[u8]) -> StoredHeaderValue {
    match std::str::from_utf8(value) {
        Ok(text) => StoredHeaderValue::Utf8(text.to_string()),
        Err(_) => StoredHeaderValue::Base64(base64::engine::general_purpose::STANDARD.encode(value)),
    }
}

fn stored_to_recorded_header(header: StoredHeader) -> RecordedHeader {
    let value = match header.value {
        StoredHeaderValue::Utf8(text) => RecordedHeaderValue::Utf8(text),
        StoredHeaderValue::Base64(encoded) => RecordedHeaderValue::Base64(encoded),
        StoredHeaderValue::Redacted => RecordedHeaderValue::Redacted,
    };
    RecordedHeader::new(header.name, value)
}

struct StreamBodySuccess {
    total: u64,
    sha256: String,
}

struct StreamBodyFailure {
    error: HttpBodyStreamingError,
    bytes_recorded: u64,
    partial_body_sha256: Option<String>,
    partial_body_sync_error: Option<std::io::Error>,
}

fn stream_body(reader: &mut dyn Read, path: &Path) -> Result<StreamBodySuccess, StreamBodyFailure> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|error| StreamBodyFailure {
            error: HttpBodyStreamingError::Io(error),
            bytes_recorded: 0,
            partial_body_sha256: None,
            partial_body_sync_error: None,
        })?;

    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 8192];

    loop {
        let n = match reader.read(&mut buffer) {
            Ok(n) => n,
            Err(error) => {
                let partial_sync = file.sync_all().err();
                return Err(StreamBodyFailure {
                    error: HttpBodyStreamingError::Io(error),
                    bytes_recorded: total,
                    partial_body_sha256: (total > 0).then(|| format!("{:x}", hash.clone().finalize())),
                    partial_body_sync_error: partial_sync,
                });
            }
        };
        if n == 0 {
            break;
        }
        let next_total = total.checked_add(n as u64).ok_or_else(|| {
            let partial_sync = file.sync_all().err();
            StreamBodyFailure {
                error: HttpBodyStreamingError::LengthOverflow,
                bytes_recorded: total,
                partial_body_sha256: (total > 0).then(|| format!("{:x}", hash.clone().finalize())),
                partial_body_sync_error: partial_sync,
            }
        })?;
        if let Err(error) = file.write_all(&buffer[..n]) {
            let partial_sync = file.sync_all().err();
            return Err(StreamBodyFailure {
                error: HttpBodyStreamingError::Io(error),
                bytes_recorded: total,
                partial_body_sha256: (total > 0).then(|| format!("{:x}", hash.clone().finalize())),
                partial_body_sync_error: partial_sync,
            });
        }
        hash.update(&buffer[..n]);
        total = next_total;
    }

    file.sync_all().map_err(|error| StreamBodyFailure {
        error: HttpBodyStreamingError::Io(error),
        bytes_recorded: total,
        partial_body_sha256: (total > 0).then(|| format!("{:x}", hash.clone().finalize())),
        partial_body_sync_error: None,
    })?;

    Ok(StreamBodySuccess {
        total,
        sha256: format!("{:x}", hash.finalize()),
    })
}

fn persist_incomplete_response_marker(
    trusted_root: &Path,
    metadata_path: &Path,
    transaction_id: &str,
    body_bytes_recorded: u64,
    partial_body_sha256: Option<&str>,
    stream_error: &HttpBodyStreamingError,
) -> Result<(), HttpIncompleteMarkerError> {
    let failed_at_unix_nanos = now_nanos().map_err(HttpIncompleteMarkerError::Clock)?;
    let document = ResponseMetadataDocument {
        schema_version: HTTP_TRANSACTION_SCHEMA_VERSION,
        transaction_id: transaction_id.to_string(),
        outcome: ResponseOutcomeDocument::IncompleteResponse {
            failure_class: stream_error.stable_class().to_string(),
            body_bytes_recorded,
            partial_body_sha256: partial_body_sha256.map(ToOwned::to_owned),
            failed_at_unix_nanos,
        },
    };
    let bytes = serde_json::to_vec(&document).map_err(HttpIncompleteMarkerError::MetadataEncoding)?;
    let parent = metadata_path.parent().ok_or_else(|| {
        HttpIncompleteMarkerError::ManagedPath(super::error::HttpManagedPathError::RelativePath {
            path: metadata_path.to_path_buf(),
        })
    })?;
    validate_managed_path(
        trusted_root,
        parent,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(HttpIncompleteMarkerError::ManagedPath)?;
    validate_managed_path(
        trusted_root,
        metadata_path,
        HttpManagedPathValidationMode::CreatableRegularFile,
    )
    .map_err(HttpIncompleteMarkerError::ManagedPath)?;

    let temp_file = tempfile::Builder::new()
        .prefix(".metadata-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(HttpIncompleteMarkerError::TemporaryFile)?;

    let (mut file, temp_path) = temp_file.into_parts();
    file.write_all(&bytes)
        .map_err(HttpIncompleteMarkerError::MetadataWrite)?;
    file.sync_all()
        .map_err(HttpIncompleteMarkerError::MetadataFileSync)?;
    temp_path
        .persist(metadata_path)
        .map_err(|error| HttpIncompleteMarkerError::AtomicMarkerPublication(error.error))?;
    sync_directory(parent).map_err(HttpIncompleteMarkerError::ResponseDirectorySync)?;
    Ok(())
}

fn persist_body(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn write_json_atomic<T: serde::Serialize>(
    trusted_root: &Path,
    path: &Path,
    value: &T,
) -> Result<(), HttpRecorderError> {
    let bytes = serde_json::to_vec(value).map_err(HttpRecorderError::MetadataEncoding)?;
    write_json_bytes_atomic(trusted_root, path, &bytes).map_err(HttpRecorderError::MetadataPersistence)
}

fn write_json_bytes_atomic(
    trusted_root: &Path,
    path: &Path,
    bytes: &[u8],
) -> Result<(), HttpMetadataPersistenceError> {
    let parent = path.parent().ok_or_else(|| {
        HttpMetadataPersistenceError::ManagedPath(super::error::HttpManagedPathError::RelativePath {
            path: path.to_path_buf(),
        })
    })?;
    validate_managed_path(
        trusted_root,
        parent,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(HttpMetadataPersistenceError::ManagedPath)?;
    validate_managed_path(
        trusted_root,
        path,
        HttpManagedPathValidationMode::CreatableRegularFile,
    )
    .map_err(HttpMetadataPersistenceError::ManagedPath)?;

    let temp_file = tempfile::Builder::new()
        .prefix(".metadata-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(HttpMetadataPersistenceError::TemporaryFile)?;

    let (mut file, temp_path) = temp_file.into_parts();
    file.write_all(bytes)
        .map_err(HttpMetadataPersistenceError::Write)?;
    file.sync_all().map_err(HttpMetadataPersistenceError::FileSync)?;
    temp_path
        .persist(path)
        .map_err(|error| HttpMetadataPersistenceError::Persist(error.error))?;
    sync_directory(parent).map_err(HttpMetadataPersistenceError::DirectorySync)?;
    Ok(())
}

fn sync_published_parent(path: &Path) -> Result<(), std::io::Error> {
    sync_directory(path)
}

/// Durably synchronize an existing regular file.
///
/// This is the shared Core durability boundary for managed regular files; callers
/// outside the recorder must use it instead of re-implementing per-platform syncing.
pub(crate) fn sync_regular_file(path: &Path) -> Result<(), std::io::Error> {
    OpenOptions::new().read(true).write(true).open(path)?.sync_all()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    File::open(path)?.sync_all()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(crate) fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    File::open(path)?.sync_all()
}

#[cfg(target_os = "windows")]
pub(crate) fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_GENERIC_READ, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, FlushFileBuffers, OPEN_EXISTING,
    };

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            0,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }
    let flush_ok = unsafe { FlushFileBuffers(handle) };
    let flush_error = if flush_ok == 0 {
        Some(std::io::Error::last_os_error())
    } else {
        None
    };
    unsafe {
        CloseHandle(handle);
    }
    if let Some(error) = flush_error {
        return Err(error);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn publish_transaction_directory_no_replace(
    staging_dir: &Path,
    final_dir: &Path,
) -> Result<(), HttpTransactionPublicationError> {
    use std::os::unix::ffi::OsStrExt;

    let staging = CString::new(staging_dir.as_os_str().as_bytes()).map_err(|_| {
        HttpTransactionPublicationError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "staging path contains interior NUL",
        ))
    })?;
    let final_path = CString::new(final_dir.as_os_str().as_bytes()).map_err(|_| {
        HttpTransactionPublicationError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "final path contains interior NUL",
        ))
    })?;

    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            staging.as_ptr(),
            libc::AT_FDCWD,
            final_path.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(code) if code == libc::EEXIST => Err(HttpTransactionPublicationError::Collision),
        Some(code)
            if code == libc::ENOSYS || code == libc::EOPNOTSUPP || code == libc::EINVAL =>
        {
            Err(HttpTransactionPublicationError::UnsupportedPlatform)
        }
        _ => Err(HttpTransactionPublicationError::Io(error)),
    }
}

#[cfg(target_os = "macos")]
fn publish_transaction_directory_no_replace(
    staging_dir: &Path,
    final_dir: &Path,
) -> Result<(), HttpTransactionPublicationError> {
    use std::os::unix::ffi::OsStrExt;

    let staging = CString::new(staging_dir.as_os_str().as_bytes()).map_err(|_| {
        HttpTransactionPublicationError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "staging path contains interior NUL",
        ))
    })?;
    let final_path = CString::new(final_dir.as_os_str().as_bytes()).map_err(|_| {
        HttpTransactionPublicationError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "final path contains interior NUL",
        ))
    })?;

    let result = unsafe { libc::renamex_np(staging.as_ptr(), final_path.as_ptr(), libc::RENAME_EXCL) };
    if result == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        Err(HttpTransactionPublicationError::Collision)
    } else {
        Err(HttpTransactionPublicationError::Io(error))
    }
}

#[cfg(target_os = "windows")]
fn publish_transaction_directory_no_replace(
    staging_dir: &Path,
    final_dir: &Path,
) -> Result<(), HttpTransactionPublicationError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS};
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};

    let staging: Vec<u16> = staging_dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let final_path: Vec<u16> = final_dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let ok = unsafe { MoveFileExW(staging.as_ptr(), final_path.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok != 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(code)
            if code == ERROR_ALREADY_EXISTS as i32 || code == ERROR_FILE_EXISTS as i32 =>
        {
            Err(HttpTransactionPublicationError::Collision)
        }
        _ => Err(HttpTransactionPublicationError::Io(error)),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn publish_transaction_directory_no_replace(
    _: &Path,
    _: &Path,
) -> Result<(), HttpTransactionPublicationError> {
    Err(HttpTransactionPublicationError::UnsupportedPlatform)
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    format!("{:x}", hash.finalize())
}

fn now_nanos() -> Result<u64, HttpClockError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| HttpClockError::BeforeEpoch)?;
    duration
        .as_nanos()
        .try_into()
        .map_err(|_| HttpClockError::OutOfRange)
}
