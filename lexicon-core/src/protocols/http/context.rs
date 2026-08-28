use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tempfile::NamedTempFile;

use crate::runtime::{RuntimeOperation, RuntimeProtocol};
use crate::session::{
    SessionDataPaths, SessionLeaseState, SessionOperation, SessionState, SessionStore,
    SessionStoreError,
};

use super::error::{
    AcquisitionError, AcquisitionResult, HttpExecutionError, HttpRedirectFailure,
    HttpRedirectFailureKind, HttpRetryExhaustionError, HttpRetryFinalOutcome,
    RecordedHttpTransportFailure,
};
use super::policy::HttpRedirectPolicy;
use super::request::{FinalizedHttpRequest, HttpRequest};
use super::transaction::error::{HttpClockError, HttpRecorderError, ManagedPathKind, validate_managed_path};
use super::transaction::metadata::{
    AcquisitionProgressAdvanceError, AcquisitionProgressDocument,
    AcquisitionProgressValidationError,
};
use super::transaction::{
    FinalizedRecordedAttempt, HttpRecordedOutcome, ProgressPublishedRecordedAttempt,
    RecordedAttemptContext, RecordedTransaction, record_transaction_attempt,
};
use super::transport::{HttpTransport, HttpTransportConfigurationError, ReqwestHttpTransport};

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
            return Err("invalid LEXICON_SOURCE_DIRECTORY: must be an absolute path".to_string());
        }

        if !source_directory.is_dir() {
            return Err(
                "invalid LEXICON_SOURCE_DIRECTORY: path does not exist or is not a directory"
                    .to_string(),
            );
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
        self.validate_for_execution()
            .map_err(|e| AcquisitionError::execution(HttpExecutionError::SessionValidation(e)))?;

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

        let session_identity = self
            .session_identity
            .as_ref()
            .ok_or_else(|| AcquisitionError::execution(HttpExecutionError::UnmanagedContext))?;
        let session_id = session_identity.id().to_string();

        let mut physical_attempt_index: u32 = 0;
        let mut redirect_index: u32 = 0;
        let mut parent_transaction_id = None;
        let mut seen_redirect_targets: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        seen_redirect_targets.insert(request.url.to_string());

        'redirect: loop {
            let max_attempts = request.retry_policy.maximum_attempts();
            let mut retry_index: u32 = 0;

            loop {
                physical_attempt_index = physical_attempt_index
                    .checked_add(1)
                    .ok_or_else(|| AcquisitionError::execution(HttpExecutionError::CounterOverflow))?;

                let finalized = record_transaction_attempt(
                    RecordedAttemptContext {
                        session_id: session_id.clone(),
                        raw_data_root: self.raw_data_directory().to_path_buf(),
                        logical_request_key: request.logical_key.clone(),
                        parent_transaction_id: parent_transaction_id.clone(),
                        attempt_identity: super::transaction::HttpAttemptIdentity::new(
                            physical_attempt_index,
                            redirect_index,
                            retry_index,
                        ),
                        sensitive_query_names: request.sensitive_query_names.clone(),
                    },
                    &request,
                    transport.as_ref(),
                )
                .map_err(|e| AcquisitionError::execution(HttpExecutionError::Recorder(e)))?;

                let transport_failure = finalized.transport_failure.is_some();
                let redirect_response = matches!(
                    finalized.transaction.response().outcome(),
                    HttpRecordedOutcome::Response
                        if finalized
                            .transaction
                            .response_status()
                            .map(is_redirect_status)
                            .unwrap_or(false)
                );
                let retry_attempt = retry_index > 0;

                let published = persist_progress(
                    self.session_directory(),
                    self.operation_root(),
                    &session_id,
                    session_identity,
                    finalized,
                    transport_failure,
                    redirect_response,
                    retry_attempt,
                )
                .map_err(|(_, error)| {
                    AcquisitionError::execution(HttpExecutionError::Progress(error))
                })?;

                let ProgressPublishedRecordedAttempt {
                    transaction,
                    location_text,
                    invalid_location_encoding,
                } = published;
                let outcome = transaction.response().outcome().clone();
                match outcome {
                    HttpRecordedOutcome::TransportFailure(failure) => {
                        if failure.retryable()
                            && request.retry_policy.retryable_transport_failures()
                            && retry_index
                                .checked_add(1)
                                .map(|next| next < max_attempts)
                                .unwrap_or(false)
                        {
                            retry_index = retry_index.checked_add(1).ok_or_else(|| {
                                AcquisitionError::execution(HttpExecutionError::CounterOverflow)
                            })?;
                            parent_transaction_id = Some(transaction.identity().clone());
                            continue;
                        }

                        if failure.retryable()
                            && request.retry_policy.retryable_transport_failures()
                            && max_attempts > 1
                        {
                            return Err(AcquisitionError::execution(
                                HttpExecutionError::RetryExhausted(HttpRetryExhaustionError::new(
                                    transaction,
                                    physical_attempt_index,
                                    HttpRetryFinalOutcome::TransportFailure(failure.failure()),
                                )),
                            ));
                        }

                        return Err(AcquisitionError::execution(
                            HttpExecutionError::RecordedTransportFailure(
                                RecordedHttpTransportFailure::new(
                                    transaction,
                                    failure.failure(),
                                ),
                            ),
                        ));
                    }
                    HttpRecordedOutcome::Response => {
                        let status = transaction.response_status().ok_or_else(|| {
                            AcquisitionError::response_status(
                                super::transaction::HttpResponseStatusError::new_missing(),
                            )
                        })?;

                        if is_redirect_status(status) {
                            match request.redirect_policy {
                                HttpRedirectPolicy::None => return Ok(transaction),
                                HttpRedirectPolicy::Follow { maximum } => {
                                    let redirect_count = redirect_index.checked_add(1).ok_or_else(|| {
                                        AcquisitionError::execution(
                                            HttpExecutionError::CounterOverflow,
                                        )
                                    })?;
                                    if redirect_index >= maximum {
                                        return Err(AcquisitionError::execution(
                                            HttpExecutionError::RedirectFailure(
                                                HttpRedirectFailure::new(
                                                    transaction,
                                                    HttpRedirectFailureKind::MaximumExceeded,
                                                    redirect_count,
                                                    physical_attempt_index,
                                                ),
                                            ),
                                        ));
                                    }

                                    if invalid_location_encoding {
                                        return Err(AcquisitionError::execution(
                                            HttpExecutionError::RedirectFailure(
                                                HttpRedirectFailure::new(
                                                    transaction,
                                                    HttpRedirectFailureKind::InvalidLocationEncoding,
                                                    redirect_count,
                                                    physical_attempt_index,
                                                ),
                                            ),
                                        ));
                                    }

                                    let location = location_text.as_deref().ok_or_else(|| {
                                        AcquisitionError::execution(
                                            HttpExecutionError::RedirectFailure(
                                                HttpRedirectFailure::new(
                                                    transaction.clone(),
                                                    HttpRedirectFailureKind::MissingLocation,
                                                    redirect_count,
                                                    physical_attempt_index,
                                                ),
                                            ),
                                        )
                                    })?;

                                    let next_request = redirect_request_from(&request, status, location)
                                        .map_err(|kind| {
                                            AcquisitionError::execution(
                                                HttpExecutionError::RedirectFailure(
                                                    HttpRedirectFailure::new(
                                                        transaction.clone(),
                                                        kind,
                                                        redirect_count,
                                                        physical_attempt_index,
                                                    ),
                                                ),
                                            )
                                        })?;

                                    let canonical = next_request.url.to_string();
                                    if !seen_redirect_targets.insert(canonical) {
                                        return Err(AcquisitionError::execution(
                                            HttpExecutionError::RedirectFailure(
                                                HttpRedirectFailure::new(
                                                    transaction,
                                                    HttpRedirectFailureKind::LoopDetected,
                                                    redirect_count,
                                                    physical_attempt_index,
                                                ),
                                            ),
                                        ));
                                    }

                                    request = next_request;
                                    parent_transaction_id = Some(transaction.identity().clone());
                                    redirect_index = redirect_count;
                                    continue 'redirect;
                                }
                            }
                        }

                        if request.retry_policy.should_retry_status(status)
                            && retry_index
                                .checked_add(1)
                                .map(|next| next < max_attempts)
                                .unwrap_or(false)
                        {
                            retry_index = retry_index.checked_add(1).ok_or_else(|| {
                                AcquisitionError::execution(HttpExecutionError::CounterOverflow)
                            })?;
                            parent_transaction_id = Some(transaction.identity().clone());
                            continue;
                        }

                        if request.retry_policy.should_retry_status(status) && max_attempts > 1 {
                            return Err(AcquisitionError::execution(
                                HttpExecutionError::RetryExhausted(HttpRetryExhaustionError::new(
                                    transaction,
                                    physical_attempt_index,
                                    HttpRetryFinalOutcome::ResponseStatus(status),
                                )),
                            ));
                        }

                        return Ok(transaction);
                    }
                }
            }
        }
    }

    fn validate_for_execution(&self) -> Result<(), SessionValidationError> {
        if self.session_identity.is_none() {
            return Err(SessionValidationError::UnmanagedContext);
        }

        for path in [
            self.protocol_root(),
            self.operation_root(),
            self.session_directory(),
            self.raw_data_directory(),
        ] {
            validate_managed_path(path, ManagedPathKind::Directory)
                .map_err(map_path_validation_error)?;
        }

        validate_running_acquisition_session(
            self.operation_root(),
            self.session_identity.as_ref().unwrap(),
        )
    }
}

fn validate_running_acquisition_session(
    operation_root: &Path,
    session: &crate::session::SessionIdentity,
) -> Result<(), SessionValidationError> {
    let operation_root = crate::session::SessionOperationRoot::new(operation_root.to_path_buf())
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

fn map_session_load_error(error: SessionStoreError) -> SessionValidationError {
    match error {
        SessionStoreError::MissingSession => SessionValidationError::MissingSession,
        _ => SessionValidationError::SessionLoadFailed,
    }
}

fn map_path_validation_error(error: HttpRecorderError) -> SessionValidationError {
    match error {
        HttpRecorderError::SymlinkRejected { .. } => SessionValidationError::SymlinkRejected,
        _ => SessionValidationError::InvalidPaths,
    }
}

fn redirect_request_from(
    request: &FinalizedHttpRequest,
    status: u16,
    location: &str,
) -> Result<FinalizedHttpRequest, HttpRedirectFailureKind> {
    let next_url = request
        .url
        .join(location)
        .map_err(|_| HttpRedirectFailureKind::InvalidTarget)?;

    match next_url.scheme() {
        "http" | "https" => {}
        _ => return Err(HttpRedirectFailureKind::UnsupportedScheme),
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

    let redacted_url = super::request::redact_url(&next_url, &request.sensitive_query_names);
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

fn persist_progress(
    session_directory: &Path,
    operation_root: &Path,
    session_id: &str,
    session_identity: &crate::session::SessionIdentity,
    finalized: FinalizedRecordedAttempt,
    transport_failure: bool,
    redirect: bool,
    retry: bool,
) -> Result<ProgressPublishedRecordedAttempt, (FinalizedRecordedAttempt, ProgressPersistenceError)> {
    if let Err(error) = validate_running_acquisition_session(operation_root, session_identity) {
        let persistence_error = ProgressPersistenceError::PartialCommit {
            transaction_id: finalized.transaction.identity().id().to_string(),
            transaction_path: finalized.transaction.directory().to_path_buf(),
            source: Box::new(AcquisitionProgressError::from_session_validation(error)),
        };
        return Err((finalized, persistence_error));
    }

    if let Err(error) = validate_managed_path(session_directory, ManagedPathKind::Directory) {
        let persistence_error = partial_commit_error(
            &finalized.transaction,
            AcquisitionProgressError::ManagedPath(error),
        );
        return Err((finalized, persistence_error));
    }

    let progress_path = session_directory.join("acquisition_progress.json");
    if let Err(error) = validate_managed_path(&progress_path, ManagedPathKind::RegularFileIfPresent)
    {
        let persistence_error = partial_commit_error(
            &finalized.transaction,
            AcquisitionProgressError::ManagedPath(error),
        );
        return Err((finalized, persistence_error));
    }

    let current = if progress_path.exists() {
        let text = match std::fs::read_to_string(&progress_path) {
            Ok(text) => text,
            Err(error) => {
                let persistence_error = partial_commit_error(
                    &finalized.transaction,
                    AcquisitionProgressError::Load(error),
                );
                return Err((finalized, persistence_error));
            }
        };

        let parsed: AcquisitionProgressDocument = match serde_json::from_str(&text) {
            Ok(parsed) => parsed,
            Err(error) => {
                let persistence_error = partial_commit_error(
                    &finalized.transaction,
                    AcquisitionProgressError::Decode(error),
                );
                return Err((finalized, persistence_error));
            }
        };

        if let Err(error) =
            AcquisitionProgressDocument::validate_existing(&parsed, session_id, parsed.revision)
        {
            let persistence_error = partial_commit_error(
                &finalized.transaction,
                AcquisitionProgressError::InvalidInvariant(error),
            );
            return Err((finalized, persistence_error));
        }

        parsed
    } else {
        let now = match now_nanos() {
            Ok(now) => now,
            Err(error) => {
                let persistence_error = partial_commit_error(
                    &finalized.transaction,
                    AcquisitionProgressError::Clock(error),
                );
                return Err((finalized, persistence_error));
            }
        };
        AcquisitionProgressDocument::new_initial(session_id.to_string(), now)
    };

    let advance_now = match now_nanos() {
        Ok(now) => now,
        Err(error) => {
            let persistence_error = partial_commit_error(
                &finalized.transaction,
                AcquisitionProgressError::Clock(error),
            );
            return Err((finalized, persistence_error));
        }
    };
    let next = match current.advance(
        finalized.transaction.identity(),
        finalized.transaction.logical_request_key(),
        advance_now,
        transport_failure,
        redirect,
        retry,
    ) {
        Ok(next) => next,
        Err(error) => {
            let persistence_error = partial_commit_error(
                &finalized.transaction,
                AcquisitionProgressError::Advance(error),
            );
            return Err((finalized, persistence_error));
        }
    };

    if let Err(error) = write_progress_atomic(&progress_path, &next) {
        let persistence_error = partial_commit_error(&finalized.transaction, error);
        return Err((finalized, persistence_error));
    }

    Ok(ProgressPublishedRecordedAttempt {
        transaction: finalized.transaction,
        location_text: finalized.effective_location,
        invalid_location_encoding: finalized.invalid_location_encoding,
    })
}

fn partial_commit_error(
    transaction: &RecordedTransaction,
    source: AcquisitionProgressError,
) -> ProgressPersistenceError {
    ProgressPersistenceError::PartialCommit {
        transaction_id: transaction.identity().id().to_string(),
        transaction_path: transaction.directory().to_path_buf(),
        source: Box::new(source),
    }
}

fn write_progress_atomic(
    path: &Path,
    progress: &AcquisitionProgressDocument,
) -> Result<(), AcquisitionProgressError> {
    let parent = path
        .parent()
        .ok_or_else(|| {
            AcquisitionProgressError::Persistence(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "no parent",
            ))
        })?;

    std::fs::create_dir_all(parent).map_err(AcquisitionProgressError::Persistence)?;
    validate_managed_path(parent, ManagedPathKind::Directory)
        .map_err(AcquisitionProgressError::ManagedPath)?;
    validate_managed_path(path, ManagedPathKind::RegularFileIfPresent)
        .map_err(AcquisitionProgressError::ManagedPath)?;

    let bytes = serde_json::to_vec(progress).map_err(AcquisitionProgressError::Encode)?;

    let temp: NamedTempFile = tempfile::Builder::new()
        .prefix(".acquisition-progress-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(AcquisitionProgressError::Persistence)?;

    let (mut file, temp_path) = temp.into_parts();
    file.write_all(&bytes)
        .map_err(AcquisitionProgressError::Persistence)?;
    file.sync_all().map_err(AcquisitionProgressError::Persistence)?;
    temp_path
        .persist(path)
        .map_err(|error| AcquisitionProgressError::Persistence(error.error))?;
    std::fs::File::open(parent)
        .and_then(|file| file.sync_all())
        .map_err(AcquisitionProgressError::DirectorySyncFailed)?;
    Ok(())
}

fn now_nanos() -> Result<u64, HttpClockError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| HttpClockError::BeforeEpoch)?;
    duration
        .as_nanos()
        .try_into()
        .map_err(|_| HttpClockError::OutOfRange)
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
pub enum AcquisitionProgressError {
    ManagedPath(HttpRecorderError),
    Load(std::io::Error),
    Decode(serde_json::Error),
    Encode(serde_json::Error),
    InvalidInvariant(AcquisitionProgressValidationError),
    Advance(AcquisitionProgressAdvanceError),
    SessionMismatch,
    SessionNotRunning,
    OperationMismatch,
    RuntimeMismatch,
    LeaseUnavailable,
    LeaseInspectionFailed,
    Persistence(std::io::Error),
    DirectorySyncFailed(std::io::Error),
    Clock(HttpClockError),
}

impl AcquisitionProgressError {
    fn from_session_validation(error: SessionValidationError) -> Self {
        match error {
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
            Self::ManagedPath(_) => formatter.write_str("managed acquisition progress path is invalid"),
            Self::Load(_) => formatter.write_str("failed to load acquisition progress"),
            Self::Decode(_) => formatter.write_str("failed to decode acquisition progress"),
            Self::Encode(_) => formatter.write_str("failed to encode acquisition progress"),
            Self::InvalidInvariant(_) => formatter.write_str("acquisition progress invariant violated"),
            Self::Advance(_) => formatter.write_str("failed to advance acquisition progress"),
            Self::SessionMismatch => formatter.write_str("acquisition progress session identity mismatch"),
            Self::SessionNotRunning => formatter.write_str("session is not running at progress update"),
            Self::OperationMismatch => formatter.write_str("session operation mismatch at progress update"),
            Self::RuntimeMismatch => formatter.write_str("session runtime mismatch at progress update"),
            Self::LeaseUnavailable => formatter.write_str("supervisor lease not owned at progress update"),
            Self::LeaseInspectionFailed => formatter.write_str("failed to inspect supervisor lease at progress update"),
            Self::Persistence(_) => formatter.write_str("failed to persist acquisition progress"),
            Self::DirectorySyncFailed(_) => {
                formatter.write_str("acquisition progress replaced but session directory sync failed")
            }
            Self::Clock(_) => formatter.write_str("failed to acquire acquisition progress timestamp"),
        }
    }
}

impl std::error::Error for AcquisitionProgressError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManagedPath(error) => Some(error),
            Self::Load(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Encode(error) => Some(error),
            Self::InvalidInvariant(error) => Some(error),
            Self::Advance(error) => Some(error),
            Self::Persistence(error) => Some(error),
            Self::DirectorySyncFailed(error) => Some(error),
            Self::Clock(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ProgressPersistenceError {
    Progress(AcquisitionProgressError),
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
            Self::PartialCommit { .. } => formatter.write_str(
                "transaction finalized but progress publication failed (partial commit)",
            ),
        }
    }
}

impl std::error::Error for ProgressPersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Progress(error) => Some(error),
            Self::PartialCommit { source, .. } => Some(source.as_ref()),
        }
    }
}
