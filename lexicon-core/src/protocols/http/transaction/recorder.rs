use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use sha2::{Digest, Sha256};

use super::error::HttpRecorderError;
use super::identity::HttpTransactionIdentity;
use super::metadata::{
    HTTP_TRANSACTION_SCHEMA_VERSION, RequestMetadataDocument, ResponseMetadataDocument,
    ResponseOutcomeDocument, StoredHeader, StoredHeaderValue,
};
use super::{
    HttpRecordedOutcome, RecordedHeader, RecordedHeaderCollection, RecordedHttpRequest,
    RecordedHttpResponse, RecordedTransaction, RecordedTransportFailure,
};
use crate::protocols::http::request::{FinalizedHttpRequest, redact_url};
use crate::protocols::http::transport::{HttpTransport, HttpTransportFailure};

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

pub(crate) struct RecordedAttemptResult {
    pub(crate) transaction: RecordedTransaction,
    pub(crate) transport_failure: Option<RecordedTransportFailure>,
}

pub(crate) fn record_transaction_attempt(
    attempt: RecordedAttemptContext,
    request: &FinalizedHttpRequest,
    transport: &dyn HttpTransport,
) -> Result<RecordedAttemptResult, HttpRecorderError> {
    ensure_root(&attempt.raw_data_root)?;

    let timestamp = now_nanos();
    let identity = HttpTransactionIdentity::new();
    let staging_name = format!(".partial-{}-{}", timestamp, identity.id());
    let final_name = format!("{}-{}", timestamp, identity.id());

    let staging_directory = attempt.raw_data_root.join(staging_name);
    let final_directory = attempt.raw_data_root.join(final_name);
    if final_directory.exists() {
        return Err(HttpRecorderError::FinalPathCollision);
    }

    let request_dir = staging_directory.join("request");
    let response_dir = staging_directory.join("response");
    fs::create_dir_all(&request_dir).map_err(|_| HttpRecorderError::DirectoryCreation)?;
    fs::create_dir_all(&response_dir).map_err(|_| HttpRecorderError::DirectoryCreation)?;

    let request_body_path = request_dir.join("body");
    let request_body_bytes = request.body.clone().unwrap_or_default();
    if request.body.is_some() {
        persist_body(&request_body_path, &request_body_bytes)?;
    }

    let request_body_sha = if request.body.is_some() {
        Some(hex_sha256(&request_body_bytes))
    } else {
        None
    };

    let request_metadata = RequestMetadataDocument {
        schema_version: HTTP_TRANSACTION_SCHEMA_VERSION,
        transaction_id: identity.id().to_string(),
        session_id: attempt.session_id,
        physical_attempt_index: attempt.physical_attempt_index,
        redirect_index: attempt.redirect_index,
        retry_index: attempt.retry_index,
        parent_transaction_id: attempt.parent_transaction_id,
        logical_request_key: attempt.logical_request_key,
        method: request.method.clone(),
        url: redact_url(&request.url, &attempt.sensitive_query_names),
        headers: redact_request_headers(&request.headers),
        has_body: request.body.is_some(),
        body_length: request_body_bytes.len() as u64,
        body_sha256: request_body_sha.clone(),
        created_at: timestamp_rfc3339(),
    };

    write_json_atomic(&request_dir.join("metadata.json"), &request_metadata)
        .map_err(|_| HttpRecorderError::MetadataPersistence)?;

    let response_body_path = response_dir.join("body");
    let transport_result = transport.execute(request);

    let result = match transport_result {
        Ok(mut response) => {
            let (body_length, body_sha256) = stream_body(&mut response.body, &response_body_path)?;
            let headers = redact_response_headers(&response.headers);

            let response_metadata = ResponseMetadataDocument {
                schema_version: HTTP_TRANSACTION_SCHEMA_VERSION,
                transaction_id: identity.id().to_string(),
                outcome: ResponseOutcomeDocument::Response {
                    status: response.status,
                    http_version: response.version,
                    headers: headers.clone(),
                    body_length,
                    body_sha256: body_sha256.clone(),
                    completed_at: timestamp_rfc3339(),
                },
            };
            write_json_atomic(&response_dir.join("metadata.json"), &response_metadata)
                .map_err(|_| HttpRecorderError::MetadataPersistence)?;

            let transaction = RecordedTransaction::new(
                identity,
                final_directory.clone(),
                RecordedHttpRequest::new(
                    request.body.as_ref().map(|_| request_body_path.clone()),
                    request_body_bytes.len() as u64,
                    request_body_sha,
                ),
                RecordedHttpResponse::new(
                    Some(response.status),
                    RecordedHeaderCollection::new(
                        headers.into_iter().map(stored_to_recorded_header).collect(),
                    ),
                    final_directory.join("response/body"),
                    body_length,
                    Some(body_sha256),
                    HttpRecordedOutcome::Response,
                ),
            );

            RecordedAttemptResult {
                transaction,
                transport_failure: None,
            }
        }
        Err(failure) => {
            let _ = persist_body(&response_body_path, &[]);
            let failure_class = transport_failure_class(failure);
            let retryable = true;
            let response_metadata = ResponseMetadataDocument {
                schema_version: HTTP_TRANSACTION_SCHEMA_VERSION,
                transaction_id: identity.id().to_string(),
                outcome: ResponseOutcomeDocument::TransportFailure {
                    failure_class: failure_class.clone(),
                    retryable,
                    failed_at: timestamp_rfc3339(),
                },
            };

            write_json_atomic(&response_dir.join("metadata.json"), &response_metadata)
                .map_err(|_| HttpRecorderError::MetadataPersistence)?;

            let transport_failure = RecordedTransportFailure::new(failure_class, retryable);
            let transaction = RecordedTransaction::new(
                identity,
                final_directory.clone(),
                RecordedHttpRequest::new(
                    request.body.as_ref().map(|_| request_body_path.clone()),
                    request_body_bytes.len() as u64,
                    request_body_sha,
                ),
                RecordedHttpResponse::new(
                    None,
                    RecordedHeaderCollection::new(Vec::new()),
                    final_directory.join("response/body"),
                    0,
                    None,
                    HttpRecordedOutcome::TransportFailure(transport_failure.clone()),
                ),
            );

            RecordedAttemptResult {
                transaction,
                transport_failure: Some(transport_failure),
            }
        }
    };

    sync_directory(&staging_directory).map_err(|_| HttpRecorderError::DurableSync)?;
    fs::rename(&staging_directory, &final_directory).map_err(|_| HttpRecorderError::AtomicFinalize)?;

    Ok(result)
}

fn ensure_root(root: &Path) -> Result<(), HttpRecorderError> {
    if root.is_relative() {
        return Err(HttpRecorderError::InvalidManagedRoot);
    }
    if root.is_symlink() {
        return Err(HttpRecorderError::SymlinkRejected);
    }
    fs::create_dir_all(root).map_err(|_| HttpRecorderError::DirectoryCreation)?;
    Ok(())
}

fn redact_request_headers(headers: &[crate::protocols::http::request::FinalizedHeader]) -> Vec<StoredHeader> {
    headers
        .iter()
        .map(|header| {
            let lower = header.name.to_ascii_lowercase();
            let redact = header.sensitive
                || matches!(
                    lower.as_str(),
                    "authorization" | "proxy-authorization" | "cookie"
                );
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
        StoredHeaderValue::Utf8(text) => super::RecordedHeaderValue::Utf8(text),
        StoredHeaderValue::Base64(encoded) => super::RecordedHeaderValue::Base64(encoded),
    };
    RecordedHeader::new(header.name, value)
}

fn stream_body(reader: &mut dyn Read, path: &Path) -> Result<(u64, String), HttpRecorderError> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|_| HttpRecorderError::BodyPersistence)?;

    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| HttpRecorderError::BodyStreaming)?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|_| HttpRecorderError::BodyPersistence)?;
        hash.update(&buffer[..read]);
        total += read as u64;
    }

    file.sync_all().map_err(|_| HttpRecorderError::DurableSync)?;
    Ok((total, format!("{:x}", hash.finalize())))
}

fn persist_body(path: &Path, bytes: &[u8]) -> Result<(), HttpRecorderError> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|_| HttpRecorderError::BodyPersistence)?;
    file.write_all(bytes)
        .map_err(|_| HttpRecorderError::BodyPersistence)?;
    file.sync_all().map_err(|_| HttpRecorderError::DurableSync)?;
    Ok(())
}

fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), HttpRecorderError> {
    let bytes = serde_json::to_vec(value).map_err(|_| HttpRecorderError::MetadataEncoding)?;
    let parent = path.parent().ok_or(HttpRecorderError::MetadataPersistence)?;
    let temp_name = format!(
        ".tmp-{}-{}",
        now_nanos(),
        uuid::Uuid::new_v4().simple()
    );
    let temp = parent.join(temp_name);

    let mut file = File::create(&temp).map_err(|_| HttpRecorderError::MetadataPersistence)?;
    file.write_all(&bytes)
        .map_err(|_| HttpRecorderError::MetadataPersistence)?;
    file.sync_all().map_err(|_| HttpRecorderError::DurableSync)?;
    fs::rename(&temp, path).map_err(|_| HttpRecorderError::AtomicFinalize)?;
    sync_directory(parent).map_err(|_| HttpRecorderError::DurableSync)?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    let file = File::open(path)?;
    file.sync_all()
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    format!("{:x}", hash.finalize())
}

fn transport_failure_class(failure: HttpTransportFailure) -> String {
    match failure {
        HttpTransportFailure::Configuration => "configuration".to_string(),
        HttpTransportFailure::RequestBuild => "request_build".to_string(),
        HttpTransportFailure::Io => "io".to_string(),
    }
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn timestamp_rfc3339() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", now.as_secs(), now.subsec_nanos())
}
