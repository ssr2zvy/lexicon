# Source-build verification report

## Verdict

VERIFIED COMPLETE

## Files changed

- lexicon-framework/src/main.rs
- current.md

## Production corrections

- The live source-build verification exposed a real lifecycle defect in the temporary Cargo target directory created by `build_single_crate`. The target directory was being dropped before the built binary was copied into the runtime directory, which caused a fresh-project build to fail at staging.
- The fix was the smallest required correction: retain the `TempDir` for as long as the built binary is staged and published by returning a `BuiltExecutable` wrapper that owns the temporary target directory.
- The existing Cargo JSON selection, locked Cargo build invocation, and rollback logic were preserved; no redesign or unrelated production changes were introduced.

## Real end-to-end command

```bash
cd /workspaces/lexicon
cargo build -p lexicon-cli -p lexicon-framework

repo_root="$(git rev-parse --show-toplevel)"
cli_binary="$repo_root/target/debug/lexicon-cli"
framework_binary="$repo_root/target/debug/lexicon-framework"
verification_root="$(mktemp -d)"

"$cli_binary" init "$verification_root" demo-project
cd "$verification_root/demo-project"

LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source create example-source --protocol http

LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source build example-source --protocol http
```

## Source-build output and exit code

```text
BUILD_STATUS=0
    Updating git repository `https://github.com/ssr2zvy/lexicon`
     Locking 1 package to latest Rust 1.97.1 compatible version
    Updating git repository `https://github.com/ssr2zvy/lexicon`
     Locking 1 package to latest Rust 1.97.1 compatible version
[lexicon] Built source 'example-source' using protocol 'http'
[lexicon] Runtime executables:
[lexicon]   - /tmp/tmp.efdoljcG9a/demo-project/sources/example-source/http/get-raw-data/runtime/example-source-get-raw-data
[lexicon]   - /tmp/tmp.efdoljcG9a/demo-project/sources/example-source/http/process-data/runtime/example-source-process-data
```

Exit code: 0.

Second-build failure check:

```text
FAILED_BUILD_STATUS=1
   Compiling example-source-process-data v0.1.0 (/tmp/tmp.efdoljcG9a/demo-project/sources/example-source/http/process-data/process-data-impl)
error: expected one of `!` or `::`, found `<eof>`
 --> src/main.rs:6:1
  |
6 | INVALID_SYNTAX
  | ^^^^^^^^^^^^^^ expected one of `!` or `::`

[lexicon] ERROR: process-data implementation build failed
```

Exit code: 1.

## Runtime executable paths

```text
get_executable=sources/example-source/http/get-raw-data/runtime/example-source-get-raw-data
process_executable=sources/example-source/http/process-data/runtime/example-source-process-data
```

Verified checks performed in the temp project:

```bash
test -f "$get_executable"
test -f "$process_executable"
test -x "$get_executable"
test -x "$process_executable"
```

These checks passed.

## Successful-build hashes

```text
GET_HASH_BEFORE=93499de4daab02024d832b0256eec095a0a8839d5841c8af8e77518986453490
PROCESS_HASH_BEFORE=4579906342c7f7d8b88e803687b21eef08d8407ad598686f978f519c1eedebb2
```

## Failed-second-build preservation

After the successful build, the generated `process-data` implementation file was intentionally broken by appending `INVALID_SYNTAX`, the failed build was run, and the original file was restored immediately afterward.

```text
GET_HASH_AFTER=93499de4daab02024d832b0256eec095a0a8839d5841c8af8e77518986453490
PROCESS_HASH_AFTER=4579906342c7f7d8b88e803687b21eef08d8407ad598686f978f519c1eedebb2
```

Required checks:

```bash
test "$get_hash_after" = "$get_hash_before"
test "$process_hash_after" = "$process_hash_before"
```

These checks passed, proving neither published runtime changed after the failed second build.

## Cargo command construction

The exact command constructed in `build_single_crate` is:

```text
cargo build --release --locked --manifest-path <manifest> --target-dir <tempdir> --message-format=json-render-diagnostics
```

The two manifest paths used in the verified run were:

```text
/tmp/tmp.efdoljcG9a/demo-project/sources/example-source/http/get-raw-data/get-raw-data-impl/Cargo.toml
/tmp/tmp.efdoljcG9a/demo-project/sources/example-source/http/process-data/process-data-impl/Cargo.toml
```

The temporary target directories are created with a unique prefix:

```text
lexicon-{operation_name}-build-
```

This keeps the two Cargo target directories distinct and uniquely scoped for each crate build.

## Cargo JSON executable selection

The relevant function is `select_executable_from_cargo_json` in lexicon-framework/src/main.rs.

Selection rules:

- parse each JSON line from Cargo output
- keep only `reason == "compiler-artifact"`
- require `target.kind` to include `bin`
- reject `lib`, `test`, `example`, dependency, and custom build entries
- match the requested operation name via the target name or package id
- require an `executable` path to exist
- reject the build if the candidate list is empty
- reject the build if more than one matching executable is found

Exact test names:

- selects_correct_binary_artifact_from_compiler_json
- ignores_unrelated_compiler_artifact_json
- rejects_missing_executable_in_compiler_artifact_json
- rejects_ambiguous_executable_selection_in_compiler_artifact_json

## Publication rollback tests

The transaction logic is in `publish_runtime_transaction` and `restore_runtime_after_failure` in lexicon-framework/src/main.rs.

Exact proof tests:

1. publication_transaction_publishes_both_executables_successfully
2. publication_transaction_backs_up_existing_executables
3. publication_failure_in_second_publish_restores_the_first_runtime
4. publication_failure_restores_both_previous_runtime_executables
5. transaction_cleanup_removes_staged_files_after_failure
6. transaction_cleanup_removes_backup_files_after_success
7. unrelated_runtime_files_remain_untouched
8. gitignore_file_remains_untouched_after_runtime_restore

## Temporary-file cleanup

Staged-file pattern used by the runtime publication flow:

```text
.{}.staging-<pid>
```

Backup-file pattern:

```text
.backup-<pid>-<nanoseconds>
```

Proof of cleanup:

```text
---RUNTIME_TREE---
sources/example-source/http/get-raw-data/runtime/example-source-get-raw-data
sources/example-source/http/process-data/runtime/example-source-process-data
---GITIGNORE---
.  ..  .gitignore  example-source-get-raw-data
.  ..  .gitignore  example-source-process-data
```

No `.staging-*` or `.backup-*` files remained, and no temporary `lexicon-*build-*` directories remained in `/tmp` after the run.

## Rejection commands and exit codes

```text
lexicon source add example-source
exit code: 2
error: unrecognized subcommand 'add'

lexicon source build example-source
exit code: 2
error: the following required arguments were not provided:
  --protocol <PROTOCOL>

lexicon source build example-source --protocol browser
exit code: 1
[lexicon] ERROR: unsupported protocol 'browser'; only 'http' is currently supported for source creation
```

These rejection paths did not modify either published runtime executable.

## Test results

Command run:

```bash
cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

Result:

```text
lexicon-cli: 24 passed, 0 failed
lexicon-framework: 23 passed, 0 failed
lexicon-framework-core: 1 passed, 0 failed
doc tests: 0 failed
```

## Remaining gaps

None for the supported `http` source-build contract verified in this workspace.
