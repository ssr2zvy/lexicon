mod cli;

include!(concat!(env!("OUT_DIR"), "/lexicon_runtime_layout.rs"));

use cli::{dispatch, Cli};
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    if let Err(err) = dispatch(cli) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
