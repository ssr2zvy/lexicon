Current implementation milestone: managed runner integration closure

Objective

Correct and complete the managed-runner workspace and source build integration currently pushed to main.

Do not begin sessions, data commands, HTTP transport, or processing behavior.

The previous milestone changed the generated layout, but the resulting pipeline is not yet proven operational and contains defects that prevent a generated runtime from completing the existing build/probe/verification path.

This milestone closes those defects and proves this real sequence:

lexicon init
→ lexicon source create
→ generated locked acquisition workspace
→ generated locked processing workspace
→ exact managed runner builds
→ acquisition probe on stdout
→ processing probe on stdout
→ verification
→ staging
→ paired publication

This is a corrective milestone, not a new architectural feature.

Repository-grounded defects to correct

The current main implementation has the following concrete problems.

1. Probe output uses the wrong stream

Both generated managed runner templates currently create only:

let mut stderr = io::stderr().lock();

and pass stderr to:

try_write_runtime_information_probe(...)

The framework probe machinery reads runtime-information JSON from the child’s stdout and treats stderr as diagnostic output.

Therefore, generated runners must instead use:

let stdout = io::stdout();
let mut stdout = stdout.lock();
let stderr = io::stderr();
let mut stderr = stderr.lock();

Pass &mut stdout to the probe writer.

Use stderr only for diagnostics.

Successful probe behavior must be:

stdout: exactly one JSON document followed by one newline
stderr: empty
exit: success

Do not change the established framework probe protocol to accommodate the incorrect generated runner.

2. Built executable lifetime is invalid

The current:

fn build_managed_runner(...) -> Result<PathBuf, String>

creates a local tempfile::TempDir, builds below it, returns only the executable path, and then drops the temporary directory when the function returns.

That makes the returned executable path invalid before verification.

Replace the return type with an owning value equivalent to:

pub struct BuiltManagedRunner {
    executable: PathBuf,
    target_directory: tempfile::TempDir,
}

Provide accessors such as:

impl BuiltManagedRunner {
    pub fn executable(&self) -> &Path;
}

Keep the temporary directory alive through:

* probing;
* verification;
* staging.

It may be dropped only after the verified executable has been copied into a staged runtime bundle or after failure cleanup completes.

Do not leak or persist temporary build directories.

3. Artifact selection is not exact

The current build path eventually calls:

select_executable_from_cargo_json(
    &cargo_json,
    operation_name,
)

The existing selector accepts artifacts when the target or package identifier merely contains strings such as get-raw-data.

This does not satisfy exact managed-runner selection.

Implement a dedicated exact selector for managed runners.

Representative API:

pub fn select_managed_runner_executable(
    cargo_output: &str,
    expected_package_id: &str,
    expected_binary_name: &str,
) -> Result<
    PathBuf,
    ManagedRunnerArtifactSelectionError,
>;

Resolve the expected package ID from:

cargo metadata
--manifest-path <workspace/Cargo.toml>
--locked
--no-deps

Match Cargo build JSON using:

* exact Cargo package ID;
* target kind containing bin;
* exact target name;
* non-null executable path.

Do not determine package identity using substring matching.

Reject:

* no exact artifact;
* multiple exact artifacts;
* a matching target from the wrong package;
* a matching package with the wrong binary target;
* a compiler artifact with no executable;
* malformed relevant Cargo JSON.

Unrelated compiler messages and artifacts may be ignored.

Preserve the old selector only if another still-supported legacy API genuinely uses it. It must not be used by managed runner builds.

4. Dynamic source identity currently leaks memory

build_source currently does:

Box::leak(source_name.to_string().into_boxed_str())

twice to satisfy RuntimeIdentity’s static source-name representation.

Remove these leaks.

Do not replace them with another leaked allocation or a global cache.

Use one of these bounded approaches:

1. Add a framework-side expected-identity representation that borrows or owns the dynamic source name and update verification/staging/publication comparison boundaries accordingly; or
2. Add a narrowly scoped owned runtime-identity representation in Core while preserving the existing const-compatible RuntimeIdentity used by compiled managed runners.

Prefer the smallest design that preserves:

const IDENTITY: RuntimeIdentity =
    RuntimeIdentity::http_acquisition("example-source", 1);

inside generated runners.

Do not remove const compiled identities merely to accommodate framework-side dynamic values.

All identity comparisons must still cover:

* source;
* protocol;
* operation;
* source contract version.

The completion report must explain the chosen owned/borrowed expected-identity boundary.

5. Typed build errors are missing

The current managed pipeline returns Result<_, String> and converts verification, staging, and publication errors with format!.

Introduce a typed internal managed-source-build error.

Representative structure:

#[derive(Debug)]
pub enum ManagedSourceBuildError {
    WorkspaceValidation(
        ManagedWorkspaceValidationError,
    ),
    Metadata(
        ManagedWorkspaceMetadataError,
    ),
    CargoBuild(
        ManagedRunnerBuildError,
    ),
    AcquisitionVerification(
        HttpRuntimeVerificationError,
    ),
    ProcessingVerification(
        ProcessingRuntimeVerificationError,
    ),
    AcquisitionStaging(
        RuntimeBundleStagingError,
    ),
    ProcessingStaging(
        ProcessingRuntimeBundleStagingError,
    ),
    Publication(
        RuntimePairPublicationError,
    ),
}

Equivalent organization is acceptable.

Implement:

std::fmt::Display
std::error::Error

and preserve nested errors through source().

The existing public command boundary may still return Result<SourceBuildResult, String> if changing CLI error handling is outside scope, but it must convert the typed error only once at that outer boundary.

Do not stringify errors inside the managed build pipeline.

6. Managed workspace validation is incomplete

The current validation reads manifests and checks some names, but it does not fully prove the selected Cargo graph.

Validate using parsed Cargo metadata and manifests.

Require:

* exact two workspace members;
* exact implementation package;
* exact runner package;
* exact runner binary target;
* implementation package exposes a library target;
* implementation package exposes no binary target used by the supported build;
* runner package exposes the expected binary target;
* runner depends on the exact implementation package by the expected relative path;
* both members resolve the same workspace lexicon_core dependency;
* a real root Cargo.lock exists;
* src/lib.rs exists for the implementation;
* managed runner src/main.rs exists;
* obsolete implementation src/main.rs is rejected as a legacy layout when no managed workspace exists;
* unexpected extra workspace members are rejected.

Do not treat textual substring checks of runner source as the primary Cargo-graph validation.

The generated runner source remains managed, so validate its exact generated contents through a deterministic template/version mechanism.

Managed runner template version

Define a distinct managed runner template version.

Representative value:

const MANAGED_RUNNER_TEMPLATE_VERSION: u32 = 1;

Include an unambiguous generated marker in each managed runner source, for example:

const LEXICON_MANAGED_RUNNER_TEMPLATE_VERSION: u32 = 1;

Validation must reject:

* a missing marker;
* an unsupported version;
* a runner whose managed template content differs from the canonical template for its source and operation.

Do not use only loose contains(...) checks for source identity and SOURCE.

The source implementation library remains user-owned.

The runner remains Lexicon-owned.

Generated runner stream behavior

Acquisition runner

The generated acquisition runner must follow:

collect argv excluding argv[0]
→ lock stdout and stderr separately
→ try HTTP runtime-information probe using stdout
→ Written: return ExitCode::SUCCESS
→ NotRequested: construct temporary HTTP context
→ run_http_runtime_invocation(...)
→ success/failure ExitCode

Probe JSON goes to stdout.

Errors go to stderr.

Normal successful execution must not emit probe JSON.

Continue using:

HttpCapabilitySet::empty()

until real HTTP capabilities exist.

Do not infer available capabilities from source requirements.

Processing runner

The generated processing runner follows the same stream separation:

collect argv excluding argv[0]
→ lock stdout and stderr separately
→ try processing runtime-information probe using stdout
→ Written: return ExitCode::SUCCESS
→ NotRequested: ProcessingContext::default()
→ run_processing_runtime_invocation(...)
→ success/failure ExitCode

Probe JSON goes to stdout.

Errors go to stderr.

Temporary HTTP context behavior

Preserve the current temporary normal-execution boundary:

HttpAcquisitionContext::from_env()

Do not expand it in this corrective milestone.

Do not introduce project-path transport, sessions, or new environment variables.

The generated acquisition runtime merely remains compilable and capable of reaching the completed normal-invocation execution path when the existing source-directory environment value is supplied.

Its replacement belongs to the session/context milestone.

Lockfile behavior

Preserve:

* real Cargo-generated lockfiles during source create;
* cargo build --locked during source build;
* no lockfile mutation during ordinary builds.

Add tests proving that source build leaves both Cargo.lock files byte-for-byte unchanged.

A missing or stale lockfile must produce a typed managed-build failure.

Remove or quarantine the legacy build path

The current file still contains the obsolete functions:

build_single_crate(...)
ensure_lockfile_for_manifest(...)
select_executable_from_cargo_json(...)
stage_runtime_file(...)
publish_runtime_transaction(...)
format_impl_cargo_toml(...)
format_get_raw_data_main(...)
format_process_data_main(...)
format_cargo_lockfile(...)

and tests for the old source-owned executable scaffold.

Remove obsolete private functions and their obsolete tests when they have no remaining supported caller.

If a function remains necessary for an unrelated supported path, rename or isolate it so the managed build cannot accidentally call it.

New scaffolds and source build must have only one supported build route.

Do not retain dead legacy production code solely because old unit tests reference it.

Legacy projects themselves are not automatically rewritten. They receive the established migration-required error.

Eliminate direct production eprintln! from build helpers

The current managed Cargo build helper writes Cargo stderr directly with:

eprintln!(...)

Return captured diagnostic information through the typed error instead.

The CLI boundary decides how to display it.

Bound retained Cargo stderr to a reasonable constant to prevent unbounded error capture.

The error’s Display must not dump arbitrary unbounded compiler output.

Tests may inspect structured retained diagnostic bytes or text through accessors.

End-to-end generated-project proof

Add at least one test that exercises the real generated project rather than testing template strings alone.

The test must:

1. Create a temporary Lexicon project.
2. Run the framework’s real source-creation path for example-source.
3. Confirm both real lockfiles exist.
4. Replace the generated placeholder acquisition handler with a successful minimal handler if normal execution is tested.
5. Replace the generated placeholder processing handler with a successful minimal handler if normal execution is tested.
6. Run the real managed source-build path.
7. Build both exact runner packages with --locked.
8. Probe both produced runners.
9. Verify both candidates.
10. Stage both bundles.
11. Publish the pair.
12. Admit both published bundles.
13. Confirm the published acquisition identity.
14. Confirm the published processing identity.
15. Confirm neither implementation crate was published as an executable.

The test may use a local dependency override or fixture specifically to avoid depending on a mutable remote Git state.

Production-generated manifests must retain the immutable repository revision pin.

Do not fake the probe results in this end-to-end test.

Focused regression tests

Add tests proving:

1. Acquisition probe JSON is written to stdout.
2. Acquisition probe stderr is empty.
3. Processing probe JSON is written to stdout.
4. Processing probe stderr is empty.
5. Probe exits successfully.
6. Probe does not invoke acquisition.
7. Probe does not invoke processing.
8. Normal acquisition failure writes only a sanitized diagnostic to stderr.
9. Normal processing failure writes only a sanitized diagnostic to stderr.
10. BuiltManagedRunner keeps the executable alive after the build helper returns.
11. The executable remains available through verification and staging.
12. Dropping the owning built-runner value cleans the temporary target directory after staging or failure.
13. Exact package and binary matching selects the acquisition runner.
14. Exact package and binary matching selects the processing runner.
15. A similarly named unrelated package is ignored.
16. A similarly named unrelated binary is ignored.
17. No exact artifact returns the typed missing-artifact error.
18. Multiple exact artifacts return the typed ambiguous-artifact error.
19. Missing executable fields are typed.
20. Malformed relevant Cargo JSON is typed.
21. Dynamic source build validation performs no Box::leak.
22. Repeated builds do not accumulate leaked source-name allocations.
23. Verification errors remain available as typed nested sources.
24. Staging errors remain available as typed nested sources.
25. Publication errors remain available as typed nested sources.
26. Missing lockfile is typed.
27. Stale lockfile is rejected by --locked.
28. Source build does not modify either lockfile.
29. Modified runner template is rejected.
30. Unsupported runner template version is rejected.
31. Extra workspace member is rejected.
32. Implementation binary substitution is rejected.
33. Legacy source-owned executable layout returns migration-required.
34. Existing verification tests remain unchanged.
35. Existing staging tests remain unchanged.
36. Existing bundle-admission tests remain unchanged.
37. Existing paired-publication tests remain unchanged.

Validation

Run the framework suite twice:

cargo test -p lexicon-framework --quiet
cargo test -p lexicon-framework --quiet

Run the Core suite once if the identity representation changes:

cargo test -p lexicon-core --quiet

Run the CLI package tests once because source create and source build are public CLI-backed commands:

cargo test -p lexicon-cli --quiet

Do not run:

cargo test --workspace

Workspace-wide validation remains intentionally excluded.

Do not run the bundle/install pipeline.

Preserve existing behavior

Do not change:

* CLI command names or arguments;
* lexicon init;
* source.toml schema;
* invocation-envelope JSON;
* invocation argv transport;
* acquisition admission;
* processing admission;
* normal invocation execution;
* runtime-information JSON;
* probe limits and timeout;
* executable hashing;
* runtime manifest formats;
* bundle directory formats;
* bundle admission;
* paired-publication rollback;
* source implementation handler signatures;
* MZA;
* Protocol 1;
* installer behavior.

Explicit exclusions

Do not implement:

* sessions;
* project-path invocation transport;
* data-command process launching;
* HTTP transport;
* retries;
* redirects;
* redaction;
* raw transaction recording;
* checkpoints;
* processing SQLite behavior;
* foreground supervision;
* background supervision;
* __operator-host;
* data --get;
* data --process;
* lexicon build;
* automatic source-code migration;
* cross-compilation;
* MZA or installer changes.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* each repository defect corrected;
* final probe stdout/stderr behavior;
* built-runner ownership and cleanup behavior;
* exact Cargo metadata resolution;
* exact artifact-selection behavior;
* dynamic expected-identity solution;
* confirmation that Box::leak was removed from source build;
* typed managed-build error hierarchy;
* managed runner template-version validation;
* legacy code removed or intentionally retained;
* lockfile immutability result;
* real generated-project end-to-end result;
* acquisition build/probe/verification/staging/publication result;
* processing build/probe/verification/staging/publication result;
* first framework test result;
* second framework test result;
* Core test result if applicable;
* CLI test result;
* confirmation that workspace and bundle/install tests were not run.

Then stop.

Do not begin sessions or HTTP execution until this integration closure is green.