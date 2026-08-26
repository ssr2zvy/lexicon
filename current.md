# Implementation report: paired runtime publication

Implemented the public paired publication flow required by the current source-build milestone.

Summary

- Added a new public `publish_runtime_pair` API in `lexicon-framework/src/publication/runtime_pair.rs` and exported it through the publication module and crate API.
- Kept the single-runtime replacement primitive internal-only; no public single-runtime publication API was added.
- Refactored the reversible replacement primitive to accept an internal operation-neutral staged directory boundary produced from either a staged HTTP acquisition bundle or a staged processing bundle.
- Separated logical commit from backup cleanup so both legs can be committed before cleanup, with backup deletion errors downgraded to cleanup warnings instead of publication failures.
- Added pair-level validation for destination relationships, identity checks, staged admission, post-install admission, rollback sequencing, and cleanup warnings.
- Preserved the legacy single-bundle path and existing admission/probe behavior as required.

Key behavior

- Destination validation rejects equal paths, ancestor/descendant relationships, invalid parent directories, symlinked parents, and swapped operation identities before any mutation.
- Both staged bundles are admitted before the first rename, preventing a bad processing bundle from causing a partially mutated acquisition destination.
- The publication sequence prepares acquisition and processing replacements, validates both installed destinations, marks both prepared replacements committed, and then removes obsolete backups.
- Rollback occurs in reverse order (processing then acquisition) and accumulates rollback errors rather than stopping at the first failure.
- Cleanup failures are surfaced as typed `RuntimePairCleanupWarning` values and do not invalidate an otherwise successful publication.

Validation

The required cargo validation flow succeeded in this environment:

- `cargo test -p lexicon-framework --quiet` ✅
- `cargo test --workspace --quiet` ✅

No external MZA/install helper run was needed here; the repo-level Cargo validation covered the implemented behavior. The external bundle installer path was not modified.

Files updated for the feature

- `lexicon-framework/src/publication/runtime_pair.rs`
- `lexicon-framework/src/publication/runtime_bundle_replacement.rs`
- `lexicon-framework/src/build/runtime_staging.rs`
- `lexicon-framework/src/publication/mod.rs`
- `lexicon-framework/src/lib.rs`

Status

The paired acquisition-and-processing publication coordinator is in place and validated under the project’s Cargo workspace tests, with the remaining repository behavior unchanged from the required legacy and admission contracts.
Do not implement:

* integration with source build;
* final runtime directory naming policy;
* durable cross-crash publication journal;
* managed runner generation;
* source workspace migration;
* runner::run;
* runtime execution;
* invocation envelopes;
* HTTP execution;
* raw recording;
* processing SQLite behavior;
* sessions;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* paired publication API;
* internal neutral staged-directory transfer;
* destination validation;
* prepublication admission;
* exact preparation and validation sequence;
* logical commit versus backup cleanup;
* rollback order and aggregation;
* typed publication errors and cleanup warnings;
* successful replacement results;
* forced failure and rollback results;
* confirmation that no public single-runtime publication exists;
* explicit crash-safety limitation;
* repeated workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not integrate paired publication into source build.