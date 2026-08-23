# Current task: rename source commands and introduce protocol-scoped source layouts

## Objective

Make two command-surface terminology changes:

```text
source new → source create
source add → source build
```

The resulting lifecycle terminology is:

```text
lexicon source create <source-name> --protocol <protocol>
lexicon source build <source-name>
lexicon build
```

Meanings:

- `source create` generates the editable scaffold for one source.
- `source build` will compile one source’s implementation crates.
- Root `lexicon build` will compile every discovered source.

This task fully implements the `source create` rename and protocol-scoped scaffold.

This task establishes the `source build` command name but does not implement source compilation yet. If invoked, it must return a clear not-implemented error without filesystem mutation.

The only supported acquisition protocol remains:

```text
http
```

## Final public command surface

The following command must succeed:

```bash
lexicon source create example-source --protocol http
```

The following command must parse as the future single-source build command:

```bash
lexicon source build example-source
```

Until compilation is implemented, executing it must fail clearly:

```text
[lexicon] ERROR: source build is not implemented
```

The following obsolete commands must be rejected by Clap:

```bash
lexicon source new example-source --protocol http
lexicon source add example-source
```

Do not retain `new` or `add` as aliases.

## Required terminology changes

Search tracked source code, tests, help text, templates, and active documentation for:

```text
source new
SourceAction::New
NewSourceCommand
source add
SourceAction::Add
AddSourceCommand
```

Replace active command terminology with:

```text
source create
SourceAction::Create
CreateSourceCommand
source build
SourceAction::Build
BuildSourceCommand
```

Do not rewrite Git history or historical archived reports.

Do not confuse the source-specific build action with the existing root command:

```bash
lexicon build
```

They are separate commands:

```bash
lexicon source build example-source
lexicon build
```

## Required CLI types

The resulting CLI structure should be equivalent to:

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

Adapt this to the repository’s existing Clap derive structure without changing the required public syntax.

`CreateSourceCommand.protocol` must:

- Be required.
- Have no default.
- Require a value.
- Currently accept only `http` through the established validation flow.

## Required `source build` placeholder

`lexicon source build <source-name>` must parse successfully.

Because compilation is outside this task, dispatch must return the established error form:

```text
[lexicon] ERROR: source build is not implemented
```

Required behavior:

1. Validate that `<source-name>` is present through Clap.
2. Do not invoke Cargo.
3. Do not invoke cargo-zigbuild.
4. Do not invoke the framework build runtime.
5. Do not create, remove, or modify project files.
6. Exit nonzero.
7. Emit exactly one Lexicon-owned error line.

Do not implement compilation, trait verification, executable placement, target selection, or runtime registration in this task.

## Required `source create` CLI flow

The public execution path must be:

```text
lexicon CLI
→ parse `source create`
→ require source name
→ require `--protocol`
→ invoke lexicon-framework
→ framework parses `source create`
→ framework validates source name
→ framework validates protocol
→ framework discovers the containing Lexicon project
→ framework resolves sources_directory
→ framework creates the protocol-scoped scaffold atomically
→ framework emits the only success output
→ CLI exits without duplicate output
```

The CLI must invoke the framework with these exact argument components:

```text
source
create
<source-name>
--protocol
<protocol>
```

The framework’s direct interface must also use `source create`.

No active CLI or framework dispatch path may continue using `source new`.

## Required protocol-scoped structure

For:

```bash
lexicon source create example-source --protocol http
```

generate:

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

The selected protocol is a directory directly beneath the source name:

```text
sources/<source-name>/<protocol>/
```

For the current protocol:

```text
sources/example-source/http/
```

## Old layout must not be generated

Do not generate these paths:

```text
sources/example-source/source.toml
sources/example-source/discovery.md
sources/example-source/data/
sources/example-source/get-raw-data/
sources/example-source/process-data/
```

All existing source-scaffold path construction must be updated to include the protocol directory.

## Current multiple-protocol boundary

The directory structure intentionally permits future source layouts such as:

```text
sources/example-source/
├── http/
└── <future-protocol>/
```

This task does not implement adding another protocol to an existing source.

Current behavior must be:

1. If `sources/<source-name>/` does not exist, create it atomically with the requested protocol subtree.
2. If `sources/<source-name>/` already exists, reject the operation without modifying it.
3. Do not merge a new protocol into an existing source.
4. Do not overwrite an existing protocol.
5. Do not silently select an existing protocol.
6. Do not implement protocol fallback.

A future task will define how another protocol is added to an existing source.

## Metadata placement

Generate:

```text
sources/<source-name>/<protocol>/source.toml
```

For the HTTP example:

```text
sources/example-source/http/source.toml
```

Its contents must be:

```toml
schema_version = 1

[source]
name = "example-source"
protocol = "http"
```

Continue using TOML serialization rather than constructing TOML through direct string interpolation.

Do not create another `source.toml` at the source-name root.

## Discovery documentation placement

Generate:

```text
sources/<source-name>/<protocol>/discovery.md
```

For HTTP:

```text
sources/example-source/http/discovery.md
```

Preserve these verified sections:

```text
## Source description
## Discovery method
## Acquisition endpoint or location
## Why HTTP is the correct acquisition protocol
## Required authentication or access conditions
## Attribution and usage notes
## Operational observations
```

Discovery documentation is protocol-specific because it records how that protocol locates and acquires the source.

## Data and session paths

Move all generated data and session paths beneath the protocol directory.

For HTTP:

```text
sources/example-source/http/data/raw/
sources/example-source/http/data/processed/
sources/example-source/http/get-raw-data/sessions/
sources/example-source/http/get-raw-data/session_status.json
sources/example-source/http/process-data/sessions/
sources/example-source/http/process-data/session_status.json
```

Update generated code, path construction, and tests that currently assume:

```text
sources/<source-name>/data/
```

The protocol-specific source root used by the generated HTTP runtime must be:

```text
sources/<source-name>/<protocol>/
```

`HttpAcquisitionContext` must consequently derive its raw-data directory as:

```text
sources/<source-name>/http/data/raw/
```

Do not change the `HttpAcquisition` trait signature.

## Generated implementation crates

Generate the HTTP acquisition crate at:

```text
sources/<source-name>/http/get-raw-data/get_raw_data_impl/
```

Generate the process-data crate at:

```text
sources/<source-name>/http/process-data/process_data_impl/
```

Preserve:

- The context-based `HttpAcquisition::acquire` template.
- The call to `run_http_source`.
- The portable tagged Core dependency.
- The absence of machine-local repository paths.
- The separation between acquisition and processing.
- The ability of both generated crates to pass `cargo check`.

Do not implement actual acquisition or processing behavior.

## Atomic creation boundary

The complete source-name directory must be created atomically.

The final destination is:

```text
<sources_directory>/<source-name>
```

The staging directory must be created inside the configured sources directory.

The staged structure must be:

```text
<staging-directory>/
└── <protocol>/
    └── <complete protocol scaffold>
```

Only after the entire protocol subtree has been written successfully may the staged source root be renamed to:

```text
<sources_directory>/<source-name>
```

Required sequence:

1. Validate the source name.
2. Validate the protocol.
3. Locate the Lexicon project.
4. Resolve the configured sources directory.
5. Reject an existing source-name directory.
6. Create a unique tempfile-managed staging directory.
7. Generate the entire `<protocol>/...` tree inside staging.
8. Rename the staging directory to the final source-name directory.
9. Clean up the task-created staging directory on any failure.
10. Preserve unrelated preexisting directories.
11. Leave no staging directory after success.

Preserve the existing tested staging-finalization helper where practical.

## Output contract

Successful output must identify both the source and protocol:

```text
[lexicon] Created source 'example-source' using protocol 'http' at <absolute-project-path>/sources/example-source/http
[lexicon] Files to edit next:
[lexicon]   - <absolute-project-path>/sources/example-source/http/source.toml
[lexicon]   - <absolute-project-path>/sources/example-source/http/discovery.md
[lexicon]   - <absolute-project-path>/sources/example-source/http/get-raw-data/get_raw_data_impl/src/main.rs
[lexicon]   - <absolute-project-path>/sources/example-source/http/process-data/process_data_impl/src/main.rs
```

The framework remains the sole producer of successful source-creation output.

The CLI must not print a duplicate success message.

Every Lexicon-owned human-readable line must begin with `[lexicon]`.

Failure paths must preserve the existing single-error behavior.

The unimplemented build command must produce exactly one error line equivalent to:

```text
[lexicon] ERROR: source build is not implemented
```

## Required parser and command tests

Add or update executable tests proving:

1. `lexicon source create example-source --protocol http` parses successfully.
2. Missing `--protocol` is rejected.
3. Missing protocol value is rejected.
4. Unsupported protocol is rejected before filesystem mutation.
5. `lexicon source new ...` is rejected.
6. `lexicon source build example-source` parses successfully.
7. `lexicon source add example-source` is rejected.
8. Missing source name for `source build` is rejected by Clap.
9. Executing `source build` exits nonzero.
10. Executing `source build` emits exactly one `[lexicon] ERROR:` line.
11. Executing `source build` does not mutate the project.
12. The CLI forwards `source create`, source name, and protocol unchanged.
13. The framework directly accepts `source create`.
14. The framework rejects `source new`.

## Required scaffold tests

Add or update executable tests proving:

15. Unsafe source names are rejected before mutation.
16. Running outside a Lexicon project fails without mutation.
17. A valid command generates `sources/example-source/http/`.
18. The complete required structure exists under `http/`.
19. No old scaffold files exist directly under `sources/example-source/`.
20. `source.toml` exists beneath `http/` and contains the correct values.
21. `discovery.md` exists beneath `http/` with every required prompt.
22. Raw and processed data directories exist beneath `http/`.
23. Both implementation crates exist beneath `http/`.
24. The generated HTTP crate retains the context-based acquisition contract.
25. Generated manifests remain portable.
26. Existing source roots are rejected without modification.
27. Failed creation removes task-created staging.
28. Successful creation leaves no staging directory.
29. Unrelated preexisting directories remain unchanged.
30. The public success output contains the protocol-scoped path.
31. Public output contains no duplicate success lines.
32. Failure output contains exactly one `[lexicon] ERROR:` line.

Tests must execute actual behavior wherever practical.

Do not replace executable parser, dispatch, filesystem, or process tests with source-text searches.

Source-text assertions remain acceptable for generated template contents.

## Required generated-crate compilation

Both generated crates must compile from their new protocol-scoped paths:

```bash
cargo check --manifest-path \
    sources/example-source/http/get-raw-data/get_raw_data_impl/Cargo.toml
```

```bash
cargo check --manifest-path \
    sources/example-source/http/process-data/process_data_impl/Cargo.toml
```

Report the two results separately.

## Required end-to-end verification

Build the real binaries:

```bash
cargo build -p lexicon-cli -p lexicon-framework
```

Create a fresh Lexicon project:

```bash
verification_root="$(mktemp -d)"
repo_root="$(git rev-parse --show-toplevel)"
cli_binary="$repo_root/target/debug/lexicon-cli"
framework_binary="$repo_root/target/debug/lexicon-framework"

"$cli_binary" init "$verification_root" demo-project
cd "$verification_root/demo-project"
```

Create an HTTP source:

```bash
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source create example-source --protocol http
```

Verify the protocol-scoped layout:

```bash
test -d sources/example-source/http
test -f sources/example-source/http/source.toml
test -f sources/example-source/http/discovery.md
test -d sources/example-source/http/data/raw
test -d sources/example-source/http/data/processed
test -f sources/example-source/http/get-raw-data/get_raw_data_impl/Cargo.toml
test -f sources/example-source/http/get-raw-data/get_raw_data_impl/src/main.rs
test -f sources/example-source/http/process-data/process_data_impl/Cargo.toml
test -f sources/example-source/http/process-data/process_data_impl/src/main.rs
```

Verify that the old layout is absent:

```bash
test ! -f sources/example-source/source.toml
test ! -f sources/example-source/discovery.md
test ! -d sources/example-source/data
test ! -d sources/example-source/get-raw-data
test ! -d sources/example-source/process-data
```

Compile both generated crates:

```bash
cargo check --manifest-path \
    sources/example-source/http/get-raw-data/get_raw_data_impl/Cargo.toml

cargo check --manifest-path \
    sources/example-source/http/process-data/process_data_impl/Cargo.toml
```

Verify that `source new` is gone:

```bash
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source new rejected-source --protocol http
source_new_status=$?
set -e

test "$source_new_status" -ne 0
test ! -e sources/rejected-source
```

Verify that `source add` is gone:

```bash
set +e
"$cli_binary" source add rejected-source
source_add_status=$?
set -e

test "$source_add_status" -ne 0
test ! -e sources/rejected-source
```

Verify missing protocol:

```bash
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source create missing-protocol-source
missing_protocol_status=$?
set -e

test "$missing_protocol_status" -ne 0
test ! -e sources/missing-protocol-source
```

Verify unsupported protocol:

```bash
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source create unsupported-source --protocol browser
unsupported_protocol_status=$?
set -e

test "$unsupported_protocol_status" -ne 0
test ! -e sources/unsupported-source
```

Verify the future build command name:

```bash
set +e
"$cli_binary" source build example-source
source_build_status=$?
set -e

test "$source_build_status" -ne 0
test -d sources/example-source/http
```

The build command must fail only because compilation is not implemented. It must not modify the source.

Run the complete relevant test suite:

```bash
cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

## Scope exclusions

Do not implement:

- Actual `source build` compilation behavior.
- A second acquisition protocol.
- Adding a protocol to an existing source.
- Protocol selection or fallback.
- Actual HTTP requests.
- Raw transaction recording.
- SQLite processing.
- Root `lexicon build` behavior.
- Runtime source-executable launching.
- MZA changes.
- Bundle changes.
- Installer changes.
- Migration of previously generated source directories.

## Required final report

After implementation and verification, replace `current.md` completely with a clean function-level report containing:

- Every file changed, including `current.md`.
- Every renamed type, variant, function, test, and command.
- The final `source create` parser definition.
- The final `source build` placeholder parser and dispatch behavior.
- Confirmation that `source new` is rejected.
- Confirmation that `source add` is rejected.
- The exact CLI-to-framework `source create` argument flow.
- The final protocol-scoped directory tree.
- Confirmation that the old layout is absent.
- Atomic staging and rename behavior.
- The final output and error contracts.
- Test names mapped to all 32 requirements.
- Exact end-to-end commands and exit codes.
- Separate `cargo check` results for both generated crates.
- Full package-specific test totals.
- Any remaining gap or blocker.

Do not append the old task text after the report.

Do not claim completion unless:

- `source create` generates the protocol-scoped tree.
- `source new` is rejected.
- `source build` parses and returns the single not-implemented error.
- `source add` is rejected.
- The old non-protocol layout is absent.
- Both generated crates compile from their new paths.
- Atomicity and no-overwrite behavior remain verified.
- All relevant tests pass.