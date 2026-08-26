Current implementation request: eliminate runtime-probe fixture race

Objective

Fix the intermittent ExecutableFileBusy workspace-test failure before adding another architectural feature.

The known failure is:

build::runtime_bundle_admission::tests::
    manifest_too_large_is_rejected
Probe(
    Spawn {
        source: Os {
            code: 26,
            kind: ExecutableFileBusy,
            message: "Text file busy",
        },
    },
)

This micro-step is test-infrastructure hardening only.

Do not change production runtime probing, admission, staging, publication, or invocation behavior.

Root cause to verify

Inspect every runtime-probe test fixture for shared executable paths or writes occurring near execution.

The expected cause is:

one test writes or replaces a fixture executable
while another test tries to execute that same path

On Linux this can produce ETXTBSY.

Confirm the actual cause in the completion report rather than assuming it.

Required fixture isolation

Every test that creates, writes, changes, or executes a runtime fixture must use:

* its own TempDir;
* its own executable path inside that directory;
* no shared mutable global fixture path;
* no fixed filename in a shared directory;
* no reuse of another test’s candidate executable.

The same filename may be used inside different unique temporary directories.

Write-before-execute boundary

Fixture creation must follow this order:

1. Create the test’s unique temporary directory.
2. Create the fixture executable.
3. Write all fixture bytes.
4. Flush the file.
5. Call sync_all() where appropriate.
6. Set required permissions.
7. Drop every writable file handle.
8. Only then pass the path to hashing, probing, verification, staging, or admission.

No writable handle to the executable may remain open when the process is spawned.

Immutable fixture behavior

After a fixture becomes executable, ordinary tests must treat that fixture path as immutable.

Tests requiring changed bytes must:

* use a separate unique path; or
* perform mutation through an explicitly synchronized private test seam where no process is executing the file.

Do not overwrite an executable that another thread or process may currently be running.

Shared test helper

Add or refactor a private test helper such as:

struct RuntimeProbeFixture {
    _directory: TempDir,
    executable: PathBuf,
}

Provide operation-specific fixture constructors for deterministic modes, for example:

RuntimeProbeFixture::http_valid(...)
RuntimeProbeFixture::processing_valid(...)
RuntimeProbeFixture::malformed(...)
RuntimeProbeFixture::nonzero_exit(...)
RuntimeProbeFixture::delayed(...)

Equivalent organization is acceptable.

The fixture owner must remain alive for the entire test.

Do not expose fixture helpers in production APIs.

Audit scope

Audit tests under at least:

lexicon-framework/src/build/runtime_probe.rs
lexicon-framework/src/build/runtime_verification.rs
lexicon-framework/src/build/runtime_staging.rs
lexicon-framework/src/build/runtime_bundle_admission.rs
lexicon-framework/src/publication/

Search for:

* fixed temporary executable names;
* shared paths;
* File::create followed by execution without dropping the handle;
* fs::write against an executable currently used elsewhere;
* current-directory mutation;
* global environment mutation;
* fixture cleanup while a child may still be alive.

Correct only test infrastructure needed to eliminate races.

Child cleanup verification

Ensure every successfully spawned fixture child is reaped before its fixture TempDir is dropped.

Timeout tests must:

1. kill the child;
2. wait for it;
3. join output readers;
4. then allow fixture cleanup.

Production behavior must remain unchanged

Do not change:

* probe argument;
* process timeout;
* output limits;
* error precedence;
* hashing rules;
* admission rules;
* runtime schemas;
* staging layout;
* publication behavior.

If production code is already correct, modify only tests and private test helpers.

If the failure reveals an actual child-reaping defect in production probing, fix only that defect and report it explicitly.

Required tests

Verify:

1. manifest_too_large_is_rejected passes alone.
2. It passes while other probe tests run in parallel.
3. Acquisition probe tests use isolated executable paths.
4. Processing probe tests use isolated executable paths.
5. Mutation tests do not overwrite executing fixtures.
6. Timeout children are reaped before fixture cleanup.
7. No writable fixture handle remains open at spawn.
8. No test relies on one shared mutable runtime executable.
9. Existing failure classifications remain unchanged.
10. Runtime invocation-envelope tests remain unchanged.
11. All workspace tests pass repeatedly.

Required validation

Run the formerly failing test repeatedly:

for attempt in 1 2 3 4 5; do
    cargo test -p lexicon-framework \
        build::runtime_bundle_admission::tests::manifest_too_large_is_rejected \
        --quiet || exit 1
done

Run the framework suite repeatedly:

for attempt in 1 2 3; do
    cargo test -p lexicon-framework --quiet || exit 1
done

Run the workspace suite repeatedly:

cargo test --workspace --quiet
cargo test --workspace --quiet
cargo test --workspace --quiet

Do not serialize the entire test suite behind one global lock unless a truly process-global resource cannot be isolated.

Path isolation is preferred.

Preserve existing behavior

Do not change:

* invocation-envelope APIs or JSON;
* source descriptors;
* runtime-information schemas;
* probing semantics;
* manifests;
* staging;
* bundle admission;
* paired publication;
* source scaffolding;
* source create;
* source build;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior.

Explicit exclusions

Do not implement:

* invocation command-line transport;
* child runtime admission;
* managed runners;
* runner::run;
* runtime execution;
* HTTP transport;
* raw recording;
* processing;
* sessions;
* supervision;
* __operator-host;
* build integration;
* MZA dependency recovery.

Completion report

After completion, replace current.md with a report containing:

* confirmed root cause;
* files changed;
* fixture ownership and isolation model;
* write/flush/drop ordering;
* child-reaping confirmation;
* whether production code required any correction;
* formerly failing test results across five runs;
* framework results across three runs;
* workspace results across three runs;
* confirmation that production behavior was unchanged.

Then stop. Do not add another runtime feature until the repeated workspace validation is green.