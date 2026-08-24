# Implementation Report: CLI-to-framework IPC → in-process library

## Summary

The lexicon-framework binary has been eliminated. The lexicon CLI now calls framework operations directly as Rust library functions. After this migration, `lexicon` is the only installed Lexicon control executable.

---

## Package and dependency changes

| Package | Change |
|---|---|
| `lexicon-framework` | `[[bin]]` target removed; `[lib]` now points to `src/lib.rs` instead of `core/src/lib.rs` |
| `lexicon-framework/src/main.rs` | Deleted after migrating all logic and tests to `src/lib.rs` |
| `lexicon-cli` | Added `lexicon-framework = { path = "../lexicon-framework" }` dependency |
| `lexicon-bundle` | Removed framework archive embedding, installation, and record entries |

---

## Removed framework executable and IPC paths

The following items have been removed from `lexicon-cli`:

- `--framework-path` CLI option
- `LEXICON_FRAMEWORK_PATH` environment variable
- `framework_state_path()` function
- `read_framework_path()` function
- `write_framework_path()` function
- `framework_binary_path()` function
- `FRAMEWORK_FROM_CLI` constant (and the build.rs that generated it)
- `Command::new(framework_path)` invocations for source create and source build
- `std::process::exit(status.code())` passthrough from framework subprocess
- Tests that resolved or required a framework binary path

---

## New direct command routes

```
lexicon init
  → cli dispatch (Clap parse)
  → lexicon_framework::commands::init(parent_path, project_name)
  → Ok(InitResult { project_directory })
  → CLI renders: [lexicon] Initialized project '<name>' at <path>

lexicon source create
  → cli dispatch
  → lexicon_framework::commands::source_create(source_name, protocol)
  → Ok(SourceCreateResult { source_name, protocol, protocol_dir, created_files })
  → CLI renders: [lexicon] Created source ... + files list

lexicon source build
  → cli dispatch
  → lexicon_framework::commands::source_build(source_name, protocol)
  → Ok(SourceBuildResult { source_name, protocol, get_runtime, process_runtime })
  → CLI renders: [lexicon] Built source ... + runtime paths
```

---

## Framework result and error types

All public command functions are in `lexicon_framework::commands` and return `Result<T, String>`:

```rust
pub struct InitResult { pub project_directory: PathBuf }
pub struct SourceCreateResult { pub source_name, protocol, protocol_dir, created_files }
pub struct SourceBuildResult { pub source_name, protocol, get_runtime, process_runtime }

pub fn init(parent_path: &Path, project_name: &str) -> Result<InitResult, String>
pub fn source_create(source_name: &str, protocol: &str) -> Result<SourceCreateResult, String>
pub fn source_build(source_name: &str, protocol: &str) -> Result<SourceBuildResult, String>
```

The framework library does **not** call `std::process::exit`. It does **not** print user-facing success or error messages. All errors are returned as `Err(String)` for the CLI to render.

---

## Changes to mza_artifacts.toml

Removed the `lexicon_framework` ordinary artifact entry. The bundle inputs now contain only `lexicon_cli`:

```toml
[[artifact]]
label = "lexicon_cli"
crate = "../../../lexicon-cli"
...

[[bundle]]
label = "lexicon_bundle"
crate = "../../../lexicon-bundle"
protocol = "cargo-bundler-v0.1.0"
inputs = ["lexicon_cli"]
```

---

## lexicon-bundle remains a binary installer

`lexicon-bundle` remains a binary crate. It is not converted to a library. MZA continues to compile it using `cargo-bundler-v0.1.0`. The bundle now embeds only the `lexicon_cli` archive (no `lexicon_framework` archive).

---

## cargo-bundler-v0.1.0 remains the active protocol

The `[[bundle]]` declaration in `mza_artifacts.toml` retains `protocol = "cargo-bundler-v0.1.0"`. No new protocol has been introduced.

---

## Protocol 1 input artifact list

The bundle's `inputs` field contains exactly:

```toml
inputs = ["lexicon_cli"]
```

---

## Installation layout changes

`lexicon-install.toml`:
- Removed: `[artifacts] framework`, `[platform.linux] framework`, `[platform.windows] framework`
- Kept: `[artifacts] cli`, Linux and Windows CLI paths, `record` paths

`lexicon-bundle/src/model.rs`:
- Removed `framework: PathBuf` from `Destinations`
- Removed `framework: String` from `InstallationRecord`

`lexicon-bundle/src/install.rs`:
- Removed `framework_archive` extraction and installation
- Removed `set_executable(&dest.framework)` call
- Removed `verify_framework_reachable` check
- Removed framework uninstall (`fs::remove_file(&dest.framework)`)
- `detect_state` now checks only `cli` and `record`

`lexicon-bundle/build.rs`:
- Removed `framework: String` from `InstallArtifactLabels`
- Removed `framework: String` from `InstallPlatformEntry`
- No longer generates `FRAMEWORK_ARTIFACT_LABEL` or `FRAMEWORK_INSTALL_PATH` constants

---

## Validation commands and results

### Tests (all pass)

```
cargo test (workspace)

lexicon-cli:         25 passed; 0 failed
lexicon-framework:   27 passed; 0 failed
lexicon-framework-core: 1 passed; 0 failed
```

### New tests added

- `cli::tests::cli_help_does_not_expose_framework_path` — verifies `--framework-path` is absent from help
- `cli::tests::cli_source_create_calls_framework_library_directly` — verifies direct library call
- `cli::tests::unsupported_protocol_returns_error_not_exit` — verifies error is returned not exit
- `framework::tests::framework_init_returns_typed_result_not_exit` — verifies typed InitResult
- `framework::tests::framework_init_fails_with_error_not_exit_for_bad_name` — verifies bad name returns Err
- `framework::tests::framework_source_create_fails_with_error_not_exit_for_bad_protocol` — verifies Err not exit

### Existing tests preserved

All 24 original framework tests (publication transaction, staging, rollback, source name validation, JSON artifact selection, etc.) are preserved in `lexicon-framework/src/lib.rs`.

---

## Evidence that no separate framework executable is built or installed

- `lexicon-framework/Cargo.toml` has no `[[bin]]` target
- `lexicon-framework/src/main.rs` has been deleted
- `cargo build -p lexicon-framework` produces only a `.rlib`/`.rmeta` library artifact
- `mza_artifacts.toml` has no `lexicon_framework` artifact entry
- `lexicon-install.toml` has no `framework` field
- `lexicon-bundle` does not look up or install a framework binary

---

## Remaining blockers

None. The control-plane boundary migration is complete. The next implementation request (Core relocation, typed descriptor, implementation-library workspace, Lexicon-managed runner) can proceed.
