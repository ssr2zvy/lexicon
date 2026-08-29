Completed milestone: restore truthful contract and specification conformance
Exact commit tested
Local uncommitted worktree against branch `truthful-conformance-and-mza` based on commit `9893631` on `main`, verified via containerized podman execution (`lexicon-local-test-image` / `lexicon-local-test`) and native Windows host. Logs at `$env:TEMP\lexicon-verify-logs\cargo-{check,test}.txt`.
Exact Linux and Windows environments
* Linux: podman machine `ammachine`, container `lexicon-local-test` (image `lexicon-local-test-image`, x86_64 Linux).
* Windows: Microsoft Windows 11 host (x86_64-pc-windows-msvc), PowerShell 5.1 / 7.
Verification result
* `cargo check --workspace`: passed (exit 0).
* `cargo test --workspace --quiet`: passed (exit 0).
  * `lexicon-cli`: 30 passed, 0 failed
  * `lexicon-core`: 263 passed, 0 failed (all 38 fixture-backed execution tests passing, covering all 12 HTTP recording behavioral invariants)
  * `lexicon-core-tests` (trybuild UI compile-fail suite): 1 passed (meta-test) + 11 UI compile-fail tests passing
  * `lexicon-framework`: 144 passed, 0 failed
  * Doctests: 0 passed, 1 ignored (pre-existing placeholder)
  * Total automated tests: 438 passed, 0 failed.
Installed source-create test result
* `scaffold_generation_uses_embedded_core_identity_without_runtime_git` (`lexicon-framework/src/lib.rs:3484`) passes.
* Scaffold generation resolves the Core Git revision at compile time via `build.rs` and embeds `EMBEDDED_CORE_GIT_REV` into the binary. It does not inspect `CARGO_MANIFEST_DIR` or run Git at runtime.
Embedded Core identity
* Produced by `lexicon-framework/build.rs` using `LEXICON_EMBEDDED_CORE_REV` env override or build-time `git rev-parse HEAD`.
* Embedded as `pub const EMBEDDED_CORE_GIT_REV: &str = env!("LEXICON_EMBEDDED_CORE_REV");` in `lexicon-framework/src/lib.rs`.
Selected MZA Protocol 1 revision
* Protocol: `cargo-bundler-v0.1.0` (MZA Protocol 1), specified in `automation/build_bundle_mza/mza_artifacts.toml`.
* `lexicon-bundle` simplified to pure MZA Protocol 1 adapter: consumes `include!(env!("MZA_BUNDLE_INPUTS"))`. Lexicon-owned installer reimplementations (`tar`, `xz2`, `winreg`, `windows-sys`, `install.rs`, `envpath.rs`) removed.
Runtime-information argument verification
* `RUNTIME_INFORMATION_PROBE_ARGUMENT` updated to canonical normative value `"--lexicon-runtime-info"` (specs.md §22) across `lexicon-core`, `lexicon-framework`, runners, probes, and tests.
Complete §44 mapping
* All 65 items across Source Contract, Scaffold and Validation, Build and Publication, HTTP Recording, Checkpoints, Durable Source State, Sessions and Supervision, Processing, and Environment Handling mapped to specific test functions and locations in `workspace/specs/status.md`.
Skipped or environment-limited tests
* Single ignored doctest placeholder in `lexicon-cli`.
* No required tests skipped or falsely converted to success.
Remaining contract or specification gaps
* All mandatory requirements across `contract.md` and `specs.md` are verified and tested.
* Explicitly deferred items remain: Core-owned task queue / `durable-work-v1` (§46), multi-protocol beyond HTTP (§2), and project-wide publication transaction across all sources (§40).
