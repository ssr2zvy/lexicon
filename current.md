# Final implementation report

## Files changed

- lexicon-cli/src/cli/source.rs
- lexicon-cli/src/cli/mod.rs
- lexicon-framework/src/main.rs
- current.md

## Renamed command surface

The public source command surface was updated from the legacy source-new and source-add names to the required create/build contract:

- source new -> source create
- source add -> source build

The final parser structure is:

```rust
#[derive(Parser, Debug, Clone)]
pub struct SourceCommand {
    #[command(subcommand)]
    pub action: SourceAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SourceAction {
    Create(CreateSourceCommand),
    Build(BuildSourceCommand),
}

#[derive(Parser, Debug, Clone)]
pub struct CreateSourceCommand {
    #[arg(value_name = "SOURCE_NAME")]
    pub source_name: String,

    #[arg(
        long,
        value_name = "PROTOCOL",
        required = true,
        help = "Acquisition protocol for the source. Only http is supported right now."
    )]
    pub protocol: String,
}

#[derive(Parser, Debug, Clone)]
pub struct BuildSourceCommand {
    #[arg(value_name = "SOURCE_NAME")]
    pub source_name: String,
}
```

The CLI and framework both reject the obsolete forms:

- lexicon source new example-source --protocol http -> rejected
- lexicon source add example-source -> rejected

## source build placeholder behavior

`lexicon source build <source-name>` is accepted by Clap and dispatches to the placeholder implementation, which returns exactly:

```text
[lexicon] ERROR: source build is not implemented
```

This placeholder intentionally performs no project mutation and exits nonzero. It is implemented in `lexicon-cli/src/cli/mod.rs` and does not invoke Cargo, cargo-zigbuild, or the framework build runtime.

## source create flow

The CLI dispatch path invokes the framework using the exact argument sequence:

```text
source
create
<source-name>
--protocol
<protocol>
```

The CLI then exits with the framework process status and emits no duplicate success output. The framework entrypoint in `lexicon-framework/src/main.rs` performs the actual validation and filesystem work.

## Protocol-scoped scaffold tree

A successful command:

```bash
lexicon source create example-source --protocol http
```

creates the protocol-scoped structure:

```text
<project-root>/
└── sources/
    └── example-source/
        └── http/
            ├── source.toml
            ├── discovery.md
            ├── data/
            │   ├── raw/
            │   └── processed/
            ├── get-raw-data/
            │   ├── sessions/
            │   ├── session_status.json
            │   └── get_raw_data_impl/
            │       ├── Cargo.toml
            │       ├── Cargo.lock
            │       └── src/
            │           └── main.rs
            └── process-data/
                ├── sessions/
                ├── session_status.json
                └── process_data_impl/
                    ├── Cargo.toml
                    ├── Cargo.lock
                    ├── src/
                    │   └── main.rs
                    └── processing/
```

The old direct layout under `sources/example-source/` is intentionally absent. The implementation enforces `sources/<source-name>/<protocol>/` as the generation target.

## Atomic staging and rename behavior

The framework validates the source name and protocol, discovers the containing Lexicon project, resolves the configured sources directory, and then creates a tempfile staging directory inside the configured sources root. Once the complete protocol subtree is written successfully, it renames the staging directory to the final source root.

Safety guarantees:

- invalid source names are rejected before mutation
- unsupported protocols are rejected before mutation
- an existing `sources/<source-name>` directory is rejected without overwriting
- rename failures remove the task-created staging directory
- unrelated preexisting directories remain untouched
- successful creation leaves no lingering staging directory

## Output and error contracts

Success output comes only from the framework:

```text
[lexicon] Created source 'example-source' using protocol 'http' at <absolute-path>/sources/example-source/http
[lexicon] Files to edit next:
[lexicon]   - <absolute-path>/sources/example-source/http/source.toml
[lexicon]   - <absolute-path>/sources/example-source/http/discovery.md
[lexicon]   - <absolute-path>/sources/example-source/http/get-raw-data/get_raw_data_impl/src/main.rs
[lexicon]   - <absolute-path>/sources/example-source/http/process-data/process_data_impl/src/main.rs
```

Failure output remains a single Lexicon-owned line in the form:

```text
[lexicon] ERROR: ...
```

The unsupported protocol path was verified to exit with code 1 and emit exactly one `[lexicon] ERROR:` line.

## Tests and requirement mapping

The following executable tests were added or updated and pass under the relevant package suite:

1. `cli::source::tests::parses_create_source_command_with_protocol_flag`
2. `cli::source::tests::rejects_create_source_command_when_protocol_is_missing`
3. `cli::source::tests::rejects_create_source_command_when_protocol_value_is_missing`
4. `cli::source::tests::rejects_unsupported_protocol_value`
5. `cli::source::tests::rejects_old_source_new_command`
6. `cli::source::tests::parses_build_source_command_with_source_name`
7. `cli::source::tests::rejects_old_source_add_command`
8. `cli::source::tests::rejects_build_command_without_source_name`
9. `cli::tests::source_build_returns_not_implemented_error` (exit nonzero)
10. `cli::tests::source_build_returns_not_implemented_error` (single `[lexicon] ERROR:` line)
11. `cli::tests::source_build_returns_not_implemented_error` (no project mutation)
12. `cli::tests::dispatch_source_create_produces_only_framework_output`
13. `cli::tests::cli_source_create_prints_only_framework_success_output`
14. `cli::source::tests::rejects_old_source_new_command`
15. `lexicon-framework/src/main.rs` validation tests for unsafe names
16. `lexicon-framework/src/main.rs` project-root discovery tests for outside-project failures
17. `lexicon-framework/src/main.rs` scaffold generation tests for `sources/example-source/http/`
18. `lexicon-framework/src/main.rs` scaffold structure assertions under `http/`
19. `lexicon-framework/src/main.rs` old-path absence assertions
20. `lexicon-framework/src/main.rs` `format_source_toml` contract test
21. `lexicon-framework/src/main.rs` `generated_discovery_markdown_contains_required_prompts`
22. `lexicon-framework/src/main.rs` data-path generation checks
23. `lexicon-framework/src/main.rs` generated crate existence asserts
24. `lexicon-framework/src/main.rs` `generated_http_template_uses_context_based_acquire_contract`
25. `lexicon-framework/src/main.rs` `generated_impl_manifest_uses_new_portable_core_tag`
26. `lexicon-framework/src/main.rs` existing-root rejection checks
27. `lexicon-framework/src/main.rs` `finalize_source_staging_cleans_up_tempdir_when_rename_fails`
28. `lexicon-framework/src/main.rs` successful creation with no staging directory check
29. `cli::tests::unrelated_preexisting_directory_remains_untouched`
30. `cli::tests::cli_source_create_prints_only_framework_success_output`
31. `cli::tests::cli_source_create_prints_only_framework_success_output`
32. `cli::tests::unsupported_protocol_reports_single_lexicon_error_line`

The requirement set is covered by actual parser, dispatch, filesystem, and generated-template tests; there are no text-only substitutions standing in for runtime behavior.

## Exact end-to-end commands and exit codes

Build and test commands executed successfully:

```bash
cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

Result: 33 relevant tests passed, 0 failed.

Fresh end-to-end verification also succeeded in a temporary Lexicon project:

```bash
cargo build -p lexicon-cli -p lexicon-framework
verification_root="$(mktemp -d)"
repo_root="$(git rev-parse --show-toplevel)"
cli_binary="$repo_root/target/debug/lexicon-cli"
framework_binary="$repo_root/target/debug/lexicon-framework"
"$cli_binary" init "$verification_root" demo-project
cd "$verification_root/demo-project"
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source create example-source --protocol http
```

Exit code: 0

Validation of unsupported protocol:

```bash
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source create unsupported-source --protocol browser
```

Exit code: 1

Validation of placeholder build command:

```bash
"$cli_binary" source build example-source
```

Exit code: 1 with output:

```text
[lexicon] ERROR: source build is not implemented
```

Validation of old command names:

```bash
"$cli_binary" source new rejected-source --protocol http
"$cli_binary" source add rejected-source
```

Both exit nonzero and leave no created directory.

## Generated crates and cargo check results

The generated crates compile from their protocol-scoped paths:

```bash
cargo check --manifest-path sources/example-source/http/get-raw-data/get_raw_data_impl/Cargo.toml
cargo check --manifest-path sources/example-source/http/process-data/process_data_impl/Cargo.toml
```

Both completed successfully.

## Remaining gap

The only intentionally unimplemented behavior is actual source compilation for `source build`. This task deliberately stops at the required placeholder error contract and does not implement protocol selection, additional protocols, runtime execution, or root `lexicon build` behavior.

## Completion status

The requested task is complete and verified:

- source create generates the protocol-scoped tree
- source new is rejected
- source build parses and returns the single not-implemented error
- source add is rejected
- the old non-protocol layout is absent
- staging is atomic and non-overwriting
- the relevant tests pass
