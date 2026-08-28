use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::protocols::http::transaction::metadata::{HttpTransactionAdmissionError, admit_transaction_from_disk};
use crate::protocols::http::transaction::{RecordedTransaction, error::HttpManagedPathValidationMode};
use crate::protocols::http::transaction::error::validate_managed_path;
use crate::runtime::{OwnedRuntimeIdentity, RuntimeOperation, RuntimeProtocol};
use crate::session::{ProjectIdentity, SessionIdentity, SessionOperationRoot, SessionRecordV1, SessionState, SessionStore, SessionStoreError, SessionTimestamp};

#[derive(Debug, Clone)]
pub struct ProcessingHttpTransaction {
    project: ProjectIdentity,
    acquisition_runtime: OwnedRuntimeIdentity,
    acquisition_session: SessionIdentity,
    acquisition_session_state: SessionState,
    transaction: RecordedTransaction,
}

impl ProcessingHttpTransaction {
    pub fn project(&self) -> &ProjectIdentity { &self.project }
    pub fn acquisition_runtime(&self) -> &OwnedRuntimeIdentity { &self.acquisition_runtime }
    pub fn acquisition_session(&self) -> &SessionIdentity { &self.acquisition_session }
    pub fn acquisition_session_state(&self) -> SessionState { self.acquisition_session_state }
    pub fn transaction(&self) -> &RecordedTransaction { &self.transaction }
}

#[derive(Debug, Clone)]
pub struct ProcessingHttpTransactionCatalog {
    transactions: Vec<ProcessingHttpTransaction>,
}

impl ProcessingHttpTransactionCatalog {
    pub(crate) fn new(transactions: Vec<ProcessingHttpTransaction>) -> Self {
        Self { transactions }
    }

    pub fn as_slice(&self) -> &[ProcessingHttpTransaction] { &self.transactions }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ProcessingHttpTransaction> {
        self.transactions.iter()
    }

    pub fn len(&self) -> usize { self.transactions.len() }

    pub fn is_empty(&self) -> bool { self.transactions.is_empty() }
}

#[derive(Debug)]
pub enum ProcessingTransactionDiscoveryError {
    RuntimeProtocolMismatch,
    RuntimeOperationMismatch,
    ManagedPath(crate::protocols::http::transaction::error::HttpManagedPathError),
    RawRootEnumeration(std::io::Error),
    RawEntryMetadata(std::io::Error),
    RawEntrySymlink { entry_path: PathBuf },
    RawEntryUnexpectedFile { entry_path: PathBuf },
    RawEntryUnsupportedType { entry_path: PathBuf },
    RawEntryNameInvalid { entry_path: PathBuf },
    RawEntryUnrecognizedDirectory { entry_path: PathBuf },
    TransactionAdmission {
        entry_path: PathBuf,
        source: HttpTransactionAdmissionError,
    },
    DuplicateTransactionIdentity { transaction_identity: String },
    AcquisitionStoreOpen(SessionStoreError),
    AcquisitionSessionLoad {
        acquisition_session: SessionIdentity,
        source: SessionStoreError,
    },
    Provenance(ProcessingTransactionProvenanceError),
}

impl fmt::Display for ProcessingTransactionDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeProtocolMismatch => formatter.write_str("processing transaction discovery requires HTTP runtime protocol"),
            Self::RuntimeOperationMismatch => formatter.write_str("processing transaction discovery requires processing runtime operation"),
            Self::ManagedPath(_) => formatter.write_str("processing transaction discovery path validation failed"),
            Self::RawRootEnumeration(_) => formatter.write_str("processing transaction discovery failed to enumerate raw root"),
            Self::RawEntryMetadata(_) => formatter.write_str("processing transaction discovery failed to inspect a raw entry"),
            Self::RawEntrySymlink { .. } => formatter.write_str("processing transaction discovery rejected a raw-entry symlink"),
            Self::RawEntryUnexpectedFile { .. } => formatter.write_str("processing transaction discovery rejected an unexpected raw file entry"),
            Self::RawEntryUnsupportedType { .. } => formatter.write_str("processing transaction discovery rejected an unsupported raw entry type"),
            Self::RawEntryNameInvalid { .. } => formatter.write_str("processing transaction discovery rejected an invalid raw directory name"),
            Self::RawEntryUnrecognizedDirectory { .. } => formatter.write_str("processing transaction discovery rejected an unrecognized raw directory"),
            Self::TransactionAdmission { .. } => formatter.write_str("processing transaction discovery could not admit a finalized transaction"),
            Self::DuplicateTransactionIdentity { .. } => formatter.write_str("processing transaction discovery rejected duplicate transaction identity"),
            Self::AcquisitionStoreOpen(_) => formatter.write_str("processing transaction discovery failed to open acquisition session store"),
            Self::AcquisitionSessionLoad { .. } => formatter.write_str("processing transaction discovery failed to load acquisition session provenance"),
            Self::Provenance(_) => formatter.write_str("processing transaction discovery rejected transaction provenance"),
        }
    }
}

impl std::error::Error for ProcessingTransactionDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManagedPath(error) => Some(error),
            Self::RawRootEnumeration(error) => Some(error),
            Self::RawEntryMetadata(error) => Some(error),
            Self::TransactionAdmission { source, .. } => Some(source),
            Self::AcquisitionStoreOpen(error) => Some(error),
            Self::AcquisitionSessionLoad { source, .. } => Some(source),
            Self::Provenance(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ProcessingTransactionProvenanceError {
    ProjectMismatch { acquisition_session: SessionIdentity },
    SessionIdentityMismatch { acquisition_session: SessionIdentity },
    RuntimeProtocolMismatch { acquisition_session: SessionIdentity },
    RuntimeOperationMismatch { acquisition_session: SessionIdentity },
    RuntimeSourceMismatch { acquisition_session: SessionIdentity },
    SessionNotExecutable { acquisition_session: SessionIdentity, state: SessionState },
    MissingStartedAt { acquisition_session: SessionIdentity },
    TimestampOutOfBounds {
        acquisition_session: SessionIdentity,
        transaction_identity: String,
    },
}

impl fmt::Display for ProcessingTransactionProvenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectMismatch { .. } => formatter.write_str("acquisition session project does not match processing project"),
            Self::SessionIdentityMismatch { .. } => formatter.write_str("transaction session identity does not match durable acquisition session identity"),
            Self::RuntimeProtocolMismatch { .. } => formatter.write_str("acquisition session runtime protocol is not HTTP"),
            Self::RuntimeOperationMismatch { .. } => formatter.write_str("acquisition session runtime operation is not acquisition"),
            Self::RuntimeSourceMismatch { .. } => formatter.write_str("acquisition session runtime source does not match processing runtime source"),
            Self::SessionNotExecutable { .. } => formatter.write_str("acquisition session state cannot prove execution"),
            Self::MissingStartedAt { .. } => formatter.write_str("acquisition session record does not contain execution start timestamp"),
            Self::TimestampOutOfBounds { .. } => formatter.write_str("transaction timestamps fall outside acquisition session temporal bounds"),
        }
    }
}

impl std::error::Error for ProcessingTransactionProvenanceError {}

pub(crate) fn discover_http_transactions_for_processing(
    project: &ProjectIdentity,
    processing_runtime: &OwnedRuntimeIdentity,
    protocol_root: &Path,
    raw_root: &Path,
) -> Result<ProcessingHttpTransactionCatalog, ProcessingTransactionDiscoveryError> {
    if processing_runtime.protocol() != RuntimeProtocol::Http {
        return Err(ProcessingTransactionDiscoveryError::RuntimeProtocolMismatch);
    }

    if processing_runtime.operation() != RuntimeOperation::Processing {
        return Err(ProcessingTransactionDiscoveryError::RuntimeOperationMismatch);
    }

    validate_managed_path(
        protocol_root,
        protocol_root,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(ProcessingTransactionDiscoveryError::ManagedPath)?;
    validate_managed_path(
        protocol_root,
        raw_root,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(ProcessingTransactionDiscoveryError::ManagedPath)?;

    let entries = std::fs::read_dir(raw_root)
        .map_err(ProcessingTransactionDiscoveryError::RawRootEnumeration)?;

    let mut admitted_transactions = Vec::new();
    for entry in entries {
        let entry = entry.map_err(ProcessingTransactionDiscoveryError::RawRootEnumeration)?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(ProcessingTransactionDiscoveryError::RawEntryMetadata)?;

        if file_type.is_symlink() {
            return Err(ProcessingTransactionDiscoveryError::RawEntrySymlink { entry_path: path });
        }

        if file_type.is_file() {
            return Err(ProcessingTransactionDiscoveryError::RawEntryUnexpectedFile { entry_path: path });
        }

        if !file_type.is_dir() {
            return Err(ProcessingTransactionDiscoveryError::RawEntryUnsupportedType { entry_path: path });
        }

        match classify_raw_directory_name(&entry.file_name()) {
            RawDirectoryClass::RecognizedPartial => continue,
            RawDirectoryClass::FinalizedCandidate => {
                let transaction = admit_transaction_from_disk(raw_root, &path).map_err(|source| {
                    ProcessingTransactionDiscoveryError::TransactionAdmission {
                        entry_path: path.clone(),
                        source,
                    }
                })?;
                admitted_transactions.push(transaction);
            }
            RawDirectoryClass::Unrecognized => {
                return Err(ProcessingTransactionDiscoveryError::RawEntryUnrecognizedDirectory {
                    entry_path: path,
                });
            }
            RawDirectoryClass::InvalidName => {
                return Err(ProcessingTransactionDiscoveryError::RawEntryNameInvalid { entry_path: path });
            }
        }
    }

    let acquisition_operation_root = protocol_root.join("get-raw-data");
    let acquisition_operation_root = SessionOperationRoot::new(acquisition_operation_root)
        .map_err(ProcessingTransactionDiscoveryError::AcquisitionStoreOpen)?;
    let acquisition_store = SessionStore::open(acquisition_operation_root)
        .map_err(ProcessingTransactionDiscoveryError::AcquisitionStoreOpen)?;

    let mut acquisition_records: HashMap<String, SessionRecordV1> = HashMap::new();
    let mut discovered = Vec::with_capacity(admitted_transactions.len());

    for transaction in admitted_transactions {
        let acquisition_session = transaction.session().clone();
        let session_key = acquisition_session.id().to_string();
        if !acquisition_records.contains_key(&session_key) {
            let record = acquisition_store
                .load(&acquisition_session)
                .map_err(|source| ProcessingTransactionDiscoveryError::AcquisitionSessionLoad {
                    acquisition_session: acquisition_session.clone(),
                    source,
                })?;
            validate_provenance(project, processing_runtime, &record, &transaction)
                .map_err(ProcessingTransactionDiscoveryError::Provenance)?;
            acquisition_records.insert(session_key.clone(), record);
        }

        let record = acquisition_records
            .get(&session_key)
            .expect("session provenance cache must contain loaded record");
        discovered.push(ProcessingHttpTransaction {
            project: project.clone(),
            acquisition_runtime: record.runtime().clone(),
            acquisition_session: acquisition_session.clone(),
            acquisition_session_state: record.state(),
            transaction,
        });
    }

    discovered.sort_by(|left, right| {
        (
            left.transaction.created_at_unix_nanos(),
            left.transaction.identity().id(),
        )
            .cmp(&(right.transaction.created_at_unix_nanos(), right.transaction.identity().id()))
    });

    let mut seen = HashSet::new();
    for transaction in &discovered {
        let inserted = seen.insert(transaction.transaction.identity().id().to_string());
        if !inserted {
            return Err(ProcessingTransactionDiscoveryError::DuplicateTransactionIdentity {
                transaction_identity: transaction.transaction.identity().id().to_string(),
            });
        }
    }

    Ok(ProcessingHttpTransactionCatalog::new(discovered))
}

enum RawDirectoryClass {
    RecognizedPartial,
    FinalizedCandidate,
    Unrecognized,
    InvalidName,
}

fn classify_raw_directory_name(name: &std::ffi::OsStr) -> RawDirectoryClass {
    let Some(name) = name.to_str() else {
        return RawDirectoryClass::InvalidName;
    };

    if name.starts_with(".partial-") {
        return RawDirectoryClass::RecognizedPartial;
    }

    if looks_like_finalized_directory_name(name) {
        return RawDirectoryClass::FinalizedCandidate;
    }

    RawDirectoryClass::Unrecognized
}

fn looks_like_finalized_directory_name(name: &str) -> bool {
    let Some((timestamp, identity)) = name.split_once('-') else {
        return false;
    };

    let parsed_timestamp = timestamp.parse::<u64>().ok();

    !timestamp.is_empty()
        && timestamp.bytes().all(|byte| byte.is_ascii_digit())
        && parsed_timestamp.is_some_and(|value| value > 0)
        && !identity.is_empty()
}

fn validate_provenance(
    processing_project: &ProjectIdentity,
    processing_runtime: &OwnedRuntimeIdentity,
    acquisition_record: &SessionRecordV1,
    transaction: &RecordedTransaction,
) -> Result<(), ProcessingTransactionProvenanceError> {
    if acquisition_record.project() != processing_project {
        return Err(ProcessingTransactionProvenanceError::ProjectMismatch {
            acquisition_session: acquisition_record.session().clone(),
        });
    }

    if acquisition_record.session() != transaction.session() {
        return Err(ProcessingTransactionProvenanceError::SessionIdentityMismatch {
            acquisition_session: acquisition_record.session().clone(),
        });
    }

    if acquisition_record.runtime().protocol() != RuntimeProtocol::Http {
        return Err(ProcessingTransactionProvenanceError::RuntimeProtocolMismatch {
            acquisition_session: acquisition_record.session().clone(),
        });
    }

    if acquisition_record.runtime().operation() != RuntimeOperation::Acquisition {
        return Err(ProcessingTransactionProvenanceError::RuntimeOperationMismatch {
            acquisition_session: acquisition_record.session().clone(),
        });
    }

    if acquisition_record.runtime().source_name() != processing_runtime.source_name() {
        return Err(ProcessingTransactionProvenanceError::RuntimeSourceMismatch {
            acquisition_session: acquisition_record.session().clone(),
        });
    }

    if matches!(acquisition_record.state(), SessionState::Prepared) {
        return Err(ProcessingTransactionProvenanceError::SessionNotExecutable {
            acquisition_session: acquisition_record.session().clone(),
            state: acquisition_record.state(),
        });
    }

    let Some(started_at) = acquisition_record.started_at() else {
        return Err(ProcessingTransactionProvenanceError::MissingStartedAt {
            acquisition_session: acquisition_record.session().clone(),
        });
    };

    let transaction_created = SessionTimestamp::from_nanos_since_epoch(transaction.created_at_unix_nanos());
    let transaction_completed = SessionTimestamp::from_nanos_since_epoch(transaction.completed_at_unix_nanos());

    if transaction_completed < transaction_created
        || transaction_created < acquisition_record.created_at()
        || transaction_created < started_at
    {
        return Err(ProcessingTransactionProvenanceError::TimestampOutOfBounds {
            acquisition_session: acquisition_record.session().clone(),
            transaction_identity: transaction.identity().id().to_string(),
        });
    }

    if let Some(finished_at) = acquisition_record.finished_at() {
        if transaction_completed > finished_at {
            return Err(ProcessingTransactionProvenanceError::TimestampOutOfBounds {
                acquisition_session: acquisition_record.session().clone(),
                transaction_identity: transaction.identity().id().to_string(),
            });
        }
    }

    Ok(())
}
