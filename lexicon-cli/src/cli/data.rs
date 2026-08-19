use clap::{ArgGroup, Parser};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "data",
    about = "Fetch or process raw data for a source.",
    group(
        ArgGroup::new("data_action")
            .required(true)
            .args(["get", "process"])
    )
)]
pub struct DataCommand {
    #[arg(
        long,
        value_name = "SOURCE_NAME",
        help = "Fetch raw data for SOURCE_NAME.",
        group = "data_action"
    )]
    pub get: Option<String>,

    #[arg(
        long,
        value_name = "SOURCE_NAME",
        help = "Process data for SOURCE_NAME.",
        group = "data_action"
    )]
    pub process: Option<String>,

    #[arg(long, help = "Run the operation in the background.")]
    pub bg: bool,

    #[arg(
        long,
        help = "Abandon the previous failed session before running this command."
    )]
    pub abandon_past_fail: bool,

    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        help = "Arguments forwarded after `--` to the selected source implementation."
    )]
    pub passthrough: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataMode {
    Get(String),
    Process(String),
}

impl DataCommand {
    pub fn mode(&self) -> DataMode {
        if let Some(source_name) = self.get.as_deref() {
            DataMode::Get(source_name.to_owned())
        } else if let Some(source_name) = self.process.as_deref() {
            DataMode::Process(source_name.to_owned())
        } else {
            unreachable!("data action is required by clap validation")
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, RootCommand};
    use clap::Parser;

    #[test]
    fn parses_get_command_and_passthrough() {
        let cli = Cli::try_parse_from([
            "lexicon",
            "data",
            "--get",
            "example-source",
            "--bg",
            "--",
            "--from",
            "2024-01-01",
        ])
        .expect("lexicon data --get should parse");

        match cli.command {
            Some(RootCommand::Data(command)) => {
                assert_eq!(command.get.as_deref(), Some("example-source"));
                assert_eq!(command.process, None);
                assert!(command.bg);
                assert_eq!(command.passthrough, vec!["--from", "2024-01-01"]);
            }
            other => panic!("expected Data subcommand, got {other:?}"),
        }
    }

    #[test]
    fn parses_process_command_with_abandon_flag() {
        let cli = Cli::try_parse_from([
            "lexicon",
            "data",
            "--process",
            "example-source",
            "--abandon-past-fail",
        ])
        .expect("lexicon data --process should parse");

        match cli.command {
            Some(RootCommand::Data(command)) => {
                assert_eq!(command.process.as_deref(), Some("example-source"));
                assert!(!command.bg);
                assert!(command.abandon_past_fail);
            }
            other => panic!("expected Data subcommand, got {other:?}"),
        }
    }
}
