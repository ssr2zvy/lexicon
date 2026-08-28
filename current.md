Current milestone: validate `source.toml` in parent-side `data --get` and `data --process` execution
Objective
Implement step 3 ("validate source.toml") of the parent-side data execution sequence (specs.md §24 for acquisition and §39 for processing) during `resolve_project_layout` so that `lexicon data --get` and `lexicon data --process` require and validate a schema-2 `source.toml` under `sources/<source>/<protocol>/source.toml` before runtime bundle admission, session selection, or process launch.
This milestone is derived from:
contract.md §5 (the `lexicon data` command boundary) and §7 (source structure);
specs.md §24 (Parent-side `data --get` sequence: step 1 discover project, step 2 resolve source and protocol, step 3 validate source.toml, step 4 load runtime metadata...);
specs.md §39 (Processing contract);
the prior completion reports noting that `resolve_project_layout` in `lexicon-framework/src/data/project.rs` only validates directory structure and does not validate the source manifest.
Repository-grounded starting point
`resolve_project_layout` (lexicon-framework/src/data/project.rs) currently checks for directory existence: `sources_root`, `source_dir`, `protocol_root`, `operation_root`, `raw_data_directory`, `processed_data_directory`, and `bundle_directory`. It does not check for or validate `source.toml` at all.
`load_source_metadata` and `validate_source_toml_text` (lexicon-framework/src/lib.rs) already implement schema-2 validation and return `SourceManifestError`.
`ForegroundDataExecutionError` (lexicon-framework/src/data/error.rs) does not currently have variants for missing or invalid source manifest errors.
`build_fake_project` (lexicon-framework/src/data/test_support.rs) currently does not write a `source.toml` file into its fixture `FakeProject`.
Required implementation
1. Add error variants to `ForegroundDataExecutionError`
Add typed error variants to `ForegroundDataExecutionError`:
* `MissingSourceManifest { source_name: String, path: PathBuf }`
* `InvalidSourceManifest { source_name: String, path: PathBuf, error: crate::SourceManifestError }`
Implement Display and source() for the new variants with actionable diagnostic messages.
2. Validate `source.toml` in `resolve_project_layout`
In `lexicon-framework/src/data/project.rs::resolve_project_layout`:
* After validating that `protocol_root` exists as a directory, check that `protocol_root.join("source.toml")` is a regular file. If missing, return `ForegroundDataExecutionError::MissingSourceManifest`.
* Read and validate the file contents using `crate::validate_source_toml_text(contents, source_name, protocol)`. If invalid, return `ForegroundDataExecutionError::InvalidSourceManifest`.
3. Update `FakeProject` test support fixture
Update `build_fake_project` in `lexicon-framework/src/data/test_support.rs` to write a canonical schema-2 `source.toml` into `protocol_root` using `crate::format_source_toml(source_name, "http")`.
4. Tests
Add tests proving:
* `resolve_project_layout` fails with `MissingSourceManifest` when `source.toml` is absent from `protocol_root`;
* `resolve_project_layout` fails with `InvalidSourceManifest` when `source.toml` is schema 1 or malformed;
* `resolve_project_layout` fails with `InvalidSourceManifest` when `source.toml` has mismatched source name, protocol, or contract versions;
* `resolve_project_layout` succeeds when a valid schema-2 `source.toml` is present;
* `execute_foreground_data` and background handoff reject missing or invalid `source.toml` before session selection or process launch;
* existing session and background execution tests pass with the updated `FakeProject` fixture.
Scope constraints
Do not implement during this milestone:
* changes to runtime admission or child runner execution;
* changes to `lexicon build` or `lexicon source build`;
* source-owned SQLite work ledgers;
* MZA Protocol 1 release construction;
* second-protocol support.
Completion criteria
This milestone is complete only when:
* `resolve_project_layout` validates the existence and schema-2 conformity of `source.toml` before returning;
* `ForegroundDataExecutionError` reports missing and invalid source manifests with typed variants;
* `FakeProject` writes a valid schema-2 `source.toml`;
* `cargo check --workspace` passes;
* `cargo test --workspace --quiet` passes;
* no production contract is weakened.
Completion report
When the milestone passes, replace this file with a concise report containing:
* the exact commit tested;
* confirmation that `cargo check --workspace` passed;
* confirmation that `cargo test --workspace --quiet` passed;
* where the source manifest validation was added in the execution flow;
* the number and categories of new tests added;
* confirmation that no required test remains ignored, deleted, or falsely successful.
Then stop.
The following milestone should be derived from the updated contract and specification once this one lands.
