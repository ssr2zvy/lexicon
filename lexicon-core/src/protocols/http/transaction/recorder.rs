use std::collections::HashSet;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use sha2::{Digest, Sha256};

use super::error::{
    HttpBodyStreamingError, HttpClockError, HttpRecorderError, ManagedPathKind,
    PostRenameSyncFailure, validate_managed_path,
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

pub(crate) struct RecordedAttemptContext {
    pub(crate) session_id: String,
    pub(crate) raw_data_root: PathBuf,
    pub(crate) logical_request_key: Option<HttpLogicalRequestKey>,
    pub(crate) parent_transaction_id: Option<HttpTransactionIdentity>,
    pub(crate) attempt_identity: HttpAttemptIdentity,
    pub(crate) sensitive_query_names: HashSet<String>,
}

pub(crate) fn record_transaction_attempt(
    attempt: RecordedAttemptContext,
    request: &FinalizedHttpRequest,
    transport: &dyn HttpTransport,
) -> Result<FinalizedRecordedAttempt, HttpRecorderError> {
    ensure_directory(&attempt.raw_data_root)?;

    let mut allocated = None;
    for _ in 0..MAX_STAGING_IDENTITY_ATTEMPTS {
        let timestamp = now_nanos().map_err(HttpRecorderError::Clock)?;
        let identity = HttpTransactionIdentity::new().map_err(HttpRecorderError::IdentityInvalid)?;
        let staging_dir = attempt
            .raw_data_root
            .join(format!(".partial-{}-{}", timestamp, identity.id()));
        let final_dir = attempt
            .raw_data_root
            .join(format!("{}-{}", timestamp, identity.id()));

        if final_dir.exists() {
            validate_managed_path(&final_dir, ManagedPathKind::Directory)?;
            continue;
        }

        validate_managed_path(&staging_dir, ManagedPathKind::Directory)?;
        match fs::create_dir(&staging_dir) {
            Ok(()) => {
                validate_managed_path(&staging_dir, ManagedPathKind::Directory)?;
                allocated = Some((timestamp, identity, staging_dir, final_dir));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(HttpRecorderError::ExclusiveStagingCreation(error)),
        }
    }

    let (timestamp, identity, staging_dir, final_dir) =
        allocated.ok_or(HttpRecorderError::IdentityAllocationExhausted)?;

    let request_dir = staging_dir.join("request");
    let response_dir = staging_dir.join("response");
    fs::create_dir_all(&request_dir).map_err(HttpRecorderError::DirectoryCreation)?;
    fs::create_dir_all(&response_dir).map_err(HttpRecorderError::DirectoryCreation)?;
    validate_managed_path(&request_dir, ManagedPathKind::Directory)?;
    validate_managed_path(&response_dir, ManagedPathKind::Directory)?;

    let request_metadata_path = request_dir.join("metadata.json");
    let request_body_path = request_dir.join("body");
    let response_metadata_path = response_dir.join("metadata.json");
    let response_body_path = response_dir.join("body");
    for path in [&request_metadata_path, &response_metadata_path, &request_body_path, &response_body_path] {
        validate_managed_path(path, ManagedPathKind::RegularFileIfPresent)?;
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
        headers: redact_request_headers(&request.headers),
        has_body,
        body_length: request_body_bytes.len() as u64,
        body_sha256: request_body_sha.clone(),
        created_at_unix_nanos: timestamp,
    };
    write_json_atomic(&request_metadata_path, &request_metadata)?;

    if has_body {
        persist_body(&request_body_path, &request_body_bytes).map_err(HttpRecorderError::BodyPersistence)?;
    }

    sync_directory(&request_dir).map_err(HttpRecorderError::DurableSync)?;

    let transport_result = transport.execute(request);
    let (response_status, response_headers, response_body_length, response_body_sha256, outcome, location_text, invalid_location_encoding, transport_failure) =
        match transport_result {
            Ok(mut response) => {
                let location_state = response.location_header;
                let body_stream = stream_body(&mut response.body, &response_body_path);
                let (body_length, body_sha256) = match body_stream {
                    Ok(ok) => (ok.total, ok.sha256),
                    Err(stream_failure) => {
                        if let Err(marker_cause) = persist_incomplete_response_marker(
                            &response_metadata_path,
                            identity.id(),
                            stream_failure.bytes_recorded,
                            stream_failure.partial_body_sha256.as_deref(),
                            &stream_failure.error,
                        ) {
                            return Err(HttpRecorderError::IncompleteResponseMarkerFailed {
                                stream_cause: stream_failure.error,
                                marker_cause,
                            });
                        }
                        return Err(HttpRecorderError::BodyStreaming(stream_failure.error));
                    }
                };

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
                        completed_at_unix_nanos: now_nanos().map_err(HttpRecorderError::Clock)?,
                    },
                };
                write_json_atomic(&response_metadata_path, &response_metadata)?;

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
                    HttpRecordedOutcome::Response,
                    location_text,
                    invalid_location_encoding,
                    None,
                )
            }
            Err(failure) => {
                persist_body(&response_body_path, &[])
                    .map_err(HttpRecorderError::BodyPersistence)?;

                let response_metadata = ResponseMetadataDocument {
                    schema_version: HTTP_TRANSACTION_SCHEMA_VERSION,
                    transaction_id: identity.id().to_string(),
                    outcome: ResponseOutcomeDocument::TransportFailure {
                        failure_class: StoredTransportFailureClass::from(failure),
                        retryable: failure.retryable(),
                        failed_at_unix_nanos: now_nanos().map_err(HttpRecorderError::Clock)?,
                    },
                };
                write_json_atomic(&response_metadata_path, &response_metadata)?;

                (
                    None,
                    Vec::new(),
                    0,
                    None,
                    HttpRecordedOutcome::TransportFailure(RecordedTransportFailure::new(failure)),
                    None,
                    false,
                    Some(failure),
                )
            }
        };

    sync_directory(&staging_dir).map_err(HttpRecorderError::DurableSync)?;
    publish_staging_directory(&staging_dir, &final_dir)?;
    let final_request_dir = final_dir.join("request");
    let final_response_dir = final_dir.join("response");
    let final_request_metadata_path = final_request_dir.join("metadata.json");
    let final_request_body_path = final_request_dir.join("body");
    let final_response_metadata_path = final_response_dir.join("metadata.json");
    let final_response_body_path = final_response_dir.join("body");
    validate_managed_path(&final_dir, ManagedPathKind::Directory)?;
    validate_managed_path(&final_request_dir, ManagedPathKind::Directory)?;
    validate_managed_path(&final_response_dir, ManagedPathKind::Directory)?;
    validate_managed_path(&final_request_metadata_path, ManagedPathKind::RegularFileIfPresent)?;
    validate_managed_path(&final_response_metadata_path, ManagedPathKind::RegularFileIfPresent)?;
    if has_body {
        validate_managed_path(&final_request_body_path, ManagedPathKind::RegularFileIfPresent)?;
    }
    validate_managed_path(&final_response_body_path, ManagedPathKind::RegularFileIfPresent)?;
    sync_directory(&attempt.raw_data_root).map_err(|cause| {
        HttpRecorderError::PostRenameSyncFailed(PostRenameSyncFailure {
            transaction_id: identity.id().to_string(),
            final_path: final_dir.clone(),
            cause,
        })
    })?;

    let transaction = RecordedTransaction::new(
        identity,
        attempt.attempt_identity,
        attempt.parent_transaction_id,
        attempt.logical_request_key,
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
        return Err(HttpRecorderError::InvalidManagedRoot);
    }
    if let Some(parent) = path.parent() {
        validate_managed_path(parent, ManagedPathKind::Directory)?;
    }
    fs::create_dir_all(path).map_err(HttpRecorderError::DirectoryCreation)?;
    validate_managed_path(path, ManagedPathKind::Directory)
}

fn redact_request_headers(headers: &[FinalizedHeader]) -> Vec<StoredHeader> {
    headers
        .iter()
        .map(|header| {
            let lower = header.name.to_ascii_lowercase();
            let redact =
                header.sensitive || matches!(lower.as_str(), "authorization" | "proxy-authorization" | "cookie");
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

struct StreamBodySuccess {
    total: u64,
    sha256: String,
}

struct StreamBodyFailure {
    error: HttpBodyStreamingError,
    bytes_recorded: u64,
    partial_body_sha256: Option<String>,
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
        })?;

    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 8192];

    loop {
        let n = reader.read(&mut buffer).map_err(|error| {
            let _ = file.sync_all();
            StreamBodyFailure {
                error: HttpBodyStreamingError::Io(error),
                bytes_recorded: total,
                partial_body_sha256: (total > 0).then(|| format!("{:x}", hash.clone().finalize())),
            }
        })?;
        if n == 0 {
            break;
        }
        let next_total = total.checked_add(n as u64).ok_or_else(|| {
            let _ = file.sync_all();
            StreamBodyFailure {
                error: HttpBodyStreamingError::LengthOverflow,
                bytes_recorded: total,
                partial_body_sha256: (total > 0).then(|| format!("{:x}", hash.clone().finalize())),
            }
        })?;
        file.write_all(&buffer[..n]).map_err(|error| {
            let _ = file.sync_all();
            StreamBodyFailure {
                error: HttpBodyStreamingError::Io(error),
                bytes_recorded: total,
                partial_body_sha256: (total > 0).then(|| format!("{:x}", hash.clone().finalize())),
            }
        })?;
        hash.update(&buffer[..n]);
        total = next_total;
    }

    file.sync_all().map_err(|error| StreamBodyFailure {
        error: HttpBodyStreamingError::Io(error),
        bytes_recorded: total,
        partial_body_sha256: (total > 0).then(|| format!("{:x}", hash.clone().finalize())),
    })?;

    Ok(StreamBodySuccess {
        total,
        sha256: format!("{:x}", hash.finalize()),
    })
}

fn persist_incomplete_response_marker(
    metadata_path: &Path,
    transaction_id: &str,
    body_bytes_recorded: u64,
    partial_body_sha256: Option<&str>,
    stream_error: &HttpBodyStreamingError,
) -> Result<(), std::io::Error> {
    let failed_at_unix_nanos = now_nanos().map_err(|error| std::io::Error::other(error.to_string()))?;
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
    write_json_bytes_atomic(
        metadata_path,
        &serde_json::to_vec(&document)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?,
    )
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

fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), HttpRecorderError> {
    let bytes = serde_json::to_vec(value).map_err(HttpRecorderError::MetadataEncoding)?;
    write_json_bytes_atomic(path, &bytes).map_err(HttpRecorderError::MetadataPersistence)
}

fn write_json_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent"))?;
    validate_managed_path(parent, ManagedPathKind::Directory)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    validate_managed_path(path, ManagedPathKind::RegularFileIfPresent)
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    let temp_file = tempfile::Builder::new()
        .prefix(".metadata-")
        .suffix(".tmp")
        .tempfile_in(parent)?;

    let (mut file, temp_path) = temp_file.into_parts();
    file.write_all(bytes)?;
    file.sync_all()?;
    temp_path.persist(path).map_err(|error| error.error)?;
    sync_directory(parent)?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    File::open(path)?.sync_all()
}

#[cfg(target_os = "linux")]
fn publish_staging_directory(staging_dir: &Path, final_dir: &Path) -> Result<(), HttpRecorderError> {
    use std::os::unix::ffi::OsStrExt;

    if final_dir.exists() {
        return Err(HttpRecorderError::FinalPublicationCollision);
    }

    let staging = CString::new(staging_dir.as_os_str().as_bytes())
        .map_err(|_| HttpRecorderError::UnsupportedPlatformPublication)?;
    let final_path = CString::new(final_dir.as_os_str().as_bytes())
        .map_err(|_| HttpRecorderError::UnsupportedPlatformPublication)?;

    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            staging.as_ptr(),
            libc::AT_FDCWD,
            final_path.as_ptr(),
            1u32,
        )
    };
    if result == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        Err(HttpRecorderError::FinalPublicationCollision)
    } else {
        Err(HttpRecorderError::AtomicFinalize(error))
    }
}

#[cfg(target_os = "macos")]
fn publish_staging_directory(staging_dir: &Path, final_dir: &Path) -> Result<(), HttpRecorderError> {
    use std::os::unix::ffi::OsStrExt;

    if final_dir.exists() {
        return Err(HttpRecorderError::FinalPublicationCollision);
    }

    let staging = CString::new(staging_dir.as_os_str().as_bytes())
        .map_err(|_| HttpRecorderError::UnsupportedPlatformPublication)?;
    let final_path = CString::new(final_dir.as_os_str().as_bytes())
        .map_err(|_| HttpRecorderError::UnsupportedPlatformPublication)?;

    let result = unsafe { libc::renamex_np(staging.as_ptr(), final_path.as_ptr(), libc::RENAME_EXCL) };
    if result == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        Err(HttpRecorderError::FinalPublicationCollision)
    } else {
        Err(HttpRecorderError::AtomicFinalize(error))
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn publish_staging_directory(_: &Path, _: &Path) -> Result<(), HttpRecorderError> {
    Err(HttpRecorderError::UnsupportedPlatformPublication)
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
