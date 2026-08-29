use std::path::Path;
use std::process::Command;

fn is_valid_git_rev(s: &str) -> bool {
    s.len() == 40
        && s.chars().all(|c| c.is_ascii_hexdigit())
        && !s.chars().all(|c| c == '0')
}

const EXPECTED_CORE_GIT_URL: &str = "https://github.com/ssr2zvy/lexicon";

fn main() {
    println!("cargo:rerun-if-env-changed=LEXICON_EMBEDDED_CORE_REV");
    println!("cargo:rerun-if-env-changed=LEXICON_EMBEDDED_CORE_URL");
    println!("cargo:rerun-if-changed=../lexicon-core");

    // COREID-01: emit the canonical Core URL alongside the revision so
    // generated source workspaces interpolate both from one constant.
    println!("cargo:rustc-env=LEXICON_EMBEDDED_CORE_URL={EXPECTED_CORE_GIT_URL}");

    if let Ok(rev) = std::env::var("LEXICON_EMBEDDED_CORE_REV") {
        let trimmed = rev.trim();
        if is_valid_git_rev(trimmed) {
            println!("cargo:rustc-env=LEXICON_EMBEDDED_CORE_REV={trimmed}");
            return;
        } else {
            panic!(
                "lexicon-framework/build.rs: LEXICON_EMBEDDED_CORE_REV was set to '{trimmed}', \
                 which is not a valid 40-character non-zero hexadecimal Git commit SHA."
            );
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
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            panic!(
                "lexicon-framework/build.rs: `git rev-parse HEAD` failed: {stderr}\n\
                 When building outside a Git repository, provide the exact 40-character Core \
                 Git commit SHA via the LEXICON_EMBEDDED_CORE_REV environment variable."
            );
        }
        Err(err) => {
            panic!(
                "lexicon-framework/build.rs: Failed to execute `git`: {err}\n\
                 When building without Git installed, provide the exact 40-character Core \
                 Git commit SHA via the LEXICON_EMBEDDED_CORE_REV environment variable."
            );
        }
    };

    if !is_valid_git_rev(&rev) {
        panic!(
            "lexicon-framework/build.rs: Resolved Git revision '{rev}' is not a valid 40-character hexadecimal SHA."
        );
    }

    println!("cargo:rustc-env=LEXICON_EMBEDDED_CORE_REV={rev}");
}
