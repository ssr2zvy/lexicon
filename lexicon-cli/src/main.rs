mod cli;

use cli::{dispatch, Cli};
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    if let Err(err) = dispatch(cli) {
        eprintln!("[lexicon] ERROR: {err}");
        std::process::exit(1);
    }
}
