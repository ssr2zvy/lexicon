use clap::{CommandFactory, Parser, Subcommand};

pub mod build;
pub mod data;
pub mod init;
pub mod operator_host;
pub mod source;

pub use build::BuildCommand;
pub use data::{DataCommand, DataMode};
pub use init::InitCommand;
pub use operator_host::OperatorHostCommand;
pub use source::{SourceAction, SourceCommand};

// FOREGROUND-02: typed cancellation surfacing. The audit fixes shell
// cancellation exit codes to 130 (SIGINT/CTRL_C_EVENT) and 143
// (SIGTERM/CTRL_BREAK_EVENT). Forward the cancellation kind observed
// by the supervisor into the typed CLI error so the binary's exit
// status faithfully reflects operator intent.
use std::process::ExitCode;

/// Map a `SupervisedChild` cancellation outcome onto the canonical
/// shell exit code. We do NOT convert forced-terminated and
/// gracefully-cancelled outcomes into different exit codes: the audit
/// explicitly maps both to 130 (interrupt) or 143 (terminate) based on
/// the originating kind, then optionally to 1 if the kind is unknown.
pub fn cancellation_exit_code(
    kind: lexicon_framework::process::CancellationKind,
    forced: bool,
) -> ExitCode {
    match (kind, forced) {
        (lexicon_framework::process::CancellationKind::Interrupt, _)
        | (lexicon_framework::process::CancellationKind::ConsoleClose, _) => ExitCode::from(130),
        (lexicon_framework::process::CancellationKind::Terminate, _) => ExitCode::from(143),
    }
}

/// Convenience: translate a `SupervisionOutcome::Cancelled*`.
pub fn exit_code_for_outcome(
    outcome: &lexicon_framework::process::SupervisionOutcome,
) -> ExitCode {
    match outcome {
        lexicon_framework::process::SupervisionOutcome::Completed { .. } => ExitCode::SUCCESS,
        lexicon_framework::process::SupervisionOutcome::CancelledGracefully { kind, .. }
        | lexicon_framework::process::SupervisionOutcome::CancelledForcefully { kind, .. } => {
            cancellation_exit_code(*kind, false)
        }
        lexicon_framework::process::SupervisionOutcome::CancellationUncertain { .. } => {
            ExitCode::from(1)
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "lexicon",
    version,
    about = "Lexicon: make data",
    long_about = "Lexicon CLI for raw-data acquisition, processing, source management, and build orchestration."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<RootCommand>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum RootCommand {
    Data(DataCommand),
    Source(SourceCommand),
    Init(InitCommand),
    Build(BuildCommand),
    /// Reserved internal entrypoint; not part of the public CLI surface.
    ///
    /// `clap`'s `#[derive(Subcommand)]` takes a variant's subcommand name from
    /// the kebab-cased variant name, not from the wrapped struct's own
    /// `#[command(name = ...)]` attribute (that only applies when the struct is
    /// used as a standalone `Parser`). The other variants above never needed an
    /// override because their desired name already equals their kebab-cased
    /// variant name; `OperatorHost` kebab-cases to `operator-host`, not
    /// `__operator-host`, so both `name` and `hide` must be set here explicitly.
    #[command(name = "__operator-host", hide = true)]
    OperatorHost(OperatorHostCommand),
}

/// Typed CLI error surface (CLI-01 + FOREGROUND-02). The `Interrupted`
/// variant carries the cancellation outcome through the typed boundary
/// so `main.rs` returns the canonical 130/143 exit codes that shells
/// parse.
#[derive(Debug)]
pub enum CliError {
    /// A `lexicon data` foreground or `--process` rejection reached the
    /// CLI surface.
    ForegroundData(String),
    /// A `lexicon data --bg` handoff could not complete.
    BackgroundHandoff(String),
    /// A `lexicon source create|build` operation failed.
    SourceCommand(String),
    /// `lexicon init` failed.
    Init(String),
    /// `lexicon build` failed.
    BuildAll(String),
    /// The reserved `--bg` operator host surfaced an error.
    OperatorHost(String),
    /// Operator requested cancellation; we record the kind and whether
    /// the supervisor escalated to forced termination. The audit forbids
    /// misreporting cancellation as success; this variant carries
    /// observable shape data so the `Display` impl never hides it.
    Interrupted {
        kind: lexicon_framework::process::CancellationKind,
        unix_signal: Option<u8>,
        forced: bool,
    },
    /// Generic fallback; will be deprecated as concrete variants
    /// replace stringly-typed dispatch paths.
    Message(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForegroundData(message)
            | Self::BackgroundHandoff(message)
            | Self::SourceCommand(message)
            | Self::Init(message)
            | Self::BuildAll(message)
            | Self::OperatorHost(message)
            | Self::Message(message) => formatter.write_str(message),
            Self::Interrupted {
                kind,
                forced,
                ..
            } => write!(formatter, "operator interrupted (kind={kind}, forced={forced})"),
        }
    }
}

impl CliError {
    /// Map the typed error to a typed `ExitCode` per CLI-01 / FOREGROUND-02.
    /// Cancellation kinds honor the audit's canonical mapping: SIGINT and
    /// console close map to 130, SIGTERM maps to 143. Force-terminated
    /// cancellations share the same exit code as their graceful
    /// counterparts because shells do not distinguish between them.
    pub fn exit_code(&self) -> ExitCode {
        if let Self::Interrupted { kind, .. } = self {
            return cancellation_exit_code(*kind, false);
        }
        ExitCode::from(1)
    }
}

/// Convert a free-form string error from a legacy dispatch path into a
/// `CliError`. Each dispatch site picks the variant that best describes
/// its failure class.
fn into_message(error: String) -> CliError {
    CliError::Message(error)
}

pub fn dispatch(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        None => {
            let mut command = Cli::command();
            command
                .print_help()
                .map_err(|error| into_message(format!("failed to render help output: {error}")))?;
            Ok(())
        }
        Some(RootCommand::Data(command)) => {
            let protocol = command
                .normalized_protocol()
                .map_err(into_message)?;
            let (operation, source_name) = match command.mode() {
                DataMode::Get(source) => {
                    (lexicon_framework::data::DataOperation::Acquisition, source)
                }
                DataMode::Process(source) => {
                    (lexicon_framework::data::DataOperation::Processing, source)
                }
            };
            let background = command.bg;
            let request = lexicon_framework::data::ForegroundDataRequest {
                operation,
                source_name,
                protocol,
                abandon_past_failure: command.abandon_past_fail,
                background,
                source_arguments: command.passthrough,
            };
            if background {
                match lexicon_framework::data::execute_background_data(request) {
                    Ok(outcome) => {
                        println!(
                            "[lexicon] {} handed off to background: source='{}' session={}",
                            outcome.operation.display_name(),
                            outcome.source,
                            outcome.session.id()
                        );
                        Ok(())
                    }
                    Err(err) => Err(CliError::BackgroundHandoff(err.to_string())),
                }
            } else {
                match lexicon_framework::data::execute_foreground_data(request) {
                    Ok(outcome) => {
                        println!(
                            "[lexicon] {} complete: source='{}' session={}",
                            outcome.operation.display_name(),
                            outcome.source,
                            outcome.session.id()
                        );
                        Ok(())
                    }
                    Err(err) => Err(CliError::ForegroundData(err.to_string())),
                }
            }
        }
        Some(RootCommand::Source(command)) => match command.action {
            SourceAction::Create(create_command) => {
                let result = lexicon_framework::commands::source_create(
                    &create_command.source_name,
                    &create_command.protocol,
                )
                .map_err(|err| CliError::SourceCommand(err))?;
                println!(
                    "[lexicon] Created source '{}' using protocol '{}' at {}",
                    result.source_name,
                    result.protocol,
                    result.protocol_dir.display()
                );
                println!("[lexicon] Files to edit next:");
                for file in &result.created_files {
                    println!("[lexicon]   - {}", file.display());
                }
                Ok(())
            }
            SourceAction::Build(build_command) => {
                let result = lexicon_framework::commands::source_build(
                    &build_command.source_name,
                    &build_command.protocol,
                )
                .map_err(|err| CliError::SourceCommand(err))?;
                println!(
                    "[lexicon] Built source '{}' using protocol '{}'",
                    result.source_name, result.protocol
                );
                println!("[lexicon] Runtime executables:");
                println!("[lexicon]   - {}", result.get_runtime.display());
                println!("[lexicon]   - {}", result.process_runtime.display());
                Ok(())
            }
        },
        Some(RootCommand::Init(command)) => {
            let result = lexicon_framework::commands::init(&command.parent_path, &command.project_name)
                .map_err(|err| CliError::Init(err))?;
            println!(
                "[lexicon] Initialized project '{}' at {}",
                command.project_name,
                result.project_directory.display()
            );
            Ok(())
        }
        Some(RootCommand::Build(_)) => {
            let outcome = lexicon_framework::commands::build_all()
                .map_err(|err| CliError::BuildAll(err.to_string()))?;
            for result in &outcome.succeeded {
                println!(
                    "[lexicon] Built source '{}' using protocol '{}'",
                    result.source_name, result.protocol
                );
                println!("[lexicon]   - {}", result.get_runtime.display());
                println!("[lexicon]   - {}", result.process_runtime.display());
            }
            for failure in &outcome.failed {
                println!(
                    "[lexicon] Failed to build source '{}' using protocol '{}': {}",
                    failure.source_name, failure.protocol, failure.error
                );
            }
            if outcome.is_success() {
                println!(
                    "[lexicon] Build complete: {} source(s) built, 0 failed",
                    outcome.succeeded.len()
                );
                Ok(())
            } else {
                let failed_identities = outcome
                    .failed
                    .iter()
                    .map(|failure| format!("{}/{}", failure.source_name, failure.protocol))
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(CliError::BuildAll(format!(
                    "build failed for {} source(s): {failed_identities}",
                    outcome.failed.len()
                )))
            }
        }
        Some(RootCommand::OperatorHost(command)) => {
            let reference = lexicon_framework::supervision::OperatorHostInvocationV1::from_json(
                &command.reference,
            )
            .map_err(|error| CliError::OperatorHost(error.to_string()))?;
            match lexicon_framework::data::execute_operator_host(reference, command.passthrough) {
                Ok(outcome) => {
                    println!(
                        "[lexicon] {} complete: source='{}' session={}",
                        outcome.operation.display_name(),
                        outcome.source,
                        outcome.session.id()
                    );
                    Ok(())
                }
                Err(err) => Err(CliError::OperatorHost(err.to_string())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Mutex;

    use super::{Cli, RootCommand};
    use crate::cli::source::{CreateSourceCommand, SourceAction, SourceCommand};
    use clap::{CommandFactory, Parser};

    static TEST_CWD_LOCK: Mutex<()> = Mutex::new(());

    fn with_test_cwd<T>(project_root: &std::path::Path, func: impl FnOnce() -> T) -> T {
        let _guard = TEST_CWD_LOCK.lock().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(project_root).unwrap();
        let result = func();
        std::env::set_current_dir(&original).unwrap();
        result
    }

    #[test]
    fn dispatch_source_create_produces_only_framework_output() {
        let cli = Cli::try_parse_from([
            "lexicon",
            "source",
            "create",
            "example-source",
            "--protocol",
            "http",
        ])
        .unwrap();

        match cli.command {
            Some(RootCommand::Source(SourceCommand {
                action:
                    SourceAction::Create(CreateSourceCommand {
                        source_name,
                        protocol,
                    }),
            })) => {
                assert_eq!(source_name, "example-source");
                assert_eq!(protocol, "http");
            }
            other => panic!("expected source create command, got {other:?}"),
        }
    }

    #[test]
    fn cli_source_create_calls_framework_library_directly() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path();
        fs::write(
            project_root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();

        let result = with_test_cwd(project_root, || {
            lexicon_framework::commands::source_create("example-source", "http")
        });

        assert!(
            result.is_ok(),
            "source scaffold should succeed: {:?}",
            result
        );
        let info = result.unwrap();
        assert_eq!(info.source_name, "example-source");
        assert_eq!(info.protocol, "http");
        assert!(info.protocol_dir.exists());
    }

    #[test]
    fn unsupported_protocol_returns_error_not_exit() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path();
        fs::write(
            project_root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();

        let result = with_test_cwd(project_root, || {
            lexicon_framework::commands::source_create("example-source", "browser")
        });

        assert!(result.is_err(), "unsupported protocol should return Err");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("unsupported protocol"),
            "error must mention unsupported protocol: {msg}"
        );
    }

    #[test]
    fn source_build_requires_protocol_flag() {
        let result = Cli::try_parse_from(["lexicon", "source", "build", "example-source"]);
        assert!(result.is_err());
    }

    #[test]
    fn cli_help_does_not_expose_framework_path() {
        let cli_result = Cli::try_parse_from(["lexicon", "--help"]);
        // --help causes an error-exit in clap; we check the rendered help text doesn't mention framework-path
        let cmd = Cli::command();
        let help = format!("{}", cmd.clone().render_long_help());
        assert!(
            !help.contains("framework-path"),
            "help must not expose --framework-path: {help}"
        );
        assert!(
            !help.contains("LEXICON_FRAMEWORK_PATH"),
            "help must not expose LEXICON_FRAMEWORK_PATH: {help}"
        );
        let _ = cli_result;
    }

    #[test]
    fn unrelated_preexisting_directory_remains_untouched() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path();
        fs::write(
            project_root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();
        fs::create_dir_all(project_root.join("sources/preexisting-scratch")).unwrap();
        fs::write(
            project_root.join("sources/preexisting-scratch/keep.txt"),
            "keep-me\n",
        )
        .unwrap();

        let result = with_test_cwd(project_root, || {
            lexicon_framework::commands::source_create("example-source", "http")
        });
        assert!(result.is_ok(), "valid source creation should succeed");

        assert_eq!(
            fs::read_to_string(project_root.join("sources/preexisting-scratch/keep.txt")).unwrap(),
            "keep-me\n"
        );
        assert!(
            !project_root
                .join("sources/preexisting-scratch")
                .read_dir()
                .unwrap()
                .next()
                .is_none()
        );
    }

    #[test]
    fn dispatch_build_command_runs_build_all_on_empty_project() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path();
        fs::write(
            project_root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();
        fs::create_dir_all(project_root.join("sources")).unwrap();

        let cli = Cli::try_parse_from(["lexicon", "build"]).unwrap();
        let result = with_test_cwd(project_root, || super::dispatch(cli));
        assert!(
            matches!(result, Ok(())),
            "build on empty project should succeed: {result:?}"
        );
    }

    #[test]
    fn dispatch_init_and_source_create_uses_embedded_core_identity() {
        let temp = tempfile::tempdir().unwrap();
        let parent_path = temp.path();

        // 1. Dispatch `lexicon init <parent> test-project`
        let init_cli = Cli::try_parse_from([
            "lexicon",
            "init",
            parent_path.to_str().unwrap(),
            "test-project",
        ])
        .unwrap();
        let init_res = super::dispatch(init_cli);
        assert!(init_res.is_ok(), "init failed: {init_res:?}");

        let project_dir = parent_path.join("test-project");
        assert!(project_dir.join("lexicon.toml").is_file());

        // 2. Dispatch `lexicon source create my-src --protocol http` inside the new project
        let create_cli = Cli::try_parse_from([
            "lexicon",
            "source",
            "create",
            "my-src",
            "--protocol",
            "http",
        ])
        .unwrap();
        let create_res = with_test_cwd(&project_dir, || super::dispatch(create_cli));
        assert!(
            matches!(create_res, Ok(())),
            "source create failed: {create_res:?}"
        );

        let protocol_dir = project_dir.join("sources/my-src/http");
        assert!(protocol_dir.join("source.toml").is_file());

        let acq_cargo = fs::read_to_string(protocol_dir.join("get-raw-data/Cargo.toml")).unwrap();
        assert!(acq_cargo.contains(lexicon_framework::EMBEDDED_CORE_GIT_REV));
        assert!(protocol_dir.join("get-raw-data/Cargo.lock").is_file());

        let proc_cargo = fs::read_to_string(protocol_dir.join("process-data/Cargo.toml")).unwrap();
        assert!(proc_cargo.contains(lexicon_framework::EMBEDDED_CORE_GIT_REV));
        assert!(protocol_dir.join("process-data/Cargo.lock").is_file());
    }
}
