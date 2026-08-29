// CLI-01: the public Lexicon executable is named `lexicon` (per the
// `[[bin]]` table in Cargo.toml) and uses typed exit codes.

use std::process::ExitCode;

use clap::Parser;

pub use lexicon_cli_lib::{Cli, CliError, dispatch};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[lexicon] ERROR: {error}");
            error.exit_code()
        }
    }
}
