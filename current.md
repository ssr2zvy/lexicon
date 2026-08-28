Completed milestone: implement `lexicon build` (workspace-wide discovery and build)
Exact commit tested
Local uncommitted worktree against branch `implement-lexicon-build` based on commit `7615bae` on `main`, containerized verification via podman machine ssh -> podman exec lexicon-local-test (image `lexicon-local-test-image`). Logs written to `$env:TEMP\lexicon-verify-logs\cargo-{check,test}.txt`.
Verification result
* `cargo check --workspace`: passed (exit 0). 16 pre-existing framework warnings + 2 CLI warnings; no new warnings introduced by this milestone.
* `cargo test --workspace --quiet`: passed (exit 0). Batches in order:
  * lexicon-cli:                                     30 passed, 0 failed, 0 ignored (up from 29; +1 new dispatch build test)
  * lexicon-core:                                   246 passed, 0 failed, 0 ignored
  * lexicon-core-tests (trybuild UI suite):           1 passed (meta-test), 0 failed; 11 ui compile-fail tests pass
  * lexicon-framework:                             139 passed, 0 failed, 0 ignored (up from 131; +8 new discovery and build_all tests)
  * doctests:                                         0 / 0 / 1 ignored (pre-existing placeholder)
  * integration meta:                                0 / 0
Implementation summary
* `BuildAllError` and `commands::BuildAllOutcome` / `commands::SourceBuildFailure` added in `lexicon-framework/src/lib.rs`.
* `discover_build_targets()` in `lexicon-framework/src/lib.rs` discovers containing project via `find_project_root`, resolves sources directory via `configured_sources_directory`, enumerates immediate subdirectories validating source names, enumerates protocol directories validating `http`, pre-flight validates schema-2 `source.toml` via `validate_source_toml_text`, rejects non-directory entries, unrecognized protocol directories, and missing/malformed manifests with typed `BuildAllError` variants, and returns a lexicographically sorted list of `(source_name, protocol)` targets.
* `commands::build_all()` in `lexicon-framework/src/lib.rs` calls `discover_build_targets()` and executes `source_build(&source_name, &protocol)` for each target sequentially, collecting successes and per-source failures into `BuildAllOutcome`.
* `lexicon-cli/src/cli/mod.rs` `RootCommand::Build(_)` dispatch arm calls `lexicon_framework::commands::build_all()`, prints per-source build outcomes and executables, prints summary, and returns `Err` with all failed identities if any source failed.
Precise discovery behavior
* Zero sources: returns empty list; `build_all()` succeeds with 0 built, 0 failed (`build_all_finds_zero_targets_in_empty_sources_directory`).
* Single source: discovers `("my-source", "http")` (`discover_build_targets_finds_valid_source_and_protocol`).
* Multiple sources: sorts targets lexicographically e.g. `alpha-source`, `beta-source`, `zebra-source` (`discover_build_targets_sorts_deterministically`).
* Non-directory entry in sources root (e.g. `sources/junk.txt`): rejected with `BuildAllError::NonDirectoryInSourcesRoot` (`discover_build_targets_rejects_non_directory_in_sources_root`).
* Non-directory entry in source directory (e.g. `sources/my-source/notes.txt`): rejected with `BuildAllError::NonDirectoryInSource` (`discover_build_targets_rejects_non_directory_in_source`).
* Unrecognized protocol directory (e.g. `sources/my-source/browser/`): rejected with `BuildAllError::UnrecognizedProtocolDirectory` (`discover_build_targets_rejects_unrecognized_protocol_directory`).
* Schema-1 / malformed manifest: rejected at discovery with `BuildAllError::InvalidSourceManifest` carrying the typed `SourceManifestError` (`discover_build_targets_rejects_schema_1_source_manifest`).
* Source with no protocol directories: rejected with `BuildAllError::NoRecognizedProtocolDirectories` (`discover_build_targets_rejects_source_with_no_protocol_directories`).
New tests by category (9 total)
* `lexicon-framework/src/lib.rs` (8 new tests):
  * `build_all_finds_zero_targets_in_empty_sources_directory`
  * `discover_build_targets_finds_valid_source_and_protocol`
  * `discover_build_targets_sorts_deterministically`
  * `discover_build_targets_rejects_non_directory_in_sources_root`
  * `discover_build_targets_rejects_non_directory_in_source`
  * `discover_build_targets_rejects_unrecognized_protocol_directory`
  * `discover_build_targets_rejects_schema_1_source_manifest`
  * `discover_build_targets_rejects_source_with_no_protocol_directories`
* `lexicon-cli/src/cli/mod.rs` (1 new test):
  * `dispatch_build_command_runs_build_all_on_empty_project`
Confirmations
* No required test remains ignored, deleted, or falsely successful.
* No unrelated feature work (MZA release construction, work-ledger, `lexicon data` source-manifest integration) was included.
* `lexicon source build` continues to validate the same schema-2 `source.toml` unchanged.
The following milestone should be derived from the updated contract and specification once this one lands. Candidates include: (a) parent-side `lexicon data --get/--process` step 3 "validate source.toml" (specs.md §24) wired through `RuntimeProjectLayout`, (b) the source-owned SQLite work-ledger convention built on top of `source_state_directory()` (specs.md §13-§15), or (c) MZA Protocol 1 release construction and the `lexicon-bundle` adapter (specs.md §41). The actual next choice must be re-derived from the contract and the state of `main`, not assumed in advance.
