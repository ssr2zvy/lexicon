Current milestone: add the validated durable source-state directory (get-raw-data/state/)
Objective
Establish the minimum durable source-state foundation defined by the contract, before any source-owned work-ledger or fan-out implementation is built on top of it.
contract.md §9 reserves get-raw-data/state/ as the supported durable-state boundary for an acquisition source and requires Core to expose it through a validated context API. specs.md §11-§12 define the concrete RuntimeContextPaths field and HttpAcquisitionContext::source_state_directory() accessor, and the properties that directory must satisfy. Neither exists in the repository yet: RuntimeContextPaths (lexicon-core/src/session/context.rs) has no source_state_directory field, and HttpAcquisitionContext (lexicon-core/src/protocols/http/context.rs) exposes no source_state_directory() method.
This milestone is derived from:
contract.md §9 (durable source state and work ledgers) and §10 (four distinct durable concepts);
specs.md §11 (runtime context paths) and §12 (durable source state);
specs.md §44 (required tests, "durable source state" section: validated state path, state survives sessions, state survives runtime rebuild and publication);
the prior current.md's own derived next-milestone recommendation, written after restoring trustworthy runtime-execution test coverage on commit f536a64914626679ae8d66876fe7b897152cd6db.
Repository-grounded starting point
RuntimeContextPaths::new (lexicon-core/src/session/context.rs) currently validates and stores exactly: project_root, protocol_root, operation_root, session_directory, raw_data_directory, processed_data_directory. It has three call sites that must all be updated consistently: decode_runtime_context (lexicon-core/src/session/context.rs), build_session_paths (lexicon-framework/src/session/coordinator.rs, used by both create_prepared_launch and resume_prepared_launch), and RuntimeInvocationFixture::new (lexicon-core/src/session/test_support.rs).
HttpAcquisitionContext (lexicon-core/src/protocols/http/context.rs) is constructed from SessionDataPaths (lexicon-core/src/session/context.rs), which is itself derived from RuntimeContextPaths via from_context_paths, or built directly via from_legacy_parts for the #[doc(hidden)] from_env_legacy path.
Required implementation
1. Add the validated source-state field
Add source_state_directory: Option<PathBuf> to RuntimeContextPaths. It must be Some(operation_root.join("state")) when, and only when, the operation is RuntimeOperation::Acquisition, and None for RuntimeOperation::Processing — the contract reserves this directory specifically for acquisition sources, not processing. Validate the relationship inside RuntimeContextPaths::new the same way operation_root, raw_data_directory, and processed_data_directory are already validated (reject any caller-supplied value that disagrees with the derived path), rather than trusting a caller-supplied path.
Add a source_state_directory() -> Option<&Path> accessor alongside the other RuntimeContextPaths accessors.
Propagate the new field through encode_runtime_context / RuntimeContextDocumentV1 (the on-the-wire LEXICON_RUNTIME_CONTEXT_V1 document) and decode_runtime_context, preserving the existing per-platform native-path encoding (unix-bytes-base64 / windows-utf16). Decide and document how the field round-trips when None (for processing invocations) — it must not silently become Some after a decode round trip, and a processing envelope must not be able to smuggle a source_state_directory value.
Update the three call sites enumerated above (decode_runtime_context, build_session_paths, RuntimeInvocationFixture::new) to supply/derive this field consistently with the new validation rule.
2. Extend SessionDataPaths and HttpAcquisitionContext
Add an equivalent optional field to SessionDataPaths (from_context_paths and from_legacy_parts), and expose it as source_state_directory() -> Option<&Path> on HttpAcquisitionContext, matching the accessor style of the existing protocol_root() / operation_root() / raw_data_directory() methods.
Core must create and validate the directory before calling source code, per specs.md §11: when constructing/admitting an acquisition context, ensure get-raw-data/state/ exists (create it if absent) and validate it through the same validate_managed_path machinery already used for the other managed directories, before the acquire or resume handler is invoked. Do not perform this creation/validation for processing contexts, since they have no source-state directory.
Do not derive the returned path from untrusted source arguments; it must come only from the validated RuntimeContextPaths constructed by the parent/child admission path.
3. Preserve the four-durable-concepts boundary
Do not let source_state_directory reads or writes interact with checkpoint commit/lookup, transaction recording, or session persistence code paths. It is a plain validated directory handle; Core does not interpret its contents (per contract.md §9: "managed semantically by the source").
Do not introduce any SQLite schema, work-item type, or persistence helper in this milestone — that is explicitly deferred (see Scope constraints).
4. Tests
Add tests proving at least:
RuntimeContextPaths::new rejects a caller-supplied source_state_directory that disagrees with operation_root/state for an Acquisition operation;
RuntimeContextPaths::new produces source_state_directory() == None for a Processing operation, and rejects a caller-supplied Some(_) value for Processing;
encode_runtime_context / decode_runtime_context round-trip the field faithfully for both Acquisition (Some) and Processing (None), including on the non-UTF-8/Unicode-bearing native-path encoding paths already covered by existing path round-trip tests;
HttpAcquisitionContext::source_state_directory() returns a path scoped to sources/<source>/http/get-raw-data/state/ for a real fixture-backed acquisition invocation (extend crate::session::test_support::RuntimeInvocationFixture usage in lexicon-core/src/protocols/http/runner.rs execution_tests rather than inventing a second fixture);
the directory exists and is writable from inside a real acquire handler invocation (i.e., Core created it before the handler ran);
the directory, once created, survives across two sequential sessions against the same fixture (a persistence/durability test: write a marker file from one fixture-driven invocation, then confirm a second invocation against the same operation root still sees it) — this satisfies specs.md §44's "state survives sessions" requirement at the scaffold level targeted by this milestone.
Retain all existing RuntimeContextPaths / SessionDataPaths / HttpAcquisitionContext tests unmodified except where the new field requires updating a call site's argument list.
Preserve the corrected production behavior
Do not revert any correction from the previous milestone, including the RuntimeInvocationFixture's unconditional get-raw-data creation, the three-way ETXTBSY retry coverage, or the unified TEST_CWD_LOCK.
Required verification
Run the repository's containerized verification workflow (podman machine ssh <machine> "podman exec lexicon-local-test bash -lc 'cd /lexicon && cargo check --workspace'" and the equivalent cargo test --workspace --quiet invocation), per instructions.md step 7. The agent may run this itself.
The final test output must demonstrate that:
the workspace compiles;
the complete workspace test suite passes;
the new source_state_directory field and accessor are exercised by real, fixture-backed tests, not merely by unit tests of the validation logic in isolation;
no existing test was weakened, deleted, or marked ignored to accommodate this change.
Scope constraints
Do not implement during this milestone:
any SQLite work ledger, work_items schema, WorkLedger type, or discovery/fan-out helper (specs.md §13-§15);
DurableWorkV1 or any Core-owned job-queue capability;
source-manifest schema 2 generation, lexicon source create scaffold generation, or lexicon build (specs.md §7, §19, §40) — these remain unimplemented gaps tracked by future milestones, not this one;
the public data --protocol correction;
the embedded Core revision correction;
foreground signal-forwarding changes;
background handoff race corrections;
new acquisition or processing features unrelated to exposing the validated state directory;
MZA changes;
unrelated refactoring.
Completion criteria
This milestone is complete only when:
RuntimeContextPaths has a validated source_state_directory field, present only for Acquisition, matching operation_root/state.
The field round-trips correctly through encode_runtime_context / decode_runtime_context for both Acquisition and Processing.
HttpAcquisitionContext::source_state_directory() returns the validated path, and Core creates/validates the directory before invoking the handler.
cargo check --workspace passes.
cargo test --workspace --quiet passes.
No production contract was weakened to make test setup easier.
No SQLite work-ledger or other explicitly deferred scope was added.
Completion report
When the milestone passes, replace this file with a concise report containing:
the exact commit tested;
confirmation that cargo check --workspace passed;
confirmation that cargo test --workspace --quiet passed;
where the new field/accessor were added and how the three RuntimeContextPaths::new call sites were updated;
the number and categories of new tests added;
confirmation that no required test remains ignored, deleted, or falsely successful;
confirmation that no unrelated feature work (in particular, no work-ledger or scaffold-generation code) was included.
Then stop.
The following milestone should be derived from the updated contract and specification once this one lands. Candidates in rough dependency order include: lexicon source create scaffold generation (specs.md §7), the source-owned SQLite work-ledger convention built on top of source_state_directory() (specs.md §13-§15), or lexicon source build / native build pipeline (specs.md §18-§21). The actual next choice must be re-derived from the contract and the state of main at that time, not assumed in advance.
