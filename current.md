# Current task: provide conclusive source-build verification

## Purpose

The implementation report claims that `lexicon source build` is complete, but it does not provide enough source-build-specific evidence to verify the claim against the preceding task.

This is a verification-first task.

Do not redesign the implementation.

Do not modify production code unless one of the required executable checks fails and exposes a real defect.

The general build/bundle/install automation does not substitute for the source-build-specific verification required here.

## Command being verified

```bash
lexicon source build <source-name> --protocol <protocol>
```

Current supported example:

```bash
lexicon source build example-source --protocol http
```

## Required implementation behavior

The command must:

1. Locate the containing Lexicon project.
2. Resolve the configured sources directory.
3. Resolve:

```text
sources/<source-name>/<protocol>/
```

4. Validate `source.toml`.
5. Build:

```text
get-raw-data/get-raw-data-impl/
process-data/process-data-impl/
```

6. Use native Cargo release builds with `--locked`.
7. Parse Cargo JSON to identify each binary executable.
8. Finish both builds before publishing either executable.
9. Publish each executable into its corresponding sibling runtime directory.
10. Preserve both existing runtime executables if either compilation fails.
11. Roll back both runtime outputs if publication fails partway through.
12. Remove task-created staging and backup files.
13. Print success only after both runtime executables are published.

## First step: inspect actual code and tests

Inspect the implementation responsible for:

- CLI parsing.
- CLI-to-framework dispatch.
- Framework source-build dispatch.
- Project/source/protocol validation.
- Cargo command construction.
- Cargo JSON parsing.
- Executable selection.
- Runtime staging.
- Runtime publication.
- Backup restoration.
- Temporary-file cleanup.
- Success and failure output.

Inspect the bodies of existing tests.

Do not infer coverage from aggregate test totals or test names alone.

## Required real end-to-end flow

Build the real Lexicon executables:

```bash
cd /workspaces/lexicon
cargo build -p lexicon-cli -p lexicon-framework
```

Resolve them:

```bash
repo_root="$(git rev-parse --show-toplevel)"
cli_binary="$repo_root/target/debug/lexicon-cli"
framework_binary="$repo_root/target/debug/lexicon-framework"
verification_root="$(mktemp -d)"
```

Initialize a fresh project:

```bash
"$cli_binary" init "$verification_root" demo-project
cd "$verification_root/demo-project"
```

Create the source:

```bash
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source create example-source --protocol http
```

Build the source:

```bash
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source build example-source --protocol http
```

Record:

- Exact source-build exit code.
- Exact Lexicon-owned output.
- Exact final get-raw-data executable path.
- Exact final process-data executable path.

Do not replace these results with a statement such as “the command succeeded.”

## Required runtime checks

Resolve both runtime directories:

```bash
get_runtime="sources/example-source/http/get-raw-data/runtime"
process_runtime="sources/example-source/http/process-data/runtime"
```

Verify that each contains exactly one regular executable file other than `.gitignore`.

On Unix:

```bash
get_executable="$(
    find "$get_runtime" -maxdepth 1 -type f ! -name '.gitignore'
)"

process_executable="$(
    find "$process_runtime" -maxdepth 1 -type f ! -name '.gitignore'
)"

test "$(printf '%s\n' "$get_executable" | sed '/^$/d' | wc -l)" -eq 1
test "$(printf '%s\n' "$process_executable" | sed '/^$/d' | wc -l)" -eq 1

test -f "$get_executable"
test -f "$process_executable"
test -x "$get_executable"
test -x "$process_executable"
```

Use the platform-equivalent regular-file checks on Windows.

Report the actual filenames rather than placeholders.

## Required successful-build hashes

After the successful build, record:

```bash
get_hash_before="$(sha256sum "$get_executable" | awk '{print $1}')"
process_hash_before="$(sha256sum "$process_executable" | awk '{print $1}')"
```

Report both hashes.

Use the platform-equivalent hashing command on Windows.

## Required failed-second-build verification

Perform this only inside the temporary generated project, never against repository source files.

Introduce a compile error into:

```text
sources/example-source/http/process-data/process-data-impl/src/main.rs
```

Preserve its original contents first.

Run:

```bash
set +e
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source build example-source --protocol http
failed_build_status=$?
set -e
```

Restore the generated source file afterward.

Verify:

```bash
test "$failed_build_status" -ne 0

get_hash_after="$(sha256sum "$get_executable" | awk '{print $1}')"
process_hash_after="$(sha256sum "$process_executable" | awk '{print $1}')"

test "$get_hash_after" = "$get_hash_before"
test "$process_hash_after" = "$process_hash_before"
```

Report:

- Exact failed-build exit code.
- Get runtime hash before and after.
- Process runtime hash before and after.
- Confirmation that neither published runtime changed.

This is the required proof that a successful first compilation followed by a failed second compilation does not partially publish.

## Required Cargo-command evidence

Provide the exact constructed Cargo arguments for both crates.

They must contain:

```text
build
--release
--locked
--manifest-path
--target-dir
--message-format=json-render-diagnostics
```

Provide the two exact manifest paths and confirm that the temporary target directories are distinct and uniquely scoped.

Do not provide only an illustrative command.

## Required Cargo JSON evidence

Identify the exact functions responsible for parsing Cargo JSON.

Report:

- The relevant `compiler-artifact` selection rule.
- How the requested package is matched.
- How binary targets are distinguished from libraries, tests, examples, dependencies, and build scripts.
- How a missing executable is rejected.
- How multiple applicable executables are rejected.

Provide exact test function names for:

- Selecting the correct binary artifact.
- Ignoring unrelated artifacts.
- Rejecting a missing executable.
- Rejecting ambiguous executable output.

If any of these tests do not exist, add them.

## Required publication rollback evidence

Identify the exact function responsible for publishing both runtime executables.

Provide exact test function names proving:

1. Both staged executables publish successfully.
2. Existing runtime executables are backed up.
3. Failure while publishing the second executable restores the first executable.
4. Both previous executables are restored.
5. Staged files are removed.
6. Backup files are removed.
7. Unrelated runtime files remain untouched.
8. `.gitignore` remains untouched.

If publication failure cannot currently be injected deterministically, introduce a narrow test seam around filesystem publication operations.

Do not use unreliable permission manipulation as the primary rollback test.

Modify production code only as much as required to make the existing transaction behavior deterministically testable.

## Required cleanup evidence

After the successful and failed builds, inspect both runtime directories and the temporary build locations.

Report:

- Any filename pattern used for staged files.
- Any filename pattern used for backup files.
- The exact command or test assertion used to prove none remain.
- Confirmation that temporary Cargo target directories were removed.
- Confirmation that `.gitignore` remains in both runtime directories.

## Required command rejection evidence

Run and report exact exit codes for:

```bash
lexicon source add example-source
```

```bash
lexicon source build example-source
```

```bash
lexicon source build example-source --protocol browser
```

Confirm that none of these commands modifies either runtime executable.

## Required test suite

Run:

```bash
cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

Report package-specific totals:

- `lexicon-cli`
- `lexicon-framework`
- `lexicon-framework-core`
- doc tests

## Production-code modification rule

If all required verification succeeds, do not modify production code.

If a check fails:

1. Record the exact failing command or test.
2. Explain the violated contract.
3. Apply the smallest production correction.
4. Add or preserve a regression test.
5. Rerun the focused test.
6. Rerun the complete relevant test suite.
7. Repeat the end-to-end source-build verification when affected.

## Scope exclusions

Do not implement:

- Cross-target source builds.
- cargo-zigbuild.
- MZA integration.
- Root `lexicon build`.
- Runtime execution through data commands.
- Actual HTTP acquisition.
- Raw transaction recording.
- SQLite processing.
- Multiple protocols for one source.
- Installation or bundling changes.

## Required final report format

Replace `current.md` completely with one report. Do not append this task or duplicate report sections.

Use these exact top-level sections:

```text
# Source-build verification report

## Verdict

## Files changed

## Production corrections

## Real end-to-end command

## Source-build output and exit code

## Runtime executable paths

## Successful-build hashes

## Failed-second-build preservation

## Cargo command construction

## Cargo JSON executable selection

## Publication rollback tests

## Temporary-file cleanup

## Rejection commands and exit codes

## Test results

## Remaining gaps
```

Under `## Verdict`, use exactly one of:

```text
VERIFIED COMPLETE
```

or:

```text
NOT VERIFIED COMPLETE
```

Do not use `VERIFIED COMPLETE` unless every required source-build-specific check in this task succeeds.

The report must contain actual commands, paths, exit codes, hashes, and exact test function names. Aggregate claims such as “all relevant behavior passed” are insufficient.