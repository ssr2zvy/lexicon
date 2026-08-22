use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(name = "source", about = "Create a new source definition and project scaffold.")]
pub struct SourceCommand {
    #[command(subcommand)]
    pub action: SourceAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SourceAction {
    New(NewSourceCommand),
}

#[derive(Parser, Debug, Clone)]
pub struct NewSourceCommand {
    #[arg(value_name = "SOURCE_NAME")]
    pub source_name: String,

    #[arg(
        long,
        value_name = "PROTOCOL",
        required = true,
        help = "Acquisition protocol for the new source. Only http is supported right now."
    )]
    pub protocol: String,
}

impl NewSourceCommand {
    pub fn normalized_protocol(&self) -> Result<String, String> {
        let value = self.protocol.trim();
        if value.eq_ignore_ascii_case("http") {
            Ok("http".to_owned())
        } else {
            Err(format!(
                "unsupported protocol '{}'; only 'http' is currently supported for source creation",
                self.protocol
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NewSourceCommand, SourceAction, SourceCommand};
    use crate::cli::{Cli, RootCommand};
    use clap::Parser;

    #[test]
    fn rejects_new_source_command_when_protocol_is_missing() {
        let result = Cli::try_parse_from(["lexicon", "source", "new", "example-source"]);
        assert!(result.is_err(), "--protocol must be required and cannot be omitted");
    }

    #[test]
    fn rejects_new_source_command_when_protocol_value_is_missing() {
        let result = Cli::try_parse_from([
            "lexicon",
            "source",
            "new",
            "example-source",
            "--protocol",
        ]);
        assert!(result.is_err(), "--protocol requires a value and must fail without one");
    }

    #[test]
    fn parses_new_source_command_with_protocol_flag() {
        let cli = Cli::try_parse_from([
            "lexicon",
            "source",
            "new",
            "example-source",
            "--protocol",
            "http",
        ])
        .expect("lexicon source new --protocol http should parse");

        match cli.command {
            Some(RootCommand::Source(SourceCommand {
                action: SourceAction::New(command),
            })) => {
                assert_eq!(command.source_name, "example-source");
                assert_eq!(command.protocol, "http");
                assert_eq!(command.normalized_protocol().unwrap(), "http");
            }
            other => panic!("expected Source::New subcommand, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unsupported_protocol_value() {
        let command = NewSourceCommand {
            source_name: "example-source".to_owned(),
            protocol: "browser".to_owned(),
        };

        assert!(command.normalized_protocol().is_err());
    }
}
