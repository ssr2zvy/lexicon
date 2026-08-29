//! `lexicon-cli` library re-exports for the binary's `main.rs` and any
//! future integration test that drives dispatch headlessly.
//!
//! CLI-01 introduces this lib.rs so the binary can call into the typed
//! dispatch surface (`Cli::parse → dispatch → Result<(), CliError>`)
//! while keeping the binary target named `lexicon`.

pub mod cli;

pub use cli::{Cli, CliError, RootCommand, cancellation_exit_code, dispatch, exit_code_for_outcome};
