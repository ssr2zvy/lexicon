Completed milestone: restore trustworthy runtime-execution test coverage

Exact commit tested

f536a64914626679ae8d66876fe7b897152cd6db on branch restore-runtime-execution-test-coverage (containerized verification run via podman machine ssh -> podman exec lexicon-local-test, image lexicon-local-test-image).

Verification result

* cargo check --workspace: passed (exit code 0). Only pre-existing warnings unrelated to this milestone remain (unused imports, deprecated base64::encode/decode, a few dead-code/never-used items) — none introduced by this work.
* cargo test --workspace --quiet: passed (exit code 0). All batches reported test result: ok with 0 failed: 0, 29, 229, 1 (trybuild meta-test covering 12 UI scenarios, all ok), 123, plus 1 intentionally ignored doctest placeholder and 0 doctests. Run 3 times during verification; the last full run is the one recorded here.

Restored HTTP tests (lexicon-core/src/protocols/http/runner.rs, execution_tests, Tests 11-19; 9 tests)

All fixture-backed via crate::session::test_support::RuntimeInvocationFixture, driving run_http_runtime_invocation through the unmodified production path:

* exactly-once acquisition-handler dispatch with a real, mutable HttpAcquisitionContext;
* foreground dispatch reaches the handler;
* background dispatch reaches the handler;
* source-argument fidelity: order, duplicates, empty values, a literal --, reserved-looking flags, Unicode, and non-UTF-8 Unix bytes all preserved byte-for-byte across the invocation boundary;
* a source-authored acquisition error maps to Handler(_) with no implicit reinvocation;
* session state transitions through Prepared -> Running -> Succeeded/Failed as appropriate;
* session-identity mismatch is rejected before the handler is ever dispatched.

Restored processing tests (lexicon-core/src/processing/runner.rs, execution_tests, Tests 11-17; 7 tests)

All fixture-backed via the same RuntimeInvocationFixture, driving run_processing_runtime_invocation through the unmodified production path including the real SQLite open/commit/rollback sequence:

* exactly-once process-handler dispatch with a real, mutable ProcessingContext;
* background dispatch reaches the handler;
* source-argument order and OS representation survive dispatch unchanged;
* a source-authored failure maps to Handler(_) with exactly one dispatch and no reinvocation;
* session state transitions to Succeeded after a successful handler and to Failed after a source-authored failure;
* session/lease identity mismatch is rejected before the handler is ever dispatched.

Fixture design (lexicon-core/src/session/test_support.rs, RuntimeInvocationFixture)

Builds the real minimum execution environment the production invocation path requires: a temp directory tree (protocol_root, data/raw, data/processed, and unconditionally get-raw-data — required because processing transaction discovery validates that root exists even for processing-only invocations); a real SessionStore opened against that tree; a Prepared session created via create_prepared and read back as a genuine SessionIdentity; a held SessionLease acquired via store.acquire_lease; and a valid LEXICON_RUNTIME_CONTEXT_V1 value set through a mutex-serialized RuntimeContextEnvGuard that restores the prior environment value on drop, including during panic unwinding. foreground_run/background_run/new constructors, plus session()/store()/build_argv() accessors, are shared by both the HTTP and processing test modules. No production API was added to allow tests to inject a context directly.

Deterministic ETXTBSY / transient-spawn-race handling

* lexicon-framework/src/build/runtime_probe.rs: all 3 probe tests that spawn fixture scripts now use a bounded (3-attempt) retry_on_spawn_busy helper limited to io::ErrorKind::ExecutableFileBusy, which fails the test with the original error on exhaustion rather than printing and returning success.
* lexicon-framework/src/build/runtime_staging.rs: the prior fixture_or_skip! false-success macro was replaced with fixture_verified_processing_runtime_with_retry(), the same bounded-retry pattern, at both call sites.
* An additional, related flake was found and fixed with explicit user approval: lexicon-framework/src/lib.rs and lexicon-framework/src/data/test_support.rs each independently defined a TEST_CWD_LOCK mutex; since process CWD is global OS state, the two non-communicating locks provided no real mutual exclusion against a concurrently spawned cargo metadata subprocess. The two locks were unified into the single shared lock in data/test_support.rs, and a bounded retry (limited to the "Could not locate working directory" transient error) was added around workspace_metadata_validation_accepts_valid_workspace as defense in depth, since the unification alone did not fully eliminate the underlying container resource-contention race. This same race was observed and successfully absorbed by the retry during the final verification run.

Confirmations

* No required test remains ignored, deleted, or falsely successful; the one ignored doctest placeholder is a pre-existing, unrelated intentional stub.
* No production contract was weakened to make test setup easier, and no new feature scope was added. The TEST_CWD_LOCK unification and workspace_metadata retry are test-harness-only fixes for flakiness discovered while verifying this milestone, made with explicit user approval, and are called out here transparently as the one respect in which work went slightly beyond the milestone's original enumerated scope.

Next milestone

Per instructions.md step 11 and the contract's stated ordering, the next current.md should begin the smallest durable-state foundation: get-raw-data/state/, a validated RuntimeContextPaths field, HttpAcquisitionContext::source_state_directory(), and scaffold/persistence tests — without yet introducing a universal Core-owned job-queue schema.
