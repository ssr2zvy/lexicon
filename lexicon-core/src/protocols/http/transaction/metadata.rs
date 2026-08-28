use serde::{Deserialize, Serialize};

pub const HTTP_TRANSACTION_SCHEMA_VERSION: u32 = 1;
pub const HTTP_ACQUISITION_PROGRESS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredHeader {
    pub name: String,
    pub value: StoredHeaderValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "encoding", content = "value", rename_all = "snake_case")]
pub enum StoredHeaderValue {
    Utf8(String),
    Base64(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestMetadataDocument {
    pub schema_version: u32,
    pub transaction_id: String,
    pub session_id: String,
    pub physical_attempt_index: u32,
    pub redirect_index: u32,
    pub retry_index: u32,
    pub parent_transaction_id: Option<String>,
    pub logical_request_key: Option<String>,
    pub method: String,
    pub url: String,
    pub headers: Vec<StoredHeader>,
    pub has_body: bool,
    pub body_length: u64,
    pub body_sha256: Option<String>,
    /// Nanoseconds since the Unix epoch (u64; valid for dates through roughly year 2554).
    pub created_at_unix_nanos: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ResponseOutcomeDocument {
    Response {
        status: u16,
        http_version: Option<String>,
        headers: Vec<StoredHeader>,
        body_length: u64,
        body_sha256: String,
        /// Nanoseconds since the Unix epoch.
        completed_at_unix_nanos: u64,
    },
    TransportFailure {
        failure_class: String,
        retryable: bool,
        /// Nanoseconds since the Unix epoch.
        failed_at_unix_nanos: u64,
    },
    IncompleteResponse {
        /// Nanoseconds since the Unix epoch.
        failed_at_unix_nanos: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseMetadataDocument {
    pub schema_version: u32,
    pub transaction_id: String,
    #[serde(flatten)]
    pub outcome: ResponseOutcomeDocument,
}

/// Acquisition progress document. Opaque outside this module; construct through
/// `AcquisitionProgressDocument::new_initial` and `AcquisitionProgressDocument::advance`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquisitionProgressDocument {
    pub schema_version: u32,
    pub session_id: String,
    pub completed_transaction_count: u64,
    pub transport_failure_count: u64,
    pub redirect_count: u64,
    pub retry_count: u64,
    pub last_transaction_id: Option<String>,
    pub last_logical_request_key: Option<String>,
    /// Nanoseconds since the Unix epoch.
    pub updated_at_unix_nanos: u64,
    pub revision: u64,
}

/// Errors that can arise when validating a deserialized progress document.
#[derive(Debug)]
pub enum AcquisitionProgressValidationError {
    UnknownSchemaVersion { found: u32 },
    EmptySessionId,
    SessionMismatch,
    RevisionNotMonotonic { expected: u64, found: u64 },
    LastTransactionIdInconsistent,
    CounterOverflow,
}

impl std::fmt::Display for AcquisitionProgressValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSchemaVersion { found } => {
                write!(f, "unknown acquisition progress schema version: {found}")
            }
            Self::EmptySessionId => f.write_str("acquisition progress session_id is empty"),
            Self::SessionMismatch => f.write_str("acquisition progress session_id does not match"),
            Self::RevisionNotMonotonic { expected, found } => {
                write!(f, "acquisition progress revision not monotonic: expected {expected}, found {found}")
            }
            Self::LastTransactionIdInconsistent => {
                f.write_str("acquisition progress last_transaction_id inconsistent with completed_transaction_count")
            }
            Self::CounterOverflow => f.write_str("acquisition progress counter would overflow"),
        }
    }
}

impl std::error::Error for AcquisitionProgressValidationError {}

impl AcquisitionProgressDocument {
    pub fn new_initial(session_id: String, now_nanos: u64) -> Self {
        Self {
            schema_version: HTTP_ACQUISITION_PROGRESS_SCHEMA_VERSION,
            session_id,
            completed_transaction_count: 0,
            transport_failure_count: 0,
            redirect_count: 0,
            retry_count: 0,
            last_transaction_id: None,
            last_logical_request_key: None,
            updated_at_unix_nanos: now_nanos,
            revision: 0,
        }
    }

    /// Validate invariants of an existing document loaded from disk.
    pub fn validate_existing(
        doc: &AcquisitionProgressDocument,
        session_id: &str,
        expected_revision_min: u64,
    ) -> Result<(), AcquisitionProgressValidationError> {
        if doc.schema_version != HTTP_ACQUISITION_PROGRESS_SCHEMA_VERSION {
            return Err(AcquisitionProgressValidationError::UnknownSchemaVersion {
                found: doc.schema_version,
            });
        }
        if doc.session_id.is_empty() {
            return Err(AcquisitionProgressValidationError::EmptySessionId);
        }
        if doc.session_id != session_id {
            return Err(AcquisitionProgressValidationError::SessionMismatch);
        }
        if doc.revision < expected_revision_min {
            return Err(AcquisitionProgressValidationError::RevisionNotMonotonic {
                expected: expected_revision_min,
                found: doc.revision,
            });
        }
        // last_transaction_id must be Some when there have been completed transactions.
        if doc.completed_transaction_count > 0 && doc.last_transaction_id.is_none() {
            return Err(AcquisitionProgressValidationError::LastTransactionIdInconsistent);
        }
        if doc.completed_transaction_count == 0 && doc.last_transaction_id.is_some() {
            return Err(AcquisitionProgressValidationError::LastTransactionIdInconsistent);
        }
        Ok(())
    }
}
