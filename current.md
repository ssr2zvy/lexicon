Implementation report

- Added `lexicon-framework/src/build/runtime_staging.rs` with the staged HTTP runtime bundle API: `StagedHttpRuntimeBundle`, `RuntimeBundleStagingError`, and `stage_verified_http_runtime_bundle`.
- The staging operation creates a uniquely owned temporary directory beneath the requested parent, copies the verified runtime while preserving its source permissions, validates the staged size and SHA-256 against the verified artifact, and writes `runtime.json` using create-new semantics with the exact manifest payload plus a single trailing newline.
- The implementation synchronizes the executable and manifest files, and attempts directory sync where supported, while keeping the temporary staging directory self-cleaning by owning a `TempDir` without exposing it in the public API.
- Exported the new runtime staging API through `lexicon-framework/src/build/mod.rs`.
- Validation: `cargo test --workspace --quiet` passed.
