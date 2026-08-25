pub mod context;
pub mod error;

mod contract;

pub use contract::{ProcessDataFn, ProcessingSourceContractV1};
pub use context::ProcessingContext;
pub use error::{ProcessingError, ProcessingResult};
