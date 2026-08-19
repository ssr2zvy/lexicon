use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "build", about = "Rebuild discovered source implementations.")]
pub struct BuildCommand;

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, RootCommand};
    use clap::Parser;

    #[test]
    fn parses_build_command() {
        let cli = Cli::try_parse_from(["lexicon", "build"]).expect("lexicon build should parse");

        match cli.command {
            Some(RootCommand::Build(_)) => {}
            other => panic!("expected Build subcommand, got {other:?}"),
        }
    }
}
