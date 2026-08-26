Current implementation request: paired acquisition-and-processing publication coordinator

Objective

Add the only public runtime publication operation: publish one staged acquisition bundle and one staged processing bundle as a coordinated pair.

If preparing or validating either new bundle fails, Lexicon must attempt to restore both previous bundles.

Do not integrate this coordinator into source build yet.

Architectural position

The completed primitives now support:

acquisition:
verified → manifest → staged → admitted
processing:
verified → manifest → staged → admitted
publication:
one reversible replacement leg

This micro-step combines two reversible legs:

staged acquisition + staged processing
→ prevalidate both
→ prepare acquisition replacement
→ prepare processing replacement
→ validate both installed destinations
→ logically commit pair
→ clean old backups

No public single-runtime publication API may be added.

Required module

Create:

lexicon-framework/src/publication/runtime_pair.rs

Export the paired publication API through the framework publication module and crate API as appropriate for later use by source build.

Keep the existing single-bundle replacement primitive crate-private.

Public publication API

Provide an API equivalent to:

pub fn publish_runtime_pair(
    acquisition: StagedHttpRuntimeBundle,
    processing: StagedProcessingRuntimeBundle,
    acquisition_destination: &Path,
    processing_destination: &Path,
    expected_acquisition_identity: RuntimeIdentity,
    expected_processing_identity: RuntimeIdentity,
) -> Result<
    PublishedRuntimePair,
    RuntimePairPublicationError,
>;

The staged bundles are consumed.

Published result

Define:

#[derive(Debug)]
pub struct PublishedRuntimePair {
    acquisition_directory: PathBuf,
    processing_directory: PathBuf,
    cleanup_warnings: Vec<RuntimePairCleanupWarning>,
}

Provide accessors:

impl PublishedRuntimePair {
    pub fn acquisition_directory(&self) -> &Path;
    pub fn processing_directory(&self) -> &Path;
    pub fn cleanup_warnings(
        &self,
    ) -> &[RuntimePairCleanupWarning];
}

A successful result means both new bundles are installed and admitted.

Backup cleanup warnings do not mean the new pair failed publication.

Destination validation

Before mutating either destination, require:

1. acquisition and processing destination paths differ;
2. neither destination is an ancestor of the other;
3. both destination parents exist and are directories;
4. neither destination parent’s final component is a symlink;
5. existing destinations, when present, are non-symlink directories;
6. both staged bundles pass operation-specific admission;
7. acquisition identity has operation Acquisition;
8. processing identity has operation Processing.

Reject invalid operation identities before touching either destination.

Do not decide a permanent directory naming policy in this step; callers provide both exact bundle paths.

Prepublication admission

Before moving either destination, validate:

admit_http_runtime_bundle(
    acquisition.directory(),
    expected_acquisition_identity,
)

and:

admit_processing_runtime_bundle(
    processing.directory(),
    expected_processing_identity,
)

Both must succeed before the first publication rename.

This prevents an invalid processing bundle from causing the acquisition destination to be temporarily replaced.

Required publication sequence

Perform these steps in order:

1. Validate both identities and destination relationships.
2. Admit both staged bundles.
3. Prepare the acquisition replacement using the internal reversible primitive.
4. Prepare the processing replacement.
5. If processing preparation fails, roll back acquisition.
6. After both are prepared, admit the acquisition destination from disk.
7. Admit the processing destination from disk.
8. If either installed destination fails admission, roll back both replacements.
9. Mark both prepared replacements logically committed.
10. Disable automatic rollback for both.
11. Attempt to remove both old backup directories.
12. Synchronize affected parent directories where supported.
13. Return PublishedRuntimePair.

Required internal replacement refactor

The existing reversible replacement primitive currently handles one staged acquisition bundle.

Refactor its internal ownership boundary so it can accept an operation-neutral crate-private staged-directory transfer produced by either:

StagedHttpRuntimeBundle
StagedProcessingRuntimeBundle

For example:

pub(crate) struct OwnedStagedRuntimeDirectory {
    path: PathBuf,
}

Both public staged wrapper types may consume themselves into this internal type.

Do not erase the public distinction between acquisition and processing staged bundles.

Separate logical commit from cleanup

Refactor PreparedRuntimeBundleReplacement so logical commit does not depend on deleting its backup.

Provide internal operations conceptually equivalent to:

fn mark_committed(&mut self);
fn cleanup_backup(
    &mut self,
) -> Result<(), RuntimeBundleReplacementError>;

Requirements:

* mark_committed is in-memory and infallible;
* it disables automatic rollback;
* it is called for both legs only after both installed destinations pass admission;
* backup deletion occurs afterward;
* backup deletion failure becomes a cleanup warning, not a false publication failure.

Do not delete the first backup before the second leg is logically committed.

Rollback order

When both legs were prepared, roll back in reverse order:

1. processing;
2. acquisition.

This mirrors the mutation sequence.

If one rollback fails, still attempt the other rollback.

Return all rollback failures rather than stopping after the first.

Publication failure semantics

Before logical commit, any failure must return RuntimePairPublicationError.

The error must report:

* the primary failed phase;
* whether acquisition was prepared;
* whether processing was prepared;
* every rollback failure;
* relevant destination paths.

Do not claim that the previous pair was restored if rollback failed.

Cleanup warnings

Define a typed warning such as:

#[derive(Debug)]
pub enum RuntimePairCleanupWarning {
    AcquisitionBackup {
        path: PathBuf,
        error: String,
    },
    ProcessingBackup {
        path: PathBuf,
        error: String,
    },
    ParentSync {
        path: PathBuf,
        error: String,
    },
}

Typed nested I/O data is preferable where ownership permits.

A cleanup warning means:

* both new destinations are already installed;
* both passed admission;
* automatic rollback has been disabled;
* an obsolete backup or durability cleanup requires later attention.

Do not roll back a successfully committed pair merely because deleting an obsolete backup failed.

Typed publication error

Define an error that distinguishes at least:

pub enum RuntimePairPublicationError {
    InvalidDestinations,
    InvalidAcquisitionIdentity,
    InvalidProcessingIdentity,
    AcquisitionStagedAdmission(...),
    ProcessingStagedAdmission(...),
    PrepareAcquisition(...),
    PrepareProcessing {
        source: ...,
        rollback_errors: Vec<...>,
    },
    ValidatePublishedAcquisition {
        source: ...,
        rollback_errors: Vec<...>,
    },
    ValidatePublishedProcessing {
        source: ...,
        rollback_errors: Vec<...>,
    },
}

Equivalent organization is acceptable.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String, print diagnostics, or exit.

Crash-safety statement

This step provides in-process transactional publication and rollback.

It does not yet implement a durable publication journal capable of completing recovery after machine loss between the two destination renames.

Do not claim full crash-atomic replacement of two independent directories.

Durable transaction recovery may be added later if required by the final publication contract.

Required tests

Add tests proving:

1. Publishing with no old bundles installs both new bundles.
2. Publishing over old bundles replaces both.
3. Both new destinations pass operation-specific admission.
4. Old acquisition and processing bundles remain in backups until both replacements are prepared and validated.
5. Invalid acquisition staging changes neither destination.
6. Invalid processing staging changes neither destination.
7. Acquisition preparation failure changes neither published bundle.
8. Processing preparation failure rolls back acquisition.
9. Published acquisition validation failure rolls back both.
10. Published processing validation failure rolls back both.
11. Rollback occurs processing first, then acquisition.
12. Failure rolling back one leg does not prevent attempting the other.
13. Rollback failures remain inspectable in the publication error.
14. Successful logical commit disables both automatic rollbacks.
15. Neither backup is deleted before both legs are committed.
16. Successful publication removes both backups when cleanup succeeds.
17. Acquisition backup cleanup failure returns success with a warning.
18. Processing backup cleanup failure returns success with a warning.
19. A cleanup warning leaves both new bundles installed.
20. Acquisition and processing destination equality is rejected.
21. Ancestor/descendant destination relationships are rejected.
22. Swapped operation identities are rejected before mutation.
23. Dropping an uncommitted prepared leg still attempts rollback.
24. No public single-runtime publication function exists.
25. Existing reversible replacement tests remain unchanged.
26. Existing acquisition and processing admission tests remain unchanged.
27. All workspace tests pass repeatedly.

Use private filesystem seams to force preparation, admission, rollback, cleanup, and synchronization failures deterministically.

Preserve existing behavior

Do not change:

* runtime bundle schemas;
* acquisition or processing staging APIs;
* admission APIs;
* verification or probe behavior;
* hashing;
* source scaffolding;
* source create;
* source build;
* Cargo invocation;
* legacy publication path;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-framework --quiet

Run:

cargo test --workspace --quiet
cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

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