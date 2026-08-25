Current implementation request: stage a verified HTTP runtime bundle

Objective

Implement the framework operation that copies a verified HTTP runtime executable into a uniquely owned staging directory and writes its corresponding runtime.json.

This produces a self-consistent staged bundle for later publication.

Do not publish, replace, back up, or roll back an existing runtime.

Required module

Create:

lexicon-framework/src/build/runtime_staging.rs

Export its public API through:

lexicon-framework/src/build/mod.rs

lexicon-framework remains library-only.

Staged directory shape

A successful staging operation creates exactly:

<unique-staging-directory>/
├── <executable-name>
└── runtime.json

No additional files or directories may exist inside the staged bundle.

Staged result type

Define an opaque type:

pub struct StagedHttpRuntimeBundle {
    directory: PathBuf,
    executable_path: PathBuf,
    manifest_path: PathBuf,
    manifest: RuntimeManifestV1,
    // Private temporary-directory ownership.
}

Provide:

impl StagedHttpRuntimeBundle {
    pub fn directory(&self) -> &Path;
    pub fn executable_path(&self) -> &Path;
    pub fn manifest_path(&self) -> &Path;
    pub fn manifest(&self) -> &RuntimeManifestV1;
}

Do not provide a public unchecked constructor.

A successful value must prove that:

* the staging directory was uniquely created;
* the executable was copied;
* the copied size and SHA-256 match the verified artifact;
* runtime.json was completely written;
* the manifest describes the staged executable.

Staging ownership

Use an owned temporary-directory mechanism such as:

tempfile::TempDir

The staging directory must be removed automatically if the staged bundle is dropped before a later publication operation consumes it.

Do not expose TempDir in the public API.

Do not persist or leak the staging directory in this step.

Public API

Provide:

pub fn stage_verified_http_runtime_bundle(
    staging_parent: &Path,
    executable_name: &str,
    verified: &VerifiedHttpRuntime,
) -> Result<
    StagedHttpRuntimeBundle,
    RuntimeBundleStagingError,
>;

Required operation order

Perform these operations in order:

1. Construct RuntimeManifestV1 from executable_name and verified.
2. Verify that staging_parent exists and is a directory.
3. Create a uniquely named temporary directory directly beneath it.
4. Construct the staged executable path using the validated manifest filename.
5. Copy the verified candidate executable to that path.
6. Preserve ordinary source permissions where supported.
7. Hash the staged executable with:

hash_runtime_executable(...)

8. Compare staged size and SHA-256 against the verified artifact.
9. Encode the manifest using:

RuntimeManifestV1::to_json()

10. Create runtime.json using create-new semantics.
11. Write the JSON followed by exactly one ASCII newline.
12. Flush and synchronize the staged executable.
13. Flush and synchronize runtime.json.
14. Synchronize the staging directory where supported.
15. Return StagedHttpRuntimeBundle.

Staging directory creation

Create a unique implementation-controlled name such as:

.lexicon-http-runtime-stage-<random>

The suffix and complete staging-directory name are not public compatibility surfaces.

Do not:

* reuse an existing directory;
* delete an existing path;
* overwrite an existing path;
* create the directory outside staging_parent;
* use the final published runtime path as the staging directory.

Copy verification

Do not trust fs::copy alone.

After copying, require:

staged size == verified size
staged SHA-256 == verified SHA-256

Path equality is not required because the copy intentionally has a different path.

If the original candidate changes between verification and copying, staging must fail when the copied bytes differ from the verified artifact.

The failure must preserve expected and actual size and digest values.

Manifest file contract

The manifest path is exactly:

<staging-directory>/runtime.json

Its bytes are exactly:

RuntimeManifestV1::to_json()
+ "\n"

The file must contain:

* no leading text;
* no byte-order mark;
* no additional trailing whitespace;
* no second newline;
* no diagnostic output;
* no temporary candidate path.

Use create-new semantics. Never overwrite a preexisting manifest.

Permission behavior

The copied executable must preserve its source permissions where the platform supports them.

Do not broaden permissions or add executable bits.

runtime.json must remain an ordinary non-executable file, subject to platform defaults and the process umask.

Durability behavior

Before returning success:

* all executable bytes must be written;
* all manifest bytes must be written;
* both files must be flushed;
* sync_all() must succeed for both files;
* the staging directory must be synchronized where supported.

If directory synchronization is unsupported on a specific target, isolate and document that platform behavior. Do not weaken file synchronization.

Typed error

Define an error equivalent to:

#[derive(Debug)]
pub enum RuntimeBundleStagingError {
    ManifestConstruction(
        RuntimeManifestConstructionError,
    ),
    InvalidStagingParent {
        path: PathBuf,
    },
    CreateStagingDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    CopyExecutable {
        source_path: PathBuf,
        destination_path: PathBuf,
        source: std::io::Error,
    },
    HashStagedExecutable(
        RuntimeArtifactHashError,
    ),
    CopiedArtifactMismatch {
        expected_size: u64,
        actual_size: u64,
        expected_sha256: ExecutableSha256,
        actual_sha256: ExecutableSha256,
    },
    EncodeManifest(
        RuntimeManifestEncodingError,
    ),
    CreateManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    WriteManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    SyncExecutable {
        path: PathBuf,
        source: std::io::Error,
    },
    SyncManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    SyncDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
}

Equivalent organization is acceptable, but callers must distinguish each major staging phase.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String, print errors, or exit.

Failure cleanup

If any operation fails after the unique staging directory is created:

* return the typed primary error;
* automatically remove the partial staging directory where possible;
* do not touch existing runtime directories;
* do not produce StagedHttpRuntimeBundle.

A cleanup failure may be retained as supplementary diagnostic information, but it must not replace the primary failure.

Required tests

Add tests proving:

1. A verified runtime stages successfully.
2. The staging directory is directly under the requested parent.
3. The staged directory contains exactly two files.
4. The executable has the requested validated filename.
5. The manifest path is exactly runtime.json.
6. The staged executable bytes match the candidate.
7. The staged executable size matches the verified artifact.
8. The staged executable SHA-256 matches the verified artifact.
9. runtime.json decodes through RuntimeManifestV1::from_json().
10. The decoded manifest contains the admitted runtime information.
11. runtime.json ends with exactly one newline.
12. The manifest contains no temporary candidate path.
13. An invalid executable name fails before staging-directory creation.
14. A missing staging parent is rejected.
15. A staging parent that is a file is rejected.
16. Copy failure returns the typed copy error.
17. Changed source bytes produce CopiedArtifactMismatch.
18. Same-size changed bytes are detected through SHA-256.
19. Manifest creation or writing failure cleans up the staging directory.
20. Synchronization failure is typed using a deterministic private seam.
21. Dropping a successful staged bundle removes its directory.
22. Failure leaves unrelated directories unchanged.
23. Failure leaves existing runtime fixtures unchanged.
24. No publication, backup, or rollback operation occurs.
25. Existing manifest, verification, probe, and hashing tests pass.
26. All workspace tests pass.

Use private test seams where deterministic mutation, writing, or synchronization failures are otherwise unreliable.

Preserve existing behavior

Do not change:

* Core runtime-information behavior;
* runtime probing;
* executable hashing;
* candidate verification;
* runtime manifest schema;
* source scaffolding;
* source implementation crates;
* source create;
* source build;
* Cargo invocation;
* artifact selection;
* existing publication behavior;
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

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If the MZA checkout remains unavailable, report it separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* publication into runtime/;
* replacement of existing runtime bundles;
* backup creation;
* rollback;
* paired acquisition and processing publication;
* integration with source build;
* Cargo build-plan changes;
* managed-runner generation;
* runner main.rs;
* runner::run;
* source workspace migration;
* disk-based runtime admission;
* invocation envelopes;
* acquisition or resume execution;
* HTTP transport;
* raw recording;
* sessions;
* supervision;
* __operator-host;
* processing-runtime staging.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the staging API;
* staged-bundle ownership model;
* exact directory shape;
* executable copying and verification behavior;
* manifest file format;
* permission behavior;
* durability behavior;
* typed errors;
* failure cleanup;
* successful staging results;
* mutation and partial-failure results;
* confirmation that no runtime was published or replaced;
* framework and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not publish the staged runtime bundle.