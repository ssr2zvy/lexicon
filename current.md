# Current task: close the final source-build staging and error-message issues

## Scope

The `lexicon source build` implementation is otherwise verified complete.

Make only these two focused corrections:

1. Replace the predictable PID-only runtime staging filename with a uniquely randomized staging path.
2. Correct the unsupported-protocol error so it does not incorrectly describe a build operation as source creation.

Do not redesign the build flow or modify unrelated behavior.

## Correction 1: randomized runtime staging

The current staging pattern is equivalent to:

```text
.<executable-name>.staging-<pid>
```

A process ID alone is not a sufficiently unique staging identifier. A stale file left by an earlier process can collide after the operating system reuses that PID.

Replace the PID-only construction with a randomized tempfile-managed staging file created inside the corresponding runtime directory.

The staging file must remain in the runtime directory so final publication can use a same-filesystem rename.

The resulting behavior must be equivalent to:

```text
<runtime-directory>/.<executable-name>.staging-<random-unique-suffix>
```

Use the `tempfile` crate rather than manually combining the PID with a timestamp.

An appropriate design is:

```rust
tempfile::Builder::new()
    .prefix(&format!(".{executable_name}.staging-"))
    .tempfile_in(runtime_directory)
```

Adapt this to the existing staging and publication functions.

## Staging-file requirements

The corrected staging flow must:

1. Create the staging file inside the final runtime directory.
2. Use a randomized unique suffix.
3. Never rely only on the process ID.
4. Copy the compiled executable bytes into the staging file.
5. Preserve executable permissions.
6. Close or persist the temporary file correctly before final rename when required by the platform.
7. Keep final publication on the same filesystem.
8. Remove the staging file automatically or explicitly after failure.
9. Leave no staging file after successful publication.
10. Preserve unrelated preexisting files.
11. Preserve `.gitignore`.
12. Continue working on both Unix and Windows.
13. Preserve the existing transactional two-runtime rollback behavior.

Do not weaken the existing backup or rollback logic.

## Collision behavior

A preexisting file matching the old PID-only format must not be removed, overwritten, or reused.

For example, if this already exists:

```text
.<executable-name>.staging-<current-pid>
```

the new staging flow must create a different randomized path and leave the preexisting file unchanged.

The operation must also be able to create multiple staging files without producing the same path.

## Required staging tests

Add focused tests proving:

1. Two staging-file allocations for the same executable and runtime directory produce different paths.
2. Both staging paths are inside the requested runtime directory.
3. The staging basename begins with:

```text
.<executable-name>.staging-
```

4. The complete staging basename is not merely:

```text
.<executable-name>.staging-<current-pid>
```

5. A preexisting old PID-style staging file remains unchanged.
6. A failed staging or publication operation removes the newly created randomized staging file.
7. Successful publication leaves no randomized staging file.
8. `.gitignore` remains unchanged.
9. Unrelated runtime files remain unchanged.
10. The existing second-publication-failure test still restores both previous runtime executables.

Use isolated temporary directories.

Do not verify this only by searching production source code for the word `tempfile`. Execute the staging behavior.

## Correction 2: protocol error wording

The current source-build rejection reports:

```text
unsupported protocol 'browser'; only 'http' is currently supported for source creation
```

That is incorrect when the user invoked:

```bash
lexicon source build example-source --protocol browser
```

Change the shared validation error to the operation-neutral message:

```text
unsupported protocol 'browser'; only 'http' is currently supported
```

The complete public error must be:

```text
[lexicon] ERROR: unsupported protocol 'browser'; only 'http' is currently supported
```

Use the same neutral validation message for `source create` and `source build`.

Do not duplicate protocol-validation implementations merely to change the noun in the message.

## Required error tests

Add or update tests proving:

1. This command exits with status `1`:

```bash
lexicon source build example-source --protocol browser
```

2. Its combined output contains exactly one:

```text
[lexicon] ERROR:
```

3. Its output contains:

```text
unsupported protocol 'browser'; only 'http' is currently supported
```

4. Its output does not contain:

```text
source creation
```

5. The unsupported build command does not invoke Cargo.
6. The unsupported build command does not modify either runtime executable.
7. The unsupported create command uses the same neutral protocol message.
8. Existing valid HTTP create and build commands remain unaffected.

## Required regression verification

Run:

```bash
cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

Report package-specific results for:

- `lexicon-cli`
- `lexicon-framework`
- `lexicon-framework-core`
- doc tests

Then repeat the real supported flow:

```bash
verification_root="$(mktemp -d)"
repo_root="$(git rev-parse --show-toplevel)"
cli_binary="$repo_root/target/debug/lexicon-cli"
framework_binary="$repo_root/target/debug/lexicon-framework"

"$cli_binary" init "$verification_root" demo-project
cd "$verification_root/demo-project"

LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source create example-source --protocol http

LEXICON_FRAMEWORK_PATH="$framework_binary" \
    "$cli_binary" source build example-source --protocol http
```

Verify that both runtime executables still exist and are regular executable files.

Then run the corrected rejection:

```bash
set +e
unsupported_output="$(
    LEXICON_FRAMEWORK_PATH="$framework_binary" \
        "$cli_binary" source build example-source --protocol browser 2>&1
)"
unsupported_status=$?
set -e
```

Verify:

```bash
test "$unsupported_status" -eq 1

test "$(
    printf '%s\n' "$unsupported_output" |
        grep -c '\[lexicon\] ERROR:'
)" -eq 1

printf '%s\n' "$unsupported_output" |
    grep -F "unsupported protocol 'browser'; only 'http' is currently supported"

if printf '%s\n' "$unsupported_output" | grep -F "source creation"; then
    exit 1
fi
```

Confirm the runtime executable hashes remain unchanged after this rejected command.

## Production-code rule

Modify only the functions necessary for:

- Runtime staging-file allocation and lifecycle.
- Neutral unsupported-protocol wording.
- Their focused tests.

If another production defect is discovered, document the exact failing executable test before correcting it.

Do not modify unrelated source creation, compilation, Cargo JSON parsing, publication, rollback, MZA, bundle, installer, or data-runtime behavior.

## Scope exclusions

Do not implement:

- Cross-target builds.
- Root `lexicon build`.
- Data command execution.
- HTTP acquisition.
- Raw transaction recording.
- SQLite processing.
- Additional protocols.
- Adding protocols to existing sources.
- MZA or bundling changes.

## Required final report

Replace `current.md` completely with one clean report using these sections:

```text
# Final source-build correction report

## Verdict

## Files changed

## Randomized staging implementation

## Staging collision tests

## Staging cleanup and rollback tests

## Protocol error correction

## Unsupported-protocol command evidence

## Supported end-to-end build

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

The report must include:

- The exact staging-allocation function changed.
- The exact randomized tempfile API used.
- Exact staging-related test names.
- Proof that two staging paths differ.
- Proof that an old PID-style file remains untouched.
- Proof that failed and successful operations leave no new staging files.
- The exact corrected unsupported-protocol output.
- The exact unsupported-protocol exit code.
- Proof that existing runtime hashes remain unchanged after rejection.
- Package-specific test totals.
- Any remaining failure or blocker.

Do not append this task after the report.

Do not declare completion unless both focused corrections and every listed regression check pass.