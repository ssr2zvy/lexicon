use std::fmt;

pub type ProcessingResult<T> = Result<T, ProcessingError>;

#[derive(Debug)]
pub struct ProcessingError;

impl fmt::Display for ProcessingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("processing failed")
    }
}

impl std::error::Error for ProcessingError {}
