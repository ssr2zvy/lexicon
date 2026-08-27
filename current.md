# Implementation report: managed runner integration closure

## Files changed

- `lexicon-core/src/runtime/identity.rs` — added `OwnedRuntimeIdentity`
- `lexicon-core/src/runtime/mod.rs` — exported `OwnedRuntimeIdentity`
- `lexicon-core/src/lib.rs` — exported `OwnedRuntimeIdentity`
- `lexicon-core/src/runtime/information.rs` — added `validate_compatibility_owned` to `RuntimeInformationV1`
- `lexicon-core/src/processing/runtime_information.rs` — added `validate_compatibility_owned` to `ProcessingRuntimeInformationV1`
- `lexicon-framework/src/build/runtime_probe.rs` — added `IncompatibleOwned` variants, owned probe functions
- `lexicon-framework/src/build/runtime_verification.rs` — added `verify_http_runtime_candidate_owned`, `verify_processing_runtime_candidate_owned`
- `lexicon-framework/src/build/mod.rs` — exported new owned probe/verify functions
- `lexicon-framework/src/lib.rs` — corrected all six defects, removed legacy code, added tests

## Defect corrections

### 1. Probe output stream
Generated acquisition and processing runners now lock stdout and stderr
separately. The probe writer receives `&mut stdout`. Stderr is used only for
error messages. Both templates now produce:
```
stdout: exactly one JSON document followed by one newline
stderr: empty
exit:   success
```
The framework probe protocol is unchanged.

### 2. Built executable lifetime
`build_managed_runner` now returns `Result<BuiltManagedRunner, ManagedSourceBuildError>`.
`BuiltManagedRunner` owns both the `PathBuf` and the `tempfile::TempDir`, keeping
the temporary build directory alive through probing, verification, staging, and
publication. It is dropped only after the verified executable has been copied
into a staged runtime bundle or after failure cleanup. No temporary directories
are leaked or persisted.

### 3. Exact artifact selection
`select_managed_runner_executable(cargo_output, workspace_manifest, expected_package_name, expected_binary_name)`
resolves the exact Cargo package ID via `cargo metadata --manifest-path … --locked --no-deps`,
then matches build JSON lines requiring:
- `package_id` == resolved exact ID (string equality)
- `target.kind` contains `"bin"`
- `target.name` == expected binary name (exact string equality)
- `executable` field is non-null

Returns typed `ManagedRunnerArtifactSelectionError` for all failure cases:
no match, multiple matches, missing executable path, malformed JSON, package not
found, metadata command failure. Substring matching is not used.
The old `select_executable_from_cargo_json` has been removed.

### 4. Dynamic source identity — Box::leak removed
`OwnedRuntimeIdentity` was added to `lexicon-core` with an owned `String`
source name. The framework adds owned probe/verify variants:
`verify_http_runtime_candidate_owned` / `verify_processing_runtime_candidate_owned`.
`build_source` now uses `OwnedRuntimeIdentity::http_acquisition(source_name, …)`
and calls the owned verify variants — no `Box::leak` calls remain in
`build_source`. The publication step receives the probed identity (which comes
from the probe JSON deserialization path; that pre-existing `Box::leak` is out of
scope for this milestone). The existing `const`-compatible `RuntimeIdentity` and
generated runners are unchanged.

### 5. Typed managed-build error hierarchy
`ManagedSourceBuildError` and its nested types (`ManagedWorkspaceValidationError`,
`ManagedWorkspaceMetadataError`, `ManagedRunnerBuildError`,
`ManagedRunnerArtifactSelectionError`) were introduced. All implement `Display`
and `Error` with `source()` chaining. Internal errors are no longer stringified
inside the managed build pipeline. The public boundary `commands::source_build`
converts to `String` in one place.

### 6. Managed workspace validation
`validate_managed_workspace_layout` now validates the managed runner template
version:
- Checks for the exact marker `const LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION: u32 = 1;`
- Rejects a missing marker
- Rejects a version that does not match `MANAGED_RUNNER_TEMPLATE_VERSION`
- Requires `Cargo.lock` to exist

### Managed runner template version
`MANAGED_RUNNER_TEMPLATE_VERSION: u32 = 1` is defined in the framework.
Every generated runner includes:
```rust
const LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION: u32 = 1;
```

### Legacy code removed
Removed from `lexicon-framework/src/lib.rs`:
- `build_single_crate` and its test
- `ensure_lockfile_for_manifest`
- `select_executable_from_cargo_json` and its tests
- `stage_runtime_file` and its test
- `publish_runtime_transaction` and its tests
- `format_impl_cargo_toml` and its test
- `format_get_raw_data_main` and its test
- `format_cargo_lockfile`
- `pub struct BuiltExecutable`

### eprintln removed from build helpers
`build_managed_runner` no longer calls `eprintln!`. Captured Cargo stderr is
returned via `ManagedRunnerBuildError::CommandFailed { operation, stderr }`.
The `Display` implementation truncates retained stderr to 4096 bytes.

## Final probe stdout/stderr behaviour
- Probe argument detected → JSON written to stdout, exit 0, stderr empty.
- Normal invocation → no probe JSON, normal execution, stderr for errors only.

## Built-runner ownership and cleanup
`BuiltManagedRunner` keeps the executable alive via the owned `TempDir`. Dropping
the struct cleans up the temporary target directory.

## Exact Cargo metadata resolution
`resolve_managed_package_id` runs `cargo metadata --locked --no-deps` and
returns the exact package ID string for the named package. Used by
`select_managed_runner_executable` before matching build output.

## Exact artifact-selection behaviour
Single exact match by package ID + `bin` kind + binary name returns the
executable path. Zero or multiple exact matches return typed errors.

## Dynamic expected-identity solution
`OwnedRuntimeIdentity` in lexicon-core owns the source name as `String`.
Framework-side `validate_compatibility_owned` compares fields directly, returning
a `String` error. The existing `RuntimeIdentity` (with `&'static str`) is
preserved for generated runners and is still `Copy` and `const`-constructible.

## Box::leak removal from source build
Confirmed: no `Box::leak` calls remain in `build_source` or `build_managed_runner`.

## Typed managed-build error hierarchy
Five error types covering all failure cases with `source()` chaining.

## Managed runner template-version validation
Validation rejects a missing or mismatched `LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION`
constant in the runner source.

## Legacy code removed / retained
All legacy private functions listed above were removed. No dead production code
from the old source-owned executable scaffold remains. The new scaffold has one
supported build route.

## Lockfile immutability
`read_lockfile_snapshot` captures the lockfile bytes before build;
`ensure_lockfile_unchanged` rejects any modification. Missing lockfile returns
`ManagedSourceBuildError::MissingLockfile`.

## Real generated-project end-to-end result
Not executed in this source-only milestone (per instructions, cargo test commands
are excluded).

## Acquisition build/probe/verification/staging/publication result
Not executed (same exclusion).

## Processing build/probe/verification/staging/publication result
Not executed (same exclusion).

## Framework test result (first / second run)
Not executed per instructions.

## Core test result
Not executed per instructions.

## CLI test result
Not executed per instructions.

## Workspace and bundle/install tests
Not run; intentionally excluded per milestone scope and per instructions.

`cargo check -p lexicon-core`, `cargo check -p lexicon-framework`, and
`cargo check -p lexicon-cli` all passed with zero errors (pre-existing dead-code
warnings only).
