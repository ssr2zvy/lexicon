use std::ffi::OsString;

/// Which data operation to execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataOperation {
    Acquisition,
    Processing,
}

impl DataOperation {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Acquisition => "acquisition",
            Self::Processing => "processing",
        }
    }
}

/// A typed foreground data execution request.
pub struct ForegroundDataRequest {
    pub operation: DataOperation,
    pub source_name: String,
    pub abandon_past_failure: bool,
    pub background: bool,
    pub source_arguments: Vec<OsString>,
}
