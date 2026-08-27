use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::runtime::{OwnedRuntimeIdentity, RuntimeExecutionMode, RuntimeOperation, RuntimeSupervisionMode};
use crate::runtime::{ProjectInvocationIdentity, SessionInvocationIdentity};

/// Stable type aliases used throughout the session module.
pub type ProjectIdentity = ProjectInvocationIdentity;
pub type SessionIdentity = SessionInvocationIdentity;

// ---------------------------------------------------------------------------
// Schema version
// ---------------------------------------------------------------------------

pub const SESSION_SCHEMA_VERSION: u32 = 1;

/// Maximum bytes stored in a persisted failure summary.
pub const MAX_FAILURE_SUMMARY_BYTES: usize = 4096;

// ---------------------------------------------------------------------------
// Clock abstraction
// ---------------------------------------------------------------------------

pub trait SessionClock: Send + Sync {
    fn now(&self) -> SessionTimestamp;
}

pub struct SystemClock;

impl SessionClock for SystemClock {
    fn now(&self) -> SessionTimestamp {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        SessionTimestamp::from_nanos_since_epoch(nanos)
    }
}

// ---------------------------------------------------------------------------
// SessionTimestamp
// ---------------------------------------------------------------------------

/// A UTC timestamp stored as nanoseconds since the Unix epoch.
///
/// Formatting is deterministic and locale-independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionTimestamp {
    nanos_since_epoch: u64,
}

impl SessionTimestamp {
    pub fn from_nanos_since_epoch(nanos: u64) -> Self {
        Self { nanos_since_epoch: nanos }
    }

    pub fn nanos_since_epoch(self) -> u64 {
        self.nanos_since_epoch
    }

    /// Format as an ISO 8601-like UTC string: `YYYY-MM-DDTHH:MM:SS.nnnnnnnnnZ`.
    pub fn to_rfc3339_nanos(&self) -> String {
        let secs = self.nanos_since_epoch / 1_000_000_000;
        let nanos = (self.nanos_since_epoch % 1_000_000_000) as u32;
        fmt_unix_timestamp(secs, nanos)
    }
}

impl fmt::Display for SessionTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339_nanos())
    }
}

/// Minimal locale-independent UTC timestamp formatter.
fn fmt_unix_timestamp(secs: u64, nanos: u32) -> String {
    // Days from epoch up to year boundaries (simplified Gregorian)
    let mut days = secs / 86400;
    let time = secs % 86400;

    let hour = time / 3600;
    let minute = (time % 3600) / 60;
    let second = time % 60;

    // Gregorian calendar reconstruction
    // Using algorithm from https://www.howardhinnant.com/date_algorithms.html#civil_from_days
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        year, month, day, hour, minute, second, nanos
    )
}

// ---------------------------------------------------------------------------
// SessionOperation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOperation {
    Acquisition,
    Processing,
}

impl SessionOperation {
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Acquisition => "acquisition",
            Self::Processing => "processing",
        }
    }

    pub fn from_runtime_operation(op: RuntimeOperation) -> Self {
        match op {
            RuntimeOperation::Acquisition => Self::Acquisition,
            RuntimeOperation::Processing => Self::Processing,
        }
    }

    pub fn to_runtime_operation(self) -> RuntimeOperation {
        match self {
            Self::Acquisition => RuntimeOperation::Acquisition,
            Self::Processing => RuntimeOperation::Processing,
        }
    }
}

// ---------------------------------------------------------------------------
// SessionState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Prepared,
    Running,
    Succeeded,
    Failed,
    Abandoned,
}

impl SessionState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Abandoned)
    }
}

// ---------------------------------------------------------------------------
// SessionFailureKind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionFailureKind {
    Source,
    Runtime,
    AbnormalTermination,
    StaleOwnership,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionFailureCode {
    SourceReturnedError,
    RuntimeInitializationFailed,
    RuntimeContextInvalid,
    HandlerStateUnavailable,
    LaunchFailed,
    AbnormalTermination,
    StaleOwnership,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeSessionFailure {
    kind: SessionFailureKind,
    code: SessionFailureCode,
    diagnostic: Option<String>,
}

impl SafeSessionFailure {
    pub fn new(
        kind: SessionFailureKind,
        code: SessionFailureCode,
        diagnostic: Option<String>,
    ) -> Self {
        let diagnostic = diagnostic.map(|s| truncate_to_bytes(s, MAX_FAILURE_SUMMARY_BYTES));
        Self {
            kind,
            code,
            diagnostic,
        }
    }

    pub fn source_failure() -> Self {
        Self::new(
            SessionFailureKind::Source,
            SessionFailureCode::SourceReturnedError,
            Some("source handler returned an error".to_string()),
        )
    }

    pub fn runtime_failure(code: SessionFailureCode, diagnostic: Option<String>) -> Self {
        Self::new(SessionFailureKind::Runtime, code, diagnostic)
    }

    pub fn stale_ownership_failure() -> Self {
        Self::new(
            SessionFailureKind::StaleOwnership,
            SessionFailureCode::StaleOwnership,
            Some("stale session ownership: prior process terminated without completing".to_string()),
        )
    }
}

// ---------------------------------------------------------------------------
// SessionFailureV1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionFailureV1 {
    kind: SessionFailureKind,
    code: SessionFailureCode,
    /// Concise Core-authored diagnostic, bounded by `MAX_FAILURE_SUMMARY_BYTES`.
    diagnostic: Option<String>,
}

impl SessionFailureV1 {
    pub fn new(
        kind: SessionFailureKind,
        code: SessionFailureCode,
        diagnostic: Option<String>,
    ) -> Self {
        let diagnostic = diagnostic.map(|s| truncate_to_bytes(s, MAX_FAILURE_SUMMARY_BYTES));
        Self {
            kind,
            code,
            diagnostic,
        }
    }

    pub fn from_safe(failure: SafeSessionFailure) -> Self {
        Self::new(failure.kind, failure.code, failure.diagnostic)
    }

    pub fn kind(&self) -> SessionFailureKind {
        self.kind
    }

    pub fn code(&self) -> SessionFailureCode {
        self.code
    }

    pub fn diagnostic(&self) -> Option<&str> {
        self.diagnostic.as_deref()
    }
}

fn truncate_to_bytes(mut s: String, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s;
    }
    // Truncate at a char boundary
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    s
}

// ---------------------------------------------------------------------------
// Session identity generation
// ---------------------------------------------------------------------------

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a new valid session identifier.
///
/// Format: `{timestamp_ms_hex}-{pid_hex}-{counter_hex}`
pub fn generate_session_id() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let pid = std::process::id() as u64;
    let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{ms:016x}-{pid:08x}-{counter:016x}")
}

// ---------------------------------------------------------------------------
// SessionTransition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTransition {
    ToRunning,
    ToSucceeded,
    ToFailed {
        failure: SafeSessionFailure,
    },
    ToAbandoned,
}

impl SessionTransition {
    pub fn target_state(&self) -> SessionState {
        match self {
            Self::ToRunning => SessionState::Running,
            Self::ToSucceeded => SessionState::Succeeded,
            Self::ToFailed { .. } => SessionState::Failed,
            Self::ToAbandoned => SessionState::Abandoned,
        }
    }
}

// ---------------------------------------------------------------------------
// NewSessionRecord
// ---------------------------------------------------------------------------

/// Input for creating a new Prepared session record.
pub struct NewSessionRecord {
    pub project: ProjectIdentity,
    pub runtime: OwnedRuntimeIdentity,
    pub operation: SessionOperation,
    pub execution_mode: RuntimeExecutionMode,
    pub supervision_mode: RuntimeSupervisionMode,
}

// ---------------------------------------------------------------------------
// Session serde documents (internal)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionRecordDocument {
    schema_version: u32,
    project: ProjectDocument,
    runtime: RuntimeDocument,
    session: SessionDocument,
    operation: SessionOperation,
    execution_mode: String,
    supervision_mode: String,
    state: SessionState,
    revision: u64,
    created_at: SessionTimestamp,
    updated_at: SessionTimestamp,
    started_at: Option<SessionTimestamp>,
    finished_at: Option<SessionTimestamp>,
    failure: Option<SessionFailureV1>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectDocument {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeDocument {
    source: String,
    protocol: String,
    operation: String,
    source_contract_version: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionDocument {
    id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionStatusDocument {
    schema_version: u32,
    project: ProjectDocument,
    runtime: RuntimeDocument,
    operation: SessionOperation,
    current_session: Option<SessionDocument>,
    current_state: Option<SessionState>,
    revision: u64,
    updated_at: SessionTimestamp,
}

// ---------------------------------------------------------------------------
// SessionRecordV1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecordV1 {
    schema_version: u32,
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
    operation: SessionOperation,
    execution_mode: RuntimeExecutionMode,
    supervision_mode: RuntimeSupervisionMode,
    pub(super) state: SessionState,
    pub(super) revision: u64,
    created_at: SessionTimestamp,
    pub(super) updated_at: SessionTimestamp,
    pub(super) started_at: Option<SessionTimestamp>,
    pub(super) finished_at: Option<SessionTimestamp>,
    pub(super) failure: Option<SessionFailureV1>,
}

impl SessionRecordV1 {
    pub(crate) fn new_prepared(
        input: NewSessionRecord,
        session: SessionIdentity,
        clock: &dyn SessionClock,
    ) -> Self {
        let now = clock.now();
        Self {
            schema_version: SESSION_SCHEMA_VERSION,
            project: input.project,
            runtime: input.runtime,
            session,
            operation: input.operation,
            execution_mode: input.execution_mode,
            supervision_mode: input.supervision_mode,
            state: SessionState::Prepared,
            revision: 0,
            created_at: now,
            updated_at: now,
            started_at: None,
            finished_at: None,
            failure: None,
        }
    }

    pub fn schema_version(&self) -> u32 { self.schema_version }
    pub fn project(&self) -> &ProjectIdentity { &self.project }
    pub fn runtime(&self) -> &OwnedRuntimeIdentity { &self.runtime }
    pub fn session(&self) -> &SessionIdentity { &self.session }
    pub fn operation(&self) -> SessionOperation { self.operation }
    pub fn execution_mode(&self) -> RuntimeExecutionMode { self.execution_mode }
    pub fn supervision_mode(&self) -> RuntimeSupervisionMode { self.supervision_mode }
    pub fn state(&self) -> SessionState { self.state }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn created_at(&self) -> SessionTimestamp { self.created_at }
    pub fn updated_at(&self) -> SessionTimestamp { self.updated_at }
    pub fn started_at(&self) -> Option<SessionTimestamp> { self.started_at }
    pub fn finished_at(&self) -> Option<SessionTimestamp> { self.finished_at }
    pub fn failure(&self) -> Option<&SessionFailureV1> { self.failure.as_ref() }

    pub fn to_json(&self) -> Result<String, crate::session::error::SessionEncodingError> {
        let doc = self.to_document();
        serde_json::to_string_pretty(&doc)
            .map_err(|e| crate::session::error::SessionEncodingError::Serialization(e.to_string()))
    }

    pub fn from_json(json: &str) -> Result<Self, crate::session::error::SessionDecodingError> {
        use crate::runtime::{RuntimeProtocol, RuntimeOperation, RuntimeExecutionMode, RuntimeSupervisionMode, ProjectInvocationIdentity, SessionInvocationIdentity};
        use crate::session::error::SessionDecodingError;

        let doc: SessionRecordDocument = serde_json::from_str(json).map_err(|e| {
            if e.is_syntax() || e.is_eof() {
                SessionDecodingError::JsonSyntax(e.to_string())
            } else if e.is_data() {
                SessionDecodingError::StructuralDocument(e.to_string())
            } else {
                SessionDecodingError::StructuralDocument(e.to_string())
            }
        })?;

        if doc.schema_version != SESSION_SCHEMA_VERSION {
            return Err(SessionDecodingError::UnknownSchemaVersion(doc.schema_version));
        }

        // Validate state-dependent field invariants before any partial moves.
        validate_record_invariants(&doc)?;

        let project = ProjectInvocationIdentity::new(doc.project.name).map_err(|e| {
            SessionDecodingError::InvalidInvariant(format!("invalid project name: {e}"))
        })?;

        let protocol = RuntimeProtocol::from_identifier(&doc.runtime.protocol)
            .map_err(|_| SessionDecodingError::UnknownField { field: "protocol", value: doc.runtime.protocol.clone() })?;

        let runtime_op = RuntimeOperation::from_identifier(&doc.runtime.operation)
            .map_err(|_| SessionDecodingError::UnknownField { field: "operation", value: doc.runtime.operation.clone() })?;

        let runtime = match (protocol, runtime_op) {
            (RuntimeProtocol::Http, RuntimeOperation::Acquisition) => {
                OwnedRuntimeIdentity::http_acquisition(&doc.runtime.source, doc.runtime.source_contract_version)
            }
            (RuntimeProtocol::Http, RuntimeOperation::Processing) => {
                OwnedRuntimeIdentity::http_processing(&doc.runtime.source, doc.runtime.source_contract_version)
            }
        };

        let session = SessionInvocationIdentity::new(doc.session.id).map_err(|e| {
            SessionDecodingError::InvalidInvariant(format!("invalid session id: {e}"))
        })?;

        let execution_mode = RuntimeExecutionMode::from_identifier(&doc.execution_mode)
            .map_err(|_| SessionDecodingError::UnknownField { field: "execution_mode", value: doc.execution_mode.clone() })?;

        let supervision_mode = RuntimeSupervisionMode::from_identifier(&doc.supervision_mode)
            .map_err(|_| SessionDecodingError::UnknownField { field: "supervision_mode", value: doc.supervision_mode.clone() })?;

        // Validate operation agrees with runtime operation
        let session_op_from_runtime = SessionOperation::from_runtime_operation(runtime_op);
        if session_op_from_runtime != doc.operation {
            return Err(SessionDecodingError::InvalidInvariant(
                "session operation does not agree with runtime operation".to_string()
            ));
        }

        Ok(Self {
            schema_version: doc.schema_version,
            project,
            runtime,
            session,
            operation: doc.operation,
            execution_mode,
            supervision_mode,
            state: doc.state,
            revision: doc.revision,
            created_at: doc.created_at,
            updated_at: doc.updated_at,
            started_at: doc.started_at,
            finished_at: doc.finished_at,
            failure: doc.failure,
        })
    }

    fn to_document(&self) -> SessionRecordDocument {
        use crate::runtime::RuntimeProtocol;
        let protocol = match self.runtime.protocol() {
            RuntimeProtocol::Http => "http",
        };
        let operation_str = self.runtime.operation().identifier();
        SessionRecordDocument {
            schema_version: self.schema_version,
            project: ProjectDocument { name: self.project.name().to_string() },
            runtime: RuntimeDocument {
                source: self.runtime.source_name().to_string(),
                protocol: protocol.to_string(),
                operation: operation_str.to_string(),
                source_contract_version: self.runtime.source_contract_version(),
            },
            session: SessionDocument { id: self.session.id().to_string() },
            operation: self.operation,
            execution_mode: self.execution_mode.identifier().to_string(),
            supervision_mode: self.supervision_mode.identifier().to_string(),
            state: self.state,
            revision: self.revision,
            created_at: self.created_at,
            updated_at: self.updated_at,
            started_at: self.started_at,
            finished_at: self.finished_at,
            failure: self.failure.clone(),
        }
    }
}

fn validate_record_invariants(
    doc: &SessionRecordDocument,
) -> Result<(), crate::session::error::SessionDecodingError> {
    use crate::session::error::SessionDecodingError;

    // started_at must exist only after Running
    match doc.state {
        SessionState::Prepared => {
            if doc.started_at.is_some() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "started_at must not be set in Prepared state".to_string(),
                ));
            }
            if doc.finished_at.is_some() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "finished_at must not be set in Prepared state".to_string(),
                ));
            }
            if doc.failure.is_some() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "failure must not be set in Prepared state".to_string(),
                ));
            }
        }
        SessionState::Running => {
            if doc.started_at.is_none() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "started_at must be set in Running state".to_string(),
                ));
            }
            if doc.finished_at.is_some() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "finished_at must not be set in Running state".to_string(),
                ));
            }
            if doc.failure.is_some() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "failure must not be set in Running state".to_string(),
                ));
            }
        }
        SessionState::Succeeded => {
            if doc.started_at.is_none() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "started_at must be set in Succeeded state".to_string(),
                ));
            }
            if doc.finished_at.is_none() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "finished_at must be set in Succeeded state".to_string(),
                ));
            }
            if doc.failure.is_some() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "failure must not be set in Succeeded state".to_string(),
                ));
            }
        }
        SessionState::Failed => {
            if doc.finished_at.is_none() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "finished_at must be set in Failed state".to_string(),
                ));
            }
            if doc.failure.is_none() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "failure must be set in Failed state".to_string(),
                ));
            }
        }
        SessionState::Abandoned => {
            if doc.finished_at.is_none() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "finished_at must be set in Abandoned state".to_string(),
                ));
            }
            if doc.failure.is_some() {
                return Err(SessionDecodingError::InvalidInvariant(
                    "failure must not be set in Abandoned state".to_string(),
                ));
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// SessionStatusV1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStatusV1 {
    schema_version: u32,
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    operation: SessionOperation,
    current_session: Option<SessionIdentity>,
    current_state: Option<SessionState>,
    revision: u64,
    updated_at: SessionTimestamp,
}

impl SessionStatusV1 {
    pub(crate) fn from_record(record: &SessionRecordV1, clock: &dyn SessionClock) -> Self {
        Self {
            schema_version: SESSION_SCHEMA_VERSION,
            project: record.project.clone(),
            runtime: record.runtime.clone(),
            operation: record.operation,
            current_session: Some(record.session.clone()),
            current_state: Some(record.state),
            revision: record.revision,
            updated_at: clock.now(),
        }
    }

    pub fn schema_version(&self) -> u32 { self.schema_version }
    pub fn project(&self) -> &ProjectIdentity { &self.project }
    pub fn runtime(&self) -> &OwnedRuntimeIdentity { &self.runtime }
    pub fn operation(&self) -> SessionOperation { self.operation }
    pub fn current_session(&self) -> Option<&SessionIdentity> { self.current_session.as_ref() }
    pub fn current_state(&self) -> Option<SessionState> { self.current_state }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn updated_at(&self) -> SessionTimestamp { self.updated_at }

    pub fn to_json(&self) -> Result<String, crate::session::error::SessionEncodingError> {
        let doc = self.to_document();
        serde_json::to_string_pretty(&doc)
            .map_err(|e| crate::session::error::SessionEncodingError::Serialization(e.to_string()))
    }

    pub fn from_json(json: &str) -> Result<Self, crate::session::error::SessionDecodingError> {
        use crate::runtime::{ProjectInvocationIdentity, SessionInvocationIdentity};
        use crate::runtime::{RuntimeProtocol, RuntimeOperation};
        use crate::session::error::SessionDecodingError;

        let doc: SessionStatusDocument = serde_json::from_str(json).map_err(|e| {
            if e.is_syntax() || e.is_eof() {
                SessionDecodingError::JsonSyntax(e.to_string())
            } else {
                SessionDecodingError::StructuralDocument(e.to_string())
            }
        })?;

        if doc.schema_version != SESSION_SCHEMA_VERSION {
            return Err(SessionDecodingError::UnknownSchemaVersion(doc.schema_version));
        }

        let project = ProjectInvocationIdentity::new(doc.project.name).map_err(|e| {
            SessionDecodingError::InvalidInvariant(format!("invalid project name: {e}"))
        })?;

        let protocol = RuntimeProtocol::from_identifier(&doc.runtime.protocol)
            .map_err(|_| SessionDecodingError::UnknownField { field: "protocol", value: doc.runtime.protocol.clone() })?;

        let runtime_op = RuntimeOperation::from_identifier(&doc.runtime.operation)
            .map_err(|_| SessionDecodingError::UnknownField { field: "operation", value: doc.runtime.operation.clone() })?;

        let runtime = match (protocol, runtime_op) {
            (RuntimeProtocol::Http, RuntimeOperation::Acquisition) => {
                OwnedRuntimeIdentity::http_acquisition(&doc.runtime.source, doc.runtime.source_contract_version)
            }
            (RuntimeProtocol::Http, RuntimeOperation::Processing) => {
                OwnedRuntimeIdentity::http_processing(&doc.runtime.source, doc.runtime.source_contract_version)
            }
        };

        let current_session = doc.current_session
            .map(|s| SessionInvocationIdentity::new(s.id)
                .map_err(|e| SessionDecodingError::InvalidInvariant(format!("invalid session id: {e}"))))
            .transpose()?;

        // Validate operation agrees with runtime operation
        let session_op_from_runtime = SessionOperation::from_runtime_operation(runtime_op);
        if session_op_from_runtime != doc.operation {
            return Err(SessionDecodingError::InvalidInvariant(
                "session operation does not agree with runtime operation".to_string()
            ));
        }

        Ok(Self {
            schema_version: doc.schema_version,
            project,
            runtime,
            operation: doc.operation,
            current_session,
            current_state: doc.current_state,
            revision: doc.revision,
            updated_at: doc.updated_at,
        })
    }

    fn to_document(&self) -> SessionStatusDocument {
        use crate::runtime::RuntimeProtocol;
        let protocol = match self.runtime.protocol() {
            RuntimeProtocol::Http => "http",
        };
        SessionStatusDocument {
            schema_version: self.schema_version,
            project: ProjectDocument { name: self.project.name().to_string() },
            runtime: RuntimeDocument {
                source: self.runtime.source_name().to_string(),
                protocol: protocol.to_string(),
                operation: self.runtime.operation().identifier().to_string(),
                source_contract_version: self.runtime.source_contract_version(),
            },
            operation: self.operation,
            current_session: self.current_session.as_ref().map(|s| SessionDocument { id: s.id().to_string() }),
            current_state: self.current_state,
            revision: self.revision,
            updated_at: self.updated_at,
        }
    }
}
