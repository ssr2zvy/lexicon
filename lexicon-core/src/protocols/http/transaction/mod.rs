use std::path::{Path, PathBuf};

use crate::protocols::http::error::AcquisitionError;
use crate::protocols::http::transport::HttpTransportFailure;

pub mod error;
pub mod identity;
pub mod metadata;
pub(crate) mod recorder;

pub use identity::HttpTransactionIdentity;
pub(crate) use recorder::{RecordedAttemptContext, record_transaction_attempt};

// ---------------------------------------------------------------------------
// Public finalized recorded type
// ---------------------------------------------------------------------------

/// A durable HTTP transaction record. Only constructable after the staging directory
/// has been renamed, the raw-data parent synced, and acquisition progress updated.
#[derive(Debug, Clone)]
pub struct RecordedTransaction {
    identity: HttpTransactionIdentity,
    directory: PathBuf,
    request: RecordedHttpRequest,
    response: RecordedHttpResponse,
}

impl RecordedTransaction {
    /// Internal constructor. Only call after finalization succeeds.
    pub(crate) fn new(
        identity: HttpTransactionIdentity,
        directory: PathBuf,
        request: RecordedHttpRequest,
        response: RecordedHttpResponse,
    ) -> Self {
        Self { identity, directory, request, response }
    }

    pub fn identity(&self) -> &HttpTransactionIdentity {
        &self.identity
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
}

// ---------------------------------------------------------------------------
// Internal type-state structs (private to crate)
// ---------------------------------------------------------------------------

/// A transaction that has been atomically renamed to its final directory and whose
/// raw-data parent has been synced. Not yet progress-published.
pub(crate) struct FinalizedRecordedAttempt {
    pub(crate) transaction: RecordedTransaction,
    /// Location header from the actual transport response (not from persisted metadata).
    pub(crate) effective_location: Option<String>,
    /// The typed transport failure, if any.
    pub(crate) transport_failure: Option<HttpTransportFailure>,
}

// ---------------------------------------------------------------------------
// Recorded request / response types
// ---------------------------------------------------------------------------

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
}

#[derive(Debug, Clone)]
pub struct RecordedHttpResponse {
    /// HTTP status code — `None` for transport-failure outcomes.
    status: Option<u16>,
    headers: RecordedHeaderCollection,
    body_path: PathBuf,
    body_length: u64,
    body_sha256: Option<String>,
    outcome: HttpRecordedOutcome,
}

impl RecordedHttpResponse {
    pub(crate) fn new(
        status: Option<u16>,
        headers: RecordedHeaderCollection,
        body_path: PathBuf,
        body_length: u64,
        body_sha256: Option<String>,
        outcome: HttpRecordedOutcome,
    ) -> Self {
        Self { status, headers, body_path, body_length, body_sha256, outcome }
    }

    /// The HTTP status code. Returns `None` for transport-failure outcomes.
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

    pub fn outcome(&self) -> &HttpRecordedOutcome {
        &self.outcome
    }

    /// Require a successful HTTP response.
    ///
    /// Returns an error if the outcome is a transport failure or if the status is not 2xx.
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

// ---------------------------------------------------------------------------
// Status error
// ---------------------------------------------------------------------------

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
