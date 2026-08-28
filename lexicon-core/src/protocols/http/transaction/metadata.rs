use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use reqwest::header::HeaderName;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::error::{HttpRecorderError, ManagedPathKind, validate_managed_path};
use super::identity::{HttpTransactionIdentity, HttpTransactionIdentityError};
use super::{
    HttpAttemptIdentity, HttpLogicalRequestKey, HttpLogicalRequestKeyError, HttpRecordedOutcome,
    RecordedHeader, RecordedHeaderCollection, RecordedHeaderValue, RecordedHttpRequest,
    RecordedHttpResponse, RecordedTransaction, RecordedTransportFailure,
};
use crate::protocols::http::transport::{HttpTransportFailure, StoredHttpVersion};

pub const HTTP_TRANSACTION_SCHEMA_VERSION: u32 = 1;
pub const HTTP_ACQUISITION_PROGRESS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredHeader {
    pub name: String,
    pub value: StoredHeaderValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "encoding", content = "value", rename_all = "snake_case")]
pub enum StoredHeaderValue {
    Utf8(String),
    Base64(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredTransportFailureClass {
    Configuration,
    RequestBuild,
    Connect,
    Timeout,
    BodyWrite,
    ExchangeIo,
    Tls,
}

impl From<HttpTransportFailure> for StoredTransportFailureClass {
    fn from(value: HttpTransportFailure) -> Self {
        match value {
            HttpTransportFailure::Configuration => Self::Configuration,
            HttpTransportFailure::RequestBuild => Self::RequestBuild,
            HttpTransportFailure::Connect => Self::Connect,
            HttpTransportFailure::Timeout => Self::Timeout,
            HttpTransportFailure::BodyWrite => Self::BodyWrite,
            HttpTransportFailure::ExchangeIo => Self::ExchangeIo,
            HttpTransportFailure::Tls => Self::Tls,
        }
    }
}

impl From<StoredTransportFailureClass> for HttpTransportFailure {
    fn from(value: StoredTransportFailureClass) -> Self {
        match value {
            StoredTransportFailureClass::Configuration => Self::Configuration,
            StoredTransportFailureClass::RequestBuild => Self::RequestBuild,
            StoredTransportFailureClass::Connect => Self::Connect,
            StoredTransportFailureClass::Timeout => Self::Timeout,
            StoredTransportFailureClass::BodyWrite => Self::BodyWrite,
            StoredTransportFailureClass::ExchangeIo => Self::ExchangeIo,
            StoredTransportFailureClass::Tls => Self::Tls,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestMetadataDocument {
    pub schema_version: u32,
    pub transaction_id: String,
    pub session_id: String,
    pub physical_attempt_index: u32,
    pub redirect_index: u32,
    pub retry_index: u32,
    pub parent_transaction_id: Option<String>,
    pub logical_request_key: Option<String>,
    pub method: String,
    pub url: String,
    pub headers: Vec<StoredHeader>,
    pub has_body: bool,
    pub body_length: u64,
    pub body_sha256: Option<String>,
    pub created_at_unix_nanos: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ResponseOutcomeDocument {
    Response {
        status: u16,
        http_version: Option<StoredHttpVersion>,
        headers: Vec<StoredHeader>,
        body_length: u64,
        body_sha256: String,
        completed_at_unix_nanos: u64,
    },
    TransportFailure {
        failure_class: StoredTransportFailureClass,
        retryable: bool,
        failed_at_unix_nanos: u64,
    },
    IncompleteResponse {
        failure_class: String,
        body_bytes_recorded: u64,
        partial_body_sha256: Option<String>,
        failed_at_unix_nanos: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseMetadataDocument {
    pub schema_version: u32,
    pub transaction_id: String,
    #[serde(flatten)]
    pub outcome: ResponseOutcomeDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquisitionProgressDocument {
    pub schema_version: u32,
    pub session_id: String,
    pub completed_transaction_count: u64,
    pub transport_failure_count: u64,
    pub redirect_count: u64,
    pub retry_count: u64,
    pub last_transaction_id: Option<String>,
    pub last_logical_request_key: Option<String>,
    pub updated_at_unix_nanos: u64,
    pub revision: u64,
}

#[derive(Debug)]
pub enum AcquisitionProgressValidationError {
    UnknownSchemaVersion { found: u32 },
    EmptySessionId,
    SessionMismatch,
    RevisionMismatch { expected: u64, found: u64 },
    CounterInvariantViolated,
    RevisionCountMismatch,
    LastTransactionInconsistent,
    InvalidTransactionIdentity(HttpTransactionIdentityError),
    InvalidLogicalRequestKey(HttpLogicalRequestKeyError),
    InvalidTimestamp,
    CounterOverflow,
}

impl std::fmt::Display for AcquisitionProgressValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSchemaVersion { .. } => {
                f.write_str("unknown acquisition progress schema version")
            }
            Self::EmptySessionId => f.write_str("acquisition progress session_id is empty"),
            Self::SessionMismatch => f.write_str("acquisition progress session_id does not match"),
            Self::RevisionMismatch { .. } => {
                f.write_str("acquisition progress revision does not match")
            }
            Self::CounterInvariantViolated => {
                f.write_str("acquisition progress counter invariant violated")
            }
            Self::RevisionCountMismatch => {
                f.write_str("acquisition progress revision does not match completed transaction count")
            }
            Self::LastTransactionInconsistent => {
                f.write_str("acquisition progress last transaction fields are inconsistent")
            }
            Self::InvalidTransactionIdentity(_) => {
                f.write_str("acquisition progress transaction identity is invalid")
            }
            Self::InvalidLogicalRequestKey(_) => {
                f.write_str("acquisition progress logical request key is invalid")
            }
            Self::InvalidTimestamp => {
                f.write_str("acquisition progress timestamp is invalid")
            }
            Self::CounterOverflow => f.write_str("acquisition progress counter would overflow"),
        }
    }
}

impl std::error::Error for AcquisitionProgressValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidTransactionIdentity(error) => Some(error),
            Self::InvalidLogicalRequestKey(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum AcquisitionProgressAdvanceError {
    Validation(AcquisitionProgressValidationError),
    InvalidTimestamp,
    CounterOverflow,
}

impl std::fmt::Display for AcquisitionProgressAdvanceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(_) => formatter.write_str("acquisition progress validation failed"),
            Self::InvalidTimestamp => formatter.write_str("acquisition progress timestamp is invalid"),
            Self::CounterOverflow => formatter.write_str("acquisition progress counter overflow"),
        }
    }
}

impl std::error::Error for AcquisitionProgressAdvanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            Self::InvalidTimestamp | Self::CounterOverflow => None,
        }
    }
}

impl AcquisitionProgressDocument {
    pub fn new_initial(session_id: String, now_nanos: u64) -> Self {
        Self {
            schema_version: HTTP_ACQUISITION_PROGRESS_SCHEMA_VERSION,
            session_id,
            completed_transaction_count: 0,
            transport_failure_count: 0,
            redirect_count: 0,
            retry_count: 0,
            last_transaction_id: None,
            last_logical_request_key: None,
            updated_at_unix_nanos: now_nanos,
            revision: 0,
        }
    }

    pub fn validate_existing(
        doc: &AcquisitionProgressDocument,
        session_id: &str,
        expected_revision: u64,
    ) -> Result<(), AcquisitionProgressValidationError> {
        if doc.schema_version != HTTP_ACQUISITION_PROGRESS_SCHEMA_VERSION {
            return Err(AcquisitionProgressValidationError::UnknownSchemaVersion {
                found: doc.schema_version,
            });
        }
        if doc.session_id.is_empty() {
            return Err(AcquisitionProgressValidationError::EmptySessionId);
        }
        if doc.session_id != session_id {
            return Err(AcquisitionProgressValidationError::SessionMismatch);
        }
        if doc.revision != expected_revision {
            return Err(AcquisitionProgressValidationError::RevisionMismatch {
                expected: expected_revision,
                found: doc.revision,
            });
        }
        if doc.transport_failure_count > doc.completed_transaction_count
            || doc.redirect_count > doc.completed_transaction_count
            || doc.retry_count > doc.completed_transaction_count
        {
            return Err(AcquisitionProgressValidationError::CounterInvariantViolated);
        }
        if doc.revision != doc.completed_transaction_count {
            return Err(AcquisitionProgressValidationError::RevisionCountMismatch);
        }
        if doc.completed_transaction_count == 0
            && (doc.last_transaction_id.is_some() || doc.last_logical_request_key.is_some())
        {
            return Err(AcquisitionProgressValidationError::LastTransactionInconsistent);
        }
        if (doc.completed_transaction_count > 0 || doc.revision > 0) && doc.last_transaction_id.is_none()
        {
            return Err(AcquisitionProgressValidationError::LastTransactionInconsistent);
        }
        if doc.revision == 0 && doc.last_transaction_id.is_some() {
            return Err(AcquisitionProgressValidationError::LastTransactionInconsistent);
        }
        if let Some(transaction_id) = &doc.last_transaction_id {
            HttpTransactionIdentity::from_validated(transaction_id.clone())
                .map_err(AcquisitionProgressValidationError::InvalidTransactionIdentity)?;
        }
        if let Some(logical_key) = &doc.last_logical_request_key {
            HttpLogicalRequestKey::new(logical_key)
                .map_err(AcquisitionProgressValidationError::InvalidLogicalRequestKey)?;
        }
        if doc.updated_at_unix_nanos == 0 {
            return Err(AcquisitionProgressValidationError::InvalidTimestamp);
        }
        if doc.completed_transaction_count == u64::MAX
            || doc.transport_failure_count == u64::MAX
            || doc.redirect_count == u64::MAX
            || doc.retry_count == u64::MAX
            || doc.revision == u64::MAX
        {
            return Err(AcquisitionProgressValidationError::CounterOverflow);
        }
        Ok(())
    }

    pub(crate) fn advance(
        self,
        transaction_id: &HttpTransactionIdentity,
        logical_key: Option<&HttpLogicalRequestKey>,
        now: u64,
        transport_failure: bool,
        redirect: bool,
        retry: bool,
    ) -> Result<Self, AcquisitionProgressAdvanceError> {
        if now == 0 {
            return Err(AcquisitionProgressAdvanceError::InvalidTimestamp);
        }
        let session_id = self.session_id.clone();
        let prior_revision = self.revision;
        Self::validate_existing(&self, &session_id, prior_revision)
            .map_err(AcquisitionProgressAdvanceError::Validation)?;

        let completed_transaction_count = self
            .completed_transaction_count
            .checked_add(1)
            .ok_or(AcquisitionProgressAdvanceError::CounterOverflow)?;
        let transport_failure_count = if transport_failure {
            self.transport_failure_count
                .checked_add(1)
                .ok_or(AcquisitionProgressAdvanceError::CounterOverflow)?
        } else {
            self.transport_failure_count
        };
        let redirect_count = if redirect {
            self.redirect_count
                .checked_add(1)
                .ok_or(AcquisitionProgressAdvanceError::CounterOverflow)?
        } else {
            self.redirect_count
        };
        let retry_count = if retry {
            self.retry_count
                .checked_add(1)
                .ok_or(AcquisitionProgressAdvanceError::CounterOverflow)?
        } else {
            self.retry_count
        };
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(AcquisitionProgressAdvanceError::CounterOverflow)?;

        let next = Self {
            schema_version: self.schema_version,
            session_id: self.session_id,
            completed_transaction_count,
            transport_failure_count,
            redirect_count,
            retry_count,
            last_transaction_id: Some(transaction_id.id().to_string()),
            last_logical_request_key: logical_key.map(|key| key.as_str().to_string()),
            updated_at_unix_nanos: now,
            revision,
        };

        Self::validate_existing(&next, &session_id, revision)
            .map_err(AcquisitionProgressAdvanceError::Validation)?;
        Ok(next)
    }
}

#[derive(Debug)]
pub enum HttpTransactionAdmissionError {
    ManagedPath(HttpRecorderError),
    RequestMetadataRead(std::io::Error),
    ResponseMetadataRead(std::io::Error),
    RequestMetadataDecode(serde_json::Error),
    ResponseMetadataDecode(serde_json::Error),
    RequestSchemaVersion { found: u32 },
    ResponseSchemaVersion { found: u32 },
    TransactionIdentity(HttpTransactionIdentityError),
    TransactionIdMismatch,
    InvalidSessionId,
    InvalidLogicalRequestKey(HttpLogicalRequestKeyError),
    InvalidParentTransactionIdentity(HttpTransactionIdentityError),
    HeaderNameInvalid,
    HeaderValueEncodingInvalid,
    SensitiveHeaderRedactionInvalid,
    RequestBodyInvariant,
    ResponseBodyInvariant,
    IncompleteResponseNotFinalized,
    TransportFailureRetryabilityMismatch,
    BodyLengthMismatch,
    BodyHashMismatch,
}

impl std::fmt::Display for HttpTransactionAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ManagedPath(_) => formatter.write_str("managed HTTP transaction path is invalid"),
            Self::RequestMetadataRead(_) => formatter.write_str("failed to read HTTP request metadata"),
            Self::ResponseMetadataRead(_) => {
                formatter.write_str("failed to read HTTP response metadata")
            }
            Self::RequestMetadataDecode(_) => {
                formatter.write_str("failed to decode HTTP request metadata")
            }
            Self::ResponseMetadataDecode(_) => {
                formatter.write_str("failed to decode HTTP response metadata")
            }
            Self::RequestSchemaVersion { .. } => {
                formatter.write_str("unsupported HTTP request metadata schema version")
            }
            Self::ResponseSchemaVersion { .. } => {
                formatter.write_str("unsupported HTTP response metadata schema version")
            }
            Self::TransactionIdentity(_) => {
                formatter.write_str("HTTP transaction identity is invalid")
            }
            Self::TransactionIdMismatch => {
                formatter.write_str("HTTP transaction metadata identifiers do not match")
            }
            Self::InvalidSessionId => formatter.write_str("HTTP transaction session identity is invalid"),
            Self::InvalidLogicalRequestKey(_) => {
                formatter.write_str("HTTP transaction logical request key is invalid")
            }
            Self::InvalidParentTransactionIdentity(_) => {
                formatter.write_str("HTTP parent transaction identity is invalid")
            }
            Self::HeaderNameInvalid => formatter.write_str("HTTP recorded header name is invalid"),
            Self::HeaderValueEncodingInvalid => {
                formatter.write_str("HTTP recorded header encoding is invalid")
            }
            Self::SensitiveHeaderRedactionInvalid => {
                formatter.write_str("HTTP sensitive header redaction is invalid")
            }
            Self::RequestBodyInvariant => {
                formatter.write_str("HTTP request body metadata is inconsistent")
            }
            Self::ResponseBodyInvariant => {
                formatter.write_str("HTTP response body metadata is inconsistent")
            }
            Self::IncompleteResponseNotFinalized => {
                formatter.write_str("HTTP incomplete response cannot be admitted as finalized")
            }
            Self::TransportFailureRetryabilityMismatch => {
                formatter.write_str("HTTP transport failure retryability is inconsistent")
            }
            Self::BodyLengthMismatch => formatter.write_str("HTTP recorded body length is inconsistent"),
            Self::BodyHashMismatch => formatter.write_str("HTTP recorded body hash is inconsistent"),
        }
    }
}

impl std::error::Error for HttpTransactionAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManagedPath(error) => Some(error),
            Self::RequestMetadataRead(error) => Some(error),
            Self::ResponseMetadataRead(error) => Some(error),
            Self::RequestMetadataDecode(error) => Some(error),
            Self::ResponseMetadataDecode(error) => Some(error),
            Self::TransactionIdentity(error) => Some(error),
            Self::InvalidLogicalRequestKey(error) => Some(error),
            Self::InvalidParentTransactionIdentity(error) => Some(error),
            _ => None,
        }
    }
}

pub(crate) fn admit_transaction_from_disk(
    directory: &Path,
) -> Result<RecordedTransaction, HttpTransactionAdmissionError> {
    validate_managed_path(directory, ManagedPathKind::Directory)
        .map_err(HttpTransactionAdmissionError::ManagedPath)?;

    let request_dir = directory.join("request");
    let response_dir = directory.join("response");
    let request_metadata_path = request_dir.join("metadata.json");
    let response_metadata_path = response_dir.join("metadata.json");
    let request_body_path = request_dir.join("body");
    let response_body_path = response_dir.join("body");

    for path in [&request_dir, &response_dir] {
        validate_managed_path(path, ManagedPathKind::Directory)
            .map_err(HttpTransactionAdmissionError::ManagedPath)?;
    }
    for path in [&request_metadata_path, &response_metadata_path] {
        validate_managed_path(path, ManagedPathKind::RegularFileIfPresent)
            .map_err(HttpTransactionAdmissionError::ManagedPath)?;
    }
    for path in [&request_body_path, &response_body_path] {
        validate_managed_path(path, ManagedPathKind::RegularFileIfPresent)
            .map_err(HttpTransactionAdmissionError::ManagedPath)?;
    }

    let request_metadata: RequestMetadataDocument =
        serde_json::from_slice(&fs::read(&request_metadata_path).map_err(
            HttpTransactionAdmissionError::RequestMetadataRead,
        )?)
        .map_err(HttpTransactionAdmissionError::RequestMetadataDecode)?;
    let response_metadata: ResponseMetadataDocument =
        serde_json::from_slice(&fs::read(&response_metadata_path).map_err(
            HttpTransactionAdmissionError::ResponseMetadataRead,
        )?)
        .map_err(HttpTransactionAdmissionError::ResponseMetadataDecode)?;

    if request_metadata.schema_version != HTTP_TRANSACTION_SCHEMA_VERSION {
        return Err(HttpTransactionAdmissionError::RequestSchemaVersion {
            found: request_metadata.schema_version,
        });
    }
    if response_metadata.schema_version != HTTP_TRANSACTION_SCHEMA_VERSION {
        return Err(HttpTransactionAdmissionError::ResponseSchemaVersion {
            found: response_metadata.schema_version,
        });
    }

    let identity = HttpTransactionIdentity::from_validated(request_metadata.transaction_id.clone())
        .map_err(HttpTransactionAdmissionError::TransactionIdentity)?;
    let response_identity =
        HttpTransactionIdentity::from_validated(response_metadata.transaction_id.clone())
            .map_err(HttpTransactionAdmissionError::TransactionIdentity)?;
    if identity != response_identity {
        return Err(HttpTransactionAdmissionError::TransactionIdMismatch);
    }
    if request_metadata.session_id.is_empty() {
        return Err(HttpTransactionAdmissionError::InvalidSessionId);
    }
    if request_metadata.created_at_unix_nanos == 0 {
        return Err(HttpTransactionAdmissionError::RequestBodyInvariant);
    }

    let attempt_identity = HttpAttemptIdentity::new(
        request_metadata.physical_attempt_index,
        request_metadata.redirect_index,
        request_metadata.retry_index,
    );
    let parent_transaction_id = request_metadata
        .parent_transaction_id
        .clone()
        .map(HttpTransactionIdentity::from_validated)
        .transpose()
        .map_err(HttpTransactionAdmissionError::InvalidParentTransactionIdentity)?;
    let logical_request_key = request_metadata
        .logical_request_key
        .clone()
        .map(HttpLogicalRequestKey::new)
        .transpose()
        .map_err(HttpTransactionAdmissionError::InvalidLogicalRequestKey)?;

    let _request_headers = admit_headers(&request_metadata.headers)?;
    let response = match response_metadata.outcome {
        ResponseOutcomeDocument::Response {
            status,
            http_version: _,
            headers,
            body_length,
            body_sha256,
            completed_at_unix_nanos,
        } => {
            if completed_at_unix_nanos == 0 {
                return Err(HttpTransactionAdmissionError::ResponseBodyInvariant);
            }
            let recorded_headers = admit_headers(&headers)?;
            verify_body_file(&response_body_path, body_length, Some(body_sha256.as_str()))?;
            RecordedHttpResponse::new(
                Some(status),
                RecordedHeaderCollection::new(recorded_headers),
                response_body_path,
                body_length,
                Some(body_sha256),
                HttpRecordedOutcome::Response,
            )
        }
        ResponseOutcomeDocument::TransportFailure {
            failure_class,
            retryable,
            failed_at_unix_nanos,
        } => {
            if failed_at_unix_nanos == 0 {
                return Err(HttpTransactionAdmissionError::ResponseBodyInvariant);
            }
            let failure = HttpTransportFailure::from(failure_class);
            if failure.retryable() != retryable {
                return Err(HttpTransactionAdmissionError::TransportFailureRetryabilityMismatch);
            }
            verify_body_file(&response_body_path, 0, None)?;
            RecordedHttpResponse::new(
                None,
                RecordedHeaderCollection::new(Vec::new()),
                response_body_path,
                0,
                None,
                HttpRecordedOutcome::TransportFailure(RecordedTransportFailure::new(failure)),
            )
        }
        ResponseOutcomeDocument::IncompleteResponse { .. } => {
            return Err(HttpTransactionAdmissionError::IncompleteResponseNotFinalized);
        }
    };

    let request = if request_metadata.has_body {
        let body_sha256 = request_metadata
            .body_sha256
            .clone()
            .ok_or(HttpTransactionAdmissionError::RequestBodyInvariant)?;
        verify_body_file(
            &request_body_path,
            request_metadata.body_length,
            Some(body_sha256.as_str()),
        )?;
        RecordedHttpRequest::new(
            Some(request_body_path),
            request_metadata.body_length,
            Some(body_sha256),
        )
    } else {
        if request_metadata.body_length != 0 || request_metadata.body_sha256.is_some() {
            return Err(HttpTransactionAdmissionError::RequestBodyInvariant);
        }
        if request_body_path.exists() {
            return Err(HttpTransactionAdmissionError::RequestBodyInvariant);
        }
        RecordedHttpRequest::new(None, 0, None)
    };

    Ok(RecordedTransaction::new(
        identity,
        attempt_identity,
        parent_transaction_id,
        logical_request_key,
        PathBuf::from(directory),
        request,
        response,
    ))
}

fn admit_headers(
    headers: &[StoredHeader],
) -> Result<Vec<RecordedHeader>, HttpTransactionAdmissionError> {
    headers
        .iter()
        .map(|header| {
            HeaderName::from_bytes(header.name.as_bytes())
                .map_err(|_| HttpTransactionAdmissionError::HeaderNameInvalid)?;

            let lower = header.name.to_ascii_lowercase();
            let must_be_redacted = matches!(
                lower.as_str(),
                "authorization" | "proxy-authorization" | "cookie" | "set-cookie"
            );

            let value = match &header.value {
                StoredHeaderValue::Utf8(text) => {
                    if must_be_redacted && text != "<redacted>" {
                        return Err(HttpTransactionAdmissionError::SensitiveHeaderRedactionInvalid);
                    }
                    if must_be_redacted || text != "<redacted>" {
                        RecordedHeaderValue::Utf8(text.clone())
                    } else {
                        return Err(HttpTransactionAdmissionError::SensitiveHeaderRedactionInvalid);
                    }
                }
                StoredHeaderValue::Base64(encoded) => {
                    if must_be_redacted {
                        return Err(HttpTransactionAdmissionError::SensitiveHeaderRedactionInvalid);
                    }
                    base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .map_err(|_| HttpTransactionAdmissionError::HeaderValueEncodingInvalid)?;
                    RecordedHeaderValue::Base64(encoded.clone())
                }
            };

            Ok(RecordedHeader::new(header.name.clone(), value))
        })
        .collect()
}

fn verify_body_file(
    path: &Path,
    expected_length: u64,
    expected_sha256: Option<&str>,
) -> Result<(), HttpTransactionAdmissionError> {
    validate_managed_path(path, ManagedPathKind::RegularFileIfPresent)
        .map_err(HttpTransactionAdmissionError::ManagedPath)?;
    let metadata = fs::metadata(path).map_err(|_| HttpTransactionAdmissionError::ResponseBodyInvariant)?;
    if metadata.len() != expected_length {
        return Err(HttpTransactionAdmissionError::BodyLengthMismatch);
    }
    let bytes = fs::read(path).map_err(|_| HttpTransactionAdmissionError::ResponseBodyInvariant)?;
    if bytes.len() as u64 != expected_length {
        return Err(HttpTransactionAdmissionError::BodyLengthMismatch);
    }
    match expected_sha256 {
        Some(expected_sha256) => {
            let actual_sha256 = hex_sha256(&bytes);
            if actual_sha256 != expected_sha256 {
                return Err(HttpTransactionAdmissionError::BodyHashMismatch);
            }
        }
        None if expected_length != 0 => return Err(HttpTransactionAdmissionError::ResponseBodyInvariant),
        None => {}
    }
    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    format!("{:x}", hash.finalize())
}
