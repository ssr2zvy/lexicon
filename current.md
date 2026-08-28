Current milestone: implement `lexicon build` (workspace-wide discovery and build)
Objective
Implement the top-level `lexicon build` command so it deterministically discovers every supported source/protocol pairing in the project and invokes the same validated per-source build pipeline already used by `lexicon source build`, reporting per-source success/failure with exact identities.
contract.md §5 lists `lexicon build` as part of the public command boundary. specs.md §40 defines its required behavior in full. Neither is implemented today: `lexicon-cli/src/cli/mod.rs`'s `RootCommand::Build` arm only prints "Parsed build command: build" and performs no discovery, validation, or build invocation. This is a stub, not a partial implementation.
Repository-grounded starting point
`lexicon-framework::commands::source_build(source_name, protocol)` (lexicon-framework/src/lib.rs) is the existing validated single-source build pipeline: it validates the source name and protocol, resolves the project root via `find_project_root`, resolves the configured sources directory via `configured_sources_directory`/`load_project_config`, validates the managed acquisition/processing workspace layout and metadata, builds both managed runners, and publishes both runtime bundles atomically per source (via `publish_runtime_pair`). This per-source pipeline must not be reimplemented or weakened; `lexicon build` must reuse it directly.
`find_project_root` and `configured_sources_directory`/`load_project_config` (lexicon-framework/src/lib.rs) already provide validated project discovery and the resolved sources root; reuse them rather than re-deriving project/paths logic.
`lexicon-cli/src/cli/build.rs` defines `BuildCommand` as a unit struct (`lexicon build` takes no arguments today); `lexicon-cli/src/cli/mod.rs`'s `dispatch` function's `RootCommand::Build(_)` arm is the integration point to replace.
`lexicon source create` only supports the `http` protocol today (`validate_protocol` in lexicon-framework/src/lib.rs), so the only protocol directory that can legitimately exist under a source is `http`; discovery logic should be written generically (do not hardcode assumptions that make adding a second protocol later require restructuring), but only needs to actually recognize `http` right now.
Required implementation
1. Discovery
Add a new `lexicon-framework::commands` function (for example `build_all`) that:
1. discovers the containing project via `find_project_root` from the current directory;
2. resolves the configured sources directory via `load_project_config`/`configured_sources_directory`;
3. enumerates the immediate subdirectories of the sources root as candidate source names, applying the same `validate_source_name` rule used elsewhere (skip/reject consistently — see "ambiguous or invalid layouts" below);
4. for each candidate source, enumerates its immediate subdirectories as candidate protocol identities, recognizing only `http` as supported right now via the existing `validate_protocol`;
5. treats a source/protocol pairing as a build target only if `source.toml` exists directly under `sources/<source>/<protocol>/` (the same marker `build_source` already requires) — this is the deterministic discovery signal, not merely directory presence;
6. produces a stable, deterministically ordered list of discovered `(source_name, protocol)` pairs (e.g. sorted lexicographically by source name then protocol) so output and test behavior do not depend on filesystem iteration order.
Reject ambiguous or invalid layouts rather than silently skipping them: a source directory containing a non-`http` entry that is not itself a recognized protocol directory and not clearly incidental (decide and document a precise, narrow rule — for example, any directory entry under a candidate source directory that is neither a recognized supported protocol directory nor an expected incidental entry must cause discovery to fail with a specific, actionable error identifying the offending path) must cause `lexicon build` to fail its discovery phase with a clear error rather than proceeding with a partial or guessed source list. Do not treat an empty sources directory as an error; it is a valid (trivial) project with zero build targets.
2. Build invocation and aggregate reporting
For each discovered `(source_name, protocol)` pair, in the deterministic order established above, invoke the existing `source_build`/`build_source` pipeline unchanged. Attempt every discovered pairing even if an earlier one fails — do not stop at the first failure — and collect a per-pairing result (success with its `SourceBuildResult`, or failure with its error) into an aggregate result returned to the caller. The CLI layer must then report every failure with its exact source name and protocol identity (per specs.md §40 item 6), and must report overall command failure (non-zero/`Err` from `dispatch`) if any pairing failed, even if others succeeded.
Do not implement a project-wide all-or-nothing publication transaction across sources — specs.md §40 explicitly defers this ("may remain deferred until supported by implementation evidence"). Each source's own publish step is already atomic per `publish_runtime_pair`; that per-source atomicity is sufficient for this milestone.
3. CLI wiring
Replace the `RootCommand::Build(_)` stub arm in `lexicon-cli/src/cli/mod.rs` with a call into the new `lexicon_framework::commands::build_all` (or equivalently named) function, printing a summary line per discovered source/protocol pairing (mirroring the existing style used for `source create`/`source build` output) and a final summary indicating how many succeeded/failed. Return `Err` from `dispatch` when any pairing failed, with a message that enumerates the failed source/protocol identities and their errors, consistent with how other `dispatch` arms already convert framework errors to `Result<(), String>`.
4. Tests
Add tests proving at least:
* discovery finds zero targets in a project with an empty (or absent) sources directory and `lexicon build` reports success with nothing built;
* discovery finds and builds a single valid source/protocol pairing end-to-end (using a real project created via `commands::init` + `commands::source_create`, then `commands::build_all`, verifying the same managed runtime artifacts that `source_build` alone would produce);
* discovery finds and builds multiple valid source/protocol pairings in one `lexicon build` invocation, and results are reported in the deterministic sort order;
* a failure in one discovered pairing (for example, an intentionally corrupted or incomplete managed workspace for one source) does not prevent other valid pairings from being attempted and successfully built, and the aggregate result/error correctly names the failed source and protocol while also reporting the others' success;
* an ambiguous/invalid layout (per the narrow rule chosen above) causes discovery itself to fail with an error identifying the offending path, before any build is attempted;
* the CLI-level `dispatch` test coverage (alongside the existing `lexicon-cli/src/cli/mod.rs` tests for `source create`/`source build`) confirms `lexicon build` is wired to the framework function rather than remaining a placeholder, using the same `with_test_cwd` pattern already used by neighboring tests in that file.
Retain all existing tests for `source_build`, `source_create`, `init`, discovery (`find_project_root`, `configured_sources_directory`), and managed workspace validation unmodified except where a genuine, minimal signature/call-site change is required to reuse them from the new discovery function.
Scope constraints
Do not implement during this milestone:
* MZA Protocol 1 release construction, the `lexicon-bundle` adapter, or any complete-product release packaging (specs.md §41) — `lexicon build` only performs source builds, per specs.md's explicit separation between source build and product release construction;
* a project-wide all-or-nothing publication transaction across sources (explicitly deferred by specs.md §40);
* any SQLite work-ledger, scaffold-generation changes, or other functionality unrelated to workspace-wide build discovery;
* support for any protocol other than `http`;
* changes to `build_source`'s internal per-source validation/build/publish logic beyond what is strictly required to invoke it from the new discovery loop (e.g., do not change its error types unless required to attach source/protocol identity for aggregate reporting, and prefer wrapping over restructuring);
* parallelizing the per-source builds (sequential, deterministic order is sufficient for this milestone);
* changes to `lexicon data`, `lexicon init`, `lexicon source create`, background/operator-host execution, or any HTTP/session/runtime-context code.
Completion criteria
This milestone is complete only when:
* `lexicon build` deterministically discovers every valid source/protocol pairing under the project's configured sources directory;
* it invokes the existing validated `source_build` pipeline for each discovered pairing without weakening that pipeline;
* ambiguous or invalid layouts are rejected with an actionable error rather than silently skipped or guessed;
* failures are reported with the exact source name and protocol identity, and do not prevent other valid pairings from being attempted;
* `cargo check --workspace` passes;
* `cargo test --workspace --quiet` passes;
* no production contract was weakened to make test setup easier;
* no out-of-scope functionality (MZA/release construction, work-ledger, protocol additions) was added.
Completion report
When the milestone passes, replace this file with a concise report containing:
* the exact commit tested;
* confirmation that `cargo check --workspace` passed;
* confirmation that `cargo test --workspace --quiet` passed;
* where the new discovery/aggregation function was added and how `lexicon-cli`'s `dispatch` was wired to it;
* the number and categories of new tests added;
* confirmation that no required test remains ignored, deleted, or falsely successful;
* confirmation that no unrelated feature work (MZA/release construction, work-ledger, scaffold generation, additional protocols) was included.
Then stop.
The following milestone should be derived from the updated contract and specification once this one lands. Candidates in rough dependency order include: the source-owned SQLite work-ledger convention built on top of `source_state_directory()` (specs.md §13-§15), MZA Protocol 1 release construction and the `lexicon-bundle` adapter (specs.md §41), or closing any remaining gaps identified by re-reading contract.md/specs.md against the state of `main` at that time. The actual next choice must be re-derived, not assumed in advance.
