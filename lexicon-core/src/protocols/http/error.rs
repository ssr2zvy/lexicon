use std::fmt;

use super::request::HttpRequestError;
use super::transaction::HttpResponseStatusError;
use super::transport::{HttpTransportConfigurationError, HttpTransportFailure};
use super::{ProgressPersistenceError, SessionValidationError};
use crate::protocols::http::transaction::error::HttpRecorderError;

pub type AcquisitionResult<T> = Result<T, AcquisitionError>;

#[derive(Debug)]
pub enum AcquisitionError {
    Source {
        message: String,
    },
    Request(HttpRequestError),
    Execution(HttpExecutionError),
    ResponseStatus(HttpResponseStatusError),
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

    pub fn message(&self) -> &str {
        match self {
            Self::Source { message } => message,
            Self::Request(_) => "request error",
            Self::Execution(_) => "execution error",
            Self::ResponseStatus(_) => "response status error",
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
        }
    }
}

impl std::error::Error for AcquisitionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Request(error) => Some(error),
            Self::Execution(error) => Some(error),
            Self::ResponseStatus(error) => Some(error),
            Self::Source { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum HttpExecutionError {
    UnmanagedContext,
    TransportConfiguration(HttpTransportConfigurationError),
    SessionValidation(SessionValidationError),
    Recorder(HttpRecorderError),
    Transport(HttpTransportFailure),
    RedirectExhausted,
    RedirectLoop,
    InvalidRedirectTarget,
    RetryExhausted,
    CounterOverflow,
    Progress(ProgressPersistenceError),
}

impl fmt::Display for HttpExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmanagedContext => formatter.write_str("HTTP execute is unavailable in unmanaged context"),
            Self::TransportConfiguration(_) => formatter.write_str("HTTP transport configuration failed"),
            Self::SessionValidation(_) => formatter.write_str("HTTP execution session validation failed"),
            Self::Recorder(_) => formatter.write_str("HTTP transaction recording failed"),
            Self::Transport(_) => formatter.write_str("HTTP transport exchange failed"),
            Self::RedirectExhausted => formatter.write_str("HTTP redirect policy exhausted"),
            Self::RedirectLoop => formatter.write_str("HTTP redirect loop detected"),
            Self::InvalidRedirectTarget => formatter.write_str("HTTP redirect target is invalid or unsupported"),
            Self::RetryExhausted => formatter.write_str("HTTP retry policy exhausted"),
            Self::CounterOverflow => formatter.write_str("HTTP execution counter overflow"),
            Self::Progress(_) => formatter.write_str("HTTP acquisition progress persistence failed"),
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
            Self::Progress(error) => Some(error),
            Self::UnmanagedContext
            | Self::RedirectExhausted
            | Self::RedirectLoop
            | Self::InvalidRedirectTarget
            | Self::RetryExhausted
            | Self::CounterOverflow => None,
        }
    }
}
