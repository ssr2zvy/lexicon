//! Processing source API.
//!
//! Only source-useful types cross this boundary: the bound context, admitted
//! transactions, the source error type, the descriptor/admission/probe/runner APIs,
//! a compatible `rusqlite`, and the errors a source or supervisor genuinely needs to
//! inspect.
//!
//! Deliberately internal: raw-directory classifiers, transaction admission helpers,
//! provenance caches, running-lifecycle ownership wrappers, database-state
//! transitions, sidecar validation helpers, commit and durability helpers, and every
//! unchecked constructor.

// `runner` stays public because generated managed runners import it by path.
pub mod runner;
pub mod runtime_information;

mod context;
mod contract;
mod error;
mod invocation;
mod transactions;

pub use context::ProcessingContext;
pub use contract::{ProcessDataFn, ProcessingSourceContractV1};
pub use error::{
    ProcessingContextConstructionError, ProcessingDatabaseCommitOutcomeUncertain,
    ProcessingDatabaseConfigurationError, ProcessingDatabaseDurabilityError,
    ProcessingDatabaseOpenError, ProcessingDatabasePartialCommit,
    ProcessingDatabasePartialCommitCause, ProcessingDatabasePartialCommitPhase,
    ProcessingDatabasePathError, ProcessingDatabaseSetting, ProcessingDatabaseSidecarError,
    ProcessingDatabaseSidecarKind, ProcessingDatabaseTransactionError, ProcessingDurabilityPhase,
    ProcessingError, ProcessingLifecycleError, ProcessingManagedPathCategory, ProcessingResult,
    ProcessingSetupAndPersistenceFailure, ProcessingSetupError,
    ProcessingTransactionBoundaryPhase, ProcessingTransactionBoundaryViolation,
};
pub use invocation::{
    AdmittedProcessingHandler, AdmittedProcessingRuntimeInvocation,
    ProcessingRuntimeInvocationAdmissionError, admit_processing_runtime_invocation,
};
pub use runner::{
    ProcessingRuntimeInformationProbeError, ProcessingRuntimeInformationProbeOutcome,
    ProcessingRuntimeInvocationExecutionError, RUNTIME_INFORMATION_PROBE_ARGUMENT,
    run_processing_runtime_invocation, try_write_runtime_information_probe,
};
pub use runtime_information::{
    ProcessingRuntimeCompatibilityError, ProcessingRuntimeInformationConstructionError,
    ProcessingRuntimeInformationDecodingError, ProcessingRuntimeInformationEncodingError,
    ProcessingRuntimeInformationV1,
};
pub use transactions::{
    ProcessingHttpTransaction, ProcessingHttpTransactionCatalog,
    ProcessingTransactionDiscoveryError, ProcessingTransactionProvenanceError,
};

pub use rusqlite;
