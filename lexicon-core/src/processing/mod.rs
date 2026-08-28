pub mod context;
pub mod error;
pub mod invocation;
pub mod runner;
pub mod runtime_information;

mod contract;
mod transactions;

pub use context::ProcessingContext;
pub use contract::{ProcessDataFn, ProcessingSourceContractV1};
pub use error::{
    ProcessingContextConstructionError, ProcessingDatabaseOpenError,
    ProcessingDatabasePartialCommit, ProcessingDatabasePathError,
    ProcessingDatabaseTransactionError, ProcessingError, ProcessingResult,
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
