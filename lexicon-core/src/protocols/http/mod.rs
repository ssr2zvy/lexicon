pub mod contract;
pub mod error;

pub use crate::HttpAcquisitionContext;
pub use contract::{HttpAcquireFn, HttpSourceContractV1};
pub use error::{AcquisitionError, AcquisitionResult};
