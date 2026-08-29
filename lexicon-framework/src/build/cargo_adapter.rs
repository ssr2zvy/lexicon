//! BUILD-02 public surface over [`cargo_executor`].
pub use cargo_executor::{
    CargoExecutionError, CargoExecutor, CargoInvocation, CargoInvocationKind, CargoOutput,
    FakeCargoExecutor, FakeCargoResponse, ProductionCargoExecutor,
};

mod cargo_executor;

use std::path::PathBuf;
use thiserror::Error;

/// Errors raised when a registered invocation cannot run.
#[derive(Debug, Error)]
pub enum CommandInvocationError {
    /// The translation table does not yet include the requested
    /// invocation. Add it before releasing.
    #[error("no audit-approved translation for invocation: {0}")]
    UnauditedTranslation(&'static str),
    /// Cargo was not invokable on this host.
    #[error("failed to spawn cargo: {0}")]
    Spawn(#[source] std::io::Error),
    /// Cargo exited with a non-zero status.
    #[error("cargo invocation exited with {code:?}: {stderr}")]
    NonZeroExit {
        code: Option<i32>,
        stderr: String,
    },
}

impl From<CargoExecutionError> for CommandInvocationError {
    fn from(error: CargoExecutionError) -> Self {
        match error {
            CargoExecutionError::Spawn { source, .. } => Self::Spawn(source),
            CargoExecutionError::NonZeroExit {
                status_code, stderr, ..
            } => Self::NonZeroExit {
                code: status_code,
                    stderr: String::from_utf8_lossy(&stderr).to_string(),
                },
        }
    }
}

/// Build the standard workspace `Cargo.lock` next to the supplied
/// manifest. Returns the path of the generated lockfile on success.
pub fn generate_workspace_lockfile<F: CargoExecutor>(
    executor: &F,
    manifest: &PathBuf,
) -> Result<PathBuf, CommandInvocationError> {
    let invocation = CargoInvocation::GenerateLockfile {
        manifest: manifest.clone(),
    };
    executor
        .run(&invocation)
        .map_err(CommandInvocationError::from)?;
    Ok(manifest.parent().unwrap_or(manifest).join("Cargo.lock"))
}

/// Run a `cargo metadata --locked` invocation and return the raw
/// stdout. Callers parse it as JSON.
pub fn read_metadata_locked<F: CargoExecutor>(
    executor: &F,
    manifest: &PathBuf,
) -> Result<Vec<u8>, CommandInvocationError> {
    let invocation = CargoInvocation::MetadataLocked {
        manifest: manifest.clone(),
    };
    executor
        .run(&invocation)
        .map(|output| output.stdout)
        .map_err(CommandInvocationError::from)
}
