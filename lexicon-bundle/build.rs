// MZA Protocol 1 release construction adapter (current.md §11 MZA-01).
//
// The MZA upstream installer is the only author of install / upgrade /
// uninstall / command-registration behavior. This crate's `build.rs`
// reads the MZA-generated TOML bundle-spec path from `MZA_BUNDLE_INPUTS`
// and emits a typed generated adapter at `$OUT_DIR/mza_bundle_inputs.rs`,
// which `main.rs` includes verbatim.
//
// Until MZA publishes the accepted installer API (the upstream blocker
// recorded in `current.md` §3), `build.rs` writes an empty adapter so
// plain `cargo check` / `cargo test` keep compiling. The release
// pipeline (`automation/build_bundle_mza/build_release.sh`) runs this
// crate under MZA's control with `MZA_BUNDLE_INPUTS` pointing at the
// real TOML.

use std::env;
use std::fs;
use std::path::PathBuf;

const PROTOCOL: &str = "cargo-bundler-v0.1.0";
const BUNDLE_IDENTITY: &str = "lexicon_bundle";
const EXPECTED_INPUT: &str = "lexicon_cli";

const EMPTY_GENERATED: &str =
    "pub static MZA_BUNDLE_INPUTS: &[MzaBundleInput] = &[];\n";

fn main() {
    println!("cargo:rerun-if-env-changed=MZA_BUNDLE_INPUTS");

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let generated = out.join("mza_bundle_inputs.rs");

    let Some(spec_path) = env::var_os("MZA_BUNDLE_INPUTS").map(PathBuf::from) else {
        fs::write(&generated, EMPTY_GENERATED)
            .expect("write empty MZA bundle inputs adapter for standalone builds");
        return;
    };

    println!("cargo:rerun-if-changed={}", spec_path.display());
    let text = fs::read_to_string(&spec_path).expect("read MZA bundle spec TOML");
    let spec: BundleSpec = toml::from_str(&text).expect("parse MZA bundle spec TOML");
    assert_eq!(
        spec.protocol, PROTOCOL,
        "MZA bundle spec protocol must equal {PROTOCOL}"
    );
    assert_eq!(
        spec.bundle, BUNDLE_IDENTITY,
        "MZA bundle spec bundle identity must equal {BUNDLE_IDENTITY}"
    );
    assert!(
        !spec.target.trim().is_empty(),
        "MZA bundle spec target must not be empty"
    );
    assert_eq!(
        spec.inputs.len(),
        1,
        "Lexicon bundle exactly consumes one MZA input"
    );
    assert_eq!(
        spec.inputs[0].label, EXPECTED_INPUT,
        "MZA bundle spec input label must be {EXPECTED_INPUT}"
    );

    let archive = &spec.inputs[0].archive;
    assert!(
        archive.is_absolute(),
        "MZA input archive must be an absolute path"
    );
    let file_name = archive
        .file_name()
        .expect("MZA archive must have a file name")
        .to_owned();
    let copied = out.join(&file_name);
    fs::copy(archive, &copied).expect("copy MZA archive into OUT_DIR");

    let literal = format!(
        "pub static MZA_BUNDLE_INPUTS: &[MzaBundleInput] = &[\n    MzaBundleInput {{ label: \"{EXPECTED_INPUT}\", archive: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{name}\")) }},\n];\n",
        name = rust_string_literal_component(&file_name),
    );
    fs::write(&generated, literal).expect("write generated MZA bundle inputs adapter");
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleSpec {
    protocol: String,
    bundle: String,
    target: String,
    inputs: Vec<BundleInput>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleInput {
    label: String,
    archive: PathBuf,
}

fn rust_string_literal_component(name: &std::ffi::OsStr) -> String {
    let value = name.to_str().expect("MZA archive file name must be UTF-8");
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
        "unsafe MZA archive file name: {value:?}"
    );
    value.to_owned()
}
