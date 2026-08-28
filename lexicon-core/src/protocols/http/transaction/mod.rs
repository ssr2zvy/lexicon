use std::path::{Path, PathBuf};

use crate::protocols::http::error::AcquisitionError;
use crate::protocols::http::transport::HttpTransportFailure;

pub mod error;
pub mod identity;
pub mod metadata;
pub(crate) mod recorder;

pub use identity::{HttpTransactionIdentity, HttpTransactionIdentityError};
pub(crate) use recorder::{RecordedAttemptContext, record_transaction_attempt};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HttpLogicalRequestKey {
    key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpLogicalRequestKeyError {
    Empty,
    TooLong,
    InvalidCharacter,
}

impl HttpLogicalRequestKey {
    pub fn new(key: impl AsRef<str>) -> Result<Self, HttpLogicalRequestKeyError> {
        const MAX_LOGICAL_REQUEST_KEY_BYTES: usize = 512;

        let key = key.as_ref();
        if key.is_empty() {
            return Err(HttpLogicalRequestKeyError::Empty);
        }
        if key.as_bytes().len() > MAX_LOGICAL_REQUEST_KEY_BYTES {
            return Err(HttpLogicalRequestKeyError::TooLong);
        }
        if key.chars().any(|ch| ch == '\0' || ch.is_control()) {
            return Err(HttpLogicalRequestKeyError::InvalidCharacter);
        }
        Ok(Self { key: key.to_string() })
    }

    pub fn as_str(&self) -> &str {
        &self.key
    }
}

impl std::fmt::Display for HttpLogicalRequestKeyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => formatter.write_str("HTTP logical request key is empty"),
            Self::TooLong => formatter.write_str("HTTP logical request key is too long"),
            Self::InvalidCharacter => {
                formatter.write_str("HTTP logical request key contains invalid characters")
            }
        }
    }
}

impl std::error::Error for HttpLogicalRequestKeyError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpAttemptIdentity {
    pub(crate) physical_attempt_index: u32,
    pub(crate) redirect_index: u32,
    pub(crate) retry_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpAttemptIdentityError {
    ZeroPhysicalAttemptIndex,
    RedirectIndexOutOfRange,
    RetryIndexOutOfRange,
    FirstAttemptMustStartAtZero,
}

impl std::fmt::Display for HttpAttemptIdentityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroPhysicalAttemptIndex => {
                formatter.write_str("HTTP physical attempt index must start at one")
            }
            Self::RedirectIndexOutOfRange => {
                formatter.write_str("HTTP redirect index is out of range")
            }
            Self::RetryIndexOutOfRange => formatter.write_str("HTTP retry index is out of range"),
            Self::FirstAttemptMustStartAtZero => formatter.write_str(
                "HTTP first physical attempt must have redirect and retry index zero",
            ),
        }
    }
}

impl std::error::Error for HttpAttemptIdentityError {}

impl HttpAttemptIdentity {
    pub(crate) fn new(
        physical_attempt_index: u32,
        redirect_index: u32,
        retry_index: u32,
    ) -> Result<Self, HttpAttemptIdentityError> {
        if physical_attempt_index == 0 {
            return Err(HttpAttemptIdentityError::ZeroPhysicalAttemptIndex);
        }
        if redirect_index >= physical_attempt_index {
            return Err(HttpAttemptIdentityError::RedirectIndexOutOfRange);
        }
        if retry_index >= physical_attempt_index {
            return Err(HttpAttemptIdentityError::RetryIndexOutOfRange);
        }
        if physical_attempt_index == 1 && (redirect_index != 0 || retry_index != 0) {
            return Err(HttpAttemptIdentityError::FirstAttemptMustStartAtZero);
        }
        Ok(Self {
            physical_attempt_index,
            redirect_index,
            retry_index,
        })
    }

    pub fn physical_attempt_index(&self) -> u32 {
        self.physical_attempt_index
    }

    pub fn redirect_index(&self) -> u32 {
        self.redirect_index
    }

    pub fn retry_index(&self) -> u32 {
        self.retry_index
    }
}

#[derive(Debug, Clone)]
pub struct RecordedTransaction {
    identity: HttpTransactionIdentity,
    attempt_identity: HttpAttemptIdentity,
    parent_transaction_id: Option<HttpTransactionIdentity>,
    logical_request_key: Option<HttpLogicalRequestKey>,
    session: crate::session::SessionIdentity,
    created_at_unix_nanos: u64,
    directory: PathBuf,
    request: RecordedHttpRequest,
    response: RecordedHttpResponse,
}

impl RecordedTransaction {
    pub(crate) fn new(
        identity: HttpTransactionIdentity,
        attempt_identity: HttpAttemptIdentity,
        parent_transaction_id: Option<HttpTransactionIdentity>,
        logical_request_key: Option<HttpLogicalRequestKey>,
        session: crate::session::SessionIdentity,
        created_at_unix_nanos: u64,
        directory: PathBuf,
        request: RecordedHttpRequest,
        response: RecordedHttpResponse,
    ) -> Self {
        Self {
            identity,
            attempt_identity,
            parent_transaction_id,
            logical_request_key,
            session,
            created_at_unix_nanos,
            directory,
            request,
            response,
        }
    }

    pub fn identity(&self) -> &HttpTransactionIdentity {
        &self.identity
    }

    pub fn attempt_identity(&self) -> &HttpAttemptIdentity {
        &self.attempt_identity
    }

    pub fn parent_transaction_id(&self) -> Option<&HttpTransactionIdentity> {
        self.parent_transaction_id.as_ref()
    }

    pub fn logical_request_key(&self) -> Option<&HttpLogicalRequestKey> {
        self.logical_request_key.as_ref()
    }

    pub fn session(&self) -> &crate::session::SessionIdentity {
        &self.session
    }

    pub fn created_at_unix_nanos(&self) -> u64 {
        self.created_at_unix_nanos
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn request(&self) -> &RecordedHttpRequest {
        &self.request
    }

    pub fn response(&self) -> &RecordedHttpResponse {
        &self.response
    }

    pub fn transport_failure(&self) -> Option<&RecordedTransportFailure> {
        match self.response.outcome() {
            HttpRecordedOutcome::Response => None,
            HttpRecordedOutcome::TransportFailure(failure) => Some(failure),
        }
    }

    pub fn response_status(&self) -> Option<u16> {
        self.response.status_code()
    }

    pub fn completed_at_unix_nanos(&self) -> u64 {
        self.response.completed_at_unix_nanos()
    }
}

pub(crate) struct FinalizedRecordedAttempt {
    pub(crate) transaction: RecordedTransaction,
    pub(crate) attempt_identity: HttpAttemptIdentity,
    pub(crate) effective_location: Option<String>,
    pub(crate) invalid_location_encoding: bool,
    pub(crate) transport_failure: Option<HttpTransportFailure>,
}

pub(crate) struct ProgressPublishedRecordedAttempt {
    pub(crate) transaction: RecordedTransaction,
    pub(crate) location_text: Option<String>,
    pub(crate) invalid_location_encoding: bool,
}

#[derive(Debug, Clone)]
pub struct RecordedHttpRequest {
    body_path: Option<PathBuf>,
    body_length: u64,
    body_sha256: Option<String>,
}

impl RecordedHttpRequest {
    pub(crate) fn new(body_path: Option<PathBuf>, body_length: u64, body_sha256: Option<String>) -> Self {
        Self { body_path, body_length, body_sha256 }
    }

    pub fn body_path(&self) -> Option<&Path> {
        self.body_path.as_deref()
    }

    pub fn body_length(&self) -> u64 {
        self.body_length
    }

    pub fn body_sha256(&self) -> Option<&str> {
        self.body_sha256.as_deref()
    }
}

#[derive(Debug, Clone)]
pub struct RecordedHeaderCollection {
    headers: Vec<RecordedHeader>,
}

impl RecordedHeaderCollection {
    pub(crate) fn new(headers: Vec<RecordedHeader>) -> Self {
        Self { headers }
    }

    pub fn as_slice(&self) -> &[RecordedHeader] {
        &self.headers
    }
}

#[derive(Debug, Clone)]
pub struct RecordedHeader {
    name: String,
    value: RecordedHeaderValue,
}

impl RecordedHeader {
    pub(crate) fn new(name: String, value: RecordedHeaderValue) -> Self {
        Self { name, value }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &RecordedHeaderValue {
        &self.value
    }
}

#[derive(Debug, Clone)]
pub enum RecordedHeaderValue {
    Utf8(String),
    Base64(String),
    Redacted,
}

#[derive(Debug, Clone)]
pub struct RecordedHttpResponse {
    status: Option<u16>,
    headers: RecordedHeaderCollection,
    body_path: PathBuf,
    body_length: u64,
    body_sha256: Option<String>,
    completed_at_unix_nanos: u64,
    outcome: HttpRecordedOutcome,
}

impl RecordedHttpResponse {
    pub(crate) fn new(
        status: Option<u16>,
        headers: RecordedHeaderCollection,
        body_path: PathBuf,
        body_length: u64,
        body_sha256: Option<String>,
        completed_at_unix_nanos: u64,
        outcome: HttpRecordedOutcome,
    ) -> Self {
        Self {
            status,
            headers,
            body_path,
            body_length,
            body_sha256,
            completed_at_unix_nanos,
            outcome,
        }
    }

    pub fn status_code(&self) -> Option<u16> {
        self.status
    }

    pub fn headers(&self) -> &RecordedHeaderCollection {
        &self.headers
    }

    pub fn body_path(&self) -> &Path {
        &self.body_path
    }

    pub fn body_length(&self) -> u64 {
        self.body_length
    }

    pub fn body_sha256(&self) -> Option<&str> {
        self.body_sha256.as_deref()
    }

    pub fn completed_at_unix_nanos(&self) -> u64 {
        self.completed_at_unix_nanos
    }

    pub fn outcome(&self) -> &HttpRecordedOutcome {
        &self.outcome
    }

    pub fn require_success(&self) -> Result<(), AcquisitionError> {
        match &self.outcome {
            HttpRecordedOutcome::TransportFailure(failure) => {
                Err(AcquisitionError::transport_failure(failure.failure()))
            }
            HttpRecordedOutcome::Response => match self.status {
                Some(status) if (200..=299).contains(&status) => Ok(()),
                Some(status) => Err(AcquisitionError::response_status(
                    HttpResponseStatusError::new(status),
                )),
                None => Err(AcquisitionError::response_status(
                    HttpResponseStatusError::new_missing(),
                )),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum HttpRecordedOutcome {
    Response,
    TransportFailure(RecordedTransportFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRecordedOutcomeKind {
    Response,
    TransportFailure,
}

impl HttpRecordedOutcome {
    pub fn kind(&self) -> HttpRecordedOutcomeKind {
        match self {
            Self::Response => HttpRecordedOutcomeKind::Response,
            Self::TransportFailure(_) => HttpRecordedOutcomeKind::TransportFailure,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecordedTransportFailure {
    failure: HttpTransportFailure,
}

impl RecordedTransportFailure {
    pub(crate) fn new(failure: HttpTransportFailure) -> Self {
        Self { failure }
    }

    pub fn failure(&self) -> HttpTransportFailure {
        self.failure
    }

    pub fn failure_class(&self) -> &'static str {
        self.failure.stable_class()
    }

    pub fn retryable(&self) -> bool {
        self.failure.retryable()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpResponseStatusError {
    status: Option<u16>,
}

impl HttpResponseStatusError {
    pub fn new(status: u16) -> Self {
        Self { status: Some(status) }
    }

    pub fn new_missing() -> Self {
        Self { status: None }
    }

    pub fn status(&self) -> Option<u16> {
        self.status
    }
}

impl std::fmt::Display for HttpResponseStatusError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            Some(status) => write!(formatter, "HTTP status indicates failure: {}", status),
            None => formatter.write_str("HTTP response status is absent"),
        }
    }
}

impl std::error::Error for HttpResponseStatusError {}
