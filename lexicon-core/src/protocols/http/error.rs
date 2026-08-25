use std::fmt;

pub type AcquisitionResult<T> = Result<T, AcquisitionError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquisitionError {
    message: String,
}

impl AcquisitionError {
    pub fn source_message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AcquisitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AcquisitionError {}
