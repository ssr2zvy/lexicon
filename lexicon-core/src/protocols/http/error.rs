use std::fmt;

use super::request::HttpRequestError;
use super::transaction::{HttpResponseStatusError, RecordedTransaction};
use super::transport::{HttpTransportConfigurationError, HttpTransportFailure};
use super::{HttpProgressPartialCommit, SessionValidationError};
use crate::protocols::http::transaction::error::HttpRecorderError;

pub type AcquisitionResult<T> = Result<T, AcquisitionError>;

#[derive(Debug)]
pub enum AcquisitionError {
    Source { message: String },
    Request(HttpRequestError),
    Execution(HttpExecutionError),
    ResponseStatus(HttpResponseStatusError),
    CheckpointCommit(crate::protocols::http::checkpoint::error::HttpCheckpointCommitError),
    CheckpointLookup(crate::protocols::http::checkpoint::error::HttpCheckpointLookupError),
    TransactionAdmission(crate::protocols::http::transaction::metadata::HttpTransactionAdmissionError),
}

impl AcquisitionError {
    pub fn source_message(message: impl Into<String>) -> Self {
        Self::Source {
            message: message.into(),
        }
    }

    pub fn request(error: HttpRequestError) -> Self {
        Self::Request(error)
    }

    pub fn execution(error: HttpExecutionError) -> Self {
        Self::Execution(error)
    }

    pub fn response_status(error: HttpResponseStatusError) -> Self {
        Self::ResponseStatus(error)
    }

    pub fn transport_failure(failure: HttpTransportFailure) -> Self {
        Self::Execution(HttpExecutionError::Transport(failure))
    }

    pub(crate) fn checkpoint_commit(
        error: crate::protocols::http::checkpoint::error::HttpCheckpointCommitError,
    ) -> Self {
        Self::CheckpointCommit(error)
    }

    pub(crate) fn checkpoint_lookup(
        error: crate::protocols::http::checkpoint::error::HttpCheckpointLookupError,
    ) -> Self {
        Self::CheckpointLookup(error)
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Source { message } => message,
            Self::Request(_) => "request error",
            Self::Execution(_) => "execution error",
            Self::ResponseStatus(_) => "response status error",
            Self::CheckpointCommit(_) => "checkpoint commit error",
            Self::CheckpointLookup(_) => "checkpoint lookup error",
            Self::TransactionAdmission(_) => "transaction admission error",
        }
    }
}

impl fmt::Display for AcquisitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source { .. } => formatter.write_str("source handler returned an error"),
            Self::Request(error) => write!(formatter, "{error}"),
            Self::Execution(error) => write!(formatter, "{error}"),
            Self::ResponseStatus(error) => write!(formatter, "{error}"),
            Self::CheckpointCommit(error) => write!(formatter, "{error}"),
            Self::CheckpointLookup(error) => write!(formatter, "{error}"),
            Self::TransactionAdmission(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AcquisitionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Request(error) => Some(error),
            Self::Execution(error) => Some(error),
            Self::ResponseStatus(error) => Some(error),
            Self::CheckpointCommit(error) => Some(error),
            Self::CheckpointLookup(error) => Some(error),
            Self::TransactionAdmission(error) => Some(error),
            Self::Source { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum HttpRetryFinalOutcome {
    ResponseStatus(u16),
    TransportFailure(HttpTransportFailure),
}

#[derive(Debug, Clone)]
pub struct HttpRetryExhaustionError {
    final_transaction: RecordedTransaction,
    total_physical_attempts: u32,
    final_outcome: HttpRetryFinalOutcome,
}

impl HttpRetryExhaustionError {
    pub fn new(
        final_transaction: RecordedTransaction,
        total_physical_attempts: u32,
        final_outcome: HttpRetryFinalOutcome,
    ) -> Self {
        Self {
            final_transaction,
            total_physical_attempts,
            final_outcome,
        }
    }

    pub fn final_transaction(&self) -> &RecordedTransaction {
        &self.final_transaction
    }

    pub fn transaction_identity(&self) -> &super::transaction::HttpTransactionIdentity {
        self.final_transaction.identity()
    }

    pub fn transaction_directory(&self) -> &std::path::Path {
        self.final_transaction.directory()
    }

    pub fn total_physical_attempts(&self) -> u32 {
        self.total_physical_attempts
    }

    pub fn final_outcome(&self) -> &HttpRetryFinalOutcome {
        &self.final_outcome
    }
}

impl fmt::Display for HttpRetryExhaustionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HTTP retry policy exhausted")
    }
}

impl std::error::Error for HttpRetryExhaustionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.final_outcome {
            HttpRetryFinalOutcome::ResponseStatus(_) => None,
            HttpRetryFinalOutcome::TransportFailure(failure) => Some(failure),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecordedHttpTransportFailure {
    transaction: RecordedTransaction,
    failure: HttpTransportFailure,
}

impl RecordedHttpTransportFailure {
    pub fn new(transaction: RecordedTransaction, failure: HttpTransportFailure) -> Self {
        Self { transaction, failure }
    }

    pub fn transaction(&self) -> &RecordedTransaction {
        &self.transaction
    }

    pub fn transaction_identity(&self) -> &super::transaction::HttpTransactionIdentity {
        self.transaction.identity()
    }

    pub fn transaction_directory(&self) -> &std::path::Path {
        self.transaction.directory()
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

    pub fn physical_attempt_index(&self) -> u32 {
        self.transaction.attempt_identity().physical_attempt_index()
    }

    pub fn redirect_index(&self) -> u32 {
        self.transaction.attempt_identity().redirect_index()
    }

    pub fn retry_index(&self) -> u32 {
        self.transaction.attempt_identity().retry_index()
    }
}

impl fmt::Display for RecordedHttpTransportFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HTTP transport exchange failed after durable recording")
    }
}

impl std::error::Error for RecordedHttpTransportFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRedirectFailureKind {
    MaximumExceeded,
    LoopDetected,
    MissingLocation,
    InvalidLocationEncoding,
    InvalidTarget,
    UnsupportedScheme,
}

#[derive(Debug, Clone)]
pub struct HttpRedirectFailure {
    last_transaction: RecordedTransaction,
    kind: HttpRedirectFailureKind,
    redirect_count: u32,
    total_physical_attempt_count: u32,
}

impl HttpRedirectFailure {
    pub fn new(
        last_transaction: RecordedTransaction,
        kind: HttpRedirectFailureKind,
        redirect_count: u32,
        total_physical_attempt_count: u32,
    ) -> Self {
        Self {
            last_transaction,
            kind,
            redirect_count,
            total_physical_attempt_count,
        }
    }

    pub fn last_transaction(&self) -> &RecordedTransaction {
        &self.last_transaction
    }

    pub fn transaction_identity(&self) -> &super::transaction::HttpTransactionIdentity {
        self.last_transaction.identity()
    }

    pub fn transaction_directory(&self) -> &std::path::Path {
        self.last_transaction.directory()
    }

    pub fn kind(&self) -> HttpRedirectFailureKind {
        self.kind
    }

    pub fn redirect_count(&self) -> u32 {
        self.redirect_count
    }

    pub fn total_physical_attempt_count(&self) -> u32 {
        self.total_physical_attempt_count
    }
}

impl fmt::Display for HttpRedirectFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HTTP redirect handling failed")
    }
}

impl std::error::Error for HttpRedirectFailure {}

#[derive(Debug)]
pub enum HttpExecutionError {
    UnmanagedContext,
    TransportConfiguration(HttpTransportConfigurationError),
    SessionValidation(SessionValidationError),
    Recorder(HttpRecorderError),
    Transport(HttpTransportFailure),
    RecordedTransportFailure(RecordedHttpTransportFailure),
    RedirectFailure(HttpRedirectFailure),
    RetryExhausted(HttpRetryExhaustionError),
    CounterOverflow,
    ProgressPartialCommit(HttpProgressPartialCommit),
}

impl fmt::Display for HttpExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmanagedContext => {
                formatter.write_str("HTTP execute is unavailable in unmanaged context")
            }
            Self::TransportConfiguration(_) => {
                formatter.write_str("HTTP transport configuration failed")
            }
            Self::SessionValidation(_) => {
                formatter.write_str("HTTP execution session validation failed")
            }
            Self::Recorder(_) => formatter.write_str("HTTP transaction recording failed"),
            Self::Transport(_) => formatter.write_str("HTTP transport exchange failed"),
            Self::RecordedTransportFailure(_) => {
                formatter.write_str("HTTP transport exchange failed after durable recording")
            }
            Self::RedirectFailure(_) => formatter.write_str("HTTP redirect handling failed"),
            Self::RetryExhausted(_) => formatter.write_str("HTTP retry policy exhausted"),
            Self::CounterOverflow => formatter.write_str("HTTP execution counter overflow"),
            Self::ProgressPartialCommit(_) => {
                formatter.write_str("HTTP acquisition progress persistence failed")
            }
        }
    }
}

impl std::error::Error for HttpExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TransportConfiguration(error) => Some(error),
            Self::SessionValidation(error) => Some(error),
            Self::Recorder(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::RecordedTransportFailure(error) => Some(error),
            Self::RedirectFailure(error) => Some(error),
            Self::RetryExhausted(error) => Some(error),
            Self::ProgressPartialCommit(error) => Some(error),
            Self::UnmanagedContext | Self::CounterOverflow => None,
        }
    }
}
