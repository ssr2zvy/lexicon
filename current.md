# Current state and next micro implementation

## Verdict

No. The summary in the request does not match the current main-branch code in this repository.

## Verified current state

The repository is still on the legacy CLI-to-installed-framework IPC pattern:

- `lexicon-framework/Cargo.toml` still contains both a library target and a binary target:
  - `[lib] path = "core/src/lib.rs"`
  - `[[bin]] name = "lexicon-framework" path = "src/main.rs"`
- `lexicon-framework/src/main.rs` is still present and still contains the framework command parser and `std::process::exit(...)` calls.
- `lexicon-cli/src/cli/mod.rs` still exposes `--framework-path`, resolves a framework binary via `framework_binary_path()`, and invokes the framework with `Command::new(framework_path)`.
- `lexicon-cli/src/cli/mod.rs` still persists a framework path under `~/.local/share/lexicon/framework-path` and honors `LEXICON_FRAMEWORK_PATH`.
- `lexicon-cli/Cargo.toml` does not yet add a direct dependency on `lexicon-framework`.

This is the opposite of the reported “library-only, direct-call migration.” The branch is still in the binary dispatch stage.

## Very next micro implementation

The smallest useful next step is to convert the first command route from subprocess dispatch to direct library calls while keeping the existing behavior intact.

### Target

`lexicon source create` and `lexicon source build` should call a Rust library function instead of spawning the framework binary.

### Minimal sequence

1. Define a direct framework API in the library root, with functions like:
   - `lexicon_framework::commands::source_create(...)`
   - `lexicon_framework::commands::source_build(...)`
   - return `Result<T, String>` values instead of exiting the process
2. Remove the `[[bin]]` target from `lexicon-framework/Cargo.toml` once the library API is in place and the binary is no longer the primary entry point.
3. Add the direct dependency in `lexicon-cli/Cargo.toml`:
   - `lexicon-framework = { path = "../lexicon-framework" }`
4. Replace the subprocess calls in `lexicon-cli/src/cli/mod.rs` with direct function calls.
5. Keep CLI parsing and rendering only in the CLI layer. The framework layer should own command logic and return typed results/errors.
6. After the direct call works, remove the legacy `--framework-path` plumbing, `framework_state_path()`, `read_framework_path()`, `write_framework_path()`, and the environment-variable logic.

## Why this is the next micro step

This preserves the current external command surface while removing the most brittle architecture boundary first: the `Command::new(...)` spawn path. It is the smallest change that moves the repo toward the summary’s desired end state without changing unrelated bundle or installation logic yet.
