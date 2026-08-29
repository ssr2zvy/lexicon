use std::env;

mod cli;
mod envpath;
mod install;
mod model;
mod pathutil;

// Protocol-agnostic part of cargo-bundler-v0.1.0: MZA_BUNDLE_INPUTS is
// captured at compile time by build.rs and embedded here via `include!`.
include!(concat!(env!("OUT_DIR"), "/mza_bundle_inputs.rs"));
// Lexicon-specific install layout, resolved at compile time from
// lexicon-install.toml (see lexicon-bundle/build.rs).
include!(concat!(env!("OUT_DIR"), "/lexicon_install_layout.rs"));

const MARKER: &str = "# Added by the lexicon installer";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    std::process::exit(cli::dispatch(&args));
}
