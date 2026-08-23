# Current task: implement `lexicon source build`

## Objective

Implement the single-source native build command:

```bash
lexicon source build <source-name> --protocol <protocol>
```

Example:

```bash
lexicon source build example-source --protocol http
```

This command compiles the source’s get-raw-data and process-data implementation crates for the machine currently running Lexicon, then publishes the resulting executables into the corresponding runtime directories.

The only currently supported protocol remains:

```text
http
```

## Required lifecycle terminology

The public source lifecycle is:

```text
lexicon source create <source-name> --protocol <protocol>
lexicon source build <source-name> --protocol <protocol>
```

Meanings:

- `source create` generates the editable source scaffold.
- The developer edits the generated implementation crates.
- `source build` compiles both implementation crates and publishes their native executables.

The obsolete command must remain rejected:

```bash
lexicon source add <source-name>
```

Do not retain `add` as an alias.

Root:

```bash
lexicon build
```

remains a separate future command for building every discovered source. Do not implement it in this task.

## Required source structure

Implement `source build` against this protocol-scoped structure:

```text
sources/
└── <source-name>/
    └── <protocol>/
        ├── source.toml
        ├── discovery.md
        ├── data/
        │   ├── raw/
        │   └── processed/
        ├── get-raw-data/
        │   ├── sessions/
        │   ├── session_status.json
        │   ├── get-raw-data-impl/
        │   │   ├── Cargo.toml
        │   │   ├── Cargo.lock
        │   │   └── src/
        │   │       └── main.rs
        │   └── runtime/
        │       └── .gitignore
        └── process-data/
            ├── sessions/
            ├── session_status.json
            ├── process-data-impl/
            │   ├── Cargo.toml
            │   ├── Cargo.lock
            │   ├── src/
            │   │   └── main.rs
            │   └── processing/
            └── runtime/
                └── .gitignore
```

The implementation crates and runtime directories are siblings:

```text
get-raw-data/
├── get-raw-data-impl/
└── runtime/
```

```text
process-data/
├── process-data-impl/
└── runtime/
```

A `runtime/` directory is not a Cargo crate. It is the final local output directory for the compiled executable belonging to that operation.

## Required scaffold reconciliation

Before implementing the build flow, inspect the current `source create` implementation.

If it still generates either old crate name:

```text
get_raw_data_impl
process_data_impl
```

replace them with:

```text
get-raw-data-impl
process-data-impl
```

Ensure `source create` generates both sibling runtime directories:

```text
get-raw-data/runtime/
process-data/runtime/
```

Each generated runtime directory must contain:

```text
.gitignore
```

with contents equivalent to:

```gitignore
*
!.gitignore
```

This keeps the runtime directory represented in source control while preventing compiled executables from being committed.

Do not commit generated runtime executables.

## Required CLI parsing

The source-specific build command must require both source name and protocol.

The CLI structure should be equivalent to:

```rust
#[derive(Parser, Debug, Clone)]
pub struct BuildSourceCommand {
    #[arg(value_name = "SOURCE_NAME")]
    pub source_name: String,

    #[arg(
        long,
        value_name = "PROTOCOL",
        required = true,
        help = "Protocol implementation to build. Only http is supported right now."
    )]
    pub protocol: String,
}
```

The public syntax is:

```bash
lexicon source build <source-name> --protocol <protocol>
```

These must be rejected by Clap:

```bash
lexicon source build
lexicon source build example-source
lexicon source build example-source --protocol
```

This must be rejected by protocol validation before Cargo runs:

```bash
lexicon source build example-source --protocol browser
```

## Required CLI-to-framework flow

The real execution path must be:

```text
lexicon CLI
→ parse `source build`
→ require source name and protocol
→ invoke lexicon-framework
→ framework parses `source build`
→ framework locates the Lexicon project
→ framework resolves sources_directory
→ framework validates source and protocol metadata
→ framework resolves both implementation manifests
→ framework compiles both crates into temporary build locations
→ framework verifies both executable outputs
→ framework transactionally publishes both executables
→ framework prints the final runtime paths
→ CLI exits without duplicate success output
```

The CLI must invoke the framework with:

```text
source
build
<source-name>
--protocol
<protocol>
```

Remove the existing not-implemented `source build` placeholder.

The framework owns source-build success output.

The CLI must not print another success message after the framework completes.

## Project and source resolution

Starting from the current directory:

1. Locate the containing Lexicon project through the existing `lexicon.toml` discovery logic.
2. Resolve the configured `sources_directory`.
3. Validate `<source-name>` using the existing safe source-name rules.
4. Validate `<protocol>`.
5. Resolve:

```text
<sources_directory>/<source-name>/<protocol>/
```

6. Require that the source root exists and is a directory.
7. Require that the protocol root exists and is a directory.
8. Parse:

```text
<sources_directory>/<source-name>/<protocol>/source.toml
```

9. Require:

```toml
schema_version = 1

[source]
name = "<source-name>"
protocol = "<protocol>"
```

10. Reject a mismatch between command arguments and `source.toml`.
11. Reject symlink or path traversal that resolves outside the configured project sources directory.

For HTTP, resolve these exact manifests:

```text
<sources/<source-name>/http/get-raw-data/get-raw-data-impl/Cargo.toml
sources/<source-name>/http/process-data/process-data-impl/Cargo.toml
```

Missing or invalid manifests must fail before runtime publication.

## Native-target scope

`source build` builds for the current machine only.

Use Cargo’s native default target. Do not pass a cross-compilation `--target` value.

Use:

```bash
cargo build --release --locked
```

for each implementation crate.

Every Cargo invocation must include:

```text
--release
--locked
--manifest-path
--message-format=json-render-diagnostics
```

Do not use cargo-zigbuild in this command.

Do not invoke MZA.

Cross-target release builds remain outside `source build`.

## Cargo availability

If `cargo` cannot be executed, return a clear Lexicon error explaining that building source implementations requires a Rust development toolchain.

Equivalent output:

```text
[lexicon] ERROR: source build requires Cargo and a Rust development toolchain
```

Do not report this as a generic missing-file or framework error.

Cargo and compiler diagnostics may pass through directly. The final Lexicon-owned failure summary must use the `[lexicon] ERROR:` prefix exactly once.

## Locked builds

Each generated implementation crate is required to have its own committed:

```text
Cargo.lock
```

Run both builds with `--locked`.

A missing or stale lockfile must cause the build to fail.

Do not silently regenerate or update a lockfile during `source build`.

Do not retry without `--locked`.

## Temporary build directories

Do not rely on an assumed Cargo output path such as:

```text
target/release/<package-name>
```

Build each crate with an isolated temporary target directory.

Equivalent command shape:

```bash
cargo build \
    --release \
    --locked \
    --manifest-path <absolute-manifest-path> \
    --target-dir <temporary-target-directory> \
    --message-format=json-render-diagnostics
```

Use uniquely scoped temporary directories so concurrent source builds cannot collide.

Temporary build output must not be created inside:

```text
runtime/
```

The runtime directories contain only published final executables and their `.gitignore` files.

Temporary Cargo build directories must be cleaned after success or failure.

## Executable discovery

Determine the produced executable from Cargo’s JSON messages.

Parse `compiler-artifact` messages and use the `executable` field belonging to the requested package’s binary target.

Do not construct the executable path by assuming:

```text
target/release/<name>
```

For each implementation crate:

1. Resolve its Cargo package and targets.
2. Require exactly one applicable binary target.
3. Run Cargo with JSON messages.
4. locate exactly one executable artifact belonging to that binary target.
5. Require the executable path to exist.
6. Require it to be a regular file.
7. Reject missing or ambiguous executable results.

Do not treat libraries, build scripts, examples, tests, or dependency artifacts as the runtime executable.

## Build order and publication boundary

Compile both implementation crates before changing either runtime directory.

Required order:

```text
resolve and validate both manifests
→ build get-raw-data implementation
→ verify get-raw-data executable
→ build process-data implementation
→ verify process-data executable
→ stage both runtime executables
→ publish both runtime executables
```

If either Cargo build fails:

- Return a nonzero status.
- Do not replace either existing runtime executable.
- Preserve both previously working runtime executables exactly.
- Clean all task-created temporary files.

A successful first build followed by a failed second build must not update the first runtime.

## Final runtime filenames

Preserve the binary target filename reported by Cargo.

Publish the acquisition executable at:

```text
sources/<source-name>/<protocol>/get-raw-data/runtime/<cargo-binary-name>
```

Publish the processing executable at:

```text
sources/<source-name>/<protocol>/process-data/runtime/<cargo-binary-name>
```

On Windows, preserve Cargo’s `.exe` extension.

Because each operation owns a separate runtime directory, the two binary targets may use different names without collision.

Do not rename `.exe` files to extensionless names.

## Runtime staging

After both builds succeed:

1. Create a staged runtime file beside each final runtime output.
2. Copy the verified Cargo executable into that staged file.
3. Preserve executable permissions.
4. Verify the staged output is a regular file.
5. Only then replace the corresponding final runtime executable.
6. Remove task-created staged files after any failure.
7. Preserve each runtime directory’s `.gitignore`.

Use unique staged filenames. Do not use a predictable PID-only name.

## Transactional publication

Publishing two executables requires rollback protection.

Before replacing existing runtime executables:

1. Record whether each final executable already exists.
2. Move each existing executable to a unique temporary backup in the same runtime directory.
3. Move the staged get-raw-data executable into its final path.
4. Move the staged process-data executable into its final path.
5. If every move succeeds, delete the temporary backups.
6. If any move fails:
   - remove any newly published executable from this attempt,
   - restore every previous executable from its backup,
   - remove remaining staged files,
   - return an error.

Never leave one newly built runtime paired with one previous runtime after a publication failure.

Never delete a previous runtime executable until both new executables have been compiled and staged successfully.

Runtime publication must use same-filesystem renames within each runtime directory where possible.

## Successful output

After both executables have been published successfully, print:

```text
[lexicon] Built source 'example-source' using protocol 'http'
[lexicon] Runtime executables:
[lexicon]   - <absolute-path>/sources/example-source/http/get-raw-data/runtime/<get-binary>
[lexicon]   - <absolute-path>/sources/example-source/http/process-data/runtime/<process-binary>
```

Print success only after both runtime executables are finalized.

Do not print partial success after the first build.

The CLI must not print duplicate success output.

## Failure behavior

Failures must:

- Exit nonzero.
- Preserve existing runtime executables when either compilation fails.
- Clean task-created build, staging, and backup files.
- Produce one final Lexicon-owned error line.
- Never report a partial source build as successful.

Examples include:

```text
[lexicon] ERROR: source 'example-source' does not exist
[lexicon] ERROR: protocol 'http' does not exist for source 'example-source'
[lexicon] ERROR: source metadata does not match the requested source and protocol
[lexicon] ERROR: get-raw-data implementation build failed
[lexicon] ERROR: process-data implementation build failed
[lexicon] ERROR: source runtime publication failed; previous runtimes were restored
```

Cargo diagnostics may appear before the final Lexicon error and do not require the `[lexicon]` prefix.

## Required parser tests

Add executable tests proving:

1. `lexicon source build example-source --protocol http` parses successfully.
2. Missing source name is rejected.
3. Missing `--protocol` is rejected.
4. Missing protocol value is rejected.
5. Unsupported protocol is rejected before Cargo runs.
6. `lexicon source add example-source` is rejected.
7. Parsed source name and protocol are forwarded unchanged to the framework.
8. The framework directly accepts `source build`.

## Required resolution tests

Add tests proving:

9. Running outside a Lexicon project fails without mutation.
10. A missing source is rejected.
11. A missing protocol directory is rejected.
12. A missing `source.toml` is rejected.
13. A mismatched source name in `source.toml` is rejected.
14. A mismatched protocol in `source.toml` is rejected.
15. An unsupported schema version is rejected.
16. A missing get-raw-data manifest is rejected.
17. A missing process-data manifest is rejected.
18. Symlink or traversal escapes are rejected.
19. No Cargo process starts when validation fails.

## Required build tests

Add tests proving:

20. Both Cargo commands include `--release`.
21. Both Cargo commands include `--locked`.
22. Both Cargo commands use their exact manifest paths.
23. Both Cargo commands use isolated temporary target directories.
24. Cargo JSON executable discovery selects the binary target executable.
25. Dependency, test, example, library, and build-script artifacts are ignored.
26. Missing executable output is rejected.
27. Ambiguous binary executable output is rejected.
28. A missing Cargo executable produces the dedicated toolchain error.
29. A get-raw-data compilation failure prevents the process build and publication.
30. A process-data compilation failure preserves the previous get and process runtimes.
31. Successful compilation of both crates reaches publication.
32. Temporary Cargo build directories are cleaned after success and failure.

Use controlled fixture crates or a controllable command-runner boundary for failure-path tests. Do not depend on intentionally breaking the developer’s real generated source tree.

## Required publication tests

Add filesystem tests proving:

33. Runtime executables are placed in their corresponding runtime directories.
34. Cargo-reported executable filenames are preserved.
35. Windows `.exe` filenames are preserved by platform-independent path logic.
36. Existing runtime executables are not changed when either build fails.
37. Staged runtime files are cleaned after failure.
38. Backups are deleted after successful publication.
39. A failure while publishing the second executable restores both previous executables.
40. A preexisting unrelated runtime file is not deleted.
41. Each runtime `.gitignore` remains unchanged.
42. Published Unix executables retain executable permissions where Unix permission checks are available.
43. No partial runtime pair is reported as successful.

## Required public-flow tests

Add tests proving:

44. The public CLI reaches the real framework source-build flow.
45. The framework is the sole producer of success output.
46. Successful output contains both final runtime paths.
47. Successful output contains no duplicate lines.
48. A failed build produces exactly one final `[lexicon] ERROR:` line.
49. Root `lexicon build` behavior remains unchanged.
50. `source create` behavior and protocol-scoped scaffold tests continue to pass.

Tests must execute behavior where practical.

Source-text searches are not sufficient substitutes for parser, command invocation, Cargo-message parsing, filesystem, rollback, or public-output tests.

## Required end-to-end verification

Build the real Lexicon binaries:

```bash
cargo build -p lexicon-cli -p lexicon-framework
```

Create a fresh project:

```bash
verification_root="$(mktemp -d)"
repo_root="$(git rev-parse --show-toplevel)"
cli_binary="$repo_root/target/debug/lexicon-cli"
framework_binary="$repo_root/target/debug/lexicon-framework"

"$cli_binary" init "$verification_root" demo-project
cd "$verification_root/demo-project"
```

Create the source:

```bash
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source create example-source --protocol http
```

Confirm the implementation crates and runtime directories exist:

```bash
test -f sources/example-source/http/get-raw-data/get-raw-data-impl/Cargo.toml
test -d sources/example-source/http/get-raw-data/runtime
test -f sources/example-source/http/process-data/process-data-impl/Cargo.toml
test -d sources/example-source/http/process-data/runtime
```

Build the source:

```bash
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source build example-source --protocol http
```

Verify both runtime directories contain one published executable in addition to `.gitignore`.

On Unix:

```bash
get_runtime="sources/example-source/http/get-raw-data/runtime"
process_runtime="sources/example-source/http/process-data/runtime"

get_executable="$(
    find "$get_runtime" -maxdepth 1 -type f ! -name '.gitignore' | head -n 1
)"
process_executable="$(
    find "$process_runtime" -maxdepth 1 -type f ! -name '.gitignore' | head -n 1
)"

test -n "$get_executable"
test -n "$process_executable"
test -f "$get_executable"
test -f "$process_executable"
test -x "$get_executable"
test -x "$process_executable"
```

Verify exactly one runtime executable per operation:

```bash
test "$(
    find "$get_runtime" -maxdepth 1 -type f ! -name '.gitignore' | wc -l
)" -eq 1

test "$(
    find "$process_runtime" -maxdepth 1 -type f ! -name '.gitignore' | wc -l
)" -eq 1
```

Record the initial runtime hashes:

```bash
get_hash_before="$(sha256sum "$get_executable" | awk '{print $1}')"
process_hash_before="$(sha256sum "$process_executable" | awk '{print $1}')"
```

Introduce a temporary process implementation compilation error, preserving the original source so it can be restored:

```bash
process_main="sources/example-source/http/process-data/process-data-impl/src/main.rs"
cp "$process_main" "$process_main.verification-backup"
printf '\nthis_will_not_compile!\n' >> "$process_main"
```

Run the build again:

```bash
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source build example-source --protocol http
failed_build_status=$?
set -e
```

Restore the source immediately:

```bash
mv "$process_main.verification-backup" "$process_main"
```

Verify:

```bash
test "$failed_build_status" -ne 0
test "$(sha256sum "$get_executable" | awk '{print $1}')" = "$get_hash_before"
test "$(sha256sum "$process_executable" | awk '{print $1}')" = "$process_hash_before"
```

Confirm no staged or backup runtime files remain.

Run the full relevant test suite:

```bash
cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

## Scope exclusions

Do not implement:

- Cross-target source builds.
- cargo-zigbuild integration.
- MZA integration.
- Root `lexicon build`.
- Runtime execution through `lexicon data --get`.
- Runtime execution through `lexicon data --process`.
- Actual HTTP acquisition.
- Raw transaction recording.
- SQLite processing.
- Multiple protocols for one existing source.
- Protocol fallback or selection.
- Installation or bundling changes.

## Required final report

After implementation and verification, replace `current.md` completely with a clean function-level report containing:

- Every changed file, including `current.md`.
- Final CLI parser types and variants.
- Exact CLI-to-framework build argument flow.
- Project, source, protocol, and metadata validation flow.
- Cargo commands and temporary-target behavior.
- Cargo JSON executable-discovery logic.
- Transactional runtime publication and rollback flow.
- Final runtime directory structure.
- Test names mapped to all 50 requirements.
- Exact package-specific test totals.
- Exact end-to-end commands and exit codes.
- Final runtime executable paths.
- Successful runtime hashes.
- Failed-build preservation hashes.
- Confirmation that no staging or backup files remain.
- Any remaining gap or blocker.

Do not append the previous task text after the report.

Do not claim completion unless:

- Both implementation crates build successfully with `--release --locked`.
- Cargo JSON identifies exactly one executable per crate.
- Both runtime executables are published to their corresponding runtime directories.
- A failed second build preserves both previous runtime executables.
- Runtime publication rollback is tested.
- `source create` remains functional.
- `source add` remains rejected.
- Root `lexicon build` remains unchanged.
- All relevant tests pass.