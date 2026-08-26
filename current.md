Current implementation request: stage a verified processing runtime bundle

Objective

Add the framework operation that copies a verified processing executable into a uniquely owned staging directory and writes its processing runtime.json.

The staged processing bundle must be ready for later disk admission and paired publication.

Do not publish or integrate it with source build.

Required module

Extend the staging implementation under:

lexicon-framework/src/build/runtime_staging.rs

A separate internal file is acceptable if both acquisition and processing use shared private staging mechanics.

Export the processing API through:

lexicon-framework/src/build/mod.rs

Processing staged bundle shape

A successful operation creates exactly:

<unique-staging-directory>/
├── <processing-executable-name>
└── runtime.json

No other entries may exist inside the bundle.

Processing staged type

Define:

pub struct StagedProcessingRuntimeBundle {
    directory: PathBuf,
    executable_path: PathBuf,
    manifest_path: PathBuf,
    manifest: ProcessingRuntimeManifestV1,
    // Private temporary-directory ownership.
}

Provide:

impl StagedProcessingRuntimeBundle {
    pub fn directory(&self) -> &Path;
    pub fn executable_path(&self) -> &Path;
    pub fn manifest_path(&self) -> &Path;
    pub fn manifest(
        &self,
    ) -> &ProcessingRuntimeManifestV1;
}

Do not provide a public unchecked constructor.

Shared staging implementation

Refactor acquisition and processing staging to reuse one private filesystem staging implementation where practical.

The shared implementation should own:

* staging-parent validation;
* unique temporary-directory creation;
* executable copying;
* permission preservation;
* staged executable hashing;
* expected size and SHA-256 comparison;
* runtime.json creation;
* exact newline writing;
* file synchronization;
* directory synchronization;
* failure cleanup.

Operation-specific wrappers retain their typed manifest and verified-runtime APIs.

Preserve the existing acquisition staging public API and behavior.

Public processing API

Provide:

pub fn stage_verified_processing_runtime_bundle(
    staging_parent: &Path,
    executable_name: &str,
    verified: &VerifiedProcessingRuntime,
) -> Result<
    StagedProcessingRuntimeBundle,
    ProcessingRuntimeBundleStagingError,
>;

Required sequence

Perform:

1. Construct ProcessingRuntimeManifestV1 from the executable name and verified runtime.
2. Validate that staging_parent exists and is a directory.
3. Create a uniquely named directory directly beneath that parent.
4. Copy the verified processing executable into it.
5. Preserve ordinary source permissions where supported.
6. Hash the staged executable.
7. Compare its size and SHA-256 with VerifiedProcessingRuntime.
8. Encode ProcessingRuntimeManifestV1.
9. Create runtime.json with create-new semantics.
10. Write the JSON followed by exactly one ASCII newline.
11. Flush and synchronize the executable.
12. Flush and synchronize the manifest.
13. Synchronize the staging directory where supported.
14. Return StagedProcessingRuntimeBundle.

Staging ownership

Use an owned temporary-directory mechanism.

Dropping an unconsumed staged processing bundle must remove its directory where possible.

Add a crate-private consuming transfer operation for later publication:

impl StagedProcessingRuntimeBundle {
    pub(crate) fn into_staging_directory(
        self,
    ) -> Result<
        PathBuf,
        RuntimeBundleStagingTransferError,
    >;
}

Reuse the existing transfer error if it is operation-neutral.

Do not expose temporary-directory internals publicly.

Copy verification

Require:

staged size == verified size
staged SHA-256 == verified SHA-256

Do not trust copy success alone.

A source mutation after verification must cause staging failure if the copied bytes no longer match the verified artifact.

Same-size changed content must be detected by SHA-256.

Manifest bytes

Write exactly:

ProcessingRuntimeManifestV1::to_json()
+ "\n"

Requirements:

* no leading text;
* no byte-order mark;
* no additional whitespace;
* no second newline;
* no candidate build path;
* no diagnostics.

Typed staging error

Define:

#[derive(Debug)]
pub enum ProcessingRuntimeBundleStagingError {
    ManifestConstruction(
        ProcessingRuntimeManifestConstructionError,
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
        ProcessingRuntimeManifestEncodingError,
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

Equivalent organization is acceptable.

Shared private staging errors may be translated into operation-specific public errors.

Implement Display and Error.

Do not return plain String, print diagnostics, or exit.

Failure cleanup

After the staging directory is created, every failure must:

* return a typed primary error;
* clean up the partial directory through temporary-directory ownership;
* leave existing published runtime directories untouched;
* produce no staged result.

Required tests

Add tests proving:

1. A verified processing runtime stages successfully.
2. The directory is directly beneath the requested parent.
3. The bundle contains exactly two files.
4. The executable uses the requested validated name.
5. The manifest path is exactly runtime.json.
6. Staged bytes match the verified candidate.
7. Staged size matches the processing manifest.
8. Staged SHA-256 matches the processing manifest.
9. runtime.json decodes through ProcessingRuntimeManifestV1.
10. Nested processing identity is preserved.
11. runtime.json ends with exactly one newline.
12. The manifest contains no candidate path.
13. Invalid executable names fail before directory creation.
14. Missing or invalid staging parents are rejected.
15. Copy failure is typed.
16. Changed source bytes produce an artifact mismatch.
17. Same-size mutation is detected.
18. Manifest creation or writing failure cleans up.
19. Synchronization failure is typed through a private seam.
20. Dropping a successful staged processing bundle removes it.
21. Consuming ownership transfer prevents premature deletion.
22. Failure leaves unrelated and published directories unchanged.
23. Acquisition staging behavior remains unchanged.
24. Acquisition and processing share private staging mechanics where practical.
25. Existing processing manifest and verification tests pass.
26. All workspace tests pass repeatedly.

Preserve existing behavior

Do not change:

* acquisition staging public API;
* acquisition manifest behavior;
* processing manifest schema;
* processing verification;
* probing or hashing;
* acquisition bundle admission;
* reversible publication;
* source scaffolding;
* source create;
* source build;
* Cargo invocation;
* legacy publication;
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

* processing bundle admission;
* processing publication;
* paired publication;
* source build integration;
* processing runner main.rs;
* processing execution;
* SQLite operations;
* raw-data discovery;
* sessions;
* source workspace migration;
* acquisition managed runners;
* runner::run;
* invocation envelopes;
* HTTP execution;
* raw recording;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* processing staging API;
* staged processing ownership model;
* shared private staging mechanics;
* exact directory shape;
* copy and digest verification;
* manifest byte format;
* permission and synchronization behavior;
* typed errors;
* cleanup behavior;
* mutation and failure results;
* acquisition staging regressions;
* confirmation that nothing was published;
* framework and repeated workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not admit or publish the processing bundle.