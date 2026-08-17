use std::env;
use std::process::Command;

// Framework path relative to the installed CLI's own directory, resolved at
// compile time from lexicon-install.toml (see lexicon-cli/build.rs).
include!(concat!(env!("OUT_DIR"), "/lexicon_runtime_layout.rs"));

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    let cli_path = env::current_exe().unwrap_or_else(|err| {
        eprintln!("lexicon-cli: failed to determine current executable path: {err}");
        std::process::exit(1);
    });
    let cli_dir = cli_path
        .parent()
        .unwrap_or_else(|| panic!("lexicon-cli: {} has no parent directory", cli_path.display()));
    let framework_path = cli_dir.join(FRAMEWORK_FROM_CLI);

    let status = Command::new(&framework_path).args(&args).status().unwrap_or_else(|err| {
        eprintln!("lexicon-cli: failed to run lexicon-framework at {}: {err}", framework_path.display());
        std::process::exit(1);
    });

    std::process::exit(status.code().unwrap_or(1));
}