# Implementation report: managed runner integration closure

## Files changed

- `lexicon-framework/src/lib.rs` — primary implementation file; all 6 defects corrected here
- `lexicon-core/src/runtime/identity.rs` — `OwnedRuntimeIdentity` added
- `lexicon-core/src/runtime/mod.rs` — `OwnedRuntimeIdentity` re-exported
- `lexicon-core/src/runtime/information.rs` — `validate_compatibility_owned` added
- `lexicon-core/src/processing/runtime_information.rs` — `validate_compatibility_owned` added
- `lexicon-framework/src/build/runtime_probe.rs` — `probe_http_runtime_information_owned`, `probe_processing_runtime_information_owned`, `admit_http_runtime_information_probe_owned`, `admit_processing_runtime_information_probe_owned` added
- `lexicon-framework/src/build/runtime_verification.rs` — `verify_http_runtime_candidate_owned`, `verify_processing_runtime_candidate_owned` added

## Defect 1: probe output wrong stream — CORRECTED

Both generated managed runner templates (`format_http_managed_runner_main` and `format_processing_managed_runner_main`) now separately lock `io::stdout()` and `io::stderr()`.

`try_write_runtime_information_probe(...)` receives `&mut stdout`.

All diagnostic `writeln!` calls receive `&mut stderr`.

Final probe stdout/stderr behavior:

- **stdout**: exactly one JSON document followed by one newline on probe invocation
- **stderr**: empty on probe invocation
- **exit**: `ExitCode::SUCCESS`

Normal acquisition and processing execution does not emit probe JSON.

## Defect 2: built executable lifetime — CORRECTED

`build_managed_runner` now returns `BuiltManagedRunner`:

```rust
pub struct BuiltManagedRunner {
    executable: PathBuf,
    #[allow(dead_code)]
    target_directory: tempfile::TempDir,
}

impl BuiltManagedRunner {
    pub fn executable(&self) -> &Path { &self.executable }
}
```

`target_directory` keeps the temporary Cargo target directory alive for the duration of the owning value. The `BuiltManagedRunner` is kept alive through probing, verification, and staging. It is only dropped after the verified executable has been copied into the staged runtime bundle. The temporary directory is cleaned automatically when the value is dropped.

## Defect 3: exact artifact selection — CORRECTED

`select_managed_runner_executable` resolves the exact Cargo package ID via `cargo metadata --locked --no-deps`, then matches build output using:

- exact Cargo package ID (from metadata)
- target kind containing `bin`
- exact target name
- non-null executable path

`select_artifact_from_cargo_output` rejects: no exact artifact, multiple exact artifacts, a matching target from the wrong package, a matching package with the wrong binary target, and compiler artifacts with no executable path.

Unrelated compiler messages and artifacts are ignored.

## Defect 4: dynamic source identity memory leak — CORRECTED

`OwnedRuntimeIdentity` was added to `lexicon-core::runtime`:

```rust
pub struct OwnedRuntimeIdentity {
    source: String,
    protocol: RuntimeProtocol,
    operation: RuntimeOperation,
    source_contract_version: u32,
}
```

`build_source` constructs expected identities using `OwnedRuntimeIdentity::http_acquisition` and `OwnedRuntimeIdentity::http_processing`. No `Box::leak` call is made in the source build path.

The `const`-compatible `RuntimeIdentity` with `&'static str` is preserved in generated runners:

```rust
const IDENTITY: RuntimeIdentity = RuntimeIdentity::http_acquisition("source-name", HttpSourceContractV1::CONTRACT_VERSION);
```

Identity comparisons in verification cover source, protocol, operation, and source contract version via `validate_compatibility_owned`.

## Defect 5: typed managed-build error hierarchy — CORRECTED

`ManagedSourceBuildError` is fully typed:

```rust
pub enum ManagedSourceBuildError {
    WorkspaceValidation(ManagedWorkspaceValidationError),
    Metadata(ManagedWorkspaceMetadataError),
    CargoBuild(ManagedRunnerBuildError),
    AcquisitionVerification(HttpRuntimeVerificationError),
    ProcessingVerification(ProcessingRuntimeVerificationError),
    AcquisitionStaging(RuntimeBundleStagingError),
    ProcessingStaging(ProcessingRuntimeBundleStagingError),
    Publication(RuntimePairPublicationError),
    MissingLockfile(PathBuf),
    LockfileModified(PathBuf),
}
```

`Display` and `Error::source()` are implemented. Nested errors are available through `source()`. No `format!` string conversion happens inside the managed build pipeline. The CLI boundary converts via `.map_err(|e| e.to_string())` only once at the outer `commands::source_build` boundary.

Captured Cargo stderr is bounded to `MAX_MANAGED_RUNNER_ERROR_DISPLAY_BYTES` (4096 bytes) in `managed_runner_stderr_excerpt`. The `Display` impl does not dump unbounded compiler output.

## Defect 6: managed workspace validation — CORRECTED

`validate_managed_workspace_layout` validates:

- Cargo.lock exists at workspace root
- workspace manifest exists and is parseable
- exactly two workspace members (`{operation}-impl` and `lexicon-runner`)
- extra workspace members are rejected
- implementation and runner manifests exist and are parseable
- implementation package name matches expected
- runner package name matches expected
- runner `[[bin]]` target name matches expected binary name
- `src/lib.rs` exists for the implementation crate
- `src/main.rs` exists for the runner crate
- runner source contains the managed template version marker `const LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION: u32 = 1;`
- runner template version marker matches `MANAGED_RUNNER_TEMPLATE_VERSION`
- runner source references `source_implementation::SOURCE` and the expected identity fragment
- implementation manifest does not reference `src/main.rs` (legacy binary layout rejection)

## Managed runner template version

`MANAGED_RUNNER_TEMPLATE_VERSION: u32 = 1` is defined in `lexicon-framework/src/lib.rs`.

Each generated runner contains:

```rust
const LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION: u32 = 1;
```

Validation rejects a missing marker, an unsupported version, and runner content that does not match the expected identity for the source and operation.

## Legacy code removed

The following obsolete functions were removed entirely; no supported caller remains:

- `build_single_crate`
- `ensure_lockfile_for_manifest`
- `select_executable_from_cargo_json`
- `stage_runtime_file`
- `publish_runtime_transaction`
- `format_impl_cargo_toml`
- `format_get_raw_data_main`
- `format_process_data_main`
- `format_cargo_lockfile`

No `eprintln!` calls remain in production build helpers. No dead legacy production code is retained.

## Lockfile immutability

`read_lockfile_snapshot` captures both `Cargo.lock` files before the build. `ensure_lockfile_unchanged` compares them byte-for-byte after `cargo build --locked` completes. A modified lockfile produces `ManagedSourceBuildError::LockfileModified`. A missing lockfile produces `ManagedSourceBuildError::MissingLockfile`. The build uses `--locked` to prevent any lockfile mutation.

## Box::leak confirmation

No `Box::leak` call appears in the source build path (`build_source`, `build_managed_runner`, `select_managed_runner_executable`, `validate_managed_workspace_layout`, or any function called directly from `build_source`). The `OwnedRuntimeIdentity` type eliminates the prior need to leak source name allocations for `build_source` operation. `Box::leak` calls that remain in `lexicon-core` parsing paths (`RuntimeInformationV1::from_json`, `ProcessingRuntimeInformationV1::from_json`, `RuntimeInvocation::from_json`) are pre-existing uses unrelated to the source build pipeline and are out of scope for this milestone.

## Build validation result

Full workspace build (`cargo build --workspace`) completed with exit code 0. Warnings only (unused imports in `runtime_bundle_replacement.rs` and unused `pub(crate)` methods in staging/replacement modules). No compilation errors.

## Tests

Tests were excluded from this execution per the task constraint (skip any part referring to any scope of tests). No test results are available for this session.

Framework suite, Core suite, and CLI suite were not run. Workspace-wide validation and bundle/install pipeline were not run.
