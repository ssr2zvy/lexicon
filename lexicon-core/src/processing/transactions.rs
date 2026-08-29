use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::protocols::http::transaction::error::HttpManagedPathValidationMode;
use crate::protocols::http::transaction::error::validate_managed_path;
use crate::protocols::http::transaction::metadata::{
    HttpTransactionAdmissionError, admit_transaction_from_disk,
};
use crate::protocols::http::transaction::recorder::{
    StagingDirectoryNameClass, classify_staging_transaction_directory_name,
};
use crate::protocols::http::transaction::RecordedTransaction;
use crate::runtime::{OwnedRuntimeIdentity, RuntimeOperation, RuntimeProtocol};
use crate::session::{
    ProjectIdentity, SessionIdentity, SessionOperationRoot, SessionRecordV1, SessionState,
    SessionStore, SessionStoreError, SessionTimestamp,
};

/// Directory name of the acquisition operation root beneath the protocol root.
const ACQUISITION_OPERATION_DIRECTORY: &str = "get-raw-data";

#[derive(Debug, Clone)]
pub struct ProcessingHttpTransaction {
    project: ProjectIdentity,
    acquisition_runtime: OwnedRuntimeIdentity,
    acquisition_session: SessionIdentity,
    acquisition_session_state: SessionState,
    transaction: RecordedTransaction,
}

impl ProcessingHttpTransaction {
    pub fn project(&self) -> &ProjectIdentity {
        &self.project
    }
    pub fn acquisition_runtime(&self) -> &OwnedRuntimeIdentity {
        &self.acquisition_runtime
    }
    pub fn acquisition_session(&self) -> &SessionIdentity {
        &self.acquisition_session
    }
    pub fn acquisition_session_state(&self) -> SessionState {
        self.acquisition_session_state
    }
    pub fn transaction(&self) -> &RecordedTransaction {
        &self.transaction
    }
}

#[derive(Debug, Clone)]
pub struct ProcessingHttpTransactionCatalog {
    transactions: Vec<ProcessingHttpTransaction>,
}

impl ProcessingHttpTransactionCatalog {
    pub(crate) fn new(transactions: Vec<ProcessingHttpTransaction>) -> Self {
        Self { transactions }
    }

    pub fn as_slice(&self) -> &[ProcessingHttpTransaction] {
        &self.transactions
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ProcessingHttpTransaction> {
        self.transactions.iter()
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
}

#[derive(Debug)]
pub enum ProcessingTransactionDiscoveryError {
    RuntimeProtocolMismatch,
    RuntimeOperationMismatch,
    ManagedPath(crate::protocols::http::transaction::error::HttpManagedPathError),
    /// The supplied raw root is not exactly `protocol_root/data/raw`.
    RawRootDisagreement { expected: PathBuf, actual: PathBuf },
    /// `protocol_root/get-raw-data` is not a managed existing directory.
    AcquisitionRootInvalid {
        acquisition_root: PathBuf,
        source: crate::protocols::http::transaction::error::HttpManagedPathError,
    },
    RawRootEnumeration(std::io::Error),
    RawEntryMetadata(std::io::Error),
    RawEntrySymlink {
        entry_path: PathBuf,
    },
    RawEntryUnexpectedFile {
        entry_path: PathBuf,
    },
    RawEntryUnsupportedType {
        entry_path: PathBuf,
    },
    RawEntryNameInvalid {
        entry_path: PathBuf,
    },
    /// A directory carries the Core staging prefix but violates the staging grammar.
    RawEntryMalformedPartialDirectory {
        entry_path: PathBuf,
    },
    RawEntryUnrecognizedDirectory {
        entry_path: PathBuf,
    },
    TransactionAdmission {
        entry_path: PathBuf,
        source: HttpTransactionAdmissionError,
    },
    DuplicateTransactionIdentity {
        transaction_identity: String,
    },
    AcquisitionStoreOpen(SessionStoreError),
    AcquisitionSessionLoad {
        acquisition_session: SessionIdentity,
        source: SessionStoreError,
    },
    /// The typed provenance cache lost a record it had just admitted.
    ProvenanceCacheInvariant {
        acquisition_session: SessionIdentity,
    },
    Provenance(ProcessingTransactionProvenanceError),
}

impl ProcessingTransactionDiscoveryError {
    /// Whether this discovery failure was caused by transaction provenance.
    ///
    /// Used to select the stable session failure code without matching on Display.
    pub fn is_provenance_failure(&self) -> bool {
        matches!(self, Self::Provenance(_))
    }

    /// The retained provenance failure, when present.
    pub fn provenance_error(&self) -> Option<&ProcessingTransactionProvenanceError> {
        match self {
            Self::Provenance(error) => Some(error),
            _ => None,
        }
    }

    /// The raw entry path involved, when this failure names one.
    pub fn entry_path(&self) -> Option<&Path> {
        match self {
            Self::RawEntrySymlink { entry_path }
            | Self::RawEntryUnexpectedFile { entry_path }
            | Self::RawEntryUnsupportedType { entry_path }
            | Self::RawEntryNameInvalid { entry_path }
            | Self::RawEntryMalformedPartialDirectory { entry_path }
            | Self::RawEntryUnrecognizedDirectory { entry_path }
            | Self::TransactionAdmission { entry_path, .. } => Some(entry_path.as_path()),
            _ => None,
        }
    }
}

impl fmt::Display for ProcessingTransactionDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeProtocolMismatch => formatter
                .write_str("processing transaction discovery requires HTTP runtime protocol"),
            Self::RuntimeOperationMismatch => formatter.write_str(
                "processing transaction discovery requires processing runtime operation",
            ),
            Self::ManagedPath(_) => {
                formatter.write_str("processing transaction discovery path validation failed")
            }
            Self::RawRootDisagreement { .. } => formatter.write_str(
                "processing transaction discovery raw root does not match the protocol layout",
            ),
            Self::AcquisitionRootInvalid { .. } => formatter.write_str(
                "processing transaction discovery acquisition root is not a managed directory",
            ),
            Self::RawRootEnumeration(_) => {
                formatter.write_str("processing transaction discovery failed to enumerate raw root")
            }
            Self::RawEntryMetadata(_) => formatter
                .write_str("processing transaction discovery failed to inspect a raw entry"),
            Self::RawEntrySymlink { .. } => {
                formatter.write_str("processing transaction discovery rejected a raw-entry symlink")
            }
            Self::RawEntryUnexpectedFile { .. } => formatter.write_str(
                "processing transaction discovery rejected an unexpected raw file entry",
            ),
            Self::RawEntryUnsupportedType { .. } => formatter.write_str(
                "processing transaction discovery rejected an unsupported raw entry type",
            ),
            Self::RawEntryNameInvalid { .. } => formatter.write_str(
                "processing transaction discovery rejected an invalid raw directory name",
            ),
            Self::RawEntryMalformedPartialDirectory { .. } => formatter.write_str(
                "processing transaction discovery rejected a malformed partial raw directory",
            ),
            Self::RawEntryUnrecognizedDirectory { .. } => formatter.write_str(
                "processing transaction discovery rejected an unrecognized raw directory",
            ),
            Self::TransactionAdmission { .. } => formatter.write_str(
                "processing transaction discovery could not admit a finalized transaction",
            ),
            Self::DuplicateTransactionIdentity { .. } => formatter
                .write_str("processing transaction discovery rejected duplicate transaction identity"),
            Self::AcquisitionStoreOpen(_) => formatter.write_str(
                "processing transaction discovery failed to open acquisition session store",
            ),
            Self::AcquisitionSessionLoad { .. } => formatter.write_str(
                "processing transaction discovery failed to load acquisition session provenance",
            ),
            Self::ProvenanceCacheInvariant { .. } => formatter.write_str(
                "processing transaction discovery lost an admitted acquisition session record",
            ),
            Self::Provenance(_) => {
                formatter.write_str("processing transaction discovery rejected transaction provenance")
            }
        }
    }
}

impl std::error::Error for ProcessingTransactionDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManagedPath(error) => Some(error),
            Self::AcquisitionRootInvalid { source, .. } => Some(source),
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
    ProjectMismatch {
        acquisition_session: SessionIdentity,
    },
    SessionIdentityMismatch {
        acquisition_session: SessionIdentity,
    },
    RuntimeProtocolMismatch {
        acquisition_session: SessionIdentity,
    },
    RuntimeOperationMismatch {
        acquisition_session: SessionIdentity,
    },
    RuntimeSourceMismatch {
        acquisition_session: SessionIdentity,
    },
    SessionNotExecutable {
        acquisition_session: SessionIdentity,
        state: SessionState,
    },
    MissingStartedAt {
        acquisition_session: SessionIdentity,
    },
    TimestampOutOfBounds {
        acquisition_session: SessionIdentity,
        transaction_identity: String,
    },
}

impl ProcessingTransactionProvenanceError {
    /// The acquisition session this provenance failure concerns.
    pub fn acquisition_session(&self) -> &SessionIdentity {
        match self {
            Self::ProjectMismatch {
                acquisition_session,
            }
            | Self::SessionIdentityMismatch {
                acquisition_session,
            }
            | Self::RuntimeProtocolMismatch {
                acquisition_session,
            }
            | Self::RuntimeOperationMismatch {
                acquisition_session,
            }
            | Self::RuntimeSourceMismatch {
                acquisition_session,
            }
            | Self::SessionNotExecutable {
                acquisition_session, ..
            }
            | Self::MissingStartedAt {
                acquisition_session,
            }
            | Self::TimestampOutOfBounds {
                acquisition_session, ..
            } => acquisition_session,
        }
    }
}

impl fmt::Display for ProcessingTransactionProvenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectMismatch { .. } => {
                formatter.write_str("acquisition session project does not match processing project")
            }
            Self::SessionIdentityMismatch { .. } => formatter.write_str(
                "transaction session identity does not match durable acquisition session identity",
            ),
            Self::RuntimeProtocolMismatch { .. } => {
                formatter.write_str("acquisition session runtime protocol is not HTTP")
            }
            Self::RuntimeOperationMismatch { .. } => {
                formatter.write_str("acquisition session runtime operation is not acquisition")
            }
            Self::RuntimeSourceMismatch { .. } => formatter.write_str(
                "acquisition session runtime source does not match processing runtime source",
            ),
            Self::SessionNotExecutable { .. } => {
                formatter.write_str("acquisition session state cannot prove execution")
            }
            Self::MissingStartedAt { .. } => formatter
                .write_str("acquisition session record does not contain execution start timestamp"),
            Self::TimestampOutOfBounds { .. } => formatter.write_str(
                "transaction timestamps fall outside acquisition session temporal bounds",
            ),
        }
    }
}

impl std::error::Error for ProcessingTransactionProvenanceError {}

/// Discover, admit, and validate every finalized raw transaction for processing.
///
/// Sequence:
/// 1. Validate the processing runtime protocol and operation.
/// 2. Require `raw_root == protocol_root/data/raw` exactly.
/// 3. Enumerate raw entries and strictly classify each directory name.
/// 4. Admit finalized transactions from disk.
/// 5. Require `acquisition_root == protocol_root/get-raw-data` exactly and open it.
/// 6. Admit each acquisition session record once, cached by typed session identity.
/// 7. Validate **every** transaction against its durable session record.
/// 8. Build the deterministic catalog and reject duplicate identities.
///
/// This function never mutates acquisition raw data. Partial directories are ignored,
/// never deleted.
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

    // Discovery establishes its own raw-root invariant rather than trusting the caller.
    let expected_raw_root = protocol_root.join("data").join("raw");
    if raw_root != expected_raw_root {
        return Err(ProcessingTransactionDiscoveryError::RawRootDisagreement {
            expected: expected_raw_root,
            actual: raw_root.to_path_buf(),
        });
    }

    validate_managed_path(
        protocol_root,
        raw_root,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(ProcessingTransactionDiscoveryError::ManagedPath)?;

    let admitted_transactions = enumerate_finalized_transactions(raw_root)?;

    let acquisition_store = open_acquisition_session_store(protocol_root)?;

    let mut acquisition_records: HashMap<SessionIdentity, SessionRecordV1> = HashMap::new();
    let mut discovered = Vec::with_capacity(admitted_transactions.len());

    for transaction in admitted_transactions {
        let acquisition_session = transaction.session().clone();

        // 1. Session-record admission: cached once per typed session identity.
        if !acquisition_records.contains_key(&acquisition_session) {
            let record = acquisition_store.load(&acquisition_session).map_err(|source| {
                ProcessingTransactionDiscoveryError::AcquisitionSessionLoad {
                    acquisition_session: acquisition_session.clone(),
                    source,
                }
            })?;
            validate_session_record(project, processing_runtime, &record)
                .map_err(ProcessingTransactionDiscoveryError::Provenance)?;
            acquisition_records.insert(acquisition_session.clone(), record);
        }

        let Some(record) = acquisition_records.get(&acquisition_session) else {
            return Err(
                ProcessingTransactionDiscoveryError::ProvenanceCacheInvariant {
                    acquisition_session,
                },
            );
        };

        // 2. Transaction-to-session provenance: runs for every transaction, always.
        //    Cache presence never bypasses this.
        validate_transaction_against_session(record, &transaction)
            .map_err(ProcessingTransactionDiscoveryError::Provenance)?;

        discovered.push(ProcessingHttpTransaction {
            project: project.clone(),
            acquisition_runtime: record.runtime().clone(),
            acquisition_session,
            acquisition_session_state: record.state(),
            transaction,
        });
    }

    discovered.sort_by(|left, right| {
        (
            left.transaction.created_at_unix_nanos(),
            left.transaction.identity().id(),
        )
            .cmp(&(
                right.transaction.created_at_unix_nanos(),
                right.transaction.identity().id(),
            ))
    });

    let mut seen = HashSet::new();
    for transaction in &discovered {
        let inserted = seen.insert(transaction.transaction.identity().id().to_string());
        if !inserted {
            return Err(
                ProcessingTransactionDiscoveryError::DuplicateTransactionIdentity {
                    transaction_identity: transaction.transaction.identity().id().to_string(),
                },
            );
        }
    }

    Ok(ProcessingHttpTransactionCatalog::new(discovered))
}

/// Enumerate the raw root and strictly admit finalized transaction directories.
fn enumerate_finalized_transactions(
    raw_root: &Path,
) -> Result<Vec<RecordedTransaction>, ProcessingTransactionDiscoveryError> {
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
            return Err(ProcessingTransactionDiscoveryError::RawEntryUnexpectedFile {
                entry_path: path,
            });
        }

        if !file_type.is_dir() {
            return Err(ProcessingTransactionDiscoveryError::RawEntryUnsupportedType {
                entry_path: path,
            });
        }

        match classify_raw_directory_name(&entry.file_name()) {
            RawDirectoryClass::RecognizedPartial => continue,
            RawDirectoryClass::MalformedPartial => {
                return Err(
                    ProcessingTransactionDiscoveryError::RawEntryMalformedPartialDirectory {
                        entry_path: path,
                    },
                );
            }
            RawDirectoryClass::FinalizedCandidate => {
                let transaction =
                    admit_transaction_from_disk(raw_root, &path).map_err(|source| {
                        ProcessingTransactionDiscoveryError::TransactionAdmission {
                            entry_path: path.clone(),
                            source,
                        }
                    })?;
                admitted_transactions.push(transaction);
            }
            RawDirectoryClass::Unrecognized => {
                return Err(
                    ProcessingTransactionDiscoveryError::RawEntryUnrecognizedDirectory {
                        entry_path: path,
                    },
                );
            }
            RawDirectoryClass::InvalidName => {
                return Err(ProcessingTransactionDiscoveryError::RawEntryNameInvalid {
                    entry_path: path,
                });
            }
        }
    }

    Ok(admitted_transactions)
}

/// Require the exact acquisition operation root and open its managed session store.
fn open_acquisition_session_store(
    protocol_root: &Path,
) -> Result<SessionStore, ProcessingTransactionDiscoveryError> {
    let expected_acquisition_root = protocol_root.join(ACQUISITION_OPERATION_DIRECTORY);

    validate_managed_path(
        protocol_root,
        &expected_acquisition_root,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(
        |source| ProcessingTransactionDiscoveryError::AcquisitionRootInvalid {
            acquisition_root: expected_acquisition_root.clone(),
            source,
        },
    )?;

    let acquisition_operation_root = SessionOperationRoot::new(expected_acquisition_root)
        .map_err(ProcessingTransactionDiscoveryError::AcquisitionStoreOpen)?;
    SessionStore::open(acquisition_operation_root)
        .map_err(ProcessingTransactionDiscoveryError::AcquisitionStoreOpen)
}

enum RawDirectoryClass {
    /// A well-formed Core staging directory; ignore it and never delete it.
    RecognizedPartial,
    /// Carries the staging prefix but violates the recorder grammar.
    MalformedPartial,
    FinalizedCandidate,
    Unrecognized,
    InvalidName,
}

/// Classify a raw-root directory name using the authoritative Core grammars.
///
/// The staging grammar belongs to the HTTP transaction recorder and the finalized
/// grammar belongs to transaction admission; neither is re-derived here.
fn classify_raw_directory_name(name: &std::ffi::OsStr) -> RawDirectoryClass {
    let Some(name) = name.to_str() else {
        return RawDirectoryClass::InvalidName;
    };

    match classify_staging_transaction_directory_name(name) {
        StagingDirectoryNameClass::Valid { .. } => return RawDirectoryClass::RecognizedPartial,
        StagingDirectoryNameClass::Malformed => return RawDirectoryClass::MalformedPartial,
        StagingDirectoryNameClass::NotStaging => {}
    }

    if crate::protocols::http::transaction::metadata::parse_transaction_directory_name(name).is_ok()
    {
        return RawDirectoryClass::FinalizedCandidate;
    }

    RawDirectoryClass::Unrecognized
}

/// Validate the session-level invariants of a durable acquisition session record.
///
/// These properties depend only on the record, so they may be proven once per session
/// identity and cached.
fn validate_session_record(
    processing_project: &ProjectIdentity,
    processing_runtime: &OwnedRuntimeIdentity,
    acquisition_record: &SessionRecordV1,
) -> Result<(), ProcessingTransactionProvenanceError> {
    if acquisition_record.project() != processing_project {
        return Err(ProcessingTransactionProvenanceError::ProjectMismatch {
            acquisition_session: acquisition_record.session().clone(),
        });
    }

    if acquisition_record.runtime().protocol() != RuntimeProtocol::Http {
        return Err(
            ProcessingTransactionProvenanceError::RuntimeProtocolMismatch {
                acquisition_session: acquisition_record.session().clone(),
            },
        );
    }

    if acquisition_record.runtime().operation() != RuntimeOperation::Acquisition {
        return Err(
            ProcessingTransactionProvenanceError::RuntimeOperationMismatch {
                acquisition_session: acquisition_record.session().clone(),
            },
        );
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

    if acquisition_record.started_at().is_none() {
        return Err(ProcessingTransactionProvenanceError::MissingStartedAt {
            acquisition_session: acquisition_record.session().clone(),
        });
    }

    Ok(())
}

/// Validate one transaction against its durable acquisition session record.
///
/// This runs for every transaction, including transactions whose session record was
/// already admitted and cached.
fn validate_transaction_against_session(
    acquisition_record: &SessionRecordV1,
    transaction: &RecordedTransaction,
) -> Result<(), ProcessingTransactionProvenanceError> {
    if acquisition_record.session() != transaction.session() {
        return Err(
            ProcessingTransactionProvenanceError::SessionIdentityMismatch {
                acquisition_session: acquisition_record.session().clone(),
            },
        );
    }

    let Some(started_at) = acquisition_record.started_at() else {
        return Err(ProcessingTransactionProvenanceError::MissingStartedAt {
            acquisition_session: acquisition_record.session().clone(),
        });
    };

    let transaction_created =
        SessionTimestamp::from_nanos_since_epoch(transaction.created_at_unix_nanos());
    let transaction_completed =
        SessionTimestamp::from_nanos_since_epoch(transaction.completed_at_unix_nanos());

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_directory_classifier_accepts_known_partial_and_finalized_shapes() {
        let valid_partial = std::ffi::OsString::from(".partial-1700000000-abc");
        match classify_raw_directory_name(&valid_partial) {
            RawDirectoryClass::RecognizedPartial => {}
            other => panic!("expected RecognizedPartial, got {other:?}"),
        }

        let malformed = std::ffi::OsString::from(".partial-abc");
        match classify_raw_directory_name(&malformed) {
            RawDirectoryClass::MalformedPartial => {}
            other => panic!("expected MalformedPartial, got {other:?}"),
        }

        let finalized = std::ffi::OsString::from("1700000000-abc");
        match classify_raw_directory_name(&finalized) {
            RawDirectoryClass::FinalizedCandidate => {}
            other => panic!("expected FinalizedCandidate, got {other:?}"),
        }

        let unrecognized = std::ffi::OsString::from("scratch");
        match classify_raw_directory_name(&unrecognized) {
            RawDirectoryClass::Unrecognized => {}
            other => panic!("expected Unrecognized, got {other:?}"),
        }
    }

    #[test]
    fn discovery_error_is_provenance_failure_for_provenance_variants_only() {
        use std::io;
        let prov =
            ProcessingTransactionDiscoveryError::RawRootEnumeration(io::Error::other("seed"));
        assert!(prov.is_provenance_failure());
        assert!(prov.entry_path().is_none());

        let not_prov = ProcessingTransactionDiscoveryError::RuntimeProtocolMismatch;
        assert!(!not_prov.is_provenance_failure());
        assert!(not_prov.provenance_error().is_none());

        let symlink = ProcessingTransactionDiscoveryError::RawEntrySymlink {
            entry_path: PathBuf::from("/tmp/raw/symlink"),
        };
        assert!(!symlink.is_provenance_failure());
        assert_eq!(
            symlink.entry_path(),
            Some(std::path::Path::new("/tmp/raw/symlink"))
        );
    }

    #[test]
    fn catalog_iteration_is_total_and_order_preserving() {
        use std::path::PathBuf;

        let runtime = OwnedRuntimeIdentity::http_acquisition("proc-cat-source", 1);
        let make = |suffix: &str| -> ProcessingHttpTransaction {
            use crate::protocols::http::transaction::{
                HttpAttemptIdentity, HttpRecordedOutcome, HttpTransactionIdentity, RecordedHeader,
                RecordedHeaderCollection, RecordedHttpRequest, RecordedHttpResponse,
                RecordedTransaction,
            };
            use crate::session::{ProjectIdentity, SessionIdentity, SessionState};
            let project = ProjectIdentity::new("proc-cat-project").unwrap();
            let session = SessionIdentity::new(format!("proc-cat-{suffix}")).unwrap();
            let identity =
                HttpTransactionIdentity::new().expect("transaction identity");
            let attempt = HttpAttemptIdentity::new(1, 0, 0).expect("attempt");
            let tx = RecordedTransaction::new(
                identity,
                attempt,
                None,
                None,
                session.clone(),
                1_700_000_000,
                PathBuf::from(format!("/tmp/raw/{suffix}")),
                RecordedHttpRequest::new(None, 0, None),
                RecordedHttpResponse::new(
                    Some(200),
                    RecordedHeaderCollection::new(Vec::<RecordedHeader>::new()),
                    PathBuf::from(format!("/tmp/raw/{suffix}/body")),
                    0,
                    None,
                    1_700_000_000,
                    HttpRecordedOutcome::Response,
                ),
            );
            ProcessingHttpTransaction {
                project,
                acquisition_runtime: runtime.clone(),
                acquisition_session: session,
                acquisition_session_state: SessionState::Succeeded,
                transaction: tx,
            }
        };

        let items = vec![make("a"), make("b"), make("c")];
        let catalog = ProcessingHttpTransactionCatalog::new(items.clone());
        assert_eq!(catalog.len(), 3);
        assert!(!catalog.is_empty());
        assert_eq!(catalog.as_slice().len(), 3);
        let collected: Vec<&ProcessingHttpTransaction> = catalog.iter().collect();
        assert_eq!(collected.len(), 3);
    }
}
