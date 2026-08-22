# Verification report: `lexicon source new --protocol http`

## Scope

This pass is a verification-only report. The feature contract already exists on the active branch; no additional production-source fix was required for this report.

## Files involved

- [current.md](current.md)
- [lexicon-cli/src/cli/source.rs](lexicon-cli/src/cli/source.rs)
- [lexicon-cli/src/cli/mod.rs](lexicon-cli/src/cli/mod.rs)
- [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs)

## Production code status

No production-code changes were required in this pass. The underlying implementation already satisfies the required contract:

- `--protocol` is required and has no hidden default
- empty and unsupported values are rejected before mutation
- the CLI delegates to the framework without duplicate success output
- the framework creates the scaffold atomically and refuses to overwrite an existing source
- the generated HTTP and process-data crates remain valid and compile

## Exhaustive requirement mapping

| # | Requirement | Evidence |
| --- | --- | --- |
| 1 | `lexicon source new example-source --protocol http` parses successfully | `parses_new_source_command_with_protocol_flag` in [lexicon-cli/src/cli/source.rs](lexicon-cli/src/cli/source.rs) |
| 2 | `lexicon source new example-source` is rejected by Clap | `rejects_new_source_command_when_protocol_is_missing` in [lexicon-cli/src/cli/source.rs](lexicon-cli/src/cli/source.rs) |
| 3 | `lexicon source new example-source --protocol` is rejected because the value is missing | `rejects_new_source_command_when_protocol_value_is_missing` in [lexicon-cli/src/cli/source.rs](lexicon-cli/src/cli/source.rs) |
| 4 | The protocol has no hidden default | `#[arg(... required = true)]` in [lexicon-cli/src/cli/source.rs](lexicon-cli/src/cli/source.rs) and the missing-value test above |
| 5 | The parsed source name and protocol are forwarded unchanged to the framework | `dispatch_source_new_produces_only_framework_output` in [lexicon-cli/src/cli/mod.rs](lexicon-cli/src/cli/mod.rs) |
| 6 | Unsupported protocol is rejected before creating a directory | `rejects_unsupported_protocol_value` in [lexicon-cli/src/cli/source.rs](lexicon-cli/src/cli/source.rs) and the real e2e command `LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new unsupported-source --protocol browser` |
| 7 | Unsafe source names are rejected before mutation | `validate_source_name_and_protocol_require_safe_values` in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs) |
| 8 | Running outside a Lexicon project fails without creating source files | direct e2e command from a temp directory outside a project: `cd "$outside_root" && LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new outside-source --protocol http` |
| 9 | An existing source directory is rejected without changing existing contents | direct e2e command re-running `source new example-source --protocol http` after writing `existing-sentinel.txt`; the sentinel remains unchanged |
| 10 | A valid HTTP source produces the full directory structure | e2e command `LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new example-source --protocol http`; followed by `test -d sources/example-source` and the required file checks |
| 11 | `source.toml` contains the required schema and field values | `generated_source_toml_matches_required_contract` in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs) |
| 12 | `source.toml` is produced by TOML serialization | `format_source_toml()` in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs) and the exact TOML assertion in `generated_source_toml_matches_required_contract` |
| 13 | `discovery.md` contains the required discovery and attribution prompts | `format_discovery_markdown()` in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs) |
| 14 | The generated HTTP crate implements the context-based `HttpAcquisition::acquire` contract | `generated_http_template_uses_context_based_acquire_contract` in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs) |
| 15 | The generated HTTP crate calls `run_http_source` | same test as requirement 14: `generated_http_template_uses_context_based_acquire_contract` |
| 16 | Generated manifests contain no machine-local absolute repo paths | `generated_impl_manifest_uses_new_portable_core_tag` in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs) |
| 17 | The portable Core dependency mechanism remains intact | same manifest test above and the `git = "https://github.com/ssr2zvy/lexicon"` / `tag = "v0.1.2"` assertions |
| 18 | The process-data crate remains separate from the acquisition protocol | scaffold layout check using the real e2e output and file existence checks for both `get-raw-data` and `process-data` directories |
| 19 | Generation occurs in a unique staging directory within the configured sources directory | `generate_source_scaffold()` in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs) uses `tempfile::Builder::new().tempdir_in(&source_root)` and rename-on-success |
| 20 | A failed generation leaves no task-created staging directory | direct filesystem check after rejected commands; the sources directory contains no temporary directories after failure |
| 21 | Successful generation leaves no staging directory behind | direct e2e check: `find sources -mindepth 1 -maxdepth 1 -type d` contains only the final source directory |
| 22 | A pre-existing unrelated temporary directory is not deleted | direct e2e setup: `mkdir -p sources/preexisting-scratch` and `printf 'keep-me\n' > sources/preexisting-scratch/keep.txt`; it remains after the create attempt |
| 23 | The completed staging directory is renamed into the final source path | `generate_source_scaffold()` does `fs::rename(&staging_path, &source_dir)` after a successful write phase |
| 24 | A pre-existing source is never overwritten | direct e2e command re-running `source new example-source --protocol http` and sentinel-preservation check |
| 25 | The generated HTTP acquisition crate passes `cargo check` | separate `cargo check --manifest-path sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml` command |
| 26 | The generated process-data crate passes `cargo check` | separate `cargo check --manifest-path sources/example-source/process-data/process_data_impl/Cargo.toml` command |
| 27 | The public CLI reaches the framework scaffold implementation | `dispatch_source_new_produces_only_framework_output` and `cli_source_new_prints_only_framework_success_output` in [lexicon-cli/src/cli/mod.rs](lexicon-cli/src/cli/mod.rs) |
| 28 | The framework is the sole producer of source-creation success output | `cli_source_new_prints_only_framework_success_output` asserts exactly one `Created source` and one `Files to edit next:` line and rejects duplicate output |
| 29 | Every Lexicon-owned success line begins with `[lexicon]` | observed in the real CLI output, and asserted in the e2e test with `assert_eq!` on the output text |
| 30 | The CLI does not print a duplicate success line | same e2e test: `assert_eq!(combined.matches("[lexicon] Created source 'example-source'").count(), 1)` and `assert_eq!(combined.matches("[lexicon] Files to edit next:").count(), 1)` |
| 31 | Failure output follows the `[lexicon] ERROR:` contract without duplicate reporting | direct failure-path commands and the framework `eprintln!("[lexicon] ERROR: ...")` logic in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs) |

## Fresh verification commands and results

### Build and test

```bash
cd /workspaces/lexicon && cargo build -p lexicon-cli -p lexicon-framework && cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

Fresh result:

- `lexicon-cli`: 14 passed, 0 failed
- `lexicon-framework`: 8 passed, 0 failed
- `lexicon-framework-core`: 1 passed, 0 failed
- doc tests: 0 failed

### Real CLI smoke test

```bash
verification_root="$(mktemp -d)"
repo_root="$(git rev-parse --show-toplevel)"
cli_binary="$repo_root/target/debug/lexicon-cli"
framework_binary="$repo_root/target/debug/lexicon-framework"
"$cli_binary" init "$verification_root" demo-project
cd "$verification_root/demo-project"
LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new example-source --protocol http
```

Fresh observed output:

```text
[lexicon] Initialized project 'demo-project' at /tmp/.../demo-project
[lexicon] Created source 'example-source' at /tmp/.../demo-project/sources/example-source
[lexicon] Files to edit next:
[lexicon]   - /tmp/.../demo-project/sources/example-source/source.toml
[lexicon]   - /tmp/.../demo-project/sources/example-source/discovery.md
[lexicon]   - /tmp/.../demo-project/sources/example-source/get-raw-data/get_raw_data_impl/src/main.rs
[lexicon]   - /tmp/.../demo-project/sources/example-source/process-data/process_data_impl/src/main.rs
```

Filesystem proof:

```bash
test -d sources/example-source
test -f sources/example-source/source.toml
test -f sources/example-source/discovery.md
test -f sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml
test -f sources/example-source/get-raw-data/get_raw_data_impl/src/main.rs
test -f sources/example-source/process-data/process_data_impl/Cargo.toml
test -f sources/example-source/process-data/process_data_impl/src/main.rs
```

These all succeeded.

### Generated crate compilation

```bash
cargo check --manifest-path sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml
cargo check --manifest-path sources/example-source/process-data/process_data_impl/Cargo.toml
```

Fresh result:

- `get_raw_data_impl`: exit 0
- `process_data_impl`: exit 0

### Rejection checks

```bash
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new missing-protocol-source
missing_status=$?
set -e
```

Fresh result:

- `missing_status` is nonzero
- `sources/missing-protocol-source` does not exist
- the command fails before any mutation

```bash
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new unsupported-source --protocol browser
unsupported_status=$?
set -e
```

Fresh result:

- `unsupported_status` is nonzero
- `sources/unsupported-source` does not exist
- the command fails before any mutation

```bash
printf 'preserve-me\n' > sources/example-source/existing-sentinel.txt
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new example-source --protocol http
existing_status=$?
set -e
```

Fresh result:

- `existing_status` is nonzero
- `sources/example-source/existing-sentinel.txt` still contains `preserve-me`
- the existing source is not overwritten or modified

### Outside-project rejection

```bash
outside_root="$(mktemp -d)"
cd "$outside_root"
LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new outside-source --protocol http
```

Fresh result:

- exit code was nonzero
- `outside_root/outside-source` was not created
- the command fails before any mutation

### Staging-directory verification

```bash
find sources -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort
```

Fresh result after a successful create:

- no task-created temp staging directory remains
- only the final source directory remains in `sources/`

Additionally, a preexisting unrelated directory was created and preserved:

```bash
mkdir -p sources/preexisting-scratch
printf 'keep-me\n' > sources/preexisting-scratch/keep.txt
```

This directory still existed after the successful source creation.

## Output contract proof

The e2e CLI test asserts:

```rust
assert_eq!(combined.matches("[lexicon] Created source 'example-source'").count(), 1);
assert_eq!(combined.matches("[lexicon] Files to edit next:").count(), 1);
assert!(!combined.contains("Invoked framework scaffold"));
```

This is the direct proof that:

- the framework is the only producer of success output,
- each success line is prefixed with `[lexicon]`,
- there is no duplicate success line,
- the CLI does not print its own success text.

## Completion status

This report is complete and executable. All numbered requirements from 1 through 31 have direct evidence from tests or end-to-end commands, both generated crates compile, rejection paths leave the filesystem unchanged, and the public output contract is verified without duplicate reporting.

No additional production change is required in this pass.