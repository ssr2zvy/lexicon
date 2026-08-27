use std::fmt;

#[derive(Debug)]
pub enum HttpRecorderError {
    InvalidManagedRoot,
    SymlinkRejected,
    DirectoryCreation,
    MetadataEncoding,
    MetadataPersistence,
    BodyPersistence,
    BodyStreaming,
    BodyHashing,
    DurableSync,
    AtomicFinalize,
    FinalPathCollision,
}

impl fmt::Display for HttpRecorderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManagedRoot => formatter.write_str("invalid managed transaction root"),
            Self::SymlinkRejected => formatter.write_str("managed transaction path cannot be a symlink"),
            Self::DirectoryCreation => formatter.write_str("failed to create transaction staging directory"),
            Self::MetadataEncoding => formatter.write_str("failed to encode HTTP transaction metadata"),
            Self::MetadataPersistence => formatter.write_str("failed to persist HTTP transaction metadata"),
            Self::BodyPersistence => formatter.write_str("failed to persist HTTP body data"),
            Self::BodyStreaming => formatter.write_str("failed to stream HTTP response body"),
            Self::BodyHashing => formatter.write_str("failed to hash HTTP body"),
            Self::DurableSync => formatter.write_str("failed to durably sync HTTP transaction"),
            Self::AtomicFinalize => formatter.write_str("failed to finalize HTTP transaction atomically"),
            Self::FinalPathCollision => formatter.write_str("final HTTP transaction path collision"),
        }
    }
}

impl std::error::Error for HttpRecorderError {}
