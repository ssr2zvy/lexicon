//! Foreground data execution pipeline.
//!
//! Provides the typed public API for the `lexicon data` CLI command.
//! Supports foreground `--get` (acquisition) and `--process` (processing) runs.

pub mod error;
pub mod foreground;
pub mod outcome;
pub mod project;
pub mod request;
pub mod runtime;
pub mod session;

pub use error::ForegroundDataExecutionError;
pub use foreground::execute_foreground_data;
pub use outcome::ForegroundDataOutcome;
pub use request::{DataOperation, ForegroundDataRequest};
