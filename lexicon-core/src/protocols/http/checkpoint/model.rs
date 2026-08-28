use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::protocols::http::transaction::error::{
    HttpManagedPathValidationMode, validate_managed_path,
};
use crate::protocols::http::transaction::metadata::admit_transaction_from_disk;
use crate::protocols::http::transaction::{
    HttpAttemptIdentity, HttpLogicalRequestKey, HttpRecordedOutcome, HttpTransactionIdentity,
};
use crate::runtime::{RuntimeOperation, RuntimeProtocol};
use crate::session::model::{SessionIdentity, SessionRecordV1, SessionState};
use crate::session::store::SessionStore;

use super::error::{
    HttpCheckpointAdmissionError, HttpCheckpointDecodingError, HttpCheckpointEncodingError,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const HTTP_CHECKPOINT_SCHEMA_VERSION: u32 = 1;
pub const MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES: usize = 64 * 1024;

// ---------------------------------------------------------------------------
// Stored document (internal serde representation)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HttpCheckpointDocumentV1 {
    pub schema_version: u32,
    pub key: String,
    pub key_sha256: String,
    pub project_name: String,
    pub runtime_protocol: String,
    pub runtime_operation: String,
    pub session_id: String,
    pub transaction_id: String,
    pub physical_attempt_index: u32,
    pub redirect_index: u32,
    pub retry_index: u32,
    pub committed_at_unix_nanos: u64,
}

// ---------------------------------------------------------------------------
// CommittedHttpCheckpoint (public opaque)
// ---------------------------------------------------------------------------

/// An immutable, validated checkpoint record.  Returned by
/// `commit_checkpoint` and by the admission function.
#[derive(Debug, Clone)]
pub struct CommittedHttpCheckpoint {
    key: HttpLogicalRequestKey,
    key_sha256: String,
    session_id: String,
    transaction_identity: HttpTransactionIdentity,
    attempt_identity: HttpAttemptIdentity,
    checkpoint_path: PathBuf,
    committed_at_unix_nanos: u64,
}

impl CommittedHttpCheckpoint {
    pub(crate) fn new(
        key: HttpLogicalRequestKey,
        key_sha256: String,
        session_id: String,
        transaction_identity: HttpTransactionIdentity,
        attempt_identity: HttpAttemptIdentity,
        checkpoint_path: PathBuf,
        committed_at_unix_nanos: u64,
    ) -> Self {
        Self {
            key,
            key_sha256,
            session_id,
            transaction_identity,
            attempt_identity,
            checkpoint_path,
            committed_at_unix_nanos,
        }
    }

    pub fn key(&self) -> &HttpLogicalRequestKey {
        &self.key
    }

    pub fn key_sha256(&self) -> &str {
        &self.key_sha256
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
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

// ---------------------------------------------------------------------------
// Key-hash helpers
// ---------------------------------------------------------------------------

/// Compute the lowercase hex SHA-256 of the exact UTF-8 bytes of a logical key.
pub fn key_sha256_hex(key: &HttpLogicalRequestKey) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_str().as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Derive the checkpoint filename (`<sha256>.json`) from a logical key.
pub fn checkpoint_filename(key: &HttpLogicalRequestKey) -> String {
    format!("{}.json", key_sha256_hex(key))
}

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

fn decode_checkpoint_document(
    bytes: &[u8],
    expected_filename_stem: &str,
) -> Result<HttpCheckpointDocumentV1, HttpCheckpointDecodingError> {
    use crate::protocols::http::transaction::HttpTransactionIdentity;
    use crate::runtime::RuntimeIdentifierError;

    if bytes.len() > MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES {
        return Err(HttpCheckpointDecodingError::OversizedDocument {
            size: bytes.len(),
            limit: MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES,
        });
    }

    let doc: HttpCheckpointDocumentV1 = serde_json::from_slice(bytes)
        .map_err(HttpCheckpointDecodingError::Deserialization)?;

    if doc.schema_version != HTTP_CHECKPOINT_SCHEMA_VERSION {
        return Err(HttpCheckpointDecodingError::UnknownSchemaVersion { found: doc.schema_version });
    }

    // Validate logical key
    let key = HttpLogicalRequestKey::new(&doc.key)
        .map_err(HttpCheckpointDecodingError::InvalidKey)?;

    // Validate key_sha256 is lowercase hex
    if doc.key_sha256.len() != 64
        || !doc.key_sha256.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(HttpCheckpointDecodingError::InvalidKeyHash);
    }

    // Verify key hash agrees with key
    let expected_hash = key_sha256_hex(&key);
    if doc.key_sha256 != expected_hash {
        return Err(HttpCheckpointDecodingError::KeyHashMismatch);
    }

    // Verify filename agrees with key hash
    if doc.key_sha256 != expected_filename_stem {
        return Err(HttpCheckpointDecodingError::FilenameMismatch);
    }

    // Validate session id
    if doc.session_id.is_empty() {
        return Err(HttpCheckpointDecodingError::InvalidSessionId);
    }
    // Validate it can form a SessionInvocationIdentity
    crate::runtime::invocation::SessionInvocationIdentity::new(&doc.session_id)
        .map_err(|_| HttpCheckpointDecodingError::InvalidSessionId)?;

    // Validate transaction id
    HttpTransactionIdentity::from_validated(&doc.transaction_id)
        .map_err(|_| HttpCheckpointDecodingError::InvalidTransactionIdentity)?;

    // Validate attempt identity: physical_attempt_index must be >= 1
    if doc.physical_attempt_index == 0 {
        return Err(HttpCheckpointDecodingError::InvalidAttemptIdentity);
    }

    // Validate timestamp
    if doc.committed_at_unix_nanos == 0 {
        return Err(HttpCheckpointDecodingError::InvalidTimestamp);
    }

    // Validate runtime protocol / operation
    RuntimeProtocol::from_identifier(&doc.runtime_protocol)
        .map_err(|_| HttpCheckpointDecodingError::InvalidRuntimeProtocol)?;
    RuntimeOperation::from_identifier(&doc.runtime_operation)
        .map_err(|_| HttpCheckpointDecodingError::InvalidRuntimeOperation)?;

    Ok(doc)
}

// ---------------------------------------------------------------------------
// Admission
// ---------------------------------------------------------------------------

/// Admit a checkpoint file from disk, performing all structural and provenance
/// validation.  Callers must supply:
///
/// * `trusted_operation_root`: the operation root trusted by the current context
///   (used to locate session records and transactions).
/// * `trusted_raw_root`: the trusted `data/raw` root used by
///   `admit_transaction_from_disk`.
/// * `checkpoint_path`: the absolute path of the candidate checkpoint file.
/// * `expected_project_name`: project name that every valid checkpoint must have.
/// * `expected_session_id`: when `Some`, the checkpoint session must match;
///   pass `None` for cross-session lookup (validates against the session record
///   instead of a single expected identity).
pub fn admit_http_checkpoint_from_disk(
    trusted_operation_root: &Path,
    trusted_raw_root: &Path,
    checkpoint_path: &Path,
    expected_project_name: &str,
    expected_session_id: Option<&str>,
) -> Result<CommittedHttpCheckpoint, HttpCheckpointAdmissionError> {
    // ── Path containment and layout ────────────────────────────────────────
    validate_managed_path(
        trusted_operation_root,
        trusted_operation_root,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(HttpCheckpointAdmissionError::ManagedPath)?;

    // The path itself must not contain symlinks.  We check each component.
    check_no_symlinks_on_path(checkpoint_path)
        .map_err(|_| HttpCheckpointAdmissionError::SymlinkRejected)?;

    // Must be a regular file.
    let meta = fs::metadata(checkpoint_path).map_err(HttpCheckpointAdmissionError::Read)?;
    if !meta.is_file() {
        return Err(HttpCheckpointAdmissionError::NotRegularFile);
    }
    if meta.file_type().is_symlink() {
        return Err(HttpCheckpointAdmissionError::SymlinkRejected);
    }

    // Validate layout: …/sessions/<session-id>/checkpoints/<hash>.json
    let (layout_session_id, filename_stem) = extract_layout_parts(checkpoint_path)
        .ok_or(HttpCheckpointAdmissionError::PathLayoutInvalid)?;

    // ── Read and decode ────────────────────────────────────────────────────
    let bytes = fs::read(checkpoint_path).map_err(HttpCheckpointAdmissionError::Read)?;
    let doc = decode_checkpoint_document(&bytes, &filename_stem)
        .map_err(HttpCheckpointAdmissionError::Decoding)?;

    // ── Identity checks ────────────────────────────────────────────────────
    // Project
    if doc.project_name != expected_project_name {
        return Err(HttpCheckpointAdmissionError::ProjectMismatch);
    }

    // Runtime must be HTTP acquisition
    let protocol = RuntimeProtocol::from_identifier(&doc.runtime_protocol)
        .map_err(|_| HttpCheckpointAdmissionError::RuntimeProtocolMismatch)?;
    if protocol != RuntimeProtocol::Http {
        return Err(HttpCheckpointAdmissionError::RuntimeProtocolMismatch);
    }
    let operation = RuntimeOperation::from_identifier(&doc.runtime_operation)
        .map_err(|_| HttpCheckpointAdmissionError::RuntimeOperationMismatch)?;
    if operation != RuntimeOperation::Acquisition {
        return Err(HttpCheckpointAdmissionError::RuntimeOperationMismatch);
    }

    // Session: document session_id must agree with what we extracted from path
    if doc.session_id != layout_session_id {
        return Err(HttpCheckpointAdmissionError::SessionMismatch);
    }

    // If a specific session was requested, it must match
    if let Some(expected_sid) = expected_session_id {
        if doc.session_id != expected_sid {
            return Err(HttpCheckpointAdmissionError::SessionMismatch);
        }
    }

    // ── Validate the session record ────────────────────────────────────────
    let session_record = load_session_record_for_checkpoint(
        trusted_operation_root,
        &doc.session_id,
        expected_project_name,
        &doc.runtime_protocol,
        &doc.runtime_operation,
    )?;
    let _ = session_record; // used for validation side-effects above

    // ── Re-admit the referenced transaction ───────────────────────────────
    // Derive the transaction path from trusted_raw_root + transaction_id
    // We need to scan for the directory whose name contains the transaction id.
    let transaction = find_and_admit_transaction(
        trusted_raw_root,
        &doc.transaction_id,
        &doc.session_id,
    )?;

    // Transaction session must match checkpoint session
    if transaction
        .logical_request_key()
        .map(|k| k.as_str().to_string())
        != Some(doc.key.clone())
    {
        return Err(HttpCheckpointAdmissionError::TransactionKeyMismatch);
    }

    // Transaction must be a response (not transport failure)
    if matches!(transaction.response().outcome(), HttpRecordedOutcome::TransportFailure(_)) {
        return Err(HttpCheckpointAdmissionError::TransactionNotResponse);
    }

    // Attempt identity
    if transaction.attempt_identity().physical_attempt_index() != doc.physical_attempt_index
        || transaction.attempt_identity().redirect_index() != doc.redirect_index
        || transaction.attempt_identity().retry_index() != doc.retry_index
    {
        return Err(HttpCheckpointAdmissionError::AttemptMismatch);
    }

    let key = HttpLogicalRequestKey::new(&doc.key)
        .map_err(|_| HttpCheckpointAdmissionError::Decoding(HttpCheckpointDecodingError::InvalidKey(
            crate::protocols::http::transaction::HttpLogicalRequestKeyError::Empty,
        )))?;
    let transaction_identity = HttpTransactionIdentity::from_validated(&doc.transaction_id)
        .map_err(|_| HttpCheckpointAdmissionError::Decoding(
            HttpCheckpointDecodingError::InvalidTransactionIdentity,
        ))?;
    let attempt_identity = HttpAttemptIdentity::new(
        doc.physical_attempt_index,
        doc.redirect_index,
        doc.retry_index,
    );

    Ok(CommittedHttpCheckpoint::new(
        key,
        doc.key_sha256,
        doc.session_id,
        transaction_identity,
        attempt_identity,
        checkpoint_path.to_path_buf(),
        doc.committed_at_unix_nanos,
    ))
}

// ---------------------------------------------------------------------------
// Internal helpers for admission
// ---------------------------------------------------------------------------

/// Extract `(session_id, filename_stem)` from a checkpoint path of the form
/// `…/sessions/<session-id>/checkpoints/<hash>.json`.
fn extract_layout_parts(checkpoint_path: &Path) -> Option<(String, String)> {
    // filename must end with .json
    let filename = checkpoint_path.file_name()?.to_str()?;
    let stem = filename.strip_suffix(".json")?;
    // stem must be 64 lowercase hex characters (SHA-256)
    if stem.len() != 64 || !stem.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }

    // parent must be "checkpoints"
    let checkpoints_dir = checkpoint_path.parent()?;
    if checkpoints_dir.file_name()?.to_str()? != "checkpoints" {
        return None;
    }

    // grandparent is the session directory
    let session_dir = checkpoints_dir.parent()?;
    let session_id = session_dir.file_name()?.to_str()?.to_string();
    if session_id.is_empty() {
        return None;
    }

    // great-grandparent must be "sessions"
    let sessions_dir = session_dir.parent()?;
    if sessions_dir.file_name()?.to_str()? != "sessions" {
        return None;
    }

    Some((session_id, stem.to_string()))
}

/// Check that no component on the path to `target` is a symlink.
fn check_no_symlinks_on_path(target: &Path) -> Result<(), ()> {
    // Walk each ancestor to check for symlinks.
    // We check the target itself using symlink_metadata.
    let mut current = target.to_path_buf();
    loop {
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(()),
            Err(_) => return Err(()),
            Ok(_) => {}
        }
        match current.parent() {
            Some(parent) if parent != current => {
                current = parent.to_path_buf();
                if current.as_os_str().is_empty() || current == Path::new("/") {
                    break;
                }
            }
            _ => break,
        }
    }
    Ok(())
}

/// Load the session record for a checkpoint, validating that it:
/// * exists and can be decoded;
/// * belongs to the expected project;
/// * has the correct runtime (HTTP acquisition);
/// * has started (i.e., was at some point Running).
fn load_session_record_for_checkpoint(
    operation_root: &Path,
    session_id: &str,
    expected_project_name: &str,
    expected_protocol: &str,
    expected_operation: &str,
) -> Result<SessionRecordV1, HttpCheckpointAdmissionError> {
    use crate::session::store::SessionOperationRoot;

    let op_root = SessionOperationRoot::new(operation_root.to_path_buf())
        .map_err(|_| HttpCheckpointAdmissionError::PathLayoutInvalid)?;
    let store = SessionStore::open(op_root)
        .map_err(|_| HttpCheckpointAdmissionError::PathLayoutInvalid)?;

    let session_identity = crate::runtime::invocation::SessionInvocationIdentity::new(session_id)
        .map_err(|_| HttpCheckpointAdmissionError::PathLayoutInvalid)?;

    let record = store.load(&session_identity).map_err(|e| {
        HttpCheckpointAdmissionError::SessionRecord(
            crate::session::error::SessionDecodingError::StructuralDocument(e.to_string()),
        )
    })?;

    // Project
    if record.project().name() != expected_project_name {
        return Err(HttpCheckpointAdmissionError::SessionProjectMismatch);
    }

    // Runtime protocol / operation must agree
    if record.runtime().protocol().identifier() != expected_protocol {
        return Err(HttpCheckpointAdmissionError::SessionRuntimeMismatch);
    }
    if record.runtime().operation().identifier() != expected_operation {
        return Err(HttpCheckpointAdmissionError::SessionRuntimeMismatch);
    }

    // Session must have been started (started_at is Some → was Running at some point).
    // Reject Prepared sessions that never reached Running.
    if record.state() == SessionState::Prepared {
        return Err(HttpCheckpointAdmissionError::SessionNotStarted);
    }
    if record.started_at().is_none() {
        return Err(HttpCheckpointAdmissionError::SessionNotStarted);
    }

    Ok(record)
}

/// Find and admit the transaction referenced by a checkpoint.
///
/// Scans `trusted_raw_root` for a transaction directory whose name contains
/// the given `transaction_id` and returns the admitted `RecordedTransaction`.
fn find_and_admit_transaction(
    trusted_raw_root: &Path,
    transaction_id: &str,
    expected_session_id: &str,
) -> Result<crate::protocols::http::transaction::RecordedTransaction, HttpCheckpointAdmissionError> {
    let entries = fs::read_dir(trusted_raw_root)
        .map_err(|e| HttpCheckpointAdmissionError::ReferencedTransaction(
            crate::protocols::http::transaction::error::HttpTransactionAdmissionError::RequestMetadataRead(e),
        ))?;

    for entry_result in entries {
        let entry = entry_result.map_err(|e| {
            HttpCheckpointAdmissionError::ReferencedTransaction(
                crate::protocols::http::transaction::error::HttpTransactionAdmissionError::RequestMetadataRead(e),
            )
        })?;

        let file_type = entry.file_type().map_err(|e| {
            HttpCheckpointAdmissionError::ReferencedTransaction(
                crate::protocols::http::transaction::error::HttpTransactionAdmissionError::RequestMetadataRead(e),
            )
        })?;

        if !file_type.is_dir() {
            continue;
        }

        let dir_name = entry.file_name();
        let dir_name_str = match dir_name.to_str() {
            Some(s) => s,
            None => continue,
        };

        // Skip partial directories
        if dir_name_str.starts_with(".partial-") {
            continue;
        }

        // The directory name has the format `<timestamp_nanos>-<transaction_id>`.
        // Check if it ends with the transaction_id.
        if !dir_name_str.ends_with(transaction_id) {
            continue;
        }

        let path = entry.path();
        let tx = admit_transaction_from_disk(trusted_raw_root, &path)
            .map_err(HttpCheckpointAdmissionError::ReferencedTransaction)?;

        // Confirm identity
        if tx.identity().id() != transaction_id {
            continue;
        }

        // Check session
        // The session is stored in the request metadata; we verify via the
        // already-admitted transaction. We read the request metadata to check the session_id.
        // The RecordedTransaction doesn't directly expose session_id, but we can derive it:
        // the transaction's request metadata contains session_id.  For now we rely on
        // the directory path being under the raw root (which is session-scoped in production),
        // and we don't have a direct session_id on RecordedTransaction.
        // We'll validate it by reading the metadata file directly.
        let req_meta_path = path.join("request").join("metadata.json");
        if let Ok(raw) = fs::read_to_string(&req_meta_path) {
            if let Ok(meta) = serde_json::from_str::<crate::protocols::http::transaction::metadata::RequestMetadataDocument>(&raw) {
                if meta.session_id != expected_session_id {
                    return Err(HttpCheckpointAdmissionError::TransactionSessionMismatch);
                }
            }
        }

        return Ok(tx);
    }

    Err(HttpCheckpointAdmissionError::ReferencedTransaction(
        crate::protocols::http::transaction::error::HttpTransactionAdmissionError::MissingTopLevelEntry,
    ))
}

// ---------------------------------------------------------------------------
// Checkpoint atomic publication helper (used by commit)
// ---------------------------------------------------------------------------

/// Serialize and atomically publish a new immutable checkpoint file.
/// Returns `Ok(committed_at_nanos)` on full success.
/// The caller must provide a validated `checkpoint_dir` and a `target_path` that
/// does not yet exist.
pub(crate) fn write_checkpoint_atomic(
    checkpoint_dir: &Path,
    target_path: &Path,
    doc: &HttpCheckpointDocumentV1,
) -> Result<(), super::error::HttpCheckpointCommitError> {
    use super::error::{HttpCheckpointCommitError, HttpCheckpointEncodingError};

    let bytes = serde_json::to_vec(doc)
        .map_err(|e| HttpCheckpointCommitError::Encoding(HttpCheckpointEncodingError::Serialization(e)))?;

    let temp = tempfile::Builder::new()
        .prefix(".checkpoint-")
        .suffix(".tmp")
        .tempfile_in(checkpoint_dir)
        .map_err(HttpCheckpointCommitError::TempFileCreation)?;

    let (mut file, temp_path) = temp.into_parts();
    file.write_all(&bytes).map_err(HttpCheckpointCommitError::Write)?;
    file.sync_all().map_err(HttpCheckpointCommitError::FileSync)?;
    drop(file);

    // Atomic no-replace publish
    atomic_publish_no_replace(&temp_path, target_path)
        .map_err(HttpCheckpointCommitError::AtomicPublication)?;

    Ok(())
}

/// Rename `src` to `dst` only if `dst` does not exist.  Returns `Err` if `dst`
/// already exists (the caller should then admit the existing file) or if the
/// rename fails for another reason.
fn atomic_publish_no_replace(src: &tempfile::TempPath, dst: &Path) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // Use hard-link + unlink idiom for atomic no-replace on Linux.
        // If `dst` already exists, `fs::hard_link` returns an error with
        // ErrorKind::AlreadyExists (on most kernels).
        match fs::hard_link(src, dst) {
            Ok(()) => {
                // Remove the temp file (ignoring errors — the hard link is the canonical copy).
                let _ = fs::remove_file(src);
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(e),
            Err(e) => Err(e),
        }
    }

    #[cfg(not(unix))]
    {
        // Fallback: rename only if target does not exist.
        if dst.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "checkpoint target already exists",
            ));
        }
        src.persist(dst).map_err(|e| e.error)
    }
}
