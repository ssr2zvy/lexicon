use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use sha2::{Digest, Sha256};

use super::error::{HttpRecorderError, PostRenameSyncFailure};
use super::identity::HttpTransactionIdentity;
use super::metadata::{
    HTTP_TRANSACTION_SCHEMA_VERSION, RequestMetadataDocument, ResponseMetadataDocument,
    ResponseOutcomeDocument, StoredHeader, StoredHeaderValue,
};
use super::{
    FinalizedRecordedAttempt, HttpRecordedOutcome, RecordedHeader, RecordedHeaderCollection,
    RecordedHeaderValue, RecordedHttpRequest, RecordedHttpResponse, RecordedTransaction,
    RecordedTransportFailure,
};
use crate::protocols::http::request::{FinalizedHttpRequest, redact_url};
use crate::protocols::http::transport::HttpTransport;

pub(crate) struct RecordedAttemptContext {
    pub(crate) session_id: String,
    pub(crate) raw_data_root: PathBuf,
    pub(crate) logical_request_key: Option<String>,
    pub(crate) parent_transaction_id: Option<String>,
    pub(crate) physical_attempt_index: u32,
    pub(crate) redirect_index: u32,
    pub(crate) retry_index: u32,
    pub(crate) sensitive_query_names: HashSet<String>,
}

/// Record a single physical HTTP exchange.
///
/// Returns a [`FinalizedRecordedAttempt`] whose staging directory has been atomically
/// renamed and whose raw-data parent has been synced.
///
/// # Recording sequence
///
/// 1. Validate root (reject symlinks in any ancestor component).
/// 2. Exclusively create staging directory.
/// 3. Verify final directory does not exist.
/// 4. Compute request-body SHA-256 and length.
/// 5. Persist redacted request metadata.
/// 6. Persist exact request-body bytes.
/// 7. Sync request directory.
/// 8. Perform exactly one physical HTTP exchange.
/// 9. Stream or write response body.
/// 10. Persist response metadata (only after body streaming succeeds).
/// 11. Sync staging directory.
/// 12. Atomically rename staging → final.
/// 13. Sync raw-data parent (typed partial-commit error on failure).
/// 14. Construct `FinalizedRecordedAttempt` with final-directory paths.
pub(crate) fn record_transaction_attempt(
    attempt: RecordedAttemptContext,
    request: &FinalizedHttpRequest,
    transport: &dyn HttpTransport,
) -> Result<FinalizedRecordedAttempt, HttpRecorderError> {
    // Step 1: validate root path.
    validate_root(&attempt.raw_data_root)?;

    let now = now_nanos();
    let identity = HttpTransactionIdentity::new();
    let staging_dir = attempt.raw_data_root.join(format!(".partial-{}-{}", now, identity.id()));
    let final_dir = attempt.raw_data_root.join(format!("{}-{}", now, identity.id()));

    // Step 3: verify final destination is clear (immutability).
    if final_dir.exists() {
        return Err(HttpRecorderError::FinalPublicationCollision);
    }

    // Step 2: exclusively create staging directory.
    fs::create_dir(&staging_dir).map_err(HttpRecorderError::ExclusiveStagingCreation)?;

    let request_dir = staging_dir.join("request");
    let response_dir = staging_dir.join("response");
    fs::create_dir_all(&request_dir).map_err(HttpRecorderError::DirectoryCreation)?;
    fs::create_dir_all(&response_dir).map_err(HttpRecorderError::DirectoryCreation)?;

    // Step 4: compute request-body hash and length.
    let request_body_bytes: Vec<u8> = request.body.clone().unwrap_or_default();
    let has_body = request.body.is_some();
    let request_body_sha: Option<String> = if has_body {
        Some(hex_sha256(&request_body_bytes))
    } else {
        None
    };

    // Step 5: persist redacted request metadata FIRST.
    let request_metadata = RequestMetadataDocument {
        schema_version: HTTP_TRANSACTION_SCHEMA_VERSION,
        transaction_id: identity.id().to_string(),
        session_id: attempt.session_id.clone(),
        physical_attempt_index: attempt.physical_attempt_index,
        redirect_index: attempt.redirect_index,
        retry_index: attempt.retry_index,
        parent_transaction_id: attempt.parent_transaction_id.clone(),
        logical_request_key: attempt.logical_request_key.clone(),
        method: request.method.clone(),
        url: redact_url(&request.url, &attempt.sensitive_query_names),
        headers: redact_request_headers(&request.headers),
        has_body,
        body_length: request_body_bytes.len() as u64,
        body_sha256: request_body_sha.clone(),
        created_at_unix_nanos: now,
    };
    write_json_atomic(&request_dir.join("metadata.json"), &request_metadata)
        .map_err(HttpRecorderError::MetadataPersistence)?;

    // Step 6: persist exact request-body bytes.
    let staging_request_body = request_dir.join("body");
    if has_body {
        persist_body(&staging_request_body, &request_body_bytes)
            .map_err(HttpRecorderError::BodyPersistence)?;
    }

    // Step 7: sync request directory.
    sync_directory(&request_dir).map_err(HttpRecorderError::DurableSync)?;

    // Step 8: perform exactly one physical exchange.
    let staging_response_body = response_dir.join("body");
    let transport_result = transport.execute(request);

    // Steps 9–10: record outcome.
    let (response_status, response_headers_stored, body_length, body_sha256_opt, outcome,
         effective_location, transport_failure_opt) = match transport_result {
        Ok(mut response) => {
            let location = response.location_header.clone();

            // Step 9: stream response body.
            let (blen, bsha) = stream_body(&mut response.body, &staging_response_body)?;

            // Step 10: persist response metadata AFTER body streaming.
            let headers = redact_response_headers(&response.headers);
            let response_metadata = ResponseMetadataDocument {
                schema_version: HTTP_TRANSACTION_SCHEMA_VERSION,
                transaction_id: identity.id().to_string(),
                outcome: ResponseOutcomeDocument::Response {
                    status: response.status,
                    http_version: response.version,
                    headers: headers.clone(),
                    body_length: blen,
                    body_sha256: bsha.clone(),
                    completed_at_unix_nanos: now_nanos(),
                },
            };
            write_json_atomic(&response_dir.join("metadata.json"), &response_metadata)
                .map_err(HttpRecorderError::MetadataPersistence)?;

            (Some(response.status), headers, blen, Some(bsha),
             HttpRecordedOutcome::Response, location, None)
        }
        Err(failure) => {
            // Step 9: persist empty response body; error must not be discarded.
            persist_body(&staging_response_body, &[])
                .map_err(HttpRecorderError::BodyPersistence)?;

            let retryable = failure.retryable();
            let failure_class = failure.stable_class().to_string();
            let response_metadata = ResponseMetadataDocument {
                schema_version: HTTP_TRANSACTION_SCHEMA_VERSION,
                transaction_id: identity.id().to_string(),
                outcome: ResponseOutcomeDocument::TransportFailure {
                    failure_class: failure_class.clone(),
                    retryable,
                    failed_at_unix_nanos: now_nanos(),
                },
            };
            write_json_atomic(&response_dir.join("metadata.json"), &response_metadata)
                .map_err(HttpRecorderError::MetadataPersistence)?;

            let recorded_failure = RecordedTransportFailure::new(failure);
            let outcome = HttpRecordedOutcome::TransportFailure(recorded_failure);
            (None, Vec::new(), 0u64, None, outcome, None, Some(failure))
        }
    };

    // Step 11: sync staging directory.
    sync_directory(&staging_dir).map_err(HttpRecorderError::DurableSync)?;

    // Step 12: atomically rename staging → final.
    fs::rename(&staging_dir, &final_dir).map_err(HttpRecorderError::AtomicFinalize)?;

    // Step 13: sync raw-data parent (typed error on failure).
    sync_directory(&attempt.raw_data_root).map_err(|cause| {
        HttpRecorderError::PostRenameSyncFailed(PostRenameSyncFailure {
            transaction_id: identity.id().to_string(),
            final_path: final_dir.clone(),
            cause,
        })
    })?;

    // Step 14: construct with final-directory paths.
    let final_request_body = if has_body { Some(final_dir.join("request/body")) } else { None };
    let final_response_body = final_dir.join("response/body");

    let recorded_response = RecordedHttpResponse::new(
        response_status,
        RecordedHeaderCollection::new(
            response_headers_stored.into_iter().map(stored_to_recorded_header).collect(),
        ),
        final_response_body,
        body_length,
        body_sha256_opt,
        outcome,
    );

    let transaction = RecordedTransaction::new(
        identity,
        final_dir,
        RecordedHttpRequest::new(
            final_request_body,
            request_body_bytes.len() as u64,
            request_body_sha,
        ),
        recorded_response,
    );

    Ok(FinalizedRecordedAttempt {
        transaction,
        effective_location,
        transport_failure: transport_failure_opt,
    })
}

// ---------------------------------------------------------------------------
// Path validation
// ---------------------------------------------------------------------------

fn validate_root(root: &Path) -> Result<(), HttpRecorderError> {
    if root.is_relative() {
        return Err(HttpRecorderError::InvalidManagedRoot);
    }
    validate_no_symlink_components(root)?;
    fs::create_dir_all(root).map_err(HttpRecorderError::DirectoryCreation)?;
    Ok(())
}

fn validate_no_symlink_components(path: &Path) -> Result<(), HttpRecorderError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        if let Ok(meta) = current.symlink_metadata() {
            if meta.file_type().is_symlink() {
                return Err(HttpRecorderError::SymlinkRejected { path: current.clone() });
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Header helpers
// ---------------------------------------------------------------------------

fn redact_request_headers(headers: &[crate::protocols::http::request::FinalizedHeader]) -> Vec<StoredHeader> {
    headers
        .iter()
        .map(|header| {
            let lower = header.name.to_ascii_lowercase();
            let redact = header.sensitive
                || matches!(lower.as_str(), "authorization" | "proxy-authorization" | "cookie");
            StoredHeader {
                name: header.name.clone(),
                value: if redact {
                    StoredHeaderValue::Utf8("<redacted>".to_string())
                } else {
                    bytes_to_stored_value(&header.value)
                },
            }
        })
        .collect()
}

fn redact_response_headers(headers: &[(String, Vec<u8>)]) -> Vec<StoredHeader> {
    headers
        .iter()
        .map(|(name, value)| {
            let lower = name.to_ascii_lowercase();
            let redact = matches!(lower.as_str(), "set-cookie");
            StoredHeader {
                name: name.clone(),
                value: if redact {
                    StoredHeaderValue::Utf8("<redacted>".to_string())
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
    };
    RecordedHeader::new(header.name, value)
}

// ---------------------------------------------------------------------------
// I/O helpers
// ---------------------------------------------------------------------------

fn stream_body(reader: &mut dyn Read, path: &Path) -> Result<(u64, String), HttpRecorderError> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(HttpRecorderError::BodyPersistence)?;

    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 8192];
    loop {
        let n = reader.read(&mut buffer).map_err(HttpRecorderError::BodyStreaming)?;
        if n == 0 { break; }
        file.write_all(&buffer[..n]).map_err(HttpRecorderError::BodyPersistence)?;
        hash.update(&buffer[..n]);
        total += n as u64;
    }
    file.sync_all().map_err(HttpRecorderError::DurableSync)?;
    Ok((total, format!("{:x}", hash.finalize())))
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

fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), std::io::Error> {
    let bytes = serde_json::to_vec(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent"))?;

    let temp_file = tempfile::Builder::new()
        .prefix(".metadata-")
        .suffix(".tmp")
        .tempfile_in(parent)?;

    let (mut file, temp_path) = temp_file.into_parts();
    file.write_all(&bytes)?;
    file.sync_all()?;
    temp_path.persist(path).map_err(|e| e.error)?;
    sync_directory(parent)?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    File::open(path)?.sync_all()
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    format!("{:x}", hash.finalize())
}

/// Current time as nanoseconds since the Unix epoch, truncated to u64.
fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX)
}
