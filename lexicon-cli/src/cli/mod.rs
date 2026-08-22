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
        Some(RootCommand::Source(command)) => {
            match command.action {
                SourceAction::New(new_command) => {
                    let framework_path = framework_binary_path()?;
                    let status = Command::new(framework_path)
                        .args([
                            "source",
                            "new",
                            &new_command.source_name,
                            "--protocol",
                            &new_command.protocol,
                        ])
                        .status()
                        .map_err(|error| format!("failed to execute framework binary: {error}"))?;
                    if !status.success() {
                        return Err(format!(
                            "framework source scaffold step failed with exit status {}",
                            status
                        ));
                    }
                    println!(
                        "Invoked framework scaffold for source '{}' using protocol '{}'",
                        new_command.source_name, new_command.protocol
                    );
                    Ok(())
                }
            }
        }
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
