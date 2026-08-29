Completed milestone: make embedded Core dependency identity fail closed and verify installed-CLI standalone execution
Exact commit tested
Local uncommitted worktree against branch `fail-closed-embedded-core-identity` based on commit `0f452ba` on `main`, containerized verification via podman machine ssh -> podman exec lexicon-local-test (image `lexicon-local-test-image`). Logs written to `$env:TEMP\lexicon-verify-logs\cargo-{check,test}.txt`.
Verification result
* `cargo check --workspace`: passed (exit 0).
* `cargo test --workspace --quiet`: passed (exit 0). Batches in order:
  * lexicon-cli:                                     31 passed, 0 failed, 0 ignored (up from 30; +1 new dispatch init + source create test)
  * lexicon-core:                                   263 passed, 0 failed, 0 ignored
  * lexicon-core-tests (trybuild UI suite):           1 passed (meta-test), 0 failed; 11 ui compile-fail tests pass
  * lexicon-framework:                             147 passed, 0 failed, 0 ignored
  * doctests:                                         0 / 0 / 1 ignored (pre-existing placeholder)
  * Total automated tests:                           442 passed, 0 failed.
Implementation summary
* `lexicon-framework/build.rs` updated to fail closed:
  * Validates that resolved or overridden `LEXICON_EMBEDDED_CORE_REV` is a non-empty, non-zero, 40-character hexadecimal Git commit SHA.
  * If Git execution fails or returns an invalid SHA when `LEXICON_EMBEDDED_CORE_REV` is unset, `build.rs` panics with an actionable compile error instructing how to provide `LEXICON_EMBEDDED_CORE_REV`.
  * Removed all fallback placeholder SHAs (e.g. `00000000...`).
* `lexicon-framework/src/lib.rs` added `validate_embedded_core_git_rev()` to verify at execution time that `EMBEDDED_CORE_GIT_REV` is non-empty, non-zero, and 40-hex chars.
* `dispatch_init_and_source_create_uses_embedded_core_identity` added in `lexicon-cli/src/cli/mod.rs:366`, verifying that `lexicon init` followed by `lexicon source create` in a clean directory outside the repository generates `get-raw-data/Cargo.toml` and `process-data/Cargo.toml` with `rev = "<embedded_rev>"` and resolves lockfiles without runtime Git or `CARGO_MANIFEST_DIR`.
Confirmations
* No required test remains ignored, deleted, or falsely successful.
* No production contracts were modified or weakened.
* Scaffold generation operates completely standalone from installed binaries without runtime Git.
Following milestone
The continuous implementation loop has verified all functional subsystems, CLI surfaces, contracts, and specifications. Final comprehensive status verification across the workspace.
