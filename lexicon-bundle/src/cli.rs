use std::io::{self, Write};

use crate::install::{detect_state, do_install, do_uninstall, resolve_destinations};
use crate::model::{Destinations, InstallState};

pub fn dispatch(args: &[String]) -> i32 {
    let dest = resolve_destinations();
    let state = detect_state(&dest);

    match args.first().map(String::as_str) {
        None => default_flow(state, &dest),
        Some("install") => install_command(state, &dest),
        Some("uninstall") => uninstall_command(state, &dest),
        Some("--install") => install_flag(state, &dest),
        Some("--uninstall") => uninstall_flag(state, &dest),
        Some("update") => {
            print_bundle("Update is not implemented.");
            0
        }
        Some("repair") => {
            print_bundle("Repair is not implemented.");
            0
        }
        Some("--help") | Some("-h") => {
            print_help();
            0
        }
        Some(other) => {
            print_bundle_err(&format!("unknown argument \"{other}\""));
            print_help();
            2
        }
    }
}

fn default_flow(state: InstallState, dest: &Destinations) -> i32 {
    match state {
        InstallState::NotInstalled => {
            print_bundle("Installation status: Not installed.");
            loop {
                print_bundle("1. Install");
                print_bundle("2. Cancel");
                match read_line("Select an option: ").trim() {
                    "1" => return do_install(dest),
                    "2" => return 0,
                    _ => print_bundle_err("Invalid selection."),
                }
            }
        }
        other => show_maintenance_menu(other, dest),
    }
}

fn install_command(state: InstallState, dest: &Destinations) -> i32 {
    match state {
        InstallState::NotInstalled => {
            print_bundle("Installation status: Not installed.");
            loop {
                print_bundle("1. Install");
                print_bundle("2. Cancel");
                match read_line("Select an option: ").trim() {
                    "1" => return do_install(dest),
                    "2" => return 0,
                    _ => print_bundle_err("Invalid selection."),
                }
            }
        }
        other => show_maintenance_menu(other, dest),
    }
}

fn uninstall_command(state: InstallState, dest: &Destinations) -> i32 {
    match state {
        InstallState::NotInstalled => {
            print_bundle("Installation status: Not installed.");
            0
        }
        _ => run_uninstall_flow(dest),
    }
}

/// Non-interactive counterpart to `install`: installs directly with no
/// menu, and is a no-op if Lexicon is already installed.
fn install_flag(state: InstallState, dest: &Destinations) -> i32 {
    match state {
        InstallState::NotInstalled => do_install(dest),
        _ => {
            print_bundle("Installation status: Installed. Nothing to do.");
            0
        }
    }
}

/// Non-interactive counterpart to `uninstall`: uninstalls directly with no
/// menu or confirmation prompt, and is a no-op if not installed.
fn uninstall_flag(state: InstallState, dest: &Destinations) -> i32 {
    match state {
        InstallState::NotInstalled => {
            print_bundle("Installation status: Not installed. Nothing to do.");
            0
        }
        _ => do_uninstall(dest),
    }
}

fn show_maintenance_menu(state: InstallState, dest: &Destinations) -> i32 {
    loop {
        match state {
            InstallState::Damaged => print_bundle("Installation status: Damaged."),
            _ => print_bundle("Installation status: Installed."),
        }
        print_bundle("1. Uninstall");
        print_bundle("2. Other");
        print_bundle("3. Cancel");

        match read_line("Select an option: ").trim() {
            "1" => return run_uninstall_flow(dest),
            "2" => {
                print_bundle("Other is not implemented.");
                return 0;
            }
            "3" => return 0,
            _ => print_bundle_err("Invalid selection."),
        }
    }
}

fn run_uninstall_flow(dest: &Destinations) -> i32 {
    if prompt_default_no("Uninstall Lexicon?") {
        do_uninstall(dest)
    } else {
        print_bundle("Uninstallation cancelled.");
        0
    }
}

fn print_help() {
    print_bundle("Usage:");
    print_bundle("  lexicon-bundle");
    print_bundle("  lexicon-bundle install");
    print_bundle("  lexicon-bundle uninstall");
    print_bundle("  lexicon-bundle --install    (non-interactive; no-op if already installed)");
    print_bundle("  lexicon-bundle --uninstall  (non-interactive; no-op if not installed)");
    print_bundle("  lexicon-bundle update    (not implemented)");
    print_bundle("  lexicon-bundle repair    (not implemented)");
    print_bundle("  lexicon-bundle --help");
}

fn print_bundle(message: impl std::fmt::Display) {
    println!("[[LEXICON-BUNDLE]] {message}");
}

fn print_bundle_err(message: impl std::fmt::Display) {
    eprintln!("[[LEXICON-BUNDLE]] {message}");
}

fn read_line(prompt: &str) -> String {
    print!("[[LEXICON-BUNDLE]] {prompt}");
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    input
}

#[allow(dead_code)]
fn prompt_default_yes(question: &str) -> bool {
    let answer = read_line(&format!("{question} [Y/n] ")).trim().to_lowercase();
    answer.is_empty() || answer == "y" || answer == "yes"
}

fn prompt_default_no(question: &str) -> bool {
    let answer = read_line(&format!("{question} [y/N] ")).trim().to_lowercase();
    answer == "y" || answer == "yes"
}
