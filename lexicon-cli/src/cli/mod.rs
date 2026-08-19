use clap::{CommandFactory, Parser, Subcommand};

pub mod build;
pub mod data;
pub mod source;

pub use build::BuildCommand;
pub use data::{DataCommand, DataMode};
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
            let action = match command.action() {
                SourceAction::Draft(source) => format!("draft {source}"),
                SourceAction::Add(source) => format!("add {source}"),
            };
            println!("Parsed source command: {action}");
            Ok(())
        }
        Some(RootCommand::Build(_)) => {
            println!("Parsed build command: build");
            Ok(())
        }
    }
}
