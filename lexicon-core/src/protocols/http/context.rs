use std::collections::HashSet;
use std::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::runtime::{RuntimeOperation, RuntimeProtocol};
use crate::session::{
    SessionDataPaths, SessionLeaseState, SessionOperation, SessionState, SessionStore,
    SessionStoreError,
};

use super::error::{AcquisitionError, AcquisitionResult, HttpExecutionError};
use super::policy::HttpRedirectPolicy;
use super::request::{FinalizedHttpRequest, HttpRequest, HttpRequestError, redact_url};
use super::transaction::metadata::{
    AcquisitionProgressDocument, HTTP_ACQUISITION_PROGRESS_SCHEMA_VERSION,
};
use super::transaction::{
    HttpRecordedOutcome, RecordedAttemptContext, RecordedTransaction, record_transaction_attempt,
};
use super::transport::{HttpTransport, HttpTransportFailure, ReqwestHttpTransport};

/// Bound acquisition context provided to HTTP source handlers.
pub struct HttpAcquisitionContext {
    paths: SessionDataPaths,
    session_identity: Option<crate::session::SessionIdentity>,
    transport: Option<Box<dyn HttpTransport>>,
}

impl HttpAcquisitionContext {
    pub fn from_session_data_paths(
        paths: SessionDataPaths,
        session_identity: crate::session::SessionIdentity,
    ) -> Self {
        Self {
            paths,
            session_identity: Some(session_identity),
            transport: ReqwestHttpTransport::new()
                .ok()
                .map(|transport| Box::new(transport) as Box<dyn HttpTransport>),
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
                "invalid LEXICON_SOURCE_DIRECTORY '{}': must be an absolute path",
                source_directory.display()
            ));
        }

        if !source_directory.is_dir() {
            return Err(format!(
                "invalid LEXICON_SOURCE_DIRECTORY '{}': path does not exist or is not a directory",
                source_directory.display()
            ));
        }

        let raw_data_directory = source_directory.join("data/raw");
        let processed_data_directory = source_directory.join("data/processed");
        let operation_root = source_directory.join("get-raw-data");
        let session_directory = operation_root.join("sessions/legacy");

        Ok(Self {
            paths: SessionDataPaths::from_legacy_parts(
                source_directory,
                operation_root,
                session_directory,
                raw_data_directory,
                processed_data_directory,
            ),
            session_identity: None,
            transport: ReqwestHttpTransport::new()
                .ok()
                .map(|transport| Box::new(transport) as Box<dyn HttpTransport>),
        })
    }

    pub fn execute(&mut self, request: HttpRequest) -> AcquisitionResult<RecordedTransaction> {
        self.validate_for_execution()
            .map_err(|error| AcquisitionError::execution(HttpExecutionError::SessionValidation(error)))?;

        let mut request = request.finalize().map_err(AcquisitionError::request)?;

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| AcquisitionError::execution(HttpExecutionError::Transport(HttpTransportFailure::Configuration)))?;

        let session_id = self
            .session_identity
            .as_ref()
            .ok_or_else(|| AcquisitionError::execution(HttpExecutionError::UnmanagedContext))?
            .id()
            .to_string();

        let mut physical_attempt_index = 0u32;
        let mut redirect_index = 0u32;
        let mut parent_transaction_id: Option<String> = None;
        let mut seen_redirect_targets: HashSet<String> = HashSet::new();

        loop {
            let max_attempts = request.retry_policy.maximum_attempts();
            let mut retry_index = 0u32;

            loop {
                physical_attempt_index = physical_attempt_index.saturating_add(1);

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
                .map_err(|error| AcquisitionError::execution(HttpExecutionError::Recorder(error)))?;

                let status = record.transaction.response().status();
                let was_transport_failure = record.transport_failure.is_some();
                let is_redirect_response = is_redirect_status(status);

                persist_progress(
                    self.session_directory(),
                    &session_id,
                    &record.transaction,
                    request.logical_key.clone(),
                    was_transport_failure,
                    is_redirect_response,
                    retry_index > 0,
                )
                .map_err(|error| AcquisitionError::execution(HttpExecutionError::Progress(error)))?;

                if let Some(_failure) = record.transport_failure {
                    if request.retry_policy.retryable_transport_failures()
                        && retry_index + 1 < max_attempts
                    {
                        retry_index += 1;
                        parent_transaction_id = Some(record.transaction.identity().id().to_string());
                        continue;
                    }

                    if request.retry_policy.retryable_transport_failures() && max_attempts > 1 {
                        return Err(AcquisitionError::execution(HttpExecutionError::RetryExhausted));
                    }

                    return Err(AcquisitionError::execution(HttpExecutionError::Transport(
                        HttpTransportFailure::Io,
                    )));
                }

                let location = response_header(record.transaction.response(), "location");
                if is_redirect_response {
                    match request.redirect_policy {
                        HttpRedirectPolicy::None => return Ok(record.transaction),
                        HttpRedirectPolicy::Follow { maximum } => {
                            if redirect_index >= maximum {
                                return Err(AcquisitionError::execution(
                                    HttpExecutionError::RedirectExhausted,
                                ));
                            }

                            let location = location.ok_or_else(|| {
                                AcquisitionError::execution(HttpExecutionError::InvalidRedirectTarget)
                            })?;

                            request = redirect_request_from(record.transaction.request(), &request, status, &location)
                                .map_err(AcquisitionError::request)?;

                            let canonical = request.url.to_string();
                            if !seen_redirect_targets.insert(canonical) {
                                return Err(AcquisitionError::execution(HttpExecutionError::RedirectLoop));
                            }

                            parent_transaction_id = Some(record.transaction.identity().id().to_string());
                            redirect_index += 1;
                            break;
                        }
                    }
                }

                if request.retry_policy.should_retry_status(status)
                    && retry_index + 1 < max_attempts
                {
                    retry_index += 1;
                    parent_transaction_id = Some(record.transaction.identity().id().to_string());
                    continue;
                }

                if request.retry_policy.should_retry_status(status)
                    && max_attempts > 1
                {
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

        let operation_root = crate::session::SessionOperationRoot::new(self.operation_root().to_path_buf())
            .map_err(|_| SessionValidationError::StoreOpen)?;
        let store = SessionStore::open(operation_root).map_err(|_| SessionValidationError::StoreOpen)?;

        let session = self.session_identity.as_ref().unwrap();
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
}

fn redirect_request_from(
    _request_metadata: &crate::protocols::http::transaction::RecordedHttpRequest,
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
        301 | 302 => (request.method.clone(), request.body.clone()),
        _ => (request.method.clone(), request.body.clone()),
    };

    Ok(FinalizedHttpRequest {
        method,
        redacted_url: redact_url(&next_url, &request.sensitive_query_names),
        url: next_url,
        headers: next_headers,
        body,
        logical_key: request.logical_key.clone(),
        retry_policy: request.retry_policy.clone(),
        redirect_policy: request.redirect_policy,
        sensitive_query_names: request.sensitive_query_names.clone(),
    })
}

fn response_header(
    response: &crate::protocols::http::transaction::RecordedHttpResponse,
    name: &str,
) -> Option<String> {
    response
        .headers()
        .as_slice()
        .iter()
        .find(|header| header.name().eq_ignore_ascii_case(name))
        .and_then(|header| match header.value() {
            crate::protocols::http::transaction::RecordedHeaderValue::Utf8(value) => {
                Some(value.clone())
            }
            crate::protocols::http::transaction::RecordedHeaderValue::Base64(_) => None,
        })
}

fn is_redirect_status(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
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

fn persist_progress(
    session_directory: &Path,
    session_id: &str,
    transaction: &RecordedTransaction,
    logical_key: Option<String>,
    transport_failure: bool,
    redirect: bool,
    retry: bool,
) -> Result<(), ProgressPersistenceError> {
    let progress_path = session_directory.join("acquisition_progress.json");
    let mut current = if progress_path.exists() {
        let text = fs::read_to_string(&progress_path).map_err(|_| ProgressPersistenceError::Load)?;
        let parsed: AcquisitionProgressDocument =
            serde_json::from_str(&text).map_err(|_| ProgressPersistenceError::Decode)?;
        if parsed.session_id != session_id {
            return Err(ProgressPersistenceError::SessionMismatch);
        }
        parsed
    } else {
        AcquisitionProgressDocument {
            schema_version: HTTP_ACQUISITION_PROGRESS_SCHEMA_VERSION,
            session_id: session_id.to_string(),
            completed_transaction_count: 0,
            transport_failure_count: 0,
            redirect_count: 0,
            retry_count: 0,
            last_transaction_id: None,
            last_logical_request_key: None,
            updated_at: String::new(),
            revision: 0,
        }
    };

    current.completed_transaction_count += 1;
    if transport_failure {
        current.transport_failure_count += 1;
    }
    if redirect {
        current.redirect_count += 1;
    }
    if retry {
        current.retry_count += 1;
    }
    current.last_transaction_id = Some(transaction.identity().id().to_string());
    current.last_logical_request_key = logical_key;
    current.updated_at = current_timestamp();
    current.revision += 1;

    if let Err(error) = write_progress_atomic(&progress_path, &current) {
        return Err(ProgressPersistenceError::PartialCommit {
            transaction_id: transaction.identity().id().to_string(),
            transaction_path: transaction.directory().to_path_buf(),
            source: Box::new(error),
        });
    }

    Ok(())
}

fn write_progress_atomic(path: &Path, progress: &AcquisitionProgressDocument) -> Result<(), ProgressPersistenceError> {
    let parent = path.parent().ok_or(ProgressPersistenceError::Persist)?;
    fs::create_dir_all(parent).map_err(|_| ProgressPersistenceError::Persist)?;

    let temp = parent.join(format!(
        ".acquisition-progress-{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));

    let bytes = serde_json::to_vec(progress).map_err(|_| ProgressPersistenceError::Persist)?;
    let mut file = File::create(&temp).map_err(|_| ProgressPersistenceError::Persist)?;
    file.write_all(&bytes).map_err(|_| ProgressPersistenceError::Persist)?;
    file.sync_all().map_err(|_| ProgressPersistenceError::Persist)?;

    fs::rename(&temp, path).map_err(|_| ProgressPersistenceError::Persist)?;

    let dir = File::open(parent).map_err(|_| ProgressPersistenceError::Persist)?;
    let _ = dir.sync_all();
    Ok(())
}

fn current_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", now.as_secs(), now.subsec_nanos())
}

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

#[derive(Debug)]
pub enum ProgressPersistenceError {
    Load,
    Decode,
    SessionMismatch,
    Persist,
    PartialCommit {
        transaction_id: String,
        transaction_path: PathBuf,
        source: Box<ProgressPersistenceError>,
    },
}

impl fmt::Display for ProgressPersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load => formatter.write_str("failed to load acquisition progress"),
            Self::Decode => formatter.write_str("failed to decode acquisition progress"),
            Self::SessionMismatch => formatter.write_str("acquisition progress session identity mismatch"),
            Self::Persist => formatter.write_str("failed to persist acquisition progress"),
            Self::PartialCommit {
                transaction_id,
                transaction_path,
                ..
            } => write!(
                formatter,
                "transaction finalized before progress persistence: id={}, path={}",
                transaction_id,
                transaction_path.display()
            ),
        }
    }
}

impl std::error::Error for ProgressPersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PartialCommit { source, .. } => Some(source),
            _ => None,
        }
    }
}
