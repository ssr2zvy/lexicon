Current milestone: implement `lexicon build` (workspace-wide discovery and build)
Objective
Implement the top-level `lexicon build` command so it deterministically discovers every supported source/protocol pairing in the project and invokes the same validated per-source build pipeline already used by `lexicon source build`, reporting per-source success/failure with exact identities. Each discovered `sources/<source>/<protocol>/source.toml` is now schema-2 (per the previous milestone), so the discovery layer naturally re-uses the new schema-2 loader instead of re-implementing manifest validation.
This milestone is unblocked by Milestone 2 (`Restore trustworthy runtime-execution test coverage` / `add-durable-source-state-directory`) and the schema-2 milestone; without schema-2 any discovered manifest would either be rejected by `load_source_metadata` or silently disagree with the build pipeline's per-source validation. The prior Milestone 3 attempt (workspace-wide `lexicon build`) was abandoned because manifest validation had become an obstacle; schema-2 closes that gap.
This milestone is derived from:
contract.md §5 ("`lexicon build`" is part of the public command boundary) and §6 (the supported architecture: one published managed acquisition runtime per source per protocol, one per processing);
specs.md §40 (the eighteen-step requirements for `lexicon build`: discover the project, resolve the source, validate the source manifest, validate acquisition and processing workspaces and lockfiles, allocate isolated temporary targets and staging directories, run the locked release builds for both runtimes, select executables, hash, probe in information mode, validate runtime identities and capabilities, generate both `runtime.json` documents, transactionally publish the pair, restore previous bundles on any failure, and remove only temporary build material; conceptual `cargo build --locked ... --message-format=json-render-diagnostics` arguments spelled out); §16 (the ordering of capabilities on the runtime probe); §17 (checkpoint commitment ordering); and §26 (resume selection's interaction with previously published bundles).
the previous abandoned milestone's repository-grounded analysis (lexicon-framework/src/lib.rs already exposes `find_project_root`, `configured_sources_directory`, `source_build`, and `Map<source,protocol>` rejection rules that the discovery layer must follow without weakening the per-source pipeline).
Repository-grounded starting point
`lexicon-framework::commands::source_build(source_name, protocol)` (lexicon-framework/src/lib.rs around line 1005) is the existing validated single-source build pipeline: it validates the schema-2 `source.toml` via `load_source_metadata` (now strictly schema-2), validates the managed acquisition/processing workspace layout and metadata, builds both managed runners inside isolated temporary target directories, verifies each executable via `verify_http_runtime_candidate_owned` / `verify_processing_runtime_candidate_owned`, hashes them via `hash_runtime_executable`, and publishes both runtime bundles atomically per source (via `publish_runtime_pair`). This per-source pipeline must not be reimplemented or weakened; `lexicon build` must call it directly for each discovered pairing.
`find_project_root`, `configured_sources_directory`, `load_project_config` (lexicon-framework/src/lib.rs) already supply validated project discovery and the resolved sources root; `lexicon build` must reuse them rather than re-deriving project/paths logic.
`lexicon-cli/src/cli/build.rs` defines `BuildCommand` as a unit struct (`lexicon build` takes no arguments today); `lexicon-cli/src/cli/mod.rs`'s `dispatch` function's `RootCommand::Build(_)` arm is the integration point to replace (currently prints `"Parsed build command: build"`).
`validate_source_name` and `validate_protocol` (lexicon-framework/src/lib.rs) provide validated canonical form for source names and the only currently supported protocol (`http`); `lexicon build` should not duplicate or relax these.
`lexicon source create` only supports the `http` protocol today, so the only protocol directory that can legitimately exist under a source is `http`; the discovery logic must be written generically (so that adding a second protocol later will not require restructuring), but only needs to actually accept `http` for now.
Required implementation
1. Discovery
Add a new `lexicon-framework::commands` function `build_all()` that:
1. discovers the containing project via `find_project_root` from the current directory;
2. resolves the configured sources directory via `load_project_config`/`configured_sources_directory`;
3. enumerates the immediate subdirectories of the sources root as candidate source names, applying `validate_source_name` consistently (skip/reject consistently, never silently autocorrect);
4. for each candidate source, enumerates its immediate subdirectories as candidate protocol identities, recognizing only `http` via `validate_protocol` for now;
5. treats a source/protocol pairing as a build target only when `sources/<source>/<protocol>/source.toml` exists **and** passes a pre-flight schema-2 validation via the project's `load_source_metadata`. Reject the pairing with a typed `BuildAllError::InvalidSourceManifest` carrying the exact `source.toml` path and the typed `SourceManifestError` if the manifest is missing, schema-1, or has any per-field mismatch — do not silently proceed with a malformed manifest, since this is the cleanup point of the previously abandoned milestone;
6. produces a stable, deterministically ordered list of discovered `(source_name, protocol)` pairs (sorted lexicographically by source name, then protocol) so that output and test behavior do not depend on filesystem iteration order; this ordering also forms the build invocation order.
Reject ambiguous or invalid layouts rather than silently skipping them: a source directory containing a non-directory entry (`sources/<source>/<junk-file>`) or a non-`http` directory entry (`sources/<source>/<browser>/`) must cause discovery to fail with a precise per-path error. Likewise, a source directory that contains zero recognized protocol directories (e.g. only `notes.txt`) must fail discovery with an actionable "source directory contains no recognized protocol directories" error rather than silently producing an empty target list. An entirely absent or empty sources directory must NOT be an error: it represents a valid (trivial) project with zero build targets, and `lexicon build` should report success with the message `0 source(s) built, 0 failed`.
2. Build invocation and aggregate reporting
For each discovered pairing, in the deterministic order established above, invoke the existing `source_build`/`build_source` pipeline unchanged. Attempt every discovered pairing even if an earlier pairing fails — do not stop at the first failure — and collect a per-pairing result (success with its `SourceBuildResult`, or failure capturing the source name, protocol, exact error message, and which pipeline phase failed) into an aggregate `BuildAllOutcome` returned to the caller. The CLI layer must then report every failure with the exact source name and protocol identity (per specs.md §40 item 6), and must report overall command failure (non-zero `Err` from `dispatch`) if any pairing failed, even if others succeeded.
Do not implement a project-wide all-or-nothing publication transaction across sources — specs.md §40 explicitly defers this ("may remain deferred until supported by implementation evidence"). Each source's own publish step is already atomic per `publish_runtime_pair`; that per-source atomicity is sufficient for this milestone.
Define a typed error enum `BuildAllError` distinct from `ManagedSourceBuildError` for the discovery-time failures (project not found, project config load failure, sources-directory containment failure, invalid source or protocol layout, manifest validation failure). The aggregate build-phase failures raise `BuildAllError::Build { source_name, protocol, error }` to keep the per-pairing error identity unambiguous.
3. CLI wiring
Replace the `RootCommand::Build(_)` stub arm in `lexicon-cli/src/cli/mod.rs` with a call into the new `lexicon_framework::commands::build_all`, printing a per-source summary line (mirroring the existing style used for `source create`/`source build` output) and a final summary line indicating how many succeeded and how many failed. Return `Err` from `dispatch` when any pairing failed or when discovery itself failed, with a message enumerating each failed source/protocol identity and its underlying error message.
4. Tests
Add tests proving at least:
* discovery finds zero targets in a project with an empty sources directory and `lexicon build` reports success with `0 source(s) built, 0 failed` (no panic, no spurious failure);
* discovery finds and builds a single valid source/protocol pairing end-to-end using `commands::init` + `commands::source_create`, then `commands::build_all`, verifying both managed runtime artifacts exist at the paths `SourceBuildResult.get_runtime` / `SourceBuildResult.process_runtime` produce;
* discovery finds and builds multiple valid source/protocol pairings in one `lexicon build` invocation and reports the successes/failures in the deterministic sort order;
* a failure in one discovered pairing (for example, a project containing two scaffolded sources where one has its `Cargo.toml` deliberately corrupted) does not prevent the other valid pairings from being attempted and successfully built; the aggregate `BuildAllOutcome` correctly names the failed source and protocol while reporting the others' success;
* an ambiguous layout — e.g. `sources/<source>/notes.txt` (non-directory entry) — causes discovery to fail with an `BuildAllError` identifying the offending path, before any build is attempted;
* a source directory with a non-`http` unrecognized protocol directory (e.g. `sources/<source>/browser/`) causes discovery to fail with a precise error rather than silently skipping;
* a sources subdirectory whose `source.toml` is schema-1 (or otherwise fails schema-2 pre-flight) causes discovery to fail with `BuildAllError::InvalidSourceManifest` carrying the typed `SourceManifestError` (no runtime `cargo build` invocation is performed for the malformed source);
* the CLI-level `dispatch` test (alongside the existing `lexicon-cli/src/cli/mod.rs` tests for `source create`/`source build`) confirms `lexicon build` is wired to the framework function rather than remaining a placeholder, using the same `with_test_cwd` pattern already used by neighboring tests in that file.
Retain all existing tests for `source_build`, `source_create`, `init`, discovery (`find_project_root`, `configured_sources_directory`), and managed workspace validation unmodified except where a genuine, minimal signature/call-site change is required to reuse them from the new discovery function.
Scope constraints
Do not implement during this milestone:
* MZA Protocol 1 release construction, the `lexicon-bundle` adapter, or any complete-product release packaging (specs.md §41) — `lexicon build` only performs source builds, per specs.md's explicit separation between source build and product release construction;
* a project-wide all-or-nothing publication transaction across sources (specs.md §40 explicitly defers this);
* any SQLite work-ledger, runtime identity probe-decoding changes, or other functionality unrelated to workspace-wide build discovery;
* support for any protocol other than `http`;
* changes to `build_source`'s internal per-source validation/build/publish logic beyond what is strictly required to surface the typed `BuildAllError` for aggregate reporting (prefer wrapping with `BuildAllError::Build { error: ... }` over restructuring);
* parallelizing the per-source builds (sequential, deterministic order is sufficient for this milestone);
* changes to `lexicon data`, `lexicon init`, `lexicon source create`, background/operator-host execution, or any HTTP/session/runtime-context code;
* parent-side `lexicon data --get` step 3 "validate source.toml" (specs.md §24) — although the schema-2 manuscript loader that this milestone wires into discovery would be perfect for it, surfacing it through the foreground/background data path is too large for a single milestone and remains a separate follow-up;
* processing-side `RuntimeInformationV1` asymmetry — out of scope;
* wiring the discovered manifest's `contract` identifier through `RuntimeInformationV1::from_json` (the per-milestone deferred work);
* cross-platform `lexicon install` plumbing.
Preserved production behavior
* `lexicon source build <source> --protocol http` continues to validate the schema-2 `source.toml`, build both managed runners, verify identities, hash executables, and publish the runtime pair atomically. This milestone only adds a discovery loop that repeats that pipeline; it does not introduce a parallel or weaker per-source implementation;
* `lexicon source create` still emits schema-2 `source.toml` and the workspace-discovery view of it is identical to what `source create` itself writes;
* The `find_project_root`, `configured_sources_directory`, `load_project_config`, `validate_source_name`, `validate_protocol`, and `load_source_metadata` helpers are reused unchanged;
* No changes to foreground/background supervision, operator host, HTTP recording, checkpoints, durable source state, owned-lease invariants, public/private API boundaries, or any CLI surface outside `RootCommand::Build`'s dispatch arm.
Completion criteria
This milestone is complete only when:
* `lexicon build` deterministically discovers every valid source/protocol pairing under the project's configured sources directory, sorted lexicographically;
* it invokes the existing validated `source_build` pipeline for each discovered pairing without weakening that pipeline;
* ambiguous or invalid layouts are rejected with a precise, actionable error identifying the offending path;
* schema-1 or malformed `source.toml` documents are rejected at discovery time (no `cargo build` invoked for those sources);
* failures are reported with the exact source name and protocol identity, and do not prevent other valid pairings from being attempted;
* `cargo check --workspace` passes;
* `cargo test --workspace --quiet` passes;
* no production contract weakened to make test setup easier;
* no out-of-scope functionality (MZA/release, work-ledger, second protocol, `lexicon data` source-manifest integration) included.
Completion report
When the milestone passes, replace this file with a concise report containing:
* the exact commit tested;
* confirmation that `cargo check --workspace` passed;
* confirmation that `cargo test --workspace --quiet` passed;
* where the new discovery/aggregation function lives and how `lexicon-cli`'s `dispatch` was wired to it;
* the precise behavior of discovery against: zero sources, one source, multiple sources, an ambiguous layout, a non-`http` protocol directory, a schema-1 manifest, and a malformed manifest — using the new tests as evidence;
* the number and categories of new tests added;
* confirmation that no required test remains ignored, deleted, or falsely successful;
* confirmation that no unrelated feature work (MZA, work-ledger, workspace integration into `lexicon data`) was included;
* confirmation that `lexicon source build` continues to validate the same schema-2 `source.toml` unchanged.
Then stop.
The following milestone should be derived from the updated contract and specification once this one lands. With a working `lexicon build` and a schema-2 source manifest, the natural next candidates are (a) parent-side `lexicon data --get` step 3 "validate source.toml" (specs.md §24) wired through `RuntimeProjectLayout`, (b) the source-owned SQLite `durability_work`/`work_items` convention built on top of `source_state_directory()` (specs.md §13-§15), or (c) MZA Protocol 1 release construction and the `lexicon-bundle` adapter (specs.md §41). The actual next choice must be re-derived from the contract and the state of `main`, not assumed in advance.
