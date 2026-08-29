use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=MZA_BUNDLE_INPUTS");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));

    // When MZA_BUNDLE_INPUTS is not set by the MZA Protocol 1 harness (e.g. during standalone
    // `cargo check` or test runs), generate an empty stub so `include!(env!("MZA_BUNDLE_INPUTS"))`
    // compiles cleanly.
    if env::var("MZA_BUNDLE_INPUTS").is_err() {
        let stub_path = out_dir.join("mza_bundle_inputs_stub.rs");
        fs::write(
            &stub_path,
            "// MZA_BUNDLE_INPUTS stub for standalone compilation\n",
        )
        .expect("write MZA_BUNDLE_INPUTS stub");
        println!("cargo:rustc-env=MZA_BUNDLE_INPUTS={}", stub_path.display());
    }
}
