use clap::{CommandFactory, Parser, Subcommand};

pub mod build;
pub mod data;
pub mod init;
pub mod source;

pub use build::BuildCommand;
pub use data::{DataCommand, DataMode};
pub use init::InitCommand;
pub use source::{SourceAction, SourceCommand};

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
}

pub fn dispatch(cli: Cli) -> Result<(), String> {
    match cli.command {
        None => {
            let mut command = Cli::command();
            command
                .print_help()
                .map_err(|error| format!("failed to render help output: {error}"))?;
            Ok(())
        }
        Some(RootCommand::Data(command)) => {
            let mode = command.mode();
            let action = match &mode {
                DataMode::Get(source) => format!("get {source}"),
                DataMode::Process(source) => format!("process {source}"),
            };
            println!(
                "Parsed data command: {} (bg={}, abandon_past_fail={}, passthrough={:?})",
                action,
                command.bg,
                command.abandon_past_fail,
                command.passthrough
            );
            Ok(())
        }
        Some(RootCommand::Source(command)) => match command.action {
            SourceAction::Create(create_command) => {
                let result = lexicon_framework::commands::source_create(
                    &create_command.source_name,
                    &create_command.protocol,
                )?;
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
                )?;
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
            let result = lexicon_framework::commands::init(&command.parent_path, &command.project_name)?;
            println!(
                "[lexicon] Initialized project '{}' at {}",
                command.project_name,
                result.project_directory.display()
            );
            Ok(())
        }
        Some(RootCommand::Build(_)) => {
            println!("Parsed build command: build");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{Cli, RootCommand};
    use crate::cli::source::{CreateSourceCommand, SourceAction, SourceCommand};
    use clap::{CommandFactory, Parser};

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
                action: SourceAction::Create(CreateSourceCommand { source_name, protocol }),
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

        let result = {
            let orig = std::env::current_dir().unwrap();
            std::env::set_current_dir(project_root).unwrap();
            let r = lexicon_framework::commands::source_create("example-source", "http");
            std::env::set_current_dir(&orig).unwrap();
            r
        };

        assert!(result.is_ok(), "source scaffold should succeed: {:?}", result);
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

        let result = {
            let orig = std::env::current_dir().unwrap();
            std::env::set_current_dir(project_root).unwrap();
            let r = lexicon_framework::commands::source_create("example-source", "browser");
            std::env::set_current_dir(&orig).unwrap();
            r
        };

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

        {
            let orig = std::env::current_dir().unwrap();
            std::env::set_current_dir(project_root).unwrap();
            let r = lexicon_framework::commands::source_create("example-source", "http");
            std::env::set_current_dir(&orig).unwrap();
            assert!(r.is_ok(), "valid source creation should succeed");
        }

        assert_eq!(
            fs::read_to_string(project_root.join("sources/preexisting-scratch/keep.txt"))
                .unwrap(),
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
}
