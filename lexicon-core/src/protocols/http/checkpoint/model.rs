use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::protocols::http::checkpoint::error::{
    HttpCheckpointAdmissionError, HttpCheckpointCommitError, HttpCheckpointDecodingError,
    HttpCheckpointEncodingError, HttpCheckpointPartialCommit, HttpCheckpointPostPublicationError,
    HttpCheckpointPublicationError, HttpCheckpointTransactionLookupError,
};
use crate::protocols::http::transaction::error::{
    HttpManagedPathValidationMode, validate_managed_path,
};
use crate::protocols::http::transaction::metadata::admit_transaction_from_disk;
use crate::protocols::http::transaction::{
    HttpAttemptIdentity, HttpLogicalRequestKey, HttpRecordedOutcome, HttpTransactionIdentity,
    RecordedTransaction,
};
use crate::runtime::{OwnedRuntimeIdentity, RuntimeOperation, RuntimeProtocol};
use crate::session::{
    ProjectIdentity, SessionIdentity, SessionOperationRoot, SessionRecordV1, SessionState,
    SessionStore,
};

pub const HTTP_CHECKPOINT_SCHEMA_VERSION: u32 = 1;
pub const MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckpointProjectIdentityDocument {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckpointRuntimeIdentityDocument {
    pub source: String,
    pub protocol: String,
    pub operation: String,
    pub source_contract_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckpointSessionIdentityDocument {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HttpCheckpointDocumentV1 {
    pub schema_version: u32,
    pub key: String,
    pub key_sha256: String,
    pub project: CheckpointProjectIdentityDocument,
    pub runtime: CheckpointRuntimeIdentityDocument,
    pub session: CheckpointSessionIdentityDocument,
    pub transaction_id: String,
    pub physical_attempt_index: u32,
    pub redirect_index: u32,
    pub retry_index: u32,
    pub committed_at_unix_nanos: u64,
}

#[derive(Debug, Clone)]
pub struct CommittedHttpCheckpoint {
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
    key: HttpLogicalRequestKey,
    key_sha256: String,
    transaction_identity: HttpTransactionIdentity,
    attempt_identity: HttpAttemptIdentity,
    checkpoint_path: PathBuf,
    committed_at_unix_nanos: u64,
}

impl CommittedHttpCheckpoint {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        project: ProjectIdentity,
        runtime: OwnedRuntimeIdentity,
        session: SessionIdentity,
        key: HttpLogicalRequestKey,
        key_sha256: String,
        transaction_identity: HttpTransactionIdentity,
        attempt_identity: HttpAttemptIdentity,
        checkpoint_path: PathBuf,
        committed_at_unix_nanos: u64,
    ) -> Self {
        Self {
            project,
            runtime,
            session,
            key,
            key_sha256,
            transaction_identity,
            attempt_identity,
            checkpoint_path,
            committed_at_unix_nanos,
        }
    }

    pub fn project(&self) -> &ProjectIdentity {
        &self.project
    }

    pub fn runtime(&self) -> &OwnedRuntimeIdentity {
        &self.runtime
    }

    pub fn session(&self) -> &SessionIdentity {
        &self.session
    }

    pub fn session_id(&self) -> &str {
        self.session.id()
    }

    pub fn key(&self) -> &HttpLogicalRequestKey {
        &self.key
    }

    pub fn key_sha256(&self) -> &str {
        &self.key_sha256
    }

    pub fn transaction_identity(&self) -> &HttpTransactionIdentity {
        &self.transaction_identity
    }

    pub fn attempt_identity(&self) -> &HttpAttemptIdentity {
        &self.attempt_identity
    }

    pub fn checkpoint_path(&self) -> &Path {
        &self.checkpoint_path
    }

    pub fn committed_at_unix_nanos(&self) -> u64 {
        self.committed_at_unix_nanos
    }
}

pub(crate) fn key_sha256_hex(key: &HttpLogicalRequestKey) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_str().as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn checkpoint_filename(key: &HttpLogicalRequestKey) -> String {
    format!("{}.json", key_sha256_hex(key))
}

struct DecodedCheckpointDocument {
    key: HttpLogicalRequestKey,
    key_sha256: String,
    project: ProjectIdentity,
    runtime: OwnedRuntimeIdentity,
    session: SessionIdentity,
    transaction_identity: HttpTransactionIdentity,
    attempt_identity: HttpAttemptIdentity,
    committed_at_unix_nanos: u64,
}

fn decode_checkpoint_document(
    bytes: &[u8],
    expected_filename_stem: &str,
) -> Result<DecodedCheckpointDocument, HttpCheckpointDecodingError> {
    if bytes.len() > MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES {
        return Err(HttpCheckpointDecodingError::OversizedDocument {
            size: bytes.len(),
            limit: MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES,
        });
    }

    let document: HttpCheckpointDocumentV1 = serde_json::from_slice(bytes)
        .map_err(HttpCheckpointDecodingError::Deserialization)?;

    if document.schema_version != HTTP_CHECKPOINT_SCHEMA_VERSION {
        return Err(HttpCheckpointDecodingError::UnknownSchemaVersion {
            found: document.schema_version,
        });
    }

    let key = HttpLogicalRequestKey::new(&document.key)
        .map_err(HttpCheckpointDecodingError::InvalidKey)?;
    if !is_valid_sha256_hex(&document.key_sha256) {
        return Err(HttpCheckpointDecodingError::InvalidKeyHash);
    }
    if document.key_sha256 != key_sha256_hex(&key) {
        return Err(HttpCheckpointDecodingError::KeyHashMismatch);
    }
    if document.key_sha256 != expected_filename_stem {
        return Err(HttpCheckpointDecodingError::FilenameMismatch);
    }

    let project = ProjectIdentity::new(&document.project.name)
        .map_err(|_| HttpCheckpointDecodingError::InvalidProjectIdentity)?;
    let runtime = parse_runtime_identity(&document.runtime)
        .map_err(|_| HttpCheckpointDecodingError::InvalidRuntimeIdentity)?;
    let session = SessionIdentity::new(&document.session.id)
        .map_err(|_| HttpCheckpointDecodingError::InvalidSessionId)?;
    let transaction_identity =
        HttpTransactionIdentity::from_validated(document.transaction_id.clone())
            .map_err(|_| HttpCheckpointDecodingError::InvalidTransactionIdentity)?;
    let attempt_identity = HttpAttemptIdentity::new(
        document.physical_attempt_index,
        document.redirect_index,
        document.retry_index,
    )
    .map_err(HttpCheckpointDecodingError::InvalidAttemptIdentity)?;
    if document.committed_at_unix_nanos == 0 {
        return Err(HttpCheckpointDecodingError::InvalidTimestamp);
    }

    Ok(DecodedCheckpointDocument {
        key,
        key_sha256: document.key_sha256,
        project,
        runtime,
        session,
        transaction_identity,
        attempt_identity,
        committed_at_unix_nanos: document.committed_at_unix_nanos,
    })
}

pub(crate) fn admit_http_checkpoint_from_disk(
    trusted_operation_root: &Path,
    trusted_raw_root: &Path,
    checkpoint_path: &Path,
    expected_project: &ProjectIdentity,
    expected_runtime: &OwnedRuntimeIdentity,
    expected_session: Option<&SessionIdentity>,
) -> Result<CommittedHttpCheckpoint, HttpCheckpointAdmissionError> {
    if expected_runtime.protocol() != RuntimeProtocol::Http
        || expected_runtime.operation() != RuntimeOperation::Acquisition
    {
        return Err(HttpCheckpointAdmissionError::RuntimeMismatch);
    }

    validate_managed_path(
        trusted_operation_root,
        trusted_operation_root,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(HttpCheckpointAdmissionError::ManagedPath)?;
    validate_managed_path(
        trusted_raw_root,
        trusted_raw_root,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(HttpCheckpointAdmissionError::ManagedPath)?;
    validate_managed_path(
        trusted_operation_root,
        checkpoint_path,
        HttpManagedPathValidationMode::ExistingRegularFile,
    )
    .map_err(HttpCheckpointAdmissionError::ManagedPath)?;

    let (layout_session, filename_stem) = extract_checkpoint_layout(checkpoint_path)?;
    let expected_path = trusted_operation_root
        .join("sessions")
        .join(layout_session.id())
        .join("checkpoints")
        .join(format!("{filename_stem}.json"));
    if expected_path != checkpoint_path {
        return Err(HttpCheckpointAdmissionError::PathLayoutInvalid);
    }

    let bytes = fs::read(checkpoint_path).map_err(HttpCheckpointAdmissionError::Read)?;
    let decoded = decode_checkpoint_document(&bytes, &filename_stem)
        .map_err(HttpCheckpointAdmissionError::Decoding)?;

    if &decoded.project != expected_project {
        return Err(HttpCheckpointAdmissionError::ProjectMismatch);
    }
    if &decoded.runtime != expected_runtime {
        return Err(HttpCheckpointAdmissionError::RuntimeMismatch);
    }
    if decoded.session != layout_session {
        return Err(HttpCheckpointAdmissionError::SessionMismatch);
    }
    if let Some(expected_session) = expected_session {
        if &decoded.session != expected_session {
            return Err(HttpCheckpointAdmissionError::SessionMismatch);
        }
    }

    let session_record =
        load_session_record_for_checkpoint(trusted_operation_root, &decoded.session)?;
    if session_record.project() != expected_project {
        return Err(HttpCheckpointAdmissionError::SessionProjectMismatch);
    }
    if session_record.runtime() != expected_runtime {
        return Err(HttpCheckpointAdmissionError::SessionRuntimeMismatch);
    }
    if session_record.session() != &decoded.session {
        return Err(HttpCheckpointAdmissionError::SessionMismatch);
    }
    if session_record.state() == SessionState::Prepared || session_record.started_at().is_none() {
        return Err(HttpCheckpointAdmissionError::SessionNotRunning);
    }

    let transaction = find_transaction_by_identity(trusted_raw_root, &decoded.transaction_identity)?;
    validate_checkpoint_transaction(
        &decoded,
        &session_record,
        &transaction,
    )?;

    Ok(CommittedHttpCheckpoint::new(
        decoded.project,
        decoded.runtime,
        decoded.session,
        decoded.key,
        decoded.key_sha256,
        decoded.transaction_identity,
        decoded.attempt_identity,
        checkpoint_path.to_path_buf(),
        decoded.committed_at_unix_nanos,
    ))
}

fn extract_checkpoint_layout(
    checkpoint_path: &Path,
) -> Result<(SessionIdentity, String), HttpCheckpointAdmissionError> {
    let filename = checkpoint_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(HttpCheckpointAdmissionError::PathLayoutInvalid)?;
    let filename_stem = filename
        .strip_suffix(".json")
        .ok_or(HttpCheckpointAdmissionError::PathLayoutInvalid)?;
    if !is_valid_sha256_hex(filename_stem) {
        return Err(HttpCheckpointAdmissionError::PathLayoutInvalid);
    }

    let checkpoints_dir = checkpoint_path
        .parent()
        .ok_or(HttpCheckpointAdmissionError::PathLayoutInvalid)?;
    if checkpoints_dir.file_name().and_then(|name| name.to_str()) != Some("checkpoints") {
        return Err(HttpCheckpointAdmissionError::PathLayoutInvalid);
    }
    let session_dir = checkpoints_dir
        .parent()
        .ok_or(HttpCheckpointAdmissionError::PathLayoutInvalid)?;
    let session_name = session_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(HttpCheckpointAdmissionError::PathLayoutInvalid)?;
    let sessions_dir = session_dir
        .parent()
        .ok_or(HttpCheckpointAdmissionError::PathLayoutInvalid)?;
    if sessions_dir.file_name().and_then(|name| name.to_str()) != Some("sessions") {
        return Err(HttpCheckpointAdmissionError::PathLayoutInvalid);
    }

    let session = SessionIdentity::new(session_name)
        .map_err(|_| HttpCheckpointAdmissionError::PathLayoutInvalid)?;
    Ok((session, filename_stem.to_string()))
}

fn load_session_record_for_checkpoint(
    operation_root: &Path,
    session: &SessionIdentity,
) -> Result<SessionRecordV1, HttpCheckpointAdmissionError> {
    let operation_root = SessionOperationRoot::new(operation_root.to_path_buf())
        .map_err(HttpCheckpointAdmissionError::SessionStore)?;
    let store = SessionStore::open(operation_root)
        .map_err(HttpCheckpointAdmissionError::SessionStore)?;
    store.load(session).map_err(HttpCheckpointAdmissionError::SessionStore)
}

fn find_transaction_by_identity(
    trusted_raw_root: &Path,
    expected_identity: &HttpTransactionIdentity,
) -> Result<RecordedTransaction, HttpCheckpointAdmissionError> {
    let entries = fs::read_dir(trusted_raw_root).map_err(|error| {
        HttpCheckpointAdmissionError::TransactionLookup(
            HttpCheckpointTransactionLookupError::RawRootEnumeration(error),
        )
    })?;

    let mut matches = Vec::new();
    for entry_result in entries {
        let entry = entry_result.map_err(|error| {
            HttpCheckpointAdmissionError::TransactionLookup(
                HttpCheckpointTransactionLookupError::RawRootEnumeration(error),
            )
        })?;
        let metadata = entry.file_type().map_err(|error| {
            HttpCheckpointAdmissionError::TransactionLookup(
                HttpCheckpointTransactionLookupError::EntryMetadata(error),
            )
        })?;
        if metadata.is_symlink() {
            return Err(HttpCheckpointAdmissionError::TransactionLookup(
                HttpCheckpointTransactionLookupError::EntrySymlink,
            ));
        }
        if !metadata.is_dir() {
            return Err(HttpCheckpointAdmissionError::TransactionLookup(
                HttpCheckpointTransactionLookupError::UnexpectedManagedEntry,
            ));
        }

        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or(HttpCheckpointAdmissionError::TransactionLookup(
                HttpCheckpointTransactionLookupError::EntryNameInvalid,
            ))?;
        if name.starts_with(".partial-") {
            continue;
        }
        let (_, transaction_id) = parse_finalized_transaction_directory_name(name).map_err(|_| {
            HttpCheckpointAdmissionError::TransactionLookup(
                HttpCheckpointTransactionLookupError::EntryNameInvalid,
            )
        })?;
        if transaction_id != expected_identity.id() {
            continue;
        }
        matches.push(entry.path());
    }

    matches.sort();
    if matches.is_empty() {
        return Err(HttpCheckpointAdmissionError::TransactionLookup(
            HttpCheckpointTransactionLookupError::MissingTransaction,
        ));
    }
    if matches.len() != 1 {
        return Err(HttpCheckpointAdmissionError::TransactionLookup(
            HttpCheckpointTransactionLookupError::AmbiguousTransaction,
        ));
    }

    admit_transaction_from_disk(trusted_raw_root, &matches[0]).map_err(|error| {
        HttpCheckpointAdmissionError::TransactionLookup(
            HttpCheckpointTransactionLookupError::Admission(error),
        )
    })
}

fn validate_checkpoint_transaction(
    checkpoint: &DecodedCheckpointDocument,
    session_record: &SessionRecordV1,
    transaction: &RecordedTransaction,
) -> Result<(), HttpCheckpointAdmissionError> {
    if transaction.session() != session_record.session() {
        return Err(HttpCheckpointAdmissionError::TransactionSessionMismatch);
    }
    if transaction.session() != &checkpoint.session {
        return Err(HttpCheckpointAdmissionError::TransactionSessionMismatch);
    }
    if transaction.identity() != &checkpoint.transaction_identity {
        return Err(HttpCheckpointAdmissionError::TransactionLookup(
            HttpCheckpointTransactionLookupError::AmbiguousTransaction,
        ));
    }
    match transaction.logical_request_key() {
        Some(key) if key == &checkpoint.key => {}
        _ => return Err(HttpCheckpointAdmissionError::TransactionKeyMismatch),
    }
    if matches!(transaction.response().outcome(), HttpRecordedOutcome::TransportFailure(_)) {
        return Err(HttpCheckpointAdmissionError::TransactionNotResponse);
    }
    if transaction.attempt_identity() != &checkpoint.attempt_identity {
        return Err(HttpCheckpointAdmissionError::AttemptMismatch);
    }
    if checkpoint.committed_at_unix_nanos < transaction.completed_at_unix_nanos() {
        return Err(HttpCheckpointAdmissionError::TimestampBeforeTransaction);
    }
    let started_at = session_record
        .started_at()
        .ok_or(HttpCheckpointAdmissionError::SessionNotRunning)?;
    if checkpoint.committed_at_unix_nanos < started_at.nanos_since_epoch() {
        return Err(HttpCheckpointAdmissionError::TimestampBeforeSessionStart);
    }
    if let Some(finished_at) = session_record.finished_at() {
        if checkpoint.committed_at_unix_nanos > finished_at.nanos_since_epoch() {
            return Err(HttpCheckpointAdmissionError::TimestampAfterSessionFinish);
        }
    }
    Ok(())
}

pub(crate) fn encode_checkpoint_document(
    document: &HttpCheckpointDocumentV1,
) -> Result<Vec<u8>, HttpCheckpointEncodingError> {
    let bytes = serde_json::to_vec(document).map_err(HttpCheckpointEncodingError::Serialization)?;
    if bytes.len() > MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES {
        return Err(HttpCheckpointEncodingError::OversizedDocument {
            size: bytes.len(),
            limit: MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES,
        });
    }
    Ok(bytes)
}

pub(crate) fn write_checkpoint_atomic(
    checkpoint_dir: &Path,
    target_path: &Path,
    bytes: &[u8],
    checkpoint: &CommittedHttpCheckpoint,
) -> Result<(), HttpCheckpointCommitError> {
    let temp = tempfile::Builder::new()
        .prefix(".checkpoint-")
        .suffix(".tmp")
        .tempfile_in(checkpoint_dir)
        .map_err(HttpCheckpointCommitError::TemporaryFileCreation)?;
    let (mut file, temp_path) = temp.into_parts();
    file.write_all(bytes)
        .map_err(HttpCheckpointCommitError::TemporaryFileWrite)?;
    file.sync_all()
        .map_err(HttpCheckpointCommitError::TemporaryFileSync)?;
    drop(file);

    publish_checkpoint_no_replace(&temp_path, target_path)
        .map_err(HttpCheckpointCommitError::Publication)?;
    sync_directory(checkpoint_dir).map_err(|error| {
        HttpCheckpointCommitError::PartialCommit(Box::new(HttpCheckpointPartialCommit::new(
            checkpoint.clone(),
            HttpCheckpointPostPublicationError::DirectorySync(error),
        )))
    })?;
    Ok(())
}

fn parse_runtime_identity(
    runtime: &CheckpointRuntimeIdentityDocument,
) -> Result<OwnedRuntimeIdentity, ()> {
    let protocol = RuntimeProtocol::from_identifier(&runtime.protocol).map_err(|_| ())?;
    let operation = RuntimeOperation::from_identifier(&runtime.operation).map_err(|_| ())?;
    match (protocol, operation) {
        (RuntimeProtocol::Http, RuntimeOperation::Acquisition) => Ok(
            OwnedRuntimeIdentity::http_acquisition(
                runtime.source.clone(),
                runtime.source_contract_version,
            ),
        ),
        (RuntimeProtocol::Http, RuntimeOperation::Processing) => Ok(
            OwnedRuntimeIdentity::http_processing(
                runtime.source.clone(),
                runtime.source_contract_version,
            ),
        ),
    }
}

fn parse_finalized_transaction_directory_name(name: &str) -> Result<(u64, &str), ()> {
    let (timestamp, transaction_id) = name.split_once('-').ok_or(())?;
    if timestamp.is_empty()
        || !timestamp.bytes().all(|byte| byte.is_ascii_digit())
        || transaction_id.is_empty()
    {
        return Err(());
    }
    let parsed = timestamp.parse::<u64>().map_err(|_| ())?;
    if parsed == 0 {
        return Err(());
    }
    Ok((parsed, transaction_id))
}

fn is_valid_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(target_os = "linux")]
fn publish_checkpoint_no_replace(
    source: &tempfile::TempPath,
    destination: &Path,
) -> Result<(), HttpCheckpointPublicationError> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| HttpCheckpointPublicationError::InvalidPathArgument)?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| HttpCheckpointPublicationError::InvalidPathArgument)?;

    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(code) if code == libc::EEXIST => Err(HttpCheckpointPublicationError::Collision),
        Some(code)
            if code == libc::ENOSYS || code == libc::EOPNOTSUPP || code == libc::EINVAL =>
        {
            Err(HttpCheckpointPublicationError::UnsupportedPlatform)
        }
        _ => Err(HttpCheckpointPublicationError::Io(error)),
    }
}

#[cfg(target_os = "macos")]
fn publish_checkpoint_no_replace(
    source: &tempfile::TempPath,
    destination: &Path,
) -> Result<(), HttpCheckpointPublicationError> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| HttpCheckpointPublicationError::InvalidPathArgument)?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| HttpCheckpointPublicationError::InvalidPathArgument)?;

    let result = unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) };
    if result == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        Err(HttpCheckpointPublicationError::Collision)
    } else {
        Err(HttpCheckpointPublicationError::Io(error))
    }
}

#[cfg(target_os = "windows")]
fn publish_checkpoint_no_replace(
    source: &tempfile::TempPath,
    destination: &Path,
) -> Result<(), HttpCheckpointPublicationError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS};
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};

    let source: Vec<u16> = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let ok = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok != 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(code) if code == ERROR_ALREADY_EXISTS as i32 || code == ERROR_FILE_EXISTS as i32 => {
            Err(HttpCheckpointPublicationError::Collision)
        }
        _ => Err(HttpCheckpointPublicationError::Io(error)),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn publish_checkpoint_no_replace(
    _: &tempfile::TempPath,
    _: &Path,
) -> Result<(), HttpCheckpointPublicationError> {
    Err(HttpCheckpointPublicationError::UnsupportedPlatform)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    std::fs::File::open(path)?.sync_all()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    std::fs::File::open(path)?.sync_all()
}

#[cfg(target_os = "windows")]
fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_GENERIC_READ, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, FlushFileBuffers, OPEN_EXISTING,
    };

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            0,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }

    let flush_result = unsafe { FlushFileBuffers(handle) };
    let flush_error = if flush_result == 0 {
        Some(std::io::Error::last_os_error())
    } else {
        None
    };
    unsafe {
        CloseHandle(handle);
    }
    if let Some(error) = flush_error {
        return Err(error);
    }
    Ok(())
}
