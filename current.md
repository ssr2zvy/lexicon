Current implementation request: close fixture-race validation

Objective

Complete the validation required for the runtime-probe fixture-race fix.

The implementation report confirms path isolation, but it does not include the required repeated failing-test and workspace runs. Do not begin another feature until those checks are green.

Required actions

Run the formerly failing test five times:

for attempt in 1 2 3 4 5; do
    cargo test -p lexicon-framework \
        build::runtime_bundle_admission::tests::manifest_too_large_is_rejected \
        --quiet || exit 1
done

Run the complete framework suite three times:

for attempt in 1 2 3; do
    cargo test -p lexicon-framework --quiet || exit 1
done

Run the complete workspace suite three times:

for attempt in 1 2 3; do
    cargo test --workspace --quiet || exit 1
done

Failure behavior

If any run fails:

1. Record the exact failing test and error.
2. Determine whether another shared mutable fixture remains.
3. Fix only the test-isolation or child-cleanup defect.
4. Repeat all required validation from the beginning.

Do not hide the race by:

* marking tests ignored;
* forcing single-threaded workspace tests;
* adding arbitrary sleeps;
* retrying individual failed assertions internally;
* placing the entire suite behind a global lock.

Preserve existing behavior

Do not change production:

* probing;
* hashing;
* verification;
* admission;
* invocation envelopes;
* staging;
* publication;
* CLI commands;
* MZA or lexicon-bundle.

Explicit exclusions

Do not implement:

* invocation transport;
* child admission;
* managed runners;
* source scaffolding migration;
* HTTP execution;
* sessions;
* supervision;
* any new architectural feature.

Completion report

Replace current.md with a report containing:

* five formerly failing test results;
* three framework-suite results;
* three workspace-suite results;
* any additional race found and corrected;
* confirmation that no sleeps, ignored tests, global serialization, or production behavior changes were introduced.

Then stop.