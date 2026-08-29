//! Stand-alone integration test for the installed-CLI core identity
//! pipeline.
//!
//! COREID-04: the `lexicon` executable carries the embedded core
//! identity through its compiled binary and applies it to every fresh
//! source scaffold it produces, including scaffolds it makes for
//! projects that live outside the Lexicon workspace tree (i.e. without
//! any `.git` directory inherited from the Lexicon repo).
//!
//! This integration test does more than the in-binary unit test in
//! `cli::mod::tests`: it drives the actual installed binary
//! (compiled by `cargo test` via `CARGO_BIN_EXE_lexicon`) from a
//! freshly-created temporary directory that the host explicitly does
//! NOT initialize as a Git repository. The verified behavior is:
//!
//!   * the binary returns exit code 0;
//!   * the resulting `<project>/sources/<name>/http/` layout matches
//!     the audit's protocol scaffolding contract;
//!   * the embedded core identity (commit SHA / URL) baked into the
//!     binary is the same identity that ends up in the scaffold's
//!     `Cargo.toml`; and
//!   * the scaffold did NOT read its core identity from any sibling
//!     `.git` directory, because there is none.

use std::path::PathBuf;
use std::process::Command;

use lexicon_framework::EMBEDDED_CORE_GIT_REV;
use lexicon_framework::build::core_dependency::REQUIRED_CORE_GIT_URL;

fn assert_no_git_ancestry(project_root: &std::path::Path) {
    for ancestor in project_root.ancestors() {
        assert!(
            !ancestor.join(".git").exists(),
            "test project must not have any .git ancestor: {:?}",
            ancestor
        );
    }
}

fn locate_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lexicon"))
}

#[test]
fn installed_lexicon_source_create_embeds_compiled_core_identity_outside_repo() {
    let temp = tempfile::tempdir().expect("create temp project root");
    let project_root = temp.path();
    let parent_path = project_root.to_path_buf();

    // 1. The temporary directory must not see any .git directory above
    //    it. This proves COREID-04 exercises the "no inherited Git
    //    repository" path the audit demands.
    assert_no_git_ancestry(project_root);

    // 2. Run `lexicon init <parent> demo-project` against the actual
    //    installed binary. This is the precise behavior a downstream
    //    operator would see when installing Lexicon and using it on a
    //    fresh machine that has never been inside this repo.
    let lexicon_binary = locate_binary();
    let init_status = Command::new(&lexicon_binary)
        .arg("init")
        .arg(&parent_path)
        .arg("demo-project")
        .status()
        .expect("launch lexicon init");
    assert!(
        init_status.success(),
        "installed `lexicon init` must succeed; got {init_status:?}"
    );

    let project_dir = parent_path.join("demo-project");
    assert!(project_dir.join("lexicon.toml").is_file(), "lexicon.toml missing");

    // 3. Run `lexicon source create` inside the freshly-initialized
    //    but Git-less project. We invoke the binary with the project
    //    root as the current working directory, exactly as an
    //    operator would type it from their shell.
    let create_status = Command::new(&lexicon_binary)
        .arg("source")
        .arg("create")
        .arg("example-source")
        .arg("--protocol")
        .arg("http")
        .current_dir(&project_dir)
        .status()
        .expect("launch lexicon source create");
    assert!(
        create_status.success(),
        "installed `lexicon source create` must succeed; got {create_status:?}"
    );

    let protocol_dir = project_dir.join("sources/example-source/http");
    assert!(
        protocol_dir.join("source.toml").is_file(),
        "expected source.toml at {:?}",
        protocol_dir.join("source.toml")
    );

    // 4. Verify the embedded core identity round-trip. The binary's
    //    compile-time identity constants (URL + SHA) MUST equal the
    //    values that end up wedged into each protocol's Cargo.toml.
    //    Anything else means the scaffold is silently using an
    //    external resolver that the audit forbids.
    assert!(
        !EMBEDDED_CORE_GIT_REV.is_empty(),
        "EMBEDDED_CORE_GIT_REV must be populated; got empty string"
    );
    assert_eq!(
        EMBEDDED_CORE_GIT_REV.len(),
        40,
        "EMBEDDED_CORE_GIT_REV must be exactly 40 hex chars; got {EMBEDDED_CORE_GIT_REV:?}"
    );
    assert!(
        EMBEDDED_CORE_GIT_REV.chars().all(|c| c.is_ascii_hexdigit()),
        "EMBEDDED_CORE_GIT_REV must be hexadecimal; got {EMBEDDED_CORE_GIT_REV:?}"
    );
    assert_eq!(
        REQUIRED_CORE_GIT_URL, "https://github.com/ssr2zvy/lexicon",
        "REQUIRED_CORE_GIT_URL must be the audit-pinned URL"
    );

    let acquisition_cargo = std::fs::read_to_string(
        protocol_dir.join("get-raw-data/Cargo.toml"),
    )
    .expect("read get-raw-data/Cargo.toml");
    let processing_cargo = std::fs::read_to_string(
        protocol_dir.join("process-data/Cargo.toml"),
    )
    .expect("read process-data/Cargo.toml");

    for (label, text) in [
        ("get-raw-data/Cargo.toml", acquisition_cargo.as_str()),
        ("process-data/Cargo.toml", processing_cargo.as_str()),
    ] {
        assert!(
            text.contains(REQUIRED_CORE_GIT_URL),
            "{label} must contain the canonical core git URL {:?}; got:\n{text}",
            REQUIRED_CORE_GIT_URL
        );
        assert!(
            text.contains(EMBEDDED_CORE_GIT_REV),
            "{label} must contain the embedded 40-char revision {:?}; got:\n{text}",
            EMBEDDED_CORE_GIT_REV
        );
        assert!(
            text.contains("package = \"lexicon-core\""),
            "{label} must pin `package = \"lexicon-core\"` exactly; got:\n{text}"
        );
    }

    // 5. COREID-03 tightened signature: scaffold must not have spawned
    //    a .git directory inside itself.
    assert!(
        !project_dir.join(".git").exists(),
        "source create must not materialize a .git directory under {project_dir:?}"
    );
    assert!(
        !protocol_dir.join(".git").exists(),
        "source create must not materialize a .git directory under {protocol_dir:?}"
    );
}

#[test]
fn installed_lexicon_source_create_fails_clearly_when_protocol_is_unknown() {
    let temp = tempfile::tempdir().expect("create temp project root");
    let project_root = temp.path();
    let parent_path = project_root.to_path_buf();
    assert_no_git_ancestry(project_root);

    let lexicon_binary = locate_binary();

    let init_status = Command::new(&lexicon_binary)
        .args(["init", parent_path.to_str().unwrap(), "bad-protocol-project"])
        .status()
        .expect("launch lexicon init");
    assert!(init_status.success(), "init must succeed first");

    let project_dir = parent_path.join("bad-protocol-project");

    let output = Command::new(&lexicon_binary)
        .args(["source", "create", "example-source", "--protocol", "browser"])
        .current_dir(&project_dir)
        .output()
        .expect("launch lexicon source create with bad protocol");

    assert!(
        !output.status.success(),
        "unsupported protocol must cause the binary to exit non-zero; got {:?}",
        output.status
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported protocol"),
        "stderr must surface the unsupported protocol reason; got: {stderr}"
    );
    assert!(
        stderr.contains("'browser'"),
        "stderr must echo the rejected protocol name; got: {stderr}"
    );
}

