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
    #[arg(
        global = true,
        long = "framework-path",
        value_name = "PATH",
        help = "Path to the installed lexicon-framework binary; this value is remembered for future CLI invocations."
    )]
    pub framework_path: Option<std::path::PathBuf>,
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
    let framework_path_override = cli.framework_path.clone();

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
                let framework_path = framework_binary_path(framework_path_override.as_deref())?;
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
                let framework_path = framework_binary_path(framework_path_override.as_deref())?;
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

fn framework_state_path() -> Result<std::path::PathBuf, String> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| "HOME or USERPROFILE is not set".to_string())?;
    let base = std::path::PathBuf::from(home).join(".local").join("share").join("lexicon");
    Ok(base.join("framework-path"))
}

fn read_framework_path() -> Result<Option<std::path::PathBuf>, String> {
    let state_path = framework_state_path()?;
    if !state_path.is_file() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&state_path)
        .map_err(|error| format!("failed to read framework state {}: {error}", state_path.display()))?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(std::path::PathBuf::from(trimmed)))
}

fn write_framework_path(path: &std::path::Path) -> Result<(), String> {
    let state_path = framework_state_path()?;
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    std::fs::write(&state_path, path.to_string_lossy().as_ref())
        .map_err(|error| format!("failed to write {}: {error}", state_path.display()))?;
    Ok(())
}

fn framework_binary_path(explicit_path: Option<&std::path::Path>) -> Result<String, String> {
    if let Some(path) = explicit_path {
        if path.is_file() {
            write_framework_path(path)?;
            return Ok(path.to_string_lossy().into_owned());
        }
        return Err(format!(
            "framework path '{}' was provided but the framework binary does not exist",
            path.display()
        ));
    }

    if let Some(path) = read_framework_path()? {
        if path.is_file() {
            return Ok(path.to_string_lossy().into_owned());
        }
        let _ = std::fs::remove_file(framework_state_path()?);
    }

    if let Ok(current_exe) = std::env::current_exe() {
        let candidate = current_exe
            .parent()
            .map(|dir| dir.join(crate::FRAMEWORK_FROM_CLI))
            .filter(|path| path.is_file());
        if let Some(path) = candidate {
            write_framework_path(&path)?;
            return Ok(path.to_string_lossy().into_owned());
        }
    }

    Err("no framework binary path was provided, no remembered CLI framework path exists, and no installed framework binary was found next to the lexicon CLI; pass --framework-path <path> to the CLI".to_string())
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
    fn cli_requires_explicit_or_local_install_framework_path() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path();
        let cli_home = tempfile::tempdir().unwrap();
        fs::write(project_root.join("lexicon.toml"), "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n").unwrap();

        let cli_bin = resolve_cli_binary();
        let output = std::process::Command::new(&cli_bin)
            .current_dir(project_root)
            .env("HOME", cli_home.path())
            .env("USERPROFILE", cli_home.path())
            .args(["source", "create", "example-source", "--protocol", "http"])
            .output()
            .unwrap();

        let combined_bytes = [output.stdout.as_slice(), output.stderr.as_slice()].concat();
        let combined = String::from_utf8_lossy(&combined_bytes);

        assert!(!output.status.success(), "CLI should fail when no framework path is available");
        assert!(combined.contains("framework binary") || combined.contains("--framework-path"), "missing framework error should mention the required path: {combined}");
        assert!(!combined.contains("cargo build"), "CLI should not build the framework from the workspace when no real install path exists: {combined}");
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
            .env("HOME", temp.path())
            .env("USERPROFILE", temp.path())
            .args(["--framework-path", framework_bin.to_str().unwrap(), "source", "create", "example-source", "--protocol", "http"])
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
            .env("HOME", temp.path())
            .env("USERPROFILE", temp.path())
            .args(["--framework-path", framework_bin.to_str().unwrap(), "source", "create", "unsupported-source", "--protocol", "browser"])
            .output()
            .unwrap();

        let build_output = std::process::Command::new(&cli_bin)
            .current_dir(project_root)
            .env("HOME", temp.path())
            .env("USERPROFILE", temp.path())
            .args(["--framework-path", framework_bin.to_str().unwrap(), "source", "build", "unsupported-source", "--protocol", "browser"])
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
