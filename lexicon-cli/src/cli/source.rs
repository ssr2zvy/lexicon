use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(name = "source", about = "Create or build a source definition and project scaffold.")]
pub struct SourceCommand {
    #[command(subcommand)]
    pub action: SourceAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SourceAction {
    Create(CreateSourceCommand),
    Build(BuildSourceCommand),
}

#[derive(Parser, Debug, Clone)]
pub struct CreateSourceCommand {
    #[arg(value_name = "SOURCE_NAME")]
    pub source_name: String,

    #[arg(
        long,
        value_name = "PROTOCOL",
        required = true,
        help = "Acquisition protocol for the source. Only http is supported right now."
    )]
    pub protocol: String,
}

#[derive(Parser, Debug, Clone)]
pub struct BuildSourceCommand {
    #[arg(value_name = "SOURCE_NAME")]
    pub source_name: String,
}

impl CreateSourceCommand {
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
    use super::{BuildSourceCommand, CreateSourceCommand, SourceAction, SourceCommand};
    use crate::cli::{Cli, RootCommand};
    use clap::Parser;

    #[test]
    fn rejects_create_source_command_when_protocol_is_missing() {
        let result = Cli::try_parse_from(["lexicon", "source", "create", "example-source"]);
        assert!(result.is_err(), "--protocol must be required and cannot be omitted");
    }

    #[test]
    fn rejects_create_source_command_when_protocol_value_is_missing() {
        let result = Cli::try_parse_from([
            "lexicon",
            "source",
            "create",
            "example-source",
            "--protocol",
        ]);
        assert!(result.is_err(), "--protocol requires a value and must fail without one");
    }

    #[test]
    fn parses_create_source_command_with_protocol_flag() {
        let cli = Cli::try_parse_from([
            "lexicon",
            "source",
            "create",
            "example-source",
            "--protocol",
            "http",
        ])
        .expect("lexicon source create --protocol http should parse");

        match cli.command {
            Some(RootCommand::Source(SourceCommand {
                action: SourceAction::Create(command),
            })) => {
                assert_eq!(command.source_name, "example-source");
                assert_eq!(command.protocol, "http");
                assert_eq!(command.normalized_protocol().unwrap(), "http");
            }
            other => panic!("expected Source::Create subcommand, got {other:?}"),
        }
    }

    #[test]
    fn parses_build_source_command_with_source_name() {
        let cli = Cli::try_parse_from(["lexicon", "source", "build", "example-source"])
            .expect("lexicon source build example-source should parse");

        match cli.command {
            Some(RootCommand::Source(SourceCommand {
                action: SourceAction::Build(command),
            })) => {
                assert_eq!(command.source_name, "example-source");
            }
            other => panic!("expected Source::Build subcommand, got {other:?}"),
        }
    }

    #[test]
    fn rejects_old_source_new_command() {
        let result = Cli::try_parse_from(["lexicon", "source", "new", "example-source", "--protocol", "http"]);
        assert!(result.is_err(), "source new must be rejected");
    }

    #[test]
    fn rejects_old_source_add_command() {
        let result = Cli::try_parse_from(["lexicon", "source", "add", "example-source"]);
        assert!(result.is_err(), "source add must be rejected");
    }

    #[test]
    fn rejects_build_command_without_source_name() {
        let result = Cli::try_parse_from(["lexicon", "source", "build"]);
        assert!(result.is_err(), "source build requires a source name");
    }

    #[test]
    fn rejects_unsupported_protocol_value() {
        let command = CreateSourceCommand {
            source_name: "example-source".to_owned(),
            protocol: "browser".to_owned(),
        };

        assert!(command.normalized_protocol().is_err());
    }

    #[test]
    fn build_command_struct_has_source_name() {
        let command = BuildSourceCommand {
            source_name: "example-source".to_owned(),
        };

        assert_eq!(command.source_name, "example-source");
    }
}
