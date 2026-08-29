use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=LEXICON_EMBEDDED_CORE_REV");

    if let Ok(rev) = std::env::var("LEXICON_EMBEDDED_CORE_REV") {
        let trimmed = rev.trim();
        if !trimmed.is_empty() {
            println!("cargo:rustc-env=LEXICON_EMBEDDED_CORE_REV={trimmed}");
            return;
        }
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let repo_root = Path::new(&manifest_dir)
        .parent()
        .unwrap_or(Path::new(&manifest_dir));

    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("rev-parse")
        .arg("HEAD")
        .output();

    let rev = match output {
        Ok(out) if out.status.success() => {
            String::from_utf8(out.stdout)
                .unwrap_or_default()
                .trim()
                .to_string()
        }
        _ => "0000000000000000000000000000000000000000".to_string(),
    };

    let effective_rev = if rev.is_empty() {
        "0000000000000000000000000000000000000000".to_string()
    } else {
        rev
    };

    println!("cargo:rustc-env=LEXICON_EMBEDDED_CORE_REV={effective_rev}");
}
