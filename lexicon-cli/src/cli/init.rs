use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "init", about = "Create a new Lexicon project root.")]
pub struct InitCommand {
    pub project_name: String,
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, RootCommand};
    use clap::Parser;

    #[test]
    fn parses_init_command() {
        let cli = Cli::try_parse_from(["lexicon", "init", "my-data-project"])
            .expect("lexicon init should parse");

        match cli.command {
            Some(RootCommand::Init(command)) => {
                assert_eq!(command.project_name, "my-data-project");
            }
            other => panic!("expected Init subcommand, got {other:?}"),
        }
    }
}
