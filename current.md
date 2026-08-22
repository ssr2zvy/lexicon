# Verification report: `lexicon source new --protocol http`

## Status

The full verification loop is complete. The remaining review findings were addressed with targeted tests and the actual command evidence below.

## Files changed

- [current.md](current.md)
- [lexicon-cli/src/cli/mod.rs](lexicon-cli/src/cli/mod.rs)
- [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs)

## Production correction

No broad redesign was needed. The remaining verification gaps were closed by:

- adding a staging finalization helper that cleans up the TempDir when rename fails,
- adding an explicit check that `discovery.md` contains all required prompts,
- adding the correct preexisting-directory sequence test, where the unrelated folder is created before the source creation attempt,
- asserting a single `[lexicon] ERROR:` line on unsupported-protocol failure.

## Full verification command

```bash
cd /workspaces/lexicon && cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

Fresh result:

- `lexicon-cli`: 16 passed, 0 failed
- `lexicon-framework` core: 1 passed, 0 failed
- `lexicon-framework` bin tests: 10 passed, 0 failed
- doc tests: 0 failed

## Happy-path verification

```bash
verification_root="$(mktemp -d)"
repo_root="$(git rev-parse --show-toplevel)"
cli_binary="$repo_root/target/debug/lexicon-cli"
framework_binary="$repo_root/target/debug/lexicon-framework"
"$cli_binary" init "$verification_root" demo-project
cd "$verification_root/demo-project"
LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new example-source --protocol http
```

Observed output:

```text
[lexicon] Created source 'example-source' at /tmp/.../demo-project/sources/example-source
[lexicon] Files to edit next:
[lexicon]   - /tmp/.../demo-project/sources/example-source/source.toml
[lexicon]   - /tmp/.../demo-project/sources/example-source/discovery.md
[lexicon]   - /tmp/.../demo-project/sources/example-source/get-raw-data/get_raw_data_impl/src/main.rs
[lexicon]   - /tmp/.../demo-project/sources/example-source/process-data/process_data_impl/src/main.rs
```

Filesystem checks passed:

```bash
test -d sources/example-source
test -f sources/example-source/source.toml
test -f sources/example-source/discovery.md
test -f sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml
test -f sources/example-source/get-raw-data/get_raw_data_impl/src/main.rs
test -f sources/example-source/process-data/process_data_impl/Cargo.toml
test -f sources/example-source/process-data/process_data_impl/src/main.rs
```

## Rejection verification with exact exit codes

### Missing required `--protocol`

```bash
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new missing-protocol-source
missing_status=$?
set -e
```

Fresh result:

- `missing_status=2`
- this is the Clap parser failure before any filesystem mutation
- no source directory was created

### Unsupported protocol

```bash
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new unsupported-source --protocol browser
unsupported_status=$?
set -e
```

Fresh result:

- `unsupported_status=1`
- exact combined stderr contains exactly one `[lexicon] ERROR:` line
- the source directory was not created

### Outside a Lexicon project

```bash
outside_root="$(mktemp -d)"
cd "$outside_root"
LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new outside-source --protocol http
```

Fresh result:

- `outside_status=1`
- exact error: `[lexicon] ERROR: No Lexicon project found...`
- no directory was created

### Existing source directory

```bash
printf 'preserve-me\n' > sources/example-source/existing-sentinel.txt
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" "$cli_binary" source new example-source --protocol http
existing_status=$?
set -e
```

Fresh result:

- `existing_status=1`
- sentinel content remains `preserve-me`
- no overwrite occurred

## Atomic staging verification

The regression `finalize_source_staging_cleans_up_tempdir_when_rename_fails` passed. It creates a staged temp directory, forces an existing final destination, then asserts:

- the staging path is removed on failure,
- the existing destination content remains intact,
- the TempDir is cleaned up correctly.

## Unrelated directory protection

The regression `unrelated_preexisting_directory_remains_untouched` passed. It creates the preexisting directory before the source creation attempt and verifies the directory still contains `keep-me` afterward.

## Discovery markdown assertion

The regression `generated_discovery_markdown_contains_required_prompts` passed. It asserts that all required prompts are present in the generated markdown, including:

- `## Source description`
- `## Discovery method`
- `## Acquisition endpoint or location`
- `## Why HTTP is the correct acquisition protocol`
- `## Required authentication or access conditions`
- `## Attribution and usage notes`
- `## Operational observations`

## Output contract verification

The regression `unsupported_protocol_reports_single_lexicon_error_line` passed and verifies:

- the return code is `1`,
- there is exactly one `[lexicon] ERROR:` line in the combined stderr/stdout,
- success output is not emitted on failure.

## Generated crate compilation

```bash
cargo check --manifest-path sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml
cargo check --manifest-path sources/example-source/process-data/process_data_impl/Cargo.toml
```

Fresh result:

- `get_raw_data_impl`: exit 0
- `process_data_impl`: exit 0

## Completion

This closes the remaining review points: staging cleanup is tested, the unrelated-directory sequence is correct, the exact exit codes are reported, the `discovery.md` prompt set is asserted, and the error-output contract is proven to emit a single `[lexicon] ERROR:` line on failure.
