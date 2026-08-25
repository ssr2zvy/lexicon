mod cli;

use clap::Parser;
use cli::{Cli, dispatch};

fn main() {
    let cli = Cli::parse();
    if let Err(err) = dispatch(cli) {
        eprintln!("[lexicon] ERROR: {err}");
        std::process::exit(1);
    }
}
