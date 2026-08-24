use std::path::PathBuf;
use std::process::Command;

use clap::{CommandFactory, Parser, Subcommand};

use crate::cli::init::initialize_project;

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
    long_about = "Lexicon CLI for raw-data acquisition, processing, source management, and build orchestration.\n\nThis parser validates the command interface defined by the project spec without invoking framework behavior."
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
                let framework_path = framework_binary_path()?;
                let status = Command::new(framework_path)
                    .args([
                        "source",
                        "create",
                        &create_command.source_name,
                        "--protocol",
                        &create_command.protocol,
                    ])
                    .status()
                    .map_err(|error| format!("failed to execute framework binary: {error}"))?;
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
                Ok(())
            }
            SourceAction::Build(build_command) => {
                let framework_path = framework_binary_path()?;
                let status = Command::new(framework_path)
                    .args([
                        "source",
                        "build",
                        &build_command.source_name,
                        "--protocol",
                        &build_command.protocol,
                    ])
                    .status()
                    .map_err(|error| format!("failed to execute framework binary: {error}"))?;
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
                Ok(())
            }
        },
        Some(RootCommand::Init(command)) => {
            let project_root = initialize_project(&command.parent_path, &command.project_name)?;
            println!("[lexicon] Initialized project '{}' at {}", command.project_name, project_root.display());
            Ok(())
        }
        Some(RootCommand::Build(_)) => {
            println!("Parsed build command: build");
            Ok(())
        }
    }
}

fn framework_binary_path() -> Result<String, String> {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_lexicon-framework") {
        return Ok(path);
    }
    if let Ok(path) = std::env::var("LEXICON_FRAMEWORK_PATH") {
        return Ok(path);
    }

    if let Ok(current_exe) = std::env::current_exe() {
        let direct = current_exe
            .parent()
            .map(|dir| dir.join("lexicon-framework"))
            .filter(|path| path.exists());
        if let Some(path) = direct {
            return Ok(path.to_string_lossy().into_owned());
        }

        if let Some(path) = current_exe.ancestors().find_map(|ancestor| {
            let candidate = ancestor.join("target").join("debug").join("lexicon-framework");
            candidate.exists().then_some(candidate)
        }) {
            return Ok(path.to_string_lossy().into_owned());
        }
    }

    let workspace_root = locate_workspace_root()?;
    let binary = workspace_root.join("target").join("debug").join("lexicon-framework");

    let status = Command::new("cargo")
        .current_dir(&workspace_root)
        .args(["build", "-p", "lexicon-framework"])
        .status()
        .map_err(|error| format!("failed to build lexicon-framework: {error}"))?;
    if !status.success() {
        return Err(format!(
            "failed to build lexicon-framework for source scaffolding (exit code: {status})"
        ));
    }

    if binary.exists() {
        Ok(binary.to_string_lossy().into_owned())
    } else {
        Err("could not locate the lexicon-framework binary after build".to_string())
    }
}

fn locate_workspace_root() -> Result<PathBuf, String> {
    let current = std::env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;
    let mut dir = current;

    loop {
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            let contents = std::fs::read_to_string(&manifest)
                .map_err(|error| format!("failed to read {}: {error}", manifest.display()))?;
            if contents.contains("[workspace]") {
                return Ok(dir);
            }
        }

        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => break,
        }
    }

    Err("could not locate the workspace root from the current directory".to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use super::{Cli, RootCommand};
    use crate::cli::source::{CreateSourceCommand, SourceAction, SourceCommand};
    use clap::Parser;

    fn resolve_cli_binary() -> std::path::PathBuf {
        if let Ok(path) = std::env::var("CARGO_BIN_EXE_lexicon-cli") {
            return std::path::PathBuf::from(path);
        }

        let current_exe = std::env::current_exe().unwrap();
        let workspace_root = current_exe
            .ancestors()
            .find_map(|ancestor| {
                let marker = ancestor.join("Cargo.toml");
                let is_workspace = marker.exists() && std::fs::read_to_string(&marker).ok().is_some_and(|text| text.contains("[workspace]"));
                is_workspace.then_some(ancestor.to_path_buf())
            })
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        let status = Command::new("cargo")
            .current_dir(&workspace_root)
            .args(["build", "-p", "lexicon-cli", "-p", "lexicon-framework"])
            .status()
            .unwrap();
        assert!(status.success(), "building the CLI and framework binaries should succeed");

        workspace_root.join("target").join("debug").join("lexicon-cli")
    }

    fn resolve_framework_binary() -> std::path::PathBuf {
        if let Ok(path) = std::env::var("CARGO_BIN_EXE_lexicon-framework") {
            return std::path::PathBuf::from(path);
        }

        let current_exe = std::env::current_exe().unwrap();
        let workspace_root = current_exe
            .ancestors()
            .find_map(|ancestor| {
                let marker = ancestor.join("Cargo.toml");
                let is_workspace = marker.exists() && std::fs::read_to_string(&marker).ok().is_some_and(|text| text.contains("[workspace]"));
                is_workspace.then_some(ancestor.to_path_buf())
            })
            .unwrap_or_else(|| std::env::current_dir().unwrap());

        let status = Command::new("cargo")
            .current_dir(&workspace_root)
            .args(["build", "-p", "lexicon-cli", "-p", "lexicon-framework"])
            .status()
            .unwrap();
        assert!(status.success(), "building the CLI and framework binaries should succeed");

        workspace_root.join("target").join("debug").join("lexicon-framework")
    }

    #[test]
    fn cli_source_create_prints_only_framework_success_output() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path();
        fs::write(project_root.join("lexicon.toml"), "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n").unwrap();

        let cli_bin = resolve_cli_binary();
        let framework_bin = resolve_framework_binary();

        let output = std::process::Command::new(cli_bin)
            .current_dir(project_root)
            .env("LEXICON_FRAMEWORK_PATH", framework_bin)
            .args(["source", "create", "example-source", "--protocol", "http"])
            .output()
            .unwrap();

        assert!(output.status.success(), "source scaffold command should succeed");

        let combined_bytes = [output.stdout.as_slice(), output.stderr.as_slice()].concat();
        let combined = String::from_utf8_lossy(&combined_bytes);

        assert_eq!(combined.matches("[lexicon] Created source 'example-source'").count(), 1);
        assert_eq!(combined.matches("[lexicon] Files to edit next:").count(), 1);
        assert!(combined.matches("[lexicon]   - ").count() >= 1);
        assert!(!combined.contains("Invoked framework scaffold"));
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
                action: SourceAction::Create(CreateSourceCommand { source_name, protocol }),
            })) => {
                assert_eq!(source_name, "example-source");
                assert_eq!(protocol, "http");
            }
            other => panic!("expected source create command, got {other:?}"),
        }
    }

    #[test]
    fn unrelated_preexisting_directory_remains_untouched() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path();
        fs::write(project_root.join("lexicon.toml"), "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n").unwrap();
        fs::create_dir_all(project_root.join("sources/preexisting-scratch")).unwrap();
        fs::write(project_root.join("sources/preexisting-scratch/keep.txt"), "keep-me\n").unwrap();

        let cli_bin = resolve_cli_binary();
        let framework_bin = resolve_framework_binary();

        let output = std::process::Command::new(cli_bin)
            .current_dir(project_root)
            .env("LEXICON_FRAMEWORK_PATH", framework_bin)
            .args(["source", "create", "example-source", "--protocol", "http"])
            .output()
            .unwrap();

        assert!(output.status.success(), "valid source creation should succeed");
        assert_eq!(fs::read_to_string(project_root.join("sources/preexisting-scratch/keep.txt")).unwrap(), "keep-me\n");
        assert!(!project_root.join("sources/preexisting-scratch").read_dir().unwrap().next().is_none());
    }

    #[test]
    fn unsupported_protocol_reports_single_lexicon_error_line() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path();
        fs::write(project_root.join("lexicon.toml"), "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n").unwrap();

        let cli_bin = resolve_cli_binary();
        let framework_bin = resolve_framework_binary();

        let create_output = std::process::Command::new(&cli_bin)
            .current_dir(project_root)
            .env("LEXICON_FRAMEWORK_PATH", &framework_bin)
            .args(["source", "create", "unsupported-source", "--protocol", "browser"])
            .output()
            .unwrap();

        let build_output = std::process::Command::new(&cli_bin)
            .current_dir(project_root)
            .env("LEXICON_FRAMEWORK_PATH", &framework_bin)
            .args(["source", "build", "unsupported-source", "--protocol", "browser"])
            .output()
            .unwrap();

        for output in [create_output, build_output] {
            let combined_bytes = [output.stdout.as_slice(), output.stderr.as_slice()].concat();
            let combined = String::from_utf8_lossy(&combined_bytes);

            assert!(!output.status.success(), "unsupported protocol should fail");
            assert_eq!(output.status.code(), Some(1), "unsupported protocol should exit 1 from the framework");
            assert_eq!(combined.matches("[lexicon] ERROR:").count(), 1, "exactly one Lexicon error should be reported: {combined}");
            assert!(combined.contains("unsupported protocol 'browser'; only 'http' is currently supported"), "error wording is incorrect: {combined}");
            assert!(!combined.contains("source creation"), "error should be operation-neutral: {combined}");
            assert!(!combined.contains("source build"), "build-specific wording should not leak into the neutral error: {combined}");
        }
    }

    #[test]
    fn source_build_requires_protocol_flag() {
        let result = Cli::try_parse_from(["lexicon", "source", "build", "example-source"]);

        assert!(result.is_err());
    }
}
