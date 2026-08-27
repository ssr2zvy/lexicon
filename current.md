# Managed runner workspaces and source build integration

Status: complete.

## Summary

The managed-runner migration is implemented in `lexicon-framework/src/lib.rs`. Newly generated source scaffolds now use independent Cargo workspaces per operation, library-based implementation crates, and Lexicon-managed runner crates instead of source-owned executable crates and the legacy `build_single_crate(...)` flow.

## What changed

- Switched generated scaffolds from `src/main.rs` executables to library-based implementation crates and runner `main.rs` files.
- Added workspace-level `[workspace.dependencies]` dependency pinning so both `get-raw-data-impl` and `lexicon-runner` share the same immutable `lexicon_core` revision.
- Generated real `Cargo.lock` files via Cargo instead of the placeholder three-line lockfile.
- Updated source build to validate the managed workspace layout, build the exact runner package/binary target, verify it with the runtime probe flow, stage the verified bundle, and publish via the paired runtime publication flow.
- Preserved the existing verification/staging/publication machinery instead of introducing new runtime process-launching or SQLite behavior.

## Generated source layout

Each `sources/<source>/http` project now generates:

- `get-raw-data/Cargo.toml`
- `get-raw-data/Cargo.lock`
- `get-raw-data/get-raw-data-impl/src/lib.rs`
- `get-raw-data/lexicon-runner/src/main.rs`
- `process-data/Cargo.toml`
- `process-data/Cargo.lock`
- `process-data/process-data-impl/src/lib.rs`
- `process-data/lexicon-runner/src/main.rs`

The generated workspace manifests use the selected structure with `members = ["<operation>-impl", "lexicon-runner"]`, and the implementation libraries export typed descriptors such as `SOURCE: HttpSourceContractV1` and `SOURCE: ProcessingSourceContractV1`.

## Dependency pin

The generated workspaces use one centralized workspace dependency for `lexicon_core` and resolve it to the current repository revision via a real Git `rev` pin, rather than the obsolete `lexicon-framework-core` crate or a mutable `main` branch reference.

## Validation performed

The repo-level validation was kept to the package-level checks requested by the milestone and was not widened to a workspace-wide validation sweep.

- `cargo test -p lexicon-framework --quiet` → passed (`95 passed; 0 failed`)
- `cargo test -p lexicon-core --quiet` → passed (`261 passed; 0 failed`, plus the trybuild UI checks passed)

## Notes

- No workspace-wide validation was run at the end, per instruction.
- The migration remains focused on source scaffolding and managed-runner source build integration without adding data-command runtime process launching or SQLite behavior.

collect argv excluding argv[0]
→ try_write_runtime_information_probe(...)
→ Written: return success
→ NotRequested: construct HTTP context
→ run_http_runtime_invocation(...)
→ map result to ExitCode

Use the currently established available-capability set explicitly.

Do not infer available capabilities from source requirements.

Until the real HTTP transport milestone supplies capabilities, declare only capabilities genuinely implemented by the linked Core runtime.

Do not claim ClientCertificateV1 is available unless it is actually implemented.

Temporary HTTP context boundary

Normal acquisition currently requires:

&mut HttpAcquisitionContext

and the only production constructor currently available is:

HttpAcquisitionContext::from_env()

For this milestone, the managed runner may use that existing constructor so the generated executable is complete and compilable.

Do not redesign invocation JSON or add a second argv path transport here.

Do not expand or otherwise modernize HttpAcquisitionContext::from_env().

The later session/context milestone will replace this temporary construction boundary.

Document this temporary use explicitly in the completion report.

Managed processing runner

Generate:

process-data/lexicon-runner/src/main.rs

The runner must statically reference:

source_implementation::SOURCE

and a compiled identity equivalent to:

const IDENTITY: RuntimeIdentity =
    RuntimeIdentity::http_processing(
        "example-source",
        ProcessingSourceContractV1::CONTRACT_VERSION,
    );

Execution order:

collect argv excluding argv[0]
→ processing try_write_runtime_information_probe(...)
→ Written: return success
→ NotRequested: construct ProcessingContext::default()
→ run_processing_runtime_invocation(...)
→ map result to ExitCode

Do not add SQLite behavior.

Managed runner error behavior

Both generated runners must:

* return ExitCode::SUCCESS for a successfully written probe;
* return ExitCode::SUCCESS for successful normal execution;
* return ExitCode::FAILURE for context, probe, transport, admission, or handler failure;
* write a concise sanitized diagnostic to stderr;
* never print source arguments or envelope JSON;
* never panic merely to map an ordinary typed error;
* not call std::process::exit.

Handler panics may continue to unwind according to the existing Core policy.

Managed runner manifests

Each managed runner manifest must:

* define one package;
* define one exact binary target;
* depend on the operation implementation library by relative path;
* depend on the same pinned lexicon-core workspace dependency;
* contain no user-configurable alternate entrypoint.

Expected binary names:

example-source-get-raw-data
example-source-process-data

Expected runner package names:

example-source-get-raw-data-runner
example-source-process-data-runner

Expected implementation package names:

example-source-get-raw-data-impl
example-source-process-data-impl

Normalize the Rust library crate aliases deterministically from the validated source name.

Do not guess the built executable path.

Managed file validation

Before building, validate that each operation workspace has exactly the required managed runner files and expected contents or generated semantic values.

At minimum validate:

* workspace manifest exists;
* workspace contains the expected members;
* runner manifest exists;
* runner package name is exact;
* runner binary name is exact;
* runner implementation dependency points to the expected relative implementation path;
* runner source exists;
* compiled source identity matches the requested source, protocol, operation, and contract version;
* implementation manifest is a library package;
* implementation src/lib.rs exists;
* obsolete implementation src/main.rs is not used as the runtime entrypoint.

The source author owns the implementation library and may edit its dependencies and source.

The source author does not own the supported runner entrypoint.

Reject modified or incompatible managed runner definitions rather than building an arbitrary source-owned binary.

Do not attempt hostile-code sandboxing.

Source-create migration

Update generate_source_scaffold(...) and its formatting helpers to create the target structure.

Remove obsolete generator helpers or rewrite them:

format_get_raw_data_main(...)
format_process_data_main(...)
format_impl_cargo_toml(...)
format_cargo_lockfile(...)

Replace them with operation-specific helpers for:

* workspace manifests;
* implementation manifests;
* implementation lib.rs;
* runner manifests;
* managed runner main.rs.

SourceCreateResult.created_files must identify the useful author-facing implementation files and relevant manifests. Do not report obsolete src/main.rs paths.

Source creation must remain transactional: a failure produces no partially published source tree.

Existing-source behavior

Do not silently rewrite existing sources during source build.

For a source still using the legacy source-owned executable layout, return a clear migration-required error identifying the expected managed workspace structure.

Automatic migration of user-authored implementation code is excluded because mechanically converting arbitrary main.rs code into the typed library contract is unsafe.

Newly created sources must use only the managed layout.

Exact build selection

Replace direct implementation-crate building with exact managed-runner building.

For acquisition, invoke Cargo against:

get-raw-data/Cargo.toml

and require the exact:

package: example-source-get-raw-data-runner
binary:  example-source-get-raw-data

For processing, invoke Cargo against:

process-data/Cargo.toml

and require the exact:

package: example-source-process-data-runner
binary:  example-source-process-data

Cargo invocation must include:

cargo build
--manifest-path <operation-workspace/Cargo.toml>
--package <exact-runner-package>
--bin <exact-runner-binary>
--release
--locked
--message-format=json-render-diagnostics
--target-dir <isolated-temporary-target>

Keep acquisition and processing target directories isolated.

Select the executable from Cargo JSON by matching:

* package identity;
* binary target name;
* executable artifact presence.

Do not select the first executable emitted.

Do not build or publish a source implementation as a standalone executable.

Lockfile behavior

source create must produce valid workspace lockfiles.

source build --locked must not mutate them.

Remove the current build behavior that unconditionally runs:

cargo generate-lockfile

immediately before cargo build --locked.

A missing or stale lockfile must cause an actionable build error.

Lockfile creation or update belongs to source creation or an explicit future dependency-management operation, not ordinary locked builds.

Probe and verification integration

After each managed runner builds:

1. probe its runtime information using the existing framework probe API;
2. validate the expected compiled identity;
3. validate descriptor contract version;
4. validate operation identity;
5. validate declared and available capabilities;
6. hash before and after probing using existing verification;
7. reject mutation during probing.

Use:

verify_http_runtime_candidate(...)
verify_processing_runtime_candidate(...)

Do not duplicate their logic.

The acquisition runtime must probe as acquisition.

The processing runtime must probe through the processing-specific information model.

Staging integration

Stage each verified runtime using the existing APIs:

stage_verified_http_runtime_bundle(...)
stage_verified_processing_runtime_bundle(...)

Each staged bundle must contain only its established manifest and executable.

Do not revert to copying a bare executable directly into runtime/.

Paired publication

Publish the staged acquisition and processing bundles using the existing paired transactional publication API.

Use:

publish_runtime_pair(...)

Do not retain the legacy stage_runtime_file(...) and publish_runtime_transaction(...) route for the managed build path.

Required behavior:

* acquisition build failure preserves both existing runtime bundles;
* processing build failure preserves both;
* probe failure preserves both;
* verification failure preserves both;
* staging failure preserves both;
* first-publication failure preserves or restores both;
* second-publication failure rolls both back;
* successful publication replaces the pair.

Return published bundle directories through SourceBuildResult.

If its existing fields are named get_runtime and process_runtime, preserve them but make them identify the published bundle paths rather than guessed bare-executable paths.

Typed framework errors

The current source build implementation returns broad String errors.

Within the new managed build pipeline, add typed errors for at least:

* invalid managed workspace;
* missing lockfile;
* Cargo spawn;
* unsuccessful Cargo build;
* malformed Cargo JSON;
* missing exact executable artifact;
* unexpected or ambiguous executable artifacts;
* HTTP runtime verification;
* processing runtime verification;
* HTTP bundle staging;
* processing bundle staging;
* paired publication.

The public command boundary may convert the final typed build error to the CLI’s existing error representation if changing the entire CLI error architecture is outside scope.

Do not discard typed nested errors inside the framework pipeline.

Required tests

Add tests covering at least:

Scaffold

1. New source creation produces both operation workspaces.
2. Both workspace manifests have the exact members.
3. Both real workspace lockfiles exist.
4. Acquisition implementation is a library.
5. Processing implementation is a library.
6. Neither implementation contains src/main.rs.
7. Both implementations export typed SOURCE constants.
8. Both handler signatures accept &[OsString].
9. Both managed runner manifests have exact package and binary names.
10. Both managed runners depend on the expected implementation paths.
11. Acquisition runner has the exact compiled acquisition identity.
12. Processing runner has the exact compiled processing identity.
13. Existing scaffold transactionality remains intact.
14. The obsolete processing/ directory is absent.

Compilation and probes

15. A generated acquisition workspace builds with --locked.
16. A generated processing workspace builds with --locked.
17. The acquisition runner answers the existing HTTP information probe.
18. The processing runner answers the processing information probe.
19. Probes do not invoke placeholder handlers.
20. Probe output passes existing framework admission.
21. Generated runtime identities match the source name.
22. Generated descriptor versions match their contracts.

Placeholder handlers may panic or use todo!() during normal execution because probe mode must not invoke them.

Managed validation

23. Missing runner manifest is rejected.
24. Modified runner package name is rejected.
25. Modified binary name is rejected.
26. Wrong implementation path is rejected.
27. Acquisition/processing identity substitution is rejected.
28. Legacy source-owned executable layout returns migration-required error.
29. A source implementation with an invalid handler signature fails compilation.

Build selection

30. Cargo is invoked with the exact workspace manifest.
31. Cargo is invoked with the exact runner package.
32. Cargo is invoked with the exact binary target.
33. Cargo is invoked with --release, --locked, and JSON diagnostics.
34. Acquisition and processing use isolated target directories.
35. An unrelated executable artifact is ignored.
36. Missing exact artifact is rejected.
37. Multiple matching artifacts are rejected.

Verification, staging, and publication

38. Both built runners pass existing verification.
39. Both verified runners stage as manifest-bearing bundles.
40. Successful source build publishes both bundles.
41. Acquisition build failure preserves the existing pair.
42. Processing build failure preserves the existing pair.
43. Probe failure preserves the existing pair.
44. Staging failure preserves the existing pair.
45. Publication failure rolls back both.
46. No bare executable is published outside the bundle.
47. Existing runtime verification tests remain unchanged.
48. Existing staging tests remain unchanged.
49. Existing bundle-admission tests remain unchanged.
50. Existing paired-publication tests remain unchanged.

Use test seams for Cargo execution and injected failures where necessary.

Do not depend on mutating global PATH or shared temporary directories.

Do not introduce sleeps or global workspace-test serialization.

Validation

Run:

cargo test -p lexicon-framework --quiet

Run it a second time:

cargo test -p lexicon-framework --quiet

Run the complete Core suite once because generated runner APIs depend on it:

cargo test -p lexicon-core --quiet

Do not run:

cargo test --workspace

Workspace-wide validation is intentionally excluded.

Do not run the bundle/install pipeline.

Preserve existing behavior

Do not change:

* CLI command names or argument syntax;
* project initialization;
* source.toml schema;
* invocation-envelope JSON;
* argv transport;
* normal-invocation execution;
* acquisition or processing admission;
* runtime-information formats;
* probe limits or timeouts;
* hashing behavior;
* manifest formats;
* bundle admission;
* staging formats;
* paired-publication guarantees;
* MZA;
* Protocol 1;
* installer behavior.

Explicit exclusions

Do not implement:

* automatic migration of existing source code;
* normal data-command process launching;
* project-path invocation transport;
* session creation or locking;
* session reconciliation;
* HTTP transport;
* redirects or retries;
* raw transaction recording;
* SQLite processing;
* foreground supervision;
* background supervision;
* __operator-host;
* data --get;
* data --process;
* lexicon build;
* cross-compilation;
* MZA or installer changes.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* final generated directory tree;
* workspace manifest design;
* immutable Core dependency pin used;
* acquisition implementation library template;
* processing implementation library template;
* acquisition managed runner template;
* processing managed runner template;
* temporary HTTP context-construction behavior;
* managed runner validation behavior;
* exact Cargo commands;
* exact package and binary artifact selection;
* lockfile behavior;
* acquisition and processing probe results;
* verification results;
* staging results;
* paired-publication results;
* legacy-source behavior;
* scaffold tests;
* framework test result for both runs;
* Core test result;
* confirmation that workspace and bundle/install tests were not run.

Then stop.