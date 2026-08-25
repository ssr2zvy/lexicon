Current implementation request: reversible single-bundle publication leg

Objective

Add the internal reversible filesystem operation needed to publish one staged runtime bundle as one leg of a future paired acquisition-and-processing transaction.

This step must support:

* moving an existing published bundle to a backup;
* atomically moving the staged bundle into its destination;
* explicit rollback;
* explicit commit;
* automatic best-effort rollback if the prepared leg is dropped.

Do not expose a public API that allows framework callers to permanently publish only one half of the acquisition/processing pair.

Architectural position

The completed flow is:

verified candidate
→ staged runtime bundle
→ admitted staged runtime bundle

This micro-step adds an internal primitive:

staged bundle
→ prepare replacement
   ├── old destination moved to backup
   └── staged bundle moved to destination
→ either:
   ├── commit
   └── rollback

A later coordinator will prepare both acquisition and processing legs, then commit them as one paired publication transaction.

Required module

Create:

lexicon-framework/src/publication/runtime_bundle_replacement.rs

Expose it only within lexicon-framework through the existing publication module structure.

Do not publicly re-export this operation from the crate root or public build API.

The replacement primitive must be pub(crate).

Why this remains internal

Lexicon’s architecture requires acquisition and processing runtimes to be published together.

Therefore this micro-step must not create a public operation such as:

pub fn publish_one_runtime(...)

The single-bundle replacement is only a reversible transaction leg for the future paired coordinator.

Published destination

The operation accepts an explicit final bundle path:

published_bundle_path: &Path

This path represents one complete runtime bundle directory containing:

<published-bundle-path>/
├── <executable>
└── runtime.json

Do not decide the final runtime/ version-directory policy in this step.

Do not assume the destination is literally named current.

Required staging ownership change

Add a crate-private consuming operation to StagedHttpRuntimeBundle that transfers ownership of its staging directory to publication code without deleting it.

For example:

impl StagedHttpRuntimeBundle {
    pub(crate) fn into_staging_directory(
        self,
    ) -> Result<PathBuf, RuntimeBundleStagingTransferError>;
}

Equivalent internal design is acceptable.

Requirements:

* consuming transfer prevents the TempDir destructor from deleting the directory;
* the path remains owned by publication logic;
* the transfer is not public outside lexicon-framework;
* ordinary dropped staged bundles still self-clean.

Do not expose the underlying TempDir.

Prepared replacement state

Define a crate-private state type conceptually equivalent to:

pub(crate) struct PreparedRuntimeBundleReplacement {
    destination: PathBuf,
    backup: Option<PathBuf>,
    parent: PathBuf,
    state: ReplacementState,
}

Use an internal state enum such as:

enum ReplacementState {
    Prepared,
    Committed,
    RolledBack,
}

The exact representation may differ, but invalid repeated transitions must be prevented.

Prepare API

Provide a crate-private operation:

pub(crate) fn prepare_runtime_bundle_replacement(
    staged: StagedHttpRuntimeBundle,
    published_bundle_path: &Path,
) -> Result<
    PreparedRuntimeBundleReplacement,
    RuntimeBundleReplacementError,
>;

Preconditions

Before mutating the destination, require:

1. published_bundle_path has a parent directory.
2. The parent exists.
3. The parent is a directory.
4. The parent’s final component is not a symlink.
5. The staged bundle directory and destination parent support atomic rename.
6. If the destination exists, it is a non-symlink directory.
7. The backup path does not already exist.

Do not delete an existing backup to make room.

Use a unique sibling backup name under the destination parent, such as:

.<destination-name>.lexicon-backup-<random>

Prepare sequence

Perform these operations in order:

1. Resolve and validate the destination parent.
2. Validate the staged bundle through the existing disk admission path before transferring ownership.
3. Create a unique unused backup path.
4. If the destination exists, rename it atomically to the backup path.
5. Transfer ownership of the staged directory.
6. Rename the staged directory atomically to the destination path.
7. Synchronize the destination parent where supported.
8. Return PreparedRuntimeBundleReplacement.

Do not copy files during publication.

The staged directory must be renamed as one complete directory.

Failure during prepare

If moving the existing destination to backup succeeds but moving the staged directory to the destination fails:

1. attempt to rename the backup back to the original destination;
2. preserve the original publication failure as the primary error;
3. include any restore failure as typed supplementary information;
4. do not report a prepared replacement.

If the staged ownership transfer has occurred, ensure its directory is either restored to owned cleanup or explicitly removed after failure.

Do not silently abandon a staging or backup directory.

Commit behavior

Provide a consuming crate-private method:

impl PreparedRuntimeBundleReplacement {
    pub(crate) fn commit(
        self,
    ) -> Result<
        PublishedRuntimeBundle,
        RuntimeBundleReplacementError,
    >;
}

Commit must:

1. require Prepared state;
2. remove the backup directory if one exists;
3. synchronize the destination parent where supported;
4. transition permanently to committed state;
5. return an opaque internal published result.

The returned internal result may contain:

pub(crate) struct PublishedRuntimeBundle {
    path: PathBuf,
}

with a crate-private accessor.

Do not add public runtime publication yet.

Rollback behavior

Provide:

impl PreparedRuntimeBundleReplacement {
    pub(crate) fn rollback(
        self,
    ) -> Result<(), RuntimeBundleReplacementError>;
}

Rollback must:

1. remove the newly installed destination bundle;
2. if a previous bundle existed, rename its backup back to the destination;
3. synchronize the parent where supported;
4. transition permanently to rolled-back state.

If no previous destination existed, rollback leaves the destination absent.

Drop behavior

If a prepared replacement is dropped without explicit commit or rollback:

* attempt best-effort rollback;
* never panic from Drop;
* never delete the backup before attempting restoration;
* retain no claim that rollback succeeded if cleanup failed.

Explicit rollback remains the path that reports typed failures. Drop is only a safety net.

Existing destination preservation

The previous published runtime must survive every failed prepare operation unless restoration itself fails.

The previous bundle must not be modified in place.

Publication uses directory renames:

destination → backup
staging → destination

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