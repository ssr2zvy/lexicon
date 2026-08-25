# Implementation report

Implemented the framework-side runtime probe output admission in `lexicon-framework`.

## What changed
- Added `lexicon-framework/src/build/runtime_probe.rs`.
- Added `MAX_RUNTIME_INFORMATION_PROBE_BYTES` and the admitted wrapper `AdmittedRuntimeInformation`.
- Added `admit_http_runtime_information_probe(...)` with exact stdout-boundary validation, UTF-8/NUL rejection, deterministic validation order, and typed error classification.
- Reused Core decoding and compatibility validation via `RuntimeInformationV1::from_json(...)` and `validate_compatibility(...)` without duplicating the JSON schema or rules.
- Exposed the API through `lexicon-framework/src/build/mod.rs` and `pub mod build;` in the framework library.

## Validation
- `cargo test -p lexicon-framework --quiet` ✅
- `cargo test --workspace --quiet` ✅
- `bash automation/build_bundle_install/build_bundle_install.sh` attempted, but it is blocked because the external MZA checkout is unavailable in this environment (`automation/build_bundle_mza/mza/make-artifact.sh` is missing). No MZA or installer code was modified.
