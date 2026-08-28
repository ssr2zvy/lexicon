use std::ffi::OsString;

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

    #[arg(
        long,
        value_name = "PROTOCOL",
        required = true,
        help = "Protocol for the source operation. Only http is supported right now."
    )]
    pub protocol: String,

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
    pub passthrough: Vec<OsString>,
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

    pub fn normalized_protocol(&self) -> Result<String, String> {
        let value = self.protocol.trim();
        if value.eq_ignore_ascii_case("http") {
            Ok("http".to_owned())
        } else {
            Err(format!(
                "unsupported protocol '{}'; only 'http' is currently supported",
                self.protocol
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use crate::cli::{Cli, RootCommand};
    use clap::Parser;

    #[test]
    fn parses_get_command_and_passthrough() {
        let cli = Cli::try_parse_from([
            "lexicon",
            "data",
            "--get",
            "example-source",
            "--protocol",
            "http",
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
                assert_eq!(command.protocol, "http");
                assert_eq!(command.normalized_protocol().unwrap(), "http");
                assert!(command.bg);
                assert_eq!(command.passthrough, vec![
                    OsString::from("--from"),
                    OsString::from("2024-01-01"),
                ]);
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
            "--protocol",
            "http",
            "--abandon-past-fail",
        ])
        .expect("lexicon data --process should parse");

        match cli.command {
            Some(RootCommand::Data(command)) => {
                assert_eq!(command.process.as_deref(), Some("example-source"));
                assert_eq!(command.protocol, "http");
                assert_eq!(command.normalized_protocol().unwrap(), "http");
                assert!(!command.bg);
                assert!(command.abandon_past_fail);
            }
            other => panic!("expected Data subcommand, got {other:?}"),
        }
    }

    #[test]
    fn rejects_data_command_when_protocol_is_missing() {
        let result = Cli::try_parse_from(["lexicon", "data", "--get", "example-source"]);
        assert!(result.is_err(), "data command requires --protocol");
    }

    #[test]
    fn rejects_data_command_when_protocol_value_is_missing() {
        let result = Cli::try_parse_from([
            "lexicon",
            "data",
            "--get",
            "example-source",
            "--protocol",
        ]);
        assert!(result.is_err(), "--protocol requires a value");
    }

    #[test]
    fn parses_operator_host_command_with_reference_and_passthrough() {
        let reference_json = r#"{"schema_version":1,"source_name":"example-source","protocol":"http","operation":"acquisition","session_id":"session-abc"}"#;
        let cli = Cli::try_parse_from([
            "lexicon",
            "__operator-host",
            reference_json,
            "--",
            "--from",
            "2024-01-01",
        ])
        .expect("lexicon __operator-host should parse");

        match cli.command {
            Some(RootCommand::OperatorHost(command)) => {
                assert_eq!(command.reference, reference_json);
                assert_eq!(
                    command.passthrough,
                    vec![OsString::from("--from"), OsString::from("2024-01-01")]
                );
            }
            other => panic!("expected OperatorHost subcommand, got {other:?}"),
        }
    }

    #[test]
    fn operator_host_command_is_hidden_from_help_output() {
        use clap::CommandFactory;

        let mut command = Cli::command();
        let help = command.render_help().to_string();
        assert!(
            !help.contains("__operator-host"),
            "the reserved internal __operator-host entrypoint must not appear in --help output:\n{help}"
        );
    }
}
