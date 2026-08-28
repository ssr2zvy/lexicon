use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use crate::runtime::{RuntimeOperation, RuntimeProtocol};
use crate::session::{
    SessionDataPaths, SessionLeaseState, SessionOperation, SessionState, SessionStore,
    SessionStoreError,
};

use super::error::{AcquisitionError, AcquisitionResult, HttpExecutionError};
use super::policy::HttpRedirectPolicy;
use super::request::{FinalizedHttpRequest, HttpRequest, HttpRequestError, redact_url};
use super::transaction::metadata::{
    AcquisitionProgressDocument, AcquisitionProgressValidationError,
};
use super::transaction::{
    RecordedAttemptContext, RecordedTransaction, record_transaction_attempt,
};
use super::transport::{HttpTransport, HttpTransportConfigurationError, ReqwestHttpTransport};

/// Bound acquisition context provided to HTTP source handlers.
pub struct HttpAcquisitionContext {
    paths: SessionDataPaths,
    session_identity: Option<crate::session::SessionIdentity>,
    transport: Option<Box<dyn HttpTransport>>,
    transport_init_error: Option<HttpTransportConfigurationError>,
}

impl HttpAcquisitionContext {
    pub fn from_session_data_paths(
        paths: SessionDataPaths,
        session_identity: crate::session::SessionIdentity,
    ) -> Self {
        let (transport, transport_init_error) = match ReqwestHttpTransport::new() {
            Ok(t) => (Some(Box::new(t) as Box<dyn HttpTransport>), None),
            Err(e) => (None, Some(e)),
        };
        Self {
            paths,
            session_identity: Some(session_identity),
            transport,
            transport_init_error,
        }
    }

    pub fn source_directory(&self) -> &Path {
        self.paths.protocol_root()
    }

    pub fn protocol_root(&self) -> &Path {
        self.paths.protocol_root()
    }

    pub fn operation_root(&self) -> &Path {
        self.paths.operation_root()
    }

    pub fn session_directory(&self) -> &Path {
        self.paths.session_directory()
    }

    pub fn raw_data_directory(&self) -> &Path {
        self.paths.raw_data_directory()
    }

    pub fn processed_data_directory(&self) -> &Path {
        self.paths.processed_data_directory()
    }

    pub fn session_identity(&self) -> Option<&crate::session::SessionIdentity> {
        self.session_identity.as_ref()
    }

    #[doc(hidden)]
    pub fn from_env_legacy() -> Result<Self, String> {
        let value = std::env::var("LEXICON_SOURCE_DIRECTORY")
            .map_err(|_| "missing LEXICON_SOURCE_DIRECTORY; the runtime must supply the absolute source directory".to_string())?;
        let source_directory = PathBuf::from(value);

        if source_directory.is_relative() {
            return Err(format!(
                "invalid LEXICON_SOURCE_DIRECTORY: must be an absolute path"
            ));
        }

        if !source_directory.is_dir() {
            return Err("invalid LEXICON_SOURCE_DIRECTORY: path does not exist or is not a directory".to_string());
        }

        let raw_data_directory = source_directory.join("data/raw");
        let processed_data_directory = source_directory.join("data/processed");
        let operation_root = source_directory.join("get-raw-data");
        let session_directory = operation_root.join("sessions/legacy");

        let (transport, transport_init_error) = match ReqwestHttpTransport::new() {
            Ok(t) => (Some(Box::new(t) as Box<dyn HttpTransport>), None),
            Err(e) => (None, Some(e)),
        };

        Ok(Self {
            paths: SessionDataPaths::from_legacy_parts(
                source_directory,
                operation_root,
                session_directory,
                raw_data_directory,
                processed_data_directory,
            ),
            session_identity: None,
            transport,
            transport_init_error,
        })
    }

    pub fn execute(&mut self, request: HttpRequest) -> AcquisitionResult<RecordedTransaction> {
        // Validate session context.
        self.validate_for_execution()
            .map_err(|e| AcquisitionError::execution(HttpExecutionError::SessionValidation(e)))?;

        // Fail early if transport initialization failed.
        if let Some(ref e) = self.transport_init_error {
            return Err(AcquisitionError::execution(
                HttpExecutionError::TransportConfiguration(e.clone()),
            ));
        }

        let mut request = request.finalize().map_err(AcquisitionError::request)?;

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| AcquisitionError::execution(HttpExecutionError::UnmanagedContext))?;

        let session_id = self
            .session_identity
            .as_ref()
            .ok_or_else(|| AcquisitionError::execution(HttpExecutionError::UnmanagedContext))?
            .id()
            .to_string();

        let mut physical_attempt_index: u32 = 0;
        let mut redirect_index: u32 = 0;
        let mut parent_transaction_id: Option<String> = None;
        let mut seen_redirect_targets: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        // Insert initial effective URL before the first exchange (defect 20).
        seen_redirect_targets.insert(request.url.to_string());

        'redirect: loop {
            let max_attempts = request.retry_policy.maximum_attempts();
            let mut retry_index: u32 = 0;

            loop {
                physical_attempt_index = physical_attempt_index
                    .checked_add(1)
                    .ok_or_else(|| {
                        AcquisitionError::execution(HttpExecutionError::CounterOverflow)
                    })?;

                let record = record_transaction_attempt(
                    RecordedAttemptContext {
                        session_id: session_id.clone(),
                        raw_data_root: self.raw_data_directory().to_path_buf(),
                        logical_request_key: request.logical_key.clone(),
                        parent_transaction_id: parent_transaction_id.clone(),
                        physical_attempt_index,
                        redirect_index,
                        retry_index,
                        sensitive_query_names: request.sensitive_query_names.clone(),
                    },
                    &request,
                    transport.as_ref(),
                )
                .map_err(|e| AcquisitionError::execution(HttpExecutionError::Recorder(e)))?;

                // Determine status and redirect for orchestration.
                let status = record.transaction.response().status_code();
                let is_redirect_response = status.map(is_redirect_status).unwrap_or(false);
                let was_transport_failure = record.transport_failure.is_some();

                // Revalidate session and update progress after finalization.
                // All failures here are partial commits.
                let finalized_tx = record.transaction.clone();
                let progress_result = persist_progress(
                    self.session_directory(),
                    self.operation_root(),
                    &session_id,
                    self.session_identity.as_ref().unwrap(),
                    &finalized_tx,
                    request.logical_key.clone(),
                    was_transport_failure,
                    is_redirect_response,
                    retry_index > 0,
                );

                if let Err(err) = progress_result {
                    return Err(AcquisitionError::execution(HttpExecutionError::Progress(err)));
                }

                // Retry on transport failure if policy allows.
                if let Some(failure) = record.transport_failure {
                    if failure.retryable()
                        && request.retry_policy.retryable_transport_failures()
                        && retry_index
                            .checked_add(1)
                            .map(|next| next < max_attempts)
                            .unwrap_or(false)
                    {
                        retry_index = retry_index
                            .checked_add(1)
                            .ok_or_else(|| {
                                AcquisitionError::execution(HttpExecutionError::CounterOverflow)
                            })?;
                        parent_transaction_id =
                            Some(record.transaction.identity().id().to_string());
                        continue;
                    }

                    if failure.retryable() && request.retry_policy.retryable_transport_failures() && max_attempts > 1 {
                        return Err(AcquisitionError::execution(
                            HttpExecutionError::RetryExhausted,
                        ));
                    }

                    return Err(AcquisitionError::execution(HttpExecutionError::Transport(
                        failure,
                    )));
                }

                // Handle redirect.
                if is_redirect_response {
                    match request.redirect_policy {
                        HttpRedirectPolicy::None => return Ok(record.transaction),
                        HttpRedirectPolicy::Follow { maximum } => {
                            if redirect_index >= maximum {
                                return Err(AcquisitionError::execution(
                                    HttpExecutionError::RedirectExhausted,
                                ));
                            }

                            // Use effective Location from the actual transport response (defect 19).
                            let location =
                                record.effective_location.ok_or_else(|| {
                                    AcquisitionError::execution(
                                        HttpExecutionError::InvalidRedirectTarget,
                                    )
                                })?;

                            request = redirect_request_from(&request, status.unwrap_or(0), &location)
                                .map_err(AcquisitionError::request)?;

                            // Normalize and check for loop (defect 20).
                            let canonical = request.url.to_string();
                            if !seen_redirect_targets.insert(canonical) {
                                return Err(AcquisitionError::execution(
                                    HttpExecutionError::RedirectLoop,
                                ));
                            }

                            parent_transaction_id =
                                Some(record.transaction.identity().id().to_string());
                            redirect_index = redirect_index.checked_add(1).ok_or_else(|| {
                                AcquisitionError::execution(HttpExecutionError::CounterOverflow)
                            })?;
                            continue 'redirect;
                        }
                    }
                }

                // Retry on status.
                if request.retry_policy.should_retry_status(status.unwrap_or(0))
                    && retry_index
                        .checked_add(1)
                        .map(|next| next < max_attempts)
                        .unwrap_or(false)
                {
                    retry_index = retry_index.checked_add(1).ok_or_else(|| {
                        AcquisitionError::execution(HttpExecutionError::CounterOverflow)
                    })?;
                    parent_transaction_id = Some(record.transaction.identity().id().to_string());
                    continue;
                }

                if request.retry_policy.should_retry_status(status.unwrap_or(0)) && max_attempts > 1 {
                    return Err(AcquisitionError::execution(HttpExecutionError::RetryExhausted));
                }

                return Ok(record.transaction);
            }
        }
    }

    fn validate_for_execution(&self) -> Result<(), SessionValidationError> {
        if self.session_identity.is_none() {
            return Err(SessionValidationError::UnmanagedContext);
        }

        validate_managed_directory(self.raw_data_directory(), self.protocol_root())?;
        validate_managed_directory(self.session_directory(), self.operation_root())?;

        validate_running_acquisition_session(
            self.operation_root(),
            self.session_identity.as_ref().unwrap(),
        )
    }
}

// ---------------------------------------------------------------------------
// Session validation helpers
// ---------------------------------------------------------------------------

fn validate_running_acquisition_session(
    operation_root: &Path,
    session: &crate::session::SessionIdentity,
) -> Result<(), SessionValidationError> {
    let operation_root =
        crate::session::SessionOperationRoot::new(operation_root.to_path_buf())
            .map_err(|_| SessionValidationError::StoreOpen)?;
    let store = SessionStore::open(operation_root).map_err(|_| SessionValidationError::StoreOpen)?;

    let record = store.load(session).map_err(map_session_load_error)?;

    if record.state() != SessionState::Running {
        return Err(SessionValidationError::SessionNotRunning);
    }
    if record.operation() != SessionOperation::Acquisition {
        return Err(SessionValidationError::OperationMismatch);
    }
    if record.runtime().protocol() != RuntimeProtocol::Http
        || record.runtime().operation() != RuntimeOperation::Acquisition
    {
        return Err(SessionValidationError::RuntimeMismatch);
    }
    if record.session().id() != session.id() {
        return Err(SessionValidationError::SessionIdentityMismatch);
    }

    match store.inspect_lease_state(session) {
        Ok(SessionLeaseState::Owned) => Ok(()),
        Ok(SessionLeaseState::Available) => Err(SessionValidationError::LeaseUnavailable),
        Err(_) => Err(SessionValidationError::LeaseInspectionFailed),
    }
}

fn validate_managed_directory(path: &Path, root: &Path) -> Result<(), SessionValidationError> {
    if path.is_relative() || root.is_relative() {
        return Err(SessionValidationError::InvalidPaths);
    }
    if !path.starts_with(root) {
        return Err(SessionValidationError::InvalidPaths);
    }
    if path.exists() && path.is_symlink() {
        return Err(SessionValidationError::SymlinkRejected);
    }
    Ok(())
}

fn map_session_load_error(error: SessionStoreError) -> SessionValidationError {
    match error {
        SessionStoreError::MissingSession => SessionValidationError::MissingSession,
        _ => SessionValidationError::SessionLoadFailed,
    }
}

// ---------------------------------------------------------------------------
// Redirect helpers
// ---------------------------------------------------------------------------

fn redirect_request_from(
    request: &FinalizedHttpRequest,
    status: u16,
    location: &str,
) -> Result<FinalizedHttpRequest, HttpRequestError> {
    let next_url = request
        .url
        .join(location)
        .map_err(HttpRequestError::InvalidUrl)?;

    match next_url.scheme() {
        "http" | "https" => {}
        _ => return Err(HttpRequestError::UnsupportedScheme),
    }

    let cross_origin = request.url.scheme() != next_url.scheme()
        || request.url.domain() != next_url.domain()
        || request.url.port_or_known_default() != next_url.port_or_known_default();

    let mut next_headers = Vec::new();
    for header in &request.headers {
        let name = header.name.to_ascii_lowercase();
        let sensitive_cross_origin = matches!(
            name.as_str(),
            "authorization" | "proxy-authorization" | "cookie" | "host"
        );
        if cross_origin && (sensitive_cross_origin || header.sensitive) {
            continue;
        }
        next_headers.push(header.clone());
    }

    let (method, body) = match status {
        303 => {
            if request.method.eq_ignore_ascii_case("HEAD") {
                ("HEAD".to_string(), None)
            } else {
                ("GET".to_string(), None)
            }
        }
        307 | 308 => (request.method.clone(), request.body.clone()),
        _ => (request.method.clone(), request.body.clone()),
    };

    let redacted_url = redact_url(&next_url, &request.sensitive_query_names);
    Ok(FinalizedHttpRequest {
        method,
        redacted_url,
        url: next_url,
        headers: next_headers,
        body,
        logical_key: request.logical_key.clone(),
        retry_policy: request.retry_policy.clone(),
        redirect_policy: request.redirect_policy,
        sensitive_query_names: request.sensitive_query_names.clone(),
    })
}

fn is_redirect_status(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

// ---------------------------------------------------------------------------
// Progress persistence
// ---------------------------------------------------------------------------

fn persist_progress(
    session_directory: &Path,
    operation_root: &Path,
    session_id: &str,
    session_identity: &crate::session::SessionIdentity,
    transaction: &RecordedTransaction,
    logical_key: Option<String>,
    transport_failure: bool,
    redirect: bool,
    retry: bool,
) -> Result<(), ProgressPersistenceError> {
    // Revalidate session and supervisor lease before writing progress (defects 8 & 10).
    validate_running_acquisition_session(operation_root, session_identity).map_err(|e| {
        let prog_err = AcquisitionProgressError::from_session_validation(e);
        ProgressPersistenceError::PartialCommit {
            transaction_id: transaction.identity().id().to_string(),
            transaction_path: transaction.directory().to_path_buf(),
            source: Box::new(prog_err),
        }
    })?;

    let progress_path = session_directory.join("acquisition_progress.json");

    // Load existing progress document if present.
    let mut current = if progress_path.exists() {
        let text = std::fs::read_to_string(&progress_path).map_err(|e| {
            ProgressPersistenceError::PartialCommit {
                transaction_id: transaction.identity().id().to_string(),
                transaction_path: transaction.directory().to_path_buf(),
                source: Box::new(AcquisitionProgressError::Load(e)),
            }
        })?;

        let parsed: AcquisitionProgressDocument =
            serde_json::from_str(&text).map_err(|e| {
                ProgressPersistenceError::PartialCommit {
                    transaction_id: transaction.identity().id().to_string(),
                    transaction_path: transaction.directory().to_path_buf(),
                    source: Box::new(AcquisitionProgressError::Decode(e)),
                }
            })?;

        AcquisitionProgressDocument::validate_existing(&parsed, session_id, 1).map_err(|e| {
            ProgressPersistenceError::PartialCommit {
                transaction_id: transaction.identity().id().to_string(),
                transaction_path: transaction.directory().to_path_buf(),
                source: Box::new(AcquisitionProgressError::InvalidInvariant(e)),
            }
        })?;

        parsed
    } else {
        AcquisitionProgressDocument::new_initial(session_id.to_string(), now_nanos())
    };

    // Apply checked increments (defect 26).
    current.completed_transaction_count = current
        .completed_transaction_count
        .checked_add(1)
        .ok_or_else(|| ProgressPersistenceError::PartialCommit {
            transaction_id: transaction.identity().id().to_string(),
            transaction_path: transaction.directory().to_path_buf(),
            source: Box::new(AcquisitionProgressError::CounterOverflow),
        })?;

    if transport_failure {
        current.transport_failure_count = current
            .transport_failure_count
            .checked_add(1)
            .ok_or_else(|| ProgressPersistenceError::PartialCommit {
                transaction_id: transaction.identity().id().to_string(),
                transaction_path: transaction.directory().to_path_buf(),
                source: Box::new(AcquisitionProgressError::CounterOverflow),
            })?;
    }
    if redirect {
        current.redirect_count = current
            .redirect_count
            .checked_add(1)
            .ok_or_else(|| ProgressPersistenceError::PartialCommit {
                transaction_id: transaction.identity().id().to_string(),
                transaction_path: transaction.directory().to_path_buf(),
                source: Box::new(AcquisitionProgressError::CounterOverflow),
            })?;
    }
    if retry {
        current.retry_count = current
            .retry_count
            .checked_add(1)
            .ok_or_else(|| ProgressPersistenceError::PartialCommit {
                transaction_id: transaction.identity().id().to_string(),
                transaction_path: transaction.directory().to_path_buf(),
                source: Box::new(AcquisitionProgressError::CounterOverflow),
            })?;
    }

    current.revision = current.revision.checked_add(1).ok_or_else(|| {
        ProgressPersistenceError::PartialCommit {
            transaction_id: transaction.identity().id().to_string(),
            transaction_path: transaction.directory().to_path_buf(),
            source: Box::new(AcquisitionProgressError::CounterOverflow),
        }
    })?;

    current.last_transaction_id = Some(transaction.identity().id().to_string());
    current.last_logical_request_key = logical_key;
    current.updated_at_unix_nanos = now_nanos();

    write_progress_atomic(&progress_path, &current).map_err(|e| {
        ProgressPersistenceError::PartialCommit {
            transaction_id: transaction.identity().id().to_string(),
            transaction_path: transaction.directory().to_path_buf(),
            source: Box::new(AcquisitionProgressError::Persistence(e)),
        }
    })?;

    Ok(())
}

fn write_progress_atomic(
    path: &Path,
    progress: &AcquisitionProgressDocument,
) -> Result<(), std::io::Error> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no parent"))?;
    std::fs::create_dir_all(parent)?;

    let bytes = serde_json::to_vec(progress)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let temp: NamedTempFile = tempfile::Builder::new()
        .prefix(".acquisition-progress-")
        .suffix(".tmp")
        .tempfile_in(parent)?;

    let (mut file, temp_path) = temp.into_parts();
    file.write_all(&bytes)?;
    file.sync_all()?;
    temp_path.persist(path).map_err(|e| e.error)?;

    // Best-effort sync of the session directory.
    let _ = std::fs::File::open(parent).and_then(|f| f.sync_all());
    Ok(())
}

fn now_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX)
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum SessionValidationError {
    UnmanagedContext,
    InvalidPaths,
    SymlinkRejected,
    StoreOpen,
    MissingSession,
    SessionLoadFailed,
    SessionNotRunning,
    OperationMismatch,
    RuntimeMismatch,
    SessionIdentityMismatch,
    LeaseUnavailable,
    LeaseInspectionFailed,
}

impl fmt::Display for SessionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmanagedContext => formatter.write_str("managed HTTP session context is unavailable"),
            Self::InvalidPaths => formatter.write_str("managed HTTP context paths are invalid"),
            Self::SymlinkRejected => formatter.write_str("managed HTTP paths must not be symlinks"),
            Self::StoreOpen => formatter.write_str("failed to open session store"),
            Self::MissingSession => formatter.write_str("session record is missing"),
            Self::SessionLoadFailed => formatter.write_str("failed to load session record"),
            Self::SessionNotRunning => formatter.write_str("session is not in running state"),
            Self::OperationMismatch => formatter.write_str("session operation does not match HTTP acquisition"),
            Self::RuntimeMismatch => formatter.write_str("session runtime does not match HTTP acquisition"),
            Self::SessionIdentityMismatch => formatter.write_str("session identity mismatch"),
            Self::LeaseUnavailable => formatter.write_str("supervisor lease is not currently owned"),
            Self::LeaseInspectionFailed => formatter.write_str("failed to inspect supervisor lease state"),
        }
    }
}

impl std::error::Error for SessionValidationError {}

/// Detailed nested progress error. Separates individual failure modes.
#[derive(Debug)]
pub enum AcquisitionProgressError {
    Load(std::io::Error),
    Decode(serde_json::Error),
    InvalidInvariant(AcquisitionProgressValidationError),
    SessionMismatch,
    SessionNotRunning,
    OperationMismatch,
    RuntimeMismatch,
    LeaseUnavailable,
    LeaseInspectionFailed,
    Persistence(std::io::Error),
    CounterOverflow,
}

impl AcquisitionProgressError {
    fn from_session_validation(e: SessionValidationError) -> Self {
        match e {
            SessionValidationError::SessionNotRunning => Self::SessionNotRunning,
            SessionValidationError::OperationMismatch => Self::OperationMismatch,
            SessionValidationError::RuntimeMismatch => Self::RuntimeMismatch,
            SessionValidationError::SessionIdentityMismatch => Self::SessionMismatch,
            SessionValidationError::LeaseUnavailable => Self::LeaseUnavailable,
            SessionValidationError::LeaseInspectionFailed => Self::LeaseInspectionFailed,
            _ => Self::LeaseInspectionFailed,
        }
    }
}

impl fmt::Display for AcquisitionProgressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(_) => formatter.write_str("failed to load acquisition progress"),
            Self::Decode(_) => formatter.write_str("failed to decode acquisition progress"),
            Self::InvalidInvariant(_) => formatter.write_str("acquisition progress invariant violated"),
            Self::SessionMismatch => formatter.write_str("acquisition progress session identity mismatch"),
            Self::SessionNotRunning => formatter.write_str("session is not running at progress update"),
            Self::OperationMismatch => formatter.write_str("session operation mismatch at progress update"),
            Self::RuntimeMismatch => formatter.write_str("session runtime mismatch at progress update"),
            Self::LeaseUnavailable => formatter.write_str("supervisor lease not owned at progress update"),
            Self::LeaseInspectionFailed => formatter.write_str("failed to inspect supervisor lease at progress update"),
            Self::Persistence(_) => formatter.write_str("failed to persist acquisition progress"),
            Self::CounterOverflow => formatter.write_str("acquisition progress counter overflow"),
        }
    }
}

impl std::error::Error for AcquisitionProgressError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Load(e) => Some(e),
            Self::Decode(e) => Some(e),
            Self::InvalidInvariant(e) => Some(e),
            Self::Persistence(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ProgressPersistenceError {
    /// A progress failure occurred before any transaction was finalized.
    Progress(AcquisitionProgressError),
    /// A transaction was finalized but the subsequent progress update failed.
    /// The finalized transaction is preserved.
    PartialCommit {
        transaction_id: String,
        transaction_path: PathBuf,
        source: Box<AcquisitionProgressError>,
    },
}

impl fmt::Display for ProgressPersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Progress(_) => formatter.write_str("acquisition progress persistence failed"),
            Self::PartialCommit { .. } => {
                formatter.write_str("transaction finalized but progress publication failed (partial commit)")
            }
        }
    }
}

impl std::error::Error for ProgressPersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Progress(e) => Some(e),
            Self::PartialCommit { source, .. } => Some(source.as_ref()),
        }
    }
}

