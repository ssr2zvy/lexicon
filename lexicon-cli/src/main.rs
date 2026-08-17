use std::env;
use std::process::Command;
use clap::Parser;

#[derive(Parser)]
#[command(name = "lexicon", version, about = "Lexicon: make data", )]

struct Cli {}
fn main() {
    let _cli = Cli::parse();
}