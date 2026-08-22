# Current task: verify and close `lexicon source new --protocol http`

## Status entering this task

The production implementation appears substantially complete, but the previous report did not provide the required executable evidence.

Do not redesign or expand the feature.

This task is primarily a verification and regression-test pass. Modify production source code only if an executable test exposes a real defect.

## Command under verification

```bash
lexicon source new <source-name> --protocol <protocol>
```

Both `<source-name>` and `--protocol <protocol>` must be required.

The only currently supported protocol is:

```text
http
```

## Required public behavior

This must succeed:

```bash
lexicon source new example-source --protocol http
```

This must fail during CLI parsing because `--protocol` is absent:

```bash
lexicon source new missing-protocol-source
```

This must fail because the protocol is unsupported:

```bash
lexicon source new unsupported-source --protocol browser
```

Neither failed command may create its requested source directory.

## First step: inspect existing implementation

Before changing code, inspect:

- `lexicon-cli/src/cli/source.rs`
- `lexicon-cli/src/cli/mod.rs`
- `lexicon-framework/src/main.rs`
- Existing CLI and framework tests
- Generated source templates
- Existing atomic staging behavior

Determine which requirements already have executable coverage and which do not.

Do not infer coverage from test totals or function names alone. Read the test bodies.

## Required verification matrix

Every requirement below must be mapped to:

1. An executable test function, or
2. An explicit end-to-end command and observed result.

Add a regression test where neither currently exists.

### CLI parsing

1. `lexicon source new example-source --protocol http` parses successfully.
2. `lexicon source new example-source` is rejected by Clap.
3. `lexicon source new example-source --protocol` is rejected by Clap because the value is missing.
4. The protocol has no hidden default.
5. The parsed source name and protocol are passed unchanged to the framework command.

### Validation before mutation

6. An unsupported protocol is rejected before creating any source directory.
7. An unsafe source name is rejected before creating any source directory.
8. Running outside a Lexicon project fails without creating source files.
9. An existing source directory is rejected without changing its existing contents.

### Generated scaffold

10. A valid HTTP source produces the complete required directory structure.
11. `source.toml` contains:

```toml
schema_version = 1

[source]
name = "example-source"
protocol = "http"
```

12. `source.toml` is produced through TOML serialization.
13. `discovery.md` contains the required discovery and attribution prompts.
14. The generated HTTP crate implements the context-based `HttpAcquisition::acquire` contract.
15. The generated HTTP crate calls `run_http_source`.
16. Generated Cargo manifests contain no machine-local absolute repository paths.
17. The existing portable Core dependency mechanism remains intact.
18. The generated process-data crate remains separate from the acquisition protocol.

### Atomicity

19. Source generation occurs in a unique staging directory inside the configured sources directory.
20. A generation failure leaves no task-created staging directory.
21. Successful generation leaves no staging directory.
22. A preexisting unrelated temporary directory is never deleted.
23. The completed staging directory is renamed into the final source path.
24. A preexisting source is never overwritten.

### Compilation

25. The generated HTTP acquisition crate passes `cargo check`.
26. The generated process-data crate passes `cargo check`.

### Public output

27. The real public CLI reaches the framework scaffold implementation.
28. The framework is the sole producer of source-creation success output.
29. Every Lexicon-owned success line begins with `[lexicon]`.
30. The CLI does not print a duplicate success line.
31. Failure output follows the established `[lexicon] ERROR:` contract without duplicate reporting.

## Required automated-test quality

Tests must execute behavior whenever practical.

Do not claim executable coverage from tests that only search production source files for strings such as:

```text
"Ok(())"
"println!"
"required = true"
```

Source-text assertions are acceptable for validating generated template contents, but not as substitutes for running the CLI parser, dispatch path, filesystem behavior, or generated Cargo projects.

Tests involving filesystem mutation must use isolated temporary directories.

Tests must verify both the returned result and the resulting filesystem state.

## Required full test command

Run:

```bash
cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

Report the exact number of passed and failed tests for:

- `lexicon-cli`
- `lexicon-framework`
- `lexicon-framework-core`, if it is included by the selected packages

Do not report only an aggregate statement such as “all targeted tests passed.”

## Required end-to-end verification

Build the real binaries first:

```bash
cargo build -p lexicon-cli -p lexicon-framework
```

Create a fresh temporary parent directory:

```bash
verification_root="$(mktemp -d)"
```

Resolve the real binaries from the repository build output:

```bash
repo_root="$(git rev-parse --show-toplevel)"
cli_binary="$repo_root/target/debug/lexicon-cli"
framework_binary="$repo_root/target/debug/lexicon-framework"
```

Initialize a fresh project using the public CLI:

```bash
"$cli_binary" init "$verification_root" demo-project
```

Enter the created project:

```bash
cd "$verification_root/demo-project"
```

Create a source through the real public CLI-to-framework path:

```bash
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source new example-source --protocol http
```

Verify that the source exists:

```bash
test -d sources/example-source
test -f sources/example-source/source.toml
test -f sources/example-source/discovery.md
test -f sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml
test -f sources/example-source/get-raw-data/get_raw_data_impl/src/main.rs
test -f sources/example-source/process-data/process_data_impl/Cargo.toml
test -f sources/example-source/process-data/process_data_impl/src/main.rs
```

## Required generated-crate compilation

Run both commands and report each exit result independently:

```bash
cargo check --manifest-path \
    sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml
```

```bash
cargo check --manifest-path \
    sources/example-source/process-data/process_data_impl/Cargo.toml
```

The task is not complete unless both commands succeed.

## Required missing-protocol verification

Run:

```bash
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source new missing-protocol-source
missing_protocol_status=$?
set -e
```

Verify:

```bash
test "$missing_protocol_status" -ne 0
test ! -e sources/missing-protocol-source
```

Record:

- The exit status.
- The relevant Clap error.
- Confirmation that the source path was not created.

## Required unsupported-protocol verification

Run:

```bash
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source new unsupported-source --protocol browser
unsupported_protocol_status=$?
set -e
```

Verify:

```bash
test "$unsupported_protocol_status" -ne 0
test ! -e sources/unsupported-source
```

Record:

- The exit status.
- The relevant Lexicon error.
- Confirmation that the source path was not created.

## Required existing-source verification

Create a sentinel inside the completed source:

```bash
printf 'preserve-me\n' > sources/example-source/existing-sentinel.txt
```

Run the same source command again:

```bash
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source new example-source --protocol http
existing_source_status=$?
set -e
```

Verify:

```bash
test "$existing_source_status" -ne 0
test "$(cat sources/example-source/existing-sentinel.txt)" = "preserve-me"
```

Confirm that the existing source and its sentinel were not modified or deleted.

## Required staging verification

Before and after successful and failed source-generation attempts, inspect the configured sources directory for task-created staging directories.

Verify that:

- Successful generation leaves none.
- Unsupported-protocol rejection leaves none.
- Existing-source rejection leaves none.
- Unrelated preexisting temporary directories remain unchanged.

If this cannot be verified through an existing automated test, add one.

## Production-code changes

Do not change production source merely to make the report appear complete.

Production changes are permitted only when:

1. A required executable test fails.
2. The failure demonstrates a real contract violation.
3. A focused correction is implemented.
4. The failing test passes afterward.
5. The full relevant test suite still passes.

If production code changes, report the exact failure that justified each change.

## Scope exclusions

Do not implement or modify:

- Actual HTTP requests.
- Raw transaction recording.
- SQLite processing.
- `lexicon source add`.
- `lexicon build`.
- Runtime source-executable launching.
- MZA.
- Bundling.
- Installation.
- Uninstallation.
- Update behavior.
- Completed `lexicon init` or project-discovery behavior unless a regression test proves it was broken by this feature.

## Required final `current.md` report

After verification, replace this task completely with a clean implementation report.

Do not append the old task instructions after the report.

The report must include:

### Files changed

List every file changed during this pass, including:

```text
current.md
```

Separate production files from test-only files when applicable.

### Production corrections

For every production-code change, provide:

- The failing executable test or command.
- The observed incorrect behavior.
- The exact function corrected.
- The resulting correct behavior.

If no production changes were required, state that explicitly.

### Test mapping

Provide a table mapping every numbered requirement from 1 through 31 to:

- Exact test function name, or
- Exact end-to-end command.

Do not use aggregate test totals as a substitute for this mapping.

### Compilation evidence

Report the separate result of:

```text
get_raw_data_impl cargo check
process_data_impl cargo check
```

### End-to-end evidence

Report:

- `lexicon init` result.
- Valid `source new` result.
- Missing-protocol exit status and filesystem result.
- Unsupported-protocol exit status and filesystem result.
- Existing-source exit status and sentinel result.
- Staging-directory result.
- Exact public output.
- Confirmation that no duplicate success output occurred.

### Final test results

Report the exact commands and package-specific pass/fail totals.

### Completion status

Do not declare completion unless:

- All 31 requirements have executable evidence.
- Both generated crates compile.
- All rejection cases leave the filesystem unchanged.
- Atomic staging behavior is verified.
- The public output contract is verified.
- The old task text is not appended to the report.