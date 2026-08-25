use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcquisitionError {
    Source(String),
    Arguments(String),
    Context(String),
    Other(String),
}

impl AcquisitionError {
    pub fn source(message: impl Into<String>) -> Self {
        Self::Source(message.into())
    }

    pub fn arguments(message: impl Into<String>) -> Self {
        Self::Arguments(message.into())
    }

    pub fn context(message: impl Into<String>) -> Self {
        Self::Context(message.into())
    }

    pub fn source_message(message: impl Into<String>) -> Self {
        Self::Source(message.into())
    }

    pub fn arguments_from<E: fmt::Display>(error: E) -> Self {
        Self::Arguments(error.to_string())
    }
}

impl fmt::Display for AcquisitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(message) => write!(formatter, "source acquisition failed: {message}"),
            Self::Arguments(message) => write!(formatter, "invalid arguments: {message}"),
            Self::Context(message) => write!(formatter, "source context failure: {message}"),
            Self::Other(message) => write!(formatter, "acquisition failed: {message}"),
        }
    }
}

impl std::error::Error for AcquisitionError {}

pub type AcquisitionResult<T> = Result<T, AcquisitionError>;
