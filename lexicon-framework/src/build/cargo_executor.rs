//! BUILD-02 typed Cargo command seam.
//!
//! Production code converts only the typed variants in this enum into
//! exact Cargo arguments; a test-only fake implementation exercises
//! failure and success paths without invoking a real Cargo binary. The
//! seam prevents the framework from calling `Command::new("cargo")`
//! with anything other than the audited flags.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Trait abstraction shared by [`ProductionCargoExecutor`] and
/// [`FakeCargoExecutor`]. Production callers accept any clonable
/// implementation; tests inject the fake to script success and failure
/// paths without invoking real Cargo.
pub trait CargoExecutor: Send + Sync {
    fn run(
        &self,
        invocation: &CargoInvocation,
    ) -> Result<CargoOutput, CargoExecutionError>;
}

/// Closed enumeration of every Cargo invocation the framework is
/// allowed to make. Anything not represented here MUST be added
/// explicitly so a reviewer can audit it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CargoInvocation {
    /// `cargo generate-lockfile --manifest-path <manifest>`. Source
    /// scaffold runs this once per workspace, against a committed
    /// manifest, to obtain a `Cargo.lock` without compiling.
    GenerateLockfile { manifest: PathBuf },
    /// `cargo metadata --locked --format-version 1 --manifest-path <manifest>`.
    /// Used to verify the resolved Core identity agrees with the
    /// embedded Core revision.
    MetadataLocked { manifest: PathBuf },
    /// `cargo build --manifest-path <manifest> --package <pkg> --bin <bin>
    ///  --release --locked --message-format=json-render-diagnostics
    ///  --target-dir <target_dir>`.
    BuildReleaseLocked {
        manifest: PathBuf,
        target_dir: PathBuf,
    },
}

impl CargoInvocation {
    /// Public summary used when reporting invocations to operators.
    pub fn summary(&self) -> &'static str {
        match self {
            Self::GenerateLockfile { .. } => "cargo generate-lockfile",
            Self::MetadataLocked { .. } => "cargo metadata --locked",
            Self::BuildReleaseLocked { .. } => "cargo build --release --locked",
        }
    }

    /// The manifest path every Cargo invocation carries.
    pub fn manifest(&self) -> &Path {
        match self {
            Self::GenerateLockfile { manifest }
            | Self::MetadataLocked { manifest }
            | Self::BuildReleaseLocked { manifest, .. } => manifest,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CargoOutput {
    pub status_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum CargoExecutionError {
    /// Cargo was not invokable on this host.
    #[error("failed to spawn cargo for {summary}: {source}", summary = invocation.summary())]
    Spawn {
        invocation: CargoInvocation,
        #[source]
        source: std::io::Error,
    },
    /// Cargo exited with a non-zero status.
    #[error("cargo invocation {summary} exited with {code:?}", summary = invocation.summary(), code = status_code)]
    NonZeroExit {
        invocation: CargoInvocation,
        status_code: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
}

/// Production executor that shells out to real Cargo using a typed
/// translation table. Tests use the in-process [`FakeCargoExecutor`]
/// instead.
pub struct ProductionCargoExecutor;

impl ProductionCargoExecutor {
    pub fn new() -> Self {
        Self
    }

    fn translate(invocation: &CargoInvocation) -> Result<Command, CargoExecutionError> {
        match invocation {
            CargoInvocation::GenerateLockfile { manifest } => {
                let mut command = Command::new("cargo");
                command.arg("generate-lockfile");
                command.arg("--manifest-path");
                command.arg(manifest);
                Ok(command)
            }
            CargoInvocation::MetadataLocked { manifest } => {
                let mut command = Command::new("cargo");
                command.arg("metadata");
                command.arg("--locked");
                command.arg("--format-version");
                command.arg("1");
                command.arg("--manifest-path");
                command.arg(manifest);
                Ok(command)
            }
            CargoInvocation::BuildReleaseLocked {
                manifest,
                target_dir,
            } => {
                let mut command = Command::new("cargo");
                command.arg("build");
                command.arg("--manifest-path");
                command.arg(manifest);
                // Caller further specializes --package and --bin; the
                // seam leaves that to the higher-level wrapper.
                command.arg("--release");
                command.arg("--locked");
                command.arg("--message-format=json-render-diagnostics");
                command.arg("--target-dir");
                command.arg(target_dir);
                Ok(command)
            }
        }
    }
}

impl Default for ProductionCargoExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl CargoExecutor for ProductionCargoExecutor {
    fn run(&self, invocation: &CargoInvocation) -> Result<CargoOutput, CargoExecutionError> {
        let mut command = Self::translate(invocation)?;
        let output: Output = command
            .output()
            .map_err(|source| CargoExecutionError::Spawn {
                invocation: invocation.clone(),
                source,
            })?;
        Ok(CargoOutput {
            status_code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

/// In-process fake executor used by tests to script success and failure
/// sequences without invoking real Cargo. The audit's "production code
/// calls Cargo only through this seam" guarantee relies on this being
/// the only injection point.
#[derive(Debug, Default)]
pub struct FakeCargoExecutor {
    responses: std::sync::Mutex<Vec<FakeCargoResponse>>,
}

#[derive(Debug, Clone)]
pub struct FakeCargoResponse {
    pub invocation_kind: CargoInvocationKind,
    pub output: CargoOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoInvocationKind {
    GenerateLockfile,
    MetadataLocked,
    BuildReleaseLocked,
}

impl CargoInvocationKind {
    pub fn classify(invocation: &CargoInvocation) -> Self {
        match invocation {
            CargoInvocation::GenerateLockfile { .. } => Self::GenerateLockfile,
            CargoInvocation::MetadataLocked { .. } => Self::MetadataLocked,
            CargoInvocation::BuildReleaseLocked { .. } => Self::BuildReleaseLocked,
        }
    }
}

impl FakeCargoExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a fake response in FIFO order. The fake executor pops the
    /// oldest matching entry each time `run` is called.
    pub fn expect(&self, kind: CargoInvocationKind, output: CargoOutput) {
        self.responses
            .lock()
            .expect("poisoning")
            .push(FakeCargoResponse {
                invocation_kind: kind,
                output,
            });
    }

    /// Number of unconsumed responses.
    pub fn len(&self) -> usize {
        self.responses.lock().expect("poisoning").len()
    }
}

impl CargoExecutor for FakeCargoExecutor {
    fn run(&self, invocation: &CargoInvocation) -> Result<CargoOutput, CargoExecutionError> {
        let kind = CargoInvocationKind::classify(invocation);
        let mut responses = self.responses.lock().expect("poisoning");
        let index = responses
            .iter()
            .position(|r| r.invocation_kind == kind)
            .unwrap_or_else(|| {
                panic!(
                    "FakeCargoExecutor: no queued response for {:?}; queued kinds: {:?}",
                    kind,
                    responses.iter().map(|r| r.invocation_kind).collect::<Vec<_>>()
                )
            });
        let response = responses.remove(index);
        let output = response.output;
        if output.status_code.unwrap_or(0) != 0 {
            return Err(CargoExecutionError::NonZeroExit {
                invocation: invocation.clone(),
                status_code: output.status_code,
                stdout: output.stdout,
                stderr: output.stderr,
            });
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produce_translator_uses_exact_audit_arguments() {
        let manifest = PathBuf::from("/tmp/Cargo.toml");
        let command = ProductionCargoExecutor::translate(&CargoInvocation::GenerateLockfile {
            manifest: manifest.clone(),
        })
        .expect("translate");
        assert_eq!(command.get_program(), "cargo");
        let args: Vec<&std::ffi::OsStr> = command.get_args().collect();
        assert_eq!(args[0], "generate-lockfile");
        assert_eq!(args[1], "--manifest-path");
        assert_eq!(args[2], manifest.as_os_str());
    }

    #[test]
    fn metadata_translator_includes_locked_flag() {
        let manifest = PathBuf::from("/tmp/Cargo.toml");
        let command = ProductionCargoExecutor::translate(&CargoInvocation::MetadataLocked {
            manifest: manifest.clone(),
        })
        .expect("translate");
        let args: Vec<&std::ffi::OsStr> = command.get_args().collect();
        assert!(args.iter().any(|a| *a == "--locked"));
        assert!(args.iter().any(|a| *a == "--format-version"));
        assert!(args.iter().any(|a| *a == "1"));
    }

    #[test]
    fn build_translator_requires_release_and_locked() {
        let manifest = PathBuf::from("/tmp/Cargo.toml");
        let target_dir = PathBuf::from("/tmp/target");
        let command = ProductionCargoExecutor::translate(&CargoInvocation::BuildReleaseLocked {
            manifest: manifest.clone(),
            target_dir: target_dir.clone(),
        })
        .expect("translate");
        let args: Vec<&std::ffi::OsStr> = command.get_args().collect();
        assert!(args.contains(&std::ffi::OsStr::new("--release")));
        assert!(args.contains(&std::ffi::OsStr::new("--locked")));
        assert!(args.contains(&std::ffi::OsStr::new("--message-format=json-render-diagnostics")));
        assert!(args.contains(&std::ffi::OsStr::new("--target-dir")));
    }

    #[test]
    fn fake_executor_returns_queued_response() {
        let fake = FakeCargoExecutor::new();
        fake.expect(
            CargoInvocationKind::GenerateLockfile,
            CargoOutput {
                status_code: Some(0),
                stdout: b"generated".to_vec(),
                stderr: Vec::new(),
            },
        );
        let output = <FakeCargoExecutor as CargoExecutor>::run(
            &fake,
            &CargoInvocation::GenerateLockfile {
                manifest: PathBuf::from("/tmp/Cargo.toml"),
            },
        )
        .expect("run");
        assert_eq!(output.stdout, b"generated");
        assert_eq!(fake.len(), 0);
    }

    #[test]
    fn fake_executor_converts_non_zero_exit_to_typed_error() {
        let fake = FakeCargoExecutor::new();
        fake.expect(
            CargoInvocationKind::BuildReleaseLocked,
            CargoOutput {
                status_code: Some(101),
                stdout: Vec::new(),
                stderr: b"build failed".to_vec(),
            },
        );
        let err = <FakeCargoExecutor as CargoExecutor>::run(
            &fake,
            &CargoInvocation::BuildReleaseLocked {
                manifest: PathBuf::from("/tmp/Cargo.toml"),
                target_dir: PathBuf::from("/tmp/target"),
            },
        )
        .unwrap_err();
        match err {
            CargoExecutionError::NonZeroExit {
                status_code,
                stderr,
                ..
            } => {
                assert_eq!(status_code, Some(101));
                assert_eq!(stderr, b"build failed");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
}
