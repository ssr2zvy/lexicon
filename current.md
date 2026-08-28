The next milestone should restore verification integrity before implementing the new durable-state architecture. The suite now compiles, but some essential runner tests were deleted and several ETXTBSY branches return successfully without executing their assertions—directly conflicting with the updated contract.

Current milestone: restore trustworthy runtime-execution test coverage

Objective

Establish a clean, credible compilation and test baseline after the first real workspace build exposed accumulated failures.

The repository now compiles far enough for meaningful verification, but the corrective commits introduced two forms of verification debt:

1. HTTP and processing runner tests that exercised successful handler execution were deleted because they lacked the session-store, prepared-session, lease, and runtime-context fixtures now required by the production invocation path.
2. Several runtime-probe and staging tests treat ETXTBSY by printing a message and returning from the test. Rust records those returns as successful test executions, even though the intended assertions never ran.

Before beginning the newly specified durable source-state and work-ledger implementation, restore this missing coverage and obtain a trustworthy passing workspace test result.

This milestone is derived from:

* workspace/specs/contract.md, especially the required-verification and test-integrity requirements;
* workspace/specs/specs.md, especially the source-contract, session, supervision, environment-handling, and required-test sections;
* commit 24399ef883baf7dceb219893fce9712d32885d7e, which removed obsolete runtime execution tests pending a real fixture;
* commit dbc5ca83c974b596d6eedb2be6bcb9a0d3131abf, which added successful early returns for ETXTBSY;
* the obsolete current.md, which prohibited deleting, weakening, or silently skipping tests to obtain a passing suite.

The target baseline is repository commit:

6ec9d485e03691fbf25cd3e1a47e3f306420d7e4

or its direct descendant containing only work required by this milestone.

Required implementation

1. Build reusable runtime-invocation test fixtures

Add proper test support for both:

lexicon-core HTTP runtime invocation
lexicon-core processing runtime invocation

Each fixture must create the real minimum execution environment expected by the production invocation path:

* a temporary project and operation directory;
* a real SessionStore backed by the temporary directory;
* a valid prepared session record;
* a held session lease;
* a matching runtime invocation envelope;
* a valid LEXICON_RUNTIME_CONTEXT_V1 environment value;
* matching compiled project, source, protocol, operation, and session identities;
* validated runtime paths required to construct the real context.

Do not add a production backdoor that permits tests to inject a context through an API unavailable to real runtimes.

Prefer a shared test-support abstraction where HTTP and processing genuinely require the same setup. Keep operation-specific setup separate where their invariants differ.

Because environment variables are process-global, tests that mutate LEXICON_RUNTIME_CONTEXT_V1 must be serialized or otherwise isolated. Every fixture must restore the prior environment state when it is dropped, including during panic unwinding.

2. Restore HTTP handler-execution coverage

Restore behavior-level tests proving at least:

* a valid matching invocation reaches the acquisition handler;
* the handler is called exactly once;
* the real mutable HttpAcquisitionContext reaches the handler;
* foreground invocation reaches the handler;
* background invocation reaches the handler;
* source arguments arrive in exact order;
* duplicate and empty argument values are preserved;
* a source argument equal to -- is preserved;
* reserved-looking source arguments are preserved after the invocation boundary;
* Unicode arguments are preserved;
* non-UTF-8 Unix arguments are preserved byte-for-byte;
* successful acquisition returns success;
* a source-authored acquisition error returns the correct handler-error category;
* a handler error does not cause implicit reinvocation;
* the session transitions through the expected prepared, running, and terminal states;
* the lease and invocation session identities must agree.

Do not blindly restore tests whose premise depended on the removed caller-supplied-context API. Rewrite those tests against the actual production construction path or omit only the obsolete premise while preserving the underlying invariant.

3. Restore processing handler-execution coverage

Restore the corresponding processing tests proving at least:

* a valid matching invocation reaches the processing handler;
* the handler is called exactly once;
* the real mutable ProcessingContext reaches the handler;
* foreground and background invocation modes reach the handler;
* source arguments retain their exact order and operating-system representation;
* success produces the expected result and terminal session state;
* a source-authored processing failure produces the correct handler-error category and failed terminal state;
* a handler failure does not cause implicit reinvocation;
* session, lease, project, source, protocol, and operation identities are validated before dispatch.

Retain the existing pure transport and admission-rejection tests.

4. Remove false-success ETXTBSY handling

Remove test branches that handle ExecutableFileBusy by returning successfully before the assertion.

Affected areas currently include:

lexicon-framework/src/build/runtime_probe.rs
lexicon-framework/src/build/runtime_staging.rs

Replace the racy script-fixture mechanism with a deterministic approach.

Acceptable approaches include:

* creating fixture executables during test setup before the test process begins using them;
* copying an already built helper executable into a unique temporary location;
* using one dedicated helper binary with explicit probe-output modes;
* restructuring fixture creation so no writable handle or overlay copy-up operation races with execution;
* bounded retry around the fixture spawn only when retry is isolated to the test harness and an exhausted retry fails the test.

A retry must:

* be limited to ExecutableFileBusy;
* have a small fixed attempt bound;
* preserve the original error when exhausted;
* never convert exhaustion into success;
* never be added to production runtime admission merely to accommodate a test fixture.

Printing “skipping” and returning from a Rust #[test] is not a valid skip because the test harness reports it as passed.

If an environment genuinely cannot execute the required test, it must be reported distinctly by the surrounding test workflow and covered in a supported environment. It must not appear as a passing assertion.

5. Preserve the corrected production behavior

Do not revert valid corrections made during the compilation checkpoint, including:

* the corrected hidden __operator-host command name;
* byte-exact runtime-manifest boundary validation;
* generic redacted Display output for source-authored errors;
* borrow and ownership corrections in HTTP attempt execution;
* current session-store, lease, and runtime-context admission requirements;
* LF normalization rules for shell scripts and container definitions.

Required verification

The user must run the repository’s containerized verification workflow:

podman build \
    -f containerization/test-container/Containerfile \
    -t lexicon-local-test-image \
    .
podman run \
    -d \
    --name lexicon-local-test \
    -v "$PWD":/lexicon \
    --workdir /lexicon \
    lexicon-local-test-image
podman exec lexicon-local-test \
    bash -lc 'cd /lexicon && cargo check --workspace'
podman exec lexicon-local-test \
    bash -lc 'cd /lexicon && cargo test --workspace --quiet'

If the container already exists, recreate it when its image or dependencies changed; otherwise it may be started and reused.

The final test output must demonstrate that:

* the workspace compiles;
* the complete workspace test suite passes;
* restored HTTP handler tests actually invoke their handler;
* restored processing handler tests actually invoke their handler;
* no affected test reports success through an early-return ETXTBSY branch;
* no required test is marked ignored;
* no material execution test was deleted merely to obtain a passing result.

Scope constraints

Do not implement during this milestone:

* get-raw-data/state/;
* source_state_directory();
* a SQLite work ledger;
* DurableWorkV1;
* source-manifest schema 2;
* the public data --protocol correction;
* lexicon build;
* the embedded Core revision correction;
* foreground signal-forwarding changes;
* background handoff race corrections;
* new acquisition or processing features;
* MZA changes;
* unrelated refactoring.

Those are contract-derived milestones, but they must be built on a trustworthy verified baseline.

Completion criteria

This milestone is complete only when:

1. Real session-bound HTTP invocation fixtures exist.
2. Real session-bound processing invocation fixtures exist.
3. Successful handler-dispatch coverage has been restored for both operations.
4. argument-fidelity, success, failure, and exactly-once-handler-invocation behavior are tested.
5. ETXTBSY cannot cause an affected test to be reported as passed without executing its assertion.
6. cargo check --workspace passes.
7. cargo test --workspace --quiet passes.
8. No production contract was weakened to make test setup easier.
9. No new feature scope was added.

Completion report

When the milestone passes, replace this file with a concise report containing:

* the exact commit tested;
* confirmation that cargo check --workspace passed;
* confirmation that cargo test --workspace --quiet passed;
* the number and categories of restored HTTP tests;
* the number and categories of restored processing tests;
* the fixture design used to satisfy session-store, lease, and runtime-context requirements;
* the deterministic solution used for the overlay-filesystem executable race;
* confirmation that no required test remains ignored, deleted, or falsely successful;
* confirmation that no unrelated feature work was included.

Then stop.

The following milestone should be derived from the updated contract and specification. Unless new evidence changes the ordering, it should begin the smallest durable-state foundation:

get-raw-data/state/
+ validated RuntimeContextPaths field
+ HttpAcquisitionContext::source_state_directory()
+ scaffold and persistence tests

It should not yet introduce a universal Core-owned job-queue schema.