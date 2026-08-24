use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(name = "source", about = "Create or build source definitions in the current Lexicon project.")]
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
    pub source_name: String,

    #[arg(
        long,
        default_value = "http",
        help = "Acquisition protocol for the source. Only http is supported right now."
    )]
    pub protocol: String,
}

#[derive(Parser, Debug, Clone)]
pub struct BuildSourceCommand {
    #[arg(value_name = "SOURCE_NAME")]
    pub source_name: String,

    #[arg(
        long,
        value_name = "PROTOCOL",
        required = true,
        help = "Protocol implementation to build. Only http is supported right now."
    )]
    pub protocol: String,
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

impl BuildSourceCommand {
    pub fn normalized_protocol(&self) -> Result<String, String> {
        let value = self.protocol.trim();
        if value.eq_ignore_ascii_case("http") {
            Ok("http".to_owned())
        } else {
            Err(format!(
                "unsupported protocol '{}'; only 'http' is currently supported for source builds",
                self.protocol
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceAction, SourceCommand};
    use crate::cli::{Cli, RootCommand};
    use clap::Parser;

    #[test]
    fn parses_create_source_command() {
        let cli = Cli::try_parse_from(["lexicon", "source", "create", "example-source"])
            .expect("lexicon source create should parse");

        match cli.command {
            Some(RootCommand::Source(SourceCommand {
                action: SourceAction::Create(command),
            })) => {
                assert_eq!(command.source_name, "example-source");
                assert_eq!(command.protocol, "http");
            }
            other => panic!("expected Source::Create subcommand, got {other:?}"),
        }
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
    fn parses_build_source_command_with_protocol_flag() {
        let cli = Cli::try_parse_from([
            "lexicon",
            "source",
            "build",
            "example-source",
            "--protocol",
            "http",
        ])
        .expect("lexicon source build --protocol http should parse");

        match cli.command {
            Some(RootCommand::Source(SourceCommand {
                action: SourceAction::Build(command),
            })) => {
                assert_eq!(command.source_name, "example-source");
                assert_eq!(command.protocol, "http");
                assert_eq!(command.normalized_protocol().unwrap(), "http");
            }
            other => panic!("expected Source::Build subcommand, got {other:?}"),
        }
    }

    #[test]
    fn rejects_source_add_alias() {
        assert!(Cli::try_parse_from(["lexicon", "source", "add", "example-source"]).is_err());
    }
}
