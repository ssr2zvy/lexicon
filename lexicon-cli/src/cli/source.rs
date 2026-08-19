use clap::{ArgGroup, Parser};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "source",
    about = "Create or register a source implementation.",
    group(
        ArgGroup::new("source_action")
            .required(true)
            .args(["draft", "add"])
    )
)]
pub struct SourceCommand {
    pub source_name: String,

    #[arg(long, help = "Create the required source structure and scaffolding.", group = "source_action")]
    pub draft: bool,

    #[arg(long, help = "Locate, compile, and verify the source implementation.", group = "source_action")]
    pub add: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceAction {
    Draft(String),
    Add(String),
}

impl SourceCommand {
    pub fn action(&self) -> SourceAction {
        if self.draft {
            SourceAction::Draft(self.source_name.clone())
        } else if self.add {
            SourceAction::Add(self.source_name.clone())
        } else {
            unreachable!("source action is required by clap validation")
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, RootCommand};
    use clap::Parser;

    #[test]
    fn parses_draft_source_command() {
        let cli = Cli::try_parse_from([
            "lexicon",
            "source",
            "example-source",
            "--draft",
        ])
        .expect("lexicon source --draft should parse");

        match cli.command {
            Some(RootCommand::Source(command)) => {
                assert_eq!(command.source_name, "example-source");
                assert!(command.draft);
                assert!(!command.add);
            }
            other => panic!("expected Source subcommand, got {other:?}"),
        }
    }

    #[test]
    fn parses_add_source_command() {
        let cli = Cli::try_parse_from([
            "lexicon",
            "source",
            "example-source",
            "--add",
        ])
        .expect("lexicon source --add should parse");

        match cli.command {
            Some(RootCommand::Source(command)) => {
                assert_eq!(command.source_name, "example-source");
                assert!(!command.draft);
                assert!(command.add);
            }
            other => panic!("expected Source subcommand, got {other:?}"),
        }
    }
}
