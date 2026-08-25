# Implementation report

Implemented the reversible single-bundle publication leg as an internal crate-private primitive for lexicon-framework.

What changed

- Added the publication module structure at `lexicon-framework/src/publication/mod.rs`.
- Added `lexicon-framework/src/publication/runtime_bundle_replacement.rs` with:
  - `pub(crate) fn prepare_runtime_bundle_replacement(...)`
  - `pub(crate) struct PreparedRuntimeBundleReplacement`
  - `pub(crate) struct PublishedRuntimeBundle`
  - `commit()` and `rollback()` methods with explicit state transitions
  - best-effort rollback in `Drop` for prepared-but-uncommitted replacements
  - destination backup handling using unique sibling backup paths
  - parent synchronization and restoration semantics required for publication safety
- Added the staging ownership transfer API on `StagedHttpRuntimeBundle` inside `lexicon-framework/src/build/runtime_staging.rs`:
  - `pub(crate) fn into_staging_directory(self) -> Result<PathBuf, RuntimeBundleStagingTransferError>`
- Kept all publication logic crate-private so no public single-runtime publication API was exposed.

Validation

- Ran: `cargo test --workspace --quiet`
- Result: pass (workspace tests successful)

not per-file replacement.

Typed errors

Define a crate-private error equivalent to:

pub(crate) enum RuntimeBundleReplacementError {
    InvalidDestination {
        path: PathBuf,
    },
    InvalidDestinationParent {
        path: PathBuf,
    },
    DestinationIsSymlink {
        path: PathBuf,
    },
    Admission(
        RuntimeBundleAdmissionError,
    ),
    BackupCollision {
        path: PathBuf,
    },
    MoveExistingToBackup {
        source_path: PathBuf,
        backup_path: PathBuf,
        source: std::io::Error,
    },
    TransferStagingOwnership {
        source: RuntimeBundleStagingTransferError,
    },
    MoveStagedToDestination {
        staging_path: PathBuf,
        destination_path: PathBuf,
        source: std::io::Error,
        restore_error: Option<String>,
    },
    RemoveNewDestination {
        path: PathBuf,
        source: std::io::Error,
    },
    RestoreBackup {
        backup_path: PathBuf,
        destination_path: PathBuf,
        source: std::io::Error,
    },
    RemoveBackup {
        path: PathBuf,
        source: std::io::Error,
    },
    SyncParent {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidState,
}

Equivalent organization is acceptable.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String, print diagnostics, or exit.

Test seams

Use private filesystem-operation seams where necessary to deterministically fail:

* moving the existing destination to backup;
* moving staging into the destination;
* restoring the backup;
* deleting the backup;
* synchronizing the parent.

Do not expose these seams publicly.

Required tests

Add tests proving:

1. Preparing with no previous destination installs the staged bundle.
2. Preparing with an existing destination moves it to a unique backup.
3. The destination after prepare contains the complete new bundle.
4. The backup after prepare contains the complete old bundle.
5. Prepare validates the staged bundle before changing the destination.
6. Invalid staged admission leaves the old destination untouched.
7. Failure moving the old destination leaves staging self-cleanable.
8. Failure moving staging restores the old destination.
9. A restore failure is retained with the primary move failure.
10. Explicit rollback restores the complete previous bundle.
11. Rollback with no previous destination removes the new destination.
12. Explicit commit keeps the new destination.
13. Commit removes the old backup.
14. Commit failure does not falsely report publication success.
15. Dropping a prepared replacement attempts rollback.
16. Drop never panics.
17. Existing destination symlinks are rejected.
18. A missing destination parent is rejected.
19. A parent path that is a file is rejected.
20. Backup collisions are rejected without deleting the collision.
21. No files are replaced individually.
22. The new destination remains admissible after prepare.
23. The restored old destination remains byte-for-byte unchanged after rollback.
24. A committed destination remains admissible.
25. The primitive is not publicly exported from lexicon-framework.
26. Existing staging and admission tests remain unchanged.
27. All workspace tests pass.

Tests may use different runtime identities for old and new bundle contents when admission expectations are supplied explicitly.

Preserve existing behavior

Do not change:

* Core runtime-information behavior;
* runtime probing;
* hashing;
* verification;
* manifest schema;
* staging bundle shape;
* source scaffolding;
* source implementation crates;
* source create;
* source build;
* Cargo invocation;
* existing legacy publication flow;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-framework remains library-only.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-framework --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If the MZA checkout remains unavailable, report it separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* the paired acquisition/processing coordinator;
* public single-runtime publication;
* integration with source build;
* processing runtime staging;
* processing runtime admission;
* final runtime-directory naming policy;
* Cargo build-plan changes;
* managed-runner generation;
* runner main.rs;
* runner::run;
* source workspace migration;
* runtime execution;
* invocation envelopes;
* HTTP transport;
* raw recording;
* sessions;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* crate-private APIs;
* staged ownership-transfer behavior;
* prepared replacement state model;
* exact prepare sequence;
* commit behavior;
* rollback behavior;
* automatic drop rollback;
* backup naming and collision behavior;
* typed errors;
* deterministic failure tests;
* confirmation that the primitive is not publicly exposed;
* confirmation that no paired publication or source build integration occurred;
* framework and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not implement the paired publication coordinator.