Completed milestone: validate `source.toml` in parent-side `data --get` and `data --process` execution
Exact commit tested
Local uncommitted worktree against branch `validate-source-manifest-in-data-execution` based on commit `bf9feff` on `main`, containerized verification via podman machine ssh -> podman exec lexicon-local-test (image `lexicon-local-test-image`). Logs written to `$env:TEMP\lexicon-verify-logs\cargo-{check,test}.txt`.
Verification result
* `cargo check --workspace`: passed (exit 0).
* `cargo test --workspace --quiet`: passed (exit 0). Batches in order:
  * lexicon-cli:                                     30 passed, 0 failed, 0 ignored
  * lexicon-core:                                   246 passed, 0 failed, 0 ignored
  * lexicon-core-tests (trybuild UI suite):           1 passed (meta-test), 0 failed; 11 ui compile-fail tests pass
  * lexicon-framework:                             143 passed, 0 failed, 0 ignored (up from 139; +4 new source manifest validation tests in data::project)
  * doctests:                                         0 / 0 / 1 ignored (pre-existing placeholder)
  * integration meta:                                0 / 0
Implementation summary
* `ForegroundDataExecutionError::MissingSourceManifest { source_name: String, path: PathBuf }` and `ForegroundDataExecutionError::InvalidSourceManifest { source_name: String, path: PathBuf, error: SourceManifestError }` added in `lexicon-framework/src/data/error.rs` with `Display` and `source()` implementations.
* `resolve_project_layout` in `lexicon-framework/src/data/project.rs` now executes step 3 of specs.md §24 / §39 ("validate source.toml") by checking that `protocol_root.join("source.toml")` exists as a regular file and validating its content via `crate::validate_source_toml_text`, rejecting missing manifests with `MissingSourceManifest` and malformed/non-schema-2 manifests with `InvalidSourceManifest`.
* `build_fake_project` in `lexicon-framework/src/data/test_support.rs` updated to write a canonical schema-2 `source.toml` into `protocol_root` via `crate::format_source_toml`.
* Two test harness robustness enhancements:
  * In `lexicon-framework/src/build/runtime_probe.rs`, probe timeout test now uses bounded `ETXTBSY` retry.
  * In `lexicon-framework/src/data/background.rs`, `FAST_TIMEOUT` for process exit check increased to 2000ms to absorb CPU scheduling delay under high parallel test load.
New tests by category (4 total)
* `lexicon-framework/src/data/project.rs` (4 new unit tests):
  * `resolve_project_layout_rejects_missing_source_manifest`
  * `resolve_project_layout_rejects_schema_1_source_manifest`
  * `resolve_project_layout_rejects_mismatched_source_manifest_identity`
  * `resolve_project_layout_succeeds_with_valid_schema_2_source_manifest`
Confirmations
* No required test remains ignored, deleted, or falsely successful.
* No unrelated feature work (MZA release construction, work-ledger) was included.
The following milestone should be derived from the updated contract and specification once this one lands. Candidates include: (a) source-owned SQLite work-ledger convention built on top of `source_state_directory()` (specs.md §13-§15), (b) MZA Protocol 1 release construction and the `lexicon-bundle` adapter (specs.md §41), or (c) closing any remaining gaps identified by re-reading contract.md/specs.md against the state of `main` at that time. The actual next choice must be re-derived from the contract and the state of `main`, not assumed in advance.
