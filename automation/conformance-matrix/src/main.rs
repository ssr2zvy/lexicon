// `lexicon-conformance-matrix` binary entrypoint.
//
// The repository-wide audit (current.md §5, MATRIX-01) requires this binary
// to expose a single `check <conformance-toml>` subcommand that loads the
// matrix, examines each row for the structural rules listed in `lib.rs`,
// and (when connected to a real `cargo test --workspace -- --list` result)
// verifies each named test exists.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::ExitCode;

use lexicon_conformance_matrix::{
    ConformanceFile, MatrixError, check, flatten_test_index, parse_cargo_test_list,
};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(subcommand) = args.next() else {
        eprintln!("usage: lexicon-conformance-matrix check <conformance-toml>");
        return ExitCode::from(2);
    };

    match subcommand.as_str() {
        "check" => {
            let Some(path) = args.next() else {
                eprintln!("usage: lexicon-conformance-matrix check <conformance-toml>");
                return ExitCode::from(2);
            };
            run_check(PathBuf::from(path))
        }
        "--help" | "-h" | "help" => {
            println!("lexicon-conformance-matrix check <conformance-toml>");
            println!("Loads <conformance-toml> and validates rows against the");
            println!("rules enumerated in MATRIX-01. Rejects duplicate ids,");
            println!("empty implementation lists, unresolved tests, empty");
            println!("platforms lists, and conformant rows without");
            println!("durable_evidence. Exits 0 on success, 1 on failure.");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown subcommand: {other}");
            ExitCode::from(2)
        }
    }
}

fn run_check(path: PathBuf) -> ExitCode {
    let file = match ConformanceFile::load(&path) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("failed to load {}: {err}", path.display());
            return ExitCode::from(1);
        }
    };

    // Two operating modes:
    //   1. `LEXICON_CONFORMANCE_TEST_LIST=/path/to/cargo-test-list.txt` —
    //      parse and use that as the known-test index. This is the mode CI
    //      uses after capturing `cargo test --workspace -- --list`.
    //   2. Without that variable, validate purely structural rules. Tests
    //      are still required to be non-empty, but we do not assert they
    //      exist in the test list (status row may be `partial`).
    let known_tests = match std::env::var("LEXICON_CONFORMANCE_TEST_LIST") {
        Ok(list_path) if !list_path.trim().is_empty() => match load_test_index(&list_path) {
            Ok(set) => set,
            Err(err) => {
                eprintln!("failed to read test list {list_path}: {err}");
                return ExitCode::from(1);
            }
        },
        _ => BTreeSet::new(),
    };

    if let Err(err) = check(&file, &known_tests) {
        eprintln!("conformance check failed: {err}");
        return ExitCode::from(1);
    }

    println!(
        "conformance check OK: {} requirement(s) validated against {}",
        file.requirement.len(),
        if known_tests.is_empty() {
            "structural rules only".to_owned()
        } else {
            format!("{} known test(s)", known_tests.len())
        }
    );
    ExitCode::SUCCESS
}

fn load_test_index(path: &str) -> Result<BTreeSet<String>, std::io::Error> {
    let text = std::fs::read_to_string(path)?;
    let pairs = parse_cargo_test_list(&text);
    Ok(flatten_test_index(pairs.iter()))
}

impl From<MatrixError> for std::io::Error {
    fn from(value: MatrixError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, value.to_string())
    }
}
