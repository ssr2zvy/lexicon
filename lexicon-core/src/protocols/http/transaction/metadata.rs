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
    pub created_at: String,
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
        completed_at: String,
    },
    TransportFailure {
        failure_class: String,
        retryable: bool,
        failed_at: String,
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
    pub updated_at: String,
    pub revision: u64,
}
