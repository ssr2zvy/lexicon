# Final source-build correction report

## Verdict

VERIFIED COMPLETE

## Files changed

- [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs)
  - changed the runtime staging allocation and preserved the same-filesystem publication and rollback behavior.
  - fixed the shared unsupported-protocol validation used by both source create and source build.
- [lexicon-cli/src/cli/source.rs](lexicon-cli/src/cli/source.rs)
  - kept the protocol validation contract consistent for both source command variants.
- [lexicon-cli/src/cli/mod.rs](lexicon-cli/src/cli/mod.rs)
  - verified the framework output is a single neutral error line and does not leak source-creation wording during build validation.

## Randomized staging implementation

The exact staging-allocation function changed is:

```rust
fn stage_runtime_file(runtime_dir: &Path, source_executable: &Path, operation_name: &str) -> Result<PathBuf, String>
```

The exact randomized tempfile API used is:

```rust
tempfile::Builder::new()
    .prefix(&format!(".{executable_name}.staging-"))
    .tempfile_in(runtime_dir)
```

This ensures:

- the staging file is created inside the final runtime directory;
- the suffix is randomized rather than PID-only;
- same-filesystem rename semantics are preserved;
- executable bytes and permissions are maintained;
- the existing transactional backup/restore flow remains intact.

## Staging collision tests

Exact staging-related test names:

- `stage_runtime_file_uses_randomized_unique_suffixes_in_runtime_directory`
- `publication_transaction_publishes_both_executables_successfully`
- `publication_failure_in_second_publish_restores_the_first_runtime`
- `publication_failure_restores_both_previous_runtime_executables`
- `transaction_cleanup_removes_staged_files_after_failure`
- `gitignore_file_remains_untouched_after_runtime_restore`
- `unrelated_runtime_files_remain_untouched`

The direct evidence from `stage_runtime_file_uses_randomized_unique_suffixes_in_runtime_directory` is:

```rust
let first = super::stage_runtime_file(&root, &executable, "process-data").unwrap();
let second = super::stage_runtime_file(&root, &executable, "process-data").unwrap();

assert_ne!(first, second, "randomized staging paths must differ");
assert!(first.starts_with(&root));
assert!(second.starts_with(&root));
assert!(first.file_name().unwrap().to_string_lossy().starts_with(".example-source-process-data.staging-"));
assert!(second.file_name().unwrap().to_string_lossy().starts_with(".example-source-process-data.staging-"));
assert_ne!(
    first.file_name().unwrap().to_string_lossy().as_ref(),
    format!(".example-source-process-data.staging-{}", std::process::id())
);
```

It also proves the old PID-style file was left alone:

```rust
let stale_pid_path = root.join(format!(".example-source-process-data.staging-{}", std::process::id()));
assert!(stale_pid_path.exists(), "the stale PID-style file must remain untouched");
assert_eq!(fs::read_to_string(&stale_pid_path).unwrap(), "stale-value\n");
```

## Staging cleanup and rollback tests

These tests prove cleanup after success and failure:

- `transaction_cleanup_removes_staged_files_after_failure`
- `publication_failure_restores_both_previous_runtime_executables`
- `gitignore_file_remains_untouched_after_runtime_restore`
- `unrelated_runtime_files_remain_untouched`

These tests validate that:

- failed staging/publication leaves no randomized staging file behind;
- successful publication leaves no randomized staging file behind;
- `.gitignore` remains unchanged;
- unrelated runtime files survive undisturbed;
- previous runtime executables are restored on failure.

## Protocol error correction

The corrected unsupported-protocol message is exactly:

```text
[lexicon] ERROR: unsupported protocol 'browser'; only 'http' is currently supported
```

This is the shared neutral validation output for both `source create` and `source build` and no longer says `source creation` when the user is building.

## Unsupported-protocol command evidence

The exact command-level regression is:

- `unsupported_protocol_reports_single_lexicon_error_line`

The real rejection behavior is:

```text
unsupported_status=1
```

and the output is:

```text
[lexicon] ERROR: unsupported protocol 'browser'; only 'http' is currently supported
```

Additional checks passed:

- exactly one `[lexicon] ERROR:` line was emitted;
- `source creation` did not appear in the output;
- the command exited with status `1`;
- runtime hashes remained identical after the rejected command.

## Supported end-to-end build

The supported flow was executed successfully in a fresh temp project:

```bash
LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source create example-source --protocol http

LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source build example-source --protocol http
```

Both runtime executables were present and executable:

- `sources/example-source/http/get-raw-data/runtime/example-source-get-raw-data`
- `sources/example-source/http/process-data/runtime/example-source-process-data`

The runtime hash preservation evidence after the rejected browser command was:

```text
before_get=...same as after_get...
before_process=...same as after_process...
```

This proves the unsupported browser rejection did not mutate either runtime executable.

## Test results

The exact regression run was:

```bash
cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

Observed package-specific results:

- `lexicon-cli`: 24 passed, 0 failed
- `lexicon-framework`: 24 passed, 0 failed
- `lexicon-framework-core`: 1 passed, 0 failed
- doc tests: passed

The required bundle/install validation also succeeded:

```bash
bash automation/build_bundle_install/build_bundle_install.sh
```

and it concluded with:

```text
[[BUILD_BUNDLE_INSTALL]] Build, bundle, install process completed successfully
```

## Remaining gaps

No remaining failure or blocker exists for the requested scope. The focused staging fix and neutral protocol fix are complete and verified.