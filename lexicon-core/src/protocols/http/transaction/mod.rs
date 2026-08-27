use std::path::{Path, PathBuf};

use crate::protocols::http::error::AcquisitionError;

pub mod error;
pub mod identity;
pub mod metadata;
pub(crate) mod recorder;

pub use identity::HttpTransactionIdentity;
pub(crate) use recorder::{RecordedAttemptContext, RecordedAttemptResult, record_transaction_attempt};

#[derive(Debug, Clone)]
pub struct RecordedTransaction {
    identity: HttpTransactionIdentity,
    directory: PathBuf,
    request: RecordedHttpRequest,
    response: RecordedHttpResponse,
}

impl RecordedTransaction {
    pub(crate) fn new(
        identity: HttpTransactionIdentity,
        directory: PathBuf,
        request: RecordedHttpRequest,
        response: RecordedHttpResponse,
    ) -> Self {
        Self {
            identity,
            directory,
            request,
            response,
        }
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

#[derive(Debug, Clone)]
pub struct RecordedHttpRequest {
    body_path: Option<PathBuf>,
    body_length: u64,
    body_sha256: Option<String>,
}

impl RecordedHttpRequest {
    pub(crate) fn new(body_path: Option<PathBuf>, body_length: u64, body_sha256: Option<String>) -> Self {
        Self {
            body_path,
            body_length,
            body_sha256,
        }
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
        Self {
            status,
            headers,
            body_path,
            body_length,
            body_sha256,
            outcome,
        }
    }

    pub fn status(&self) -> u16 {
        self.status.unwrap_or(0)
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

    pub fn body_sha256(&self) -> &str {
        self.body_sha256.as_deref().unwrap_or("")
    }

    pub fn require_success(&self) -> Result<(), AcquisitionError> {
        match self.status {
            Some(status) if (200..=299).contains(&status) => Ok(()),
            Some(status) => Err(AcquisitionError::response_status(
                HttpResponseStatusError::new(status),
            )),
            None => Err(AcquisitionError::execution_message(
                "HTTP request did not receive a successful response",
            )),
        }
    }

    pub fn outcome(&self) -> &HttpRecordedOutcome {
        &self.outcome
    }
}

#[derive(Debug, Clone)]
pub enum HttpRecordedOutcome {
    Response,
    TransportFailure(RecordedTransportFailure),
}

#[derive(Debug, Clone)]
pub struct RecordedTransportFailure {
    failure_class: String,
    retryable: bool,
}

impl RecordedTransportFailure {
    pub(crate) fn new(failure_class: String, retryable: bool) -> Self {
        Self {
            failure_class,
            retryable,
        }
    }

    pub fn failure_class(&self) -> &str {
        &self.failure_class
    }

    pub fn retryable(&self) -> bool {
        self.retryable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpResponseStatusError {
    status: u16,
}

impl HttpResponseStatusError {
    pub fn new(status: u16) -> Self {
        Self { status }
    }

    pub fn status(&self) -> u16 {
        self.status
    }
}

impl std::fmt::Display for HttpResponseStatusError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "HTTP status indicates failure: {}", self.status)
    }
}

impl std::error::Error for HttpResponseStatusError {}
