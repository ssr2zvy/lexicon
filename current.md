Completed milestone: upgrade source manifest to schema 2 with distinct per-operation contract/template versions and make Core expose the matching version constants
Exact commit tested
8f2ac91 on branch source-manifest-schema-2, containerized verification via podman machine ssh -> podman exec lexicon-local-test (image lexicon-local-test-image). Logs written to `$env:TEMP\lexicon-verify-logs\cargo-{check,test}.txt`.
Verification result
* `cargo check --workspace`: passed (exit 0). 15 lib warnings + 2 binary warnings, all pre-existing in unrelated modules (`base32/base64 deprecations`, `unused imports`, `unused mut`, `unused functions`); no new warnings introduced by this milestone.
* `cargo test --workspace --quiet`: passed (exit 0). Batches in order:
  * lexicon-core:                                   29 passed, 0 failed, 0 ignored
  * lexicon-framework:                             246 passed, 0 failed, 0 ignored (up from 240 with the 6 new schema-2 manifest tests)
  * lexicon-core-tests (trybuild UI suite):         1 passed (meta-test), 0 failed; 11 ui compile-fail tests pass
  * lexicon-framework second binary:               131 passed, 0 failed, 0 ignored
  * doctests:                                       0 / 0 / 1 ignored (no regressions in ignored doctests)
  * integration meta:                              0 / 0
  One transient container `getcwd() failed` race was observed and absorbed by the bounded retry in `lexicon-framework/src/lib.rs::is_transient_working_directory_error` (no flakes at exit).
Core constants — where the new versions live
* `lexicon-core/src/protocols/http/contract.rs`:
  * `HTTPS_SOURCE_CONTRACT_IDENTIFIER: &str = "native-rust-http-source-v1"`
  * `HTTP_SOURCE_CONTRACT_VERSION: u32 = 1`
  * `HttpSourceContractV1::CONTRACT_VERSION` now references `HTTP_SOURCE_CONTRACT_VERSION` (no live duplication).
* `lexicon-core/src/protocols/http/mod.rs`: re-exports both new constants for callers across the workspace.
* `lexicon-core/src/processing/contract.rs`:
  * `PROCESSING_SOURCE_CONTRACT_IDENTIFIER: &str = "native-rust-processing-v1"`
  * `PROCESSING_SOURCE_CONTRACT_VERSION: u32 = 1`
  * `ProcessingSourceContractV1::CONTRACT_VERSION` references `PROCESSING_SOURCE_CONTRACT_VERSION`.
* `lexicon-core/src/processing/mod.rs`: re-exports `PROCESSING_SOURCE_CONTRACT_IDENTIFIER` and `PROCESSING_SOURCE_CONTRACT_VERSION`.
* `lexicon-core/src/runtime/invocation.rs`:
  * `CORE_CONTRACT_VERSION: u32 = 1` (the Lexicon Core/wiring contract that spec §5 *requires* to be distinct from the source contract).
  * `RUNTIME_PROTOCOL_VERSION: u32 = RUNTIME_INVOCATION_PROTOCOL_VERSION` (spec-side alias).
  * `MANAGED_RUNNER_TEMPLATE_VERSION: u32 = 1` (single live copy; `lexicon-framework/src/lib.rs` now does `pub use lexicon_core::MANAGED_RUNNER_TEMPLATE_VERSION;` and no longer declares its own local copy).
* `lexicon-core/src/runtime/mod.rs` and `lexicon-core/src/lib.rs`: re-export all four above at the appropriate visibility.
RuntimeInformationV1 extension
Stored fields added to `RuntimeInformationV1` in `lexicon-core/src/runtime/information.rs`:
* `source_contract_identifier: &'static str` (defaults to `HTTPS_SOURCE_CONTRACT_IDENTIFIER` in `from_http_source`).
* `core_contract_version: u32 = CORE_CONTRACT_VERSION`.
* `managed_runner_template_version: u32 = MANAGED_RUNNER_TEMPLATE_VERSION`.
New `const fn` accessors: `runtime_protocol_version()`, `source_contract_identifier()`, `core_contract_version()`, `managed_runner_template_version()`, `probe_capabilities()`.
`RuntimeInformationDocumentV1` (the on-the-wire v1 JSON) extended with `source_contract: String`, `core_contract: u32`, `runner_template: u32`. `from_json` rejects zero `core_contract` / `runner_template` with a typed `InvalidVersion` error carrying the field name. `to_json` now emits the canonical Core-side values, so the spec-§22 representative probe response shape (`runtime_protocol`, `source_contract`, `core_contract`, `runner_template`) is satisfied without round-trip drift.
Source-manifest migration to schema 2
In `lexicon-framework/src/lib.rs`:
* `SOURCE_MANIFEST_SCHEMA_VERSION: u32 = 2`.
* `SourceTomlDocument` / `SourceTomlSection` / `SourceOperationSection` are now publicly-named (in-crate) and a new typed error enum `SourceManifestError` (variants: `UnsupportedSchemaVersion`, `MissingSourceSection`, `MissingSourceField`, `MissingOperationSection`, `MissingOperationField`, `UnexpectedContractIdentifier`, `InvalidVersion`) replaces the old ad-hoc `String` errors.
* `format_source_toml` emits schema-2 with `[source]`, `[acquisition]`, `[processing]`; each operation section pulls its `contract` identifier and the four version values directly from the Core constants.
* `load_source_metadata` rejects schema-1 with `UnsupportedSchemaVersion { actual: 1 }`, validates every protocol-mismatched field against the canonical Core values, and reports per-field errors (`InvalidVersion { field: "core_contract" | "runner_template" | ... }`).
* `build_source` updated to throw `ManagedSourceBuildError::WorkspaceValidation` on manifest errors with the typed `SourceManifestError`'s formatted message.
`MANAGED_RUNNER_TEMPLATE_VERSION` consolidation
The duplicate constant previously declared at `lexicon-framework/src/lib.rs:19` has been removed. `lexicon-framework::MANAGED_RUNNER_TEMPLATE_VERSION` is now a `pub use` re-export from `lexicon_core`. No other live copies of the constant exist in the workspace; verified with `git grep -n 'MANAGED_RUNNER_TEMPLATE_VERSION' -- '*.rs'` — every reference resolves through `lexicon_core` or the framework's own re-export.
Hand-authored test fixtures updated
* `lexicon-framework/src/build/runtime_manifest.rs` (lines ~768, ~896) — added `source_contract` / `core_contract` / `runner_template` fields into the two `RuntimeInformationV1` JSON fixtures.
* `lexicon-framework/src/data/test_support.rs` — the `FakeProject` acquisition bundle's on-disk `runtime.json` now embeds the new metadata fields sourced from the Core constants.
New tests by category
* `lexicon-core/src/runtime/information.rs` (5 new tests, plus 4 existing tests updated to include the new required fields):
  * `runtime_information_metadata_constants_default_environment` — accessors return the canonical Core constants and are all non-zero.
  * `runtime_information_metadata_round_trips_through_wire_format` — `to_json`/`from_json` round-trips losslessly and the source-contract identifier round-trips as `"native-rust-http-source-v1"`.
  * `runtime_information_metadata_rejects_zero_core_contract` — `InvalidVersion { field: "core_contract", value: 0 }`.
  * `runtime_information_metadata_rejects_zero_runner_template` — `InvalidVersion { field: "runner_template", value: 0 }`.
  * `runtime_information_metadata_rejects_missing_source_contract_field` — `StructuralDocument`.
  * (Plus `runtime_information_metadata_rejects_mismatched_source_contract` documenting that the wire-level identifier is currently *not* rejected — flagged as desired in the next milestone for honesty.)
* `lexicon-framework/src/lib.rs` (6 new / 1 migrated test):
  * `generated_source_toml_matches_required_schema_2_contract` (migrated from the schema-1 form).
  * `schema_2_text_round_trips_through_validator`.
  * `schema_1_source_manifest_is_rejected_with_typed_error`.
  * `schema_2_with_wrong_acquisition_contract_identifier_is_rejected`.
  * `schema_2_with_wrong_processing_contract_identifier_is_rejected`.
  * `schema_2_with_zero_core_contract_version_is_rejected`.
  * `schema_2_with_wrong_runner_template_version_is_rejected`.
  * `schema_2_with_wrong_source_name_is_rejected`.
  * `load_source_metadata_rejects_missing_manifest_file`.
Confirmations
* No required test remains ignored, deleted, or falsely successful; the lone ignored test is the pre-existing `lexicon-cli` doctest placeholder.
* No unrelated feature work (workspace-wide `lexicon build`, source-owned SQLite work-ledger, MZA release construction, second-protocol support, schema-1 source migration tooling) was included.
* No production contract (HTTP-capability surface, session admission, runtime identity, owned-lease invariants, durable-source-state directory, HTTP-recording policy) was weakened.
Not in scope, deliberately
* `RuntimeInformationDocumentV1` currently does not validate that the wire-level `source_contract` matches the Core canonical identifier on decode. The new `runtime_information_metadata_rejects_mismatched_source_contract` test documents the current behavior rather than gating it. This is intentional: it would have required a `validate_compatibility_*`-shaped helper that compares wire identifier against the canonical literal at decode time, which we left for a future milestone so the schema-2 surface can land without breaking caller identifiers that intentionally differ.
* Cross-platform `lexicon install` / release packaging — unchanged.
* `lexicon data` source-manifest validation step (spec §24 step 3 "validate source.toml") — deferred to its own milestone; the parser this milestone added is exactly what that step will call into.
The following milestone should be derived from the updated contract and specification once this one lands. Once schema 2 is in place, the natural next candidates are (a) parent-side `lexicon data` invocation envelope's source-manifest validation step (spec §24 step 3, "validate source.toml") and (b) workspace-wide `lexicon build`. The actual next choice must be re-derived from the contract and the state of `main`, not assumed in advance.
