Current implementation request: disk-based processing runtime bundle admission

Objective

Add the framework operation that validates a staged or published processing runtime bundle from disk.

It must verify:

1. exact bundle directory shape;
2. processing runtime.json boundary and schema;
3. processing runtime identity compatibility;
4. executable size and SHA-256.

Do not execute or publish the bundle.

Required module

Extend:

lexicon-framework/src/build/runtime_bundle_admission.rs

Export the processing API through:

lexicon-framework/src/build/mod.rs

Shared admission mechanics

Refactor acquisition and processing bundle admission to share private filesystem mechanics where practical:

* bundle metadata and symlink validation;
* runtime.json metadata validation;
* bounded manifest reading;
* exact newline and UTF-8 boundary validation;
* direct-child executable resolution;
* directory entry enumeration;
* executable regular-file and symlink validation;
* fresh executable hashing;
* size and SHA-256 comparison.

Operation-specific decoding and compatibility validation remain separate.

Preserve the acquisition admission public API and behavior.

Admitted processing bundle

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedProcessingRuntimeBundle {
    directory: PathBuf,
    executable_path: PathBuf,
    manifest_path: PathBuf,
    manifest: ProcessingRuntimeManifestV1,
    artifact: HashedRuntimeArtifact,
}

Provide:

impl AdmittedProcessingRuntimeBundle {
    pub fn directory(&self) -> &Path;
    pub fn executable_path(&self) -> &Path;
    pub fn manifest_path(&self) -> &Path;
    pub fn manifest(
        &self,
    ) -> &ProcessingRuntimeManifestV1;
    pub fn artifact(
        &self,
    ) -> &HashedRuntimeArtifact;
    pub fn runtime_information(
        &self,
    ) -> &ProcessingRuntimeInformationV1;
}

Do not provide a public unchecked constructor.

Public API

Provide:

pub fn admit_processing_runtime_bundle(
    bundle_directory: &Path,
    expected_identity: RuntimeIdentity,
) -> Result<
    AdmittedProcessingRuntimeBundle,
    ProcessingRuntimeBundleAdmissionError,
>;

This function must not execute the processing runtime.

Exact accepted shape

Require exactly:

<bundle-directory>/
├── <manifest-declared-processing-executable>
└── runtime.json

Reject:

* missing bundle;
* non-directory bundle;
* final-component bundle symlink;
* missing or symlinked runtime.json;
* non-regular manifest;
* missing or symlinked executable;
* executable path that is a directory;
* additional files;
* additional directories;
* multiple executable candidates.

The manifest alone selects the executable filename.

Manifest boundary

Use the existing:

MAX_RUNTIME_MANIFEST_BYTES

Accept exactly:

<ProcessingRuntimeManifestV1 JSON>\n

Reject:

* empty input;
* oversized input;
* NUL;
* invalid UTF-8;
* missing newline;
* multiple final newlines;
* \r\n;
* leading whitespace;
* trailing whitespace before the newline;
* diagnostic text;
* multiple JSON documents.

Remove exactly one newline before calling:

ProcessingRuntimeManifestV1::from_json(...)

Do not generally trim.

Required admission order

Perform:

1. Validate bundle path metadata.
2. Require a non-symlink directory.
3. Validate runtime.json as a non-symlink regular file.
4. Read it with the shared bounded reader.
5. Validate the shared exact file boundary.
6. Decode ProcessingRuntimeManifestV1.
7. Call:

manifest
    .runtime_information()
    .validate_compatibility(expected_identity)

8. Resolve the declared executable as a direct child.
9. Validate it as a non-symlink regular file.
10. Reject unexpected directory entries.
11. Hash it using hash_runtime_executable(...).
12. Compare actual size and SHA-256 against the manifest.
13. Return AdmittedProcessingRuntimeBundle.

Do not duplicate Core compatibility rules.

Path containment

Construct the executable path using:

bundle_directory.join(
    manifest.executable_name(),
)

The validated filename must remain one direct child component.

Do not canonicalize it to an external path or guess an executable based on extension.

Typed error

Define:

#[derive(Debug)]
pub enum ProcessingRuntimeBundleAdmissionError {
    BundleMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    BundleIsSymlink {
        path: PathBuf,
    },
    BundleNotDirectory {
        path: PathBuf,
    },
    ManifestMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    ManifestIsSymlink {
        path: PathBuf,
    },
    ManifestNotRegularFile {
        path: PathBuf,
    },
    ManifestTooLarge {
        maximum: usize,
        actual: u64,
    },
    ReadManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidManifestBoundary,
    DecodeManifest(
        ProcessingRuntimeManifestDecodingError,
    ),
    Incompatible(
        ProcessingRuntimeCompatibilityError,
    ),
    ReadDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    UnexpectedDirectoryEntry {
        path: PathBuf,
    },
    MissingExecutable {
        path: PathBuf,
    },
    ExecutableIsSymlink {
        path: PathBuf,
    },
    ExecutableNotRegularFile {
        path: PathBuf,
    },
    HashExecutable(
        RuntimeArtifactHashError,
    ),
    ArtifactMismatch {
        expected_size: u64,
        actual_size: u64,
        expected_sha256: ExecutableSha256,
        actual_sha256: ExecutableSha256,
    },
}

Equivalent organization is acceptable.

Shared private errors may be translated into acquisition- or processing-specific public errors.

Implement Display and Error.

Do not return plain String, print diagnostics, or exit.

No process execution

Admission must not call:

probe_processing_runtime_information(...)

It validates:

* recorded processing runtime information;
* recorded artifact integrity;
* a fresh hash of the executable currently on disk.

Required tests

Add tests proving:

1. A successfully staged processing bundle is admitted.
2. All admitted paths and metadata are preserved.
3. Processing identity is accessible.
4. Missing and non-directory bundle paths are rejected.
5. Bundle-directory symlinks are rejected.
6. Missing, symlinked, or non-regular manifests are rejected.
7. Empty and oversized manifests are rejected.
8. Every invalid manifest boundary is rejected.
9. Invalid processing manifest JSON is rejected.
10. Unknown manifest schema versions are rejected.
11. Acquisition runtime information is rejected.
12. Processing identity mismatch is rejected.
13. Descriptor-version mismatch is rejected.
14. Missing declared executable is rejected.
15. Symlinked or non-regular executables are rejected.
16. Extra files and directories are rejected.
17. Modified executable size is rejected.
18. Same-size executable substitution is rejected by SHA-256.
19. Mismatch errors preserve expected and actual values.
20. Admission does not execute or modify the runtime.
21. Dropping the admitted value does not delete the bundle.
22. Acquisition bundle admission remains unchanged.
23. Acquisition and processing share private mechanics where practical.
24. Existing processing staging tests remain unchanged.
25. All workspace tests pass repeatedly.

Use StagedProcessingRuntimeBundle as the valid fixture and keep its owner alive during admission tests.

Preserve existing behavior

Do not change:

* acquisition bundle admission API;
* acquisition staging;
* processing staging ownership;
* processing manifest schema;
* processing verification or probing;
* hashing;
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

* processing publication;
* paired publication;
* integration with source build;
* runtime execution;
* re-probing during admission;
* processing runner main.rs;
* processing logic or SQLite;
* raw-data discovery;
* sessions;
* source workspace migration;
* managed acquisition runners;
* runner::run;
* invocation envelopes;
* HTTP execution;
* raw recording;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* processing bundle-admission API;
* admitted processing bundle representation;
* shared filesystem admission mechanics;
* exact directory and manifest boundary rules;
* processing compatibility delegation;
* executable containment and fresh hashing;
* typed errors;
* malformed bundle rejection results;
* confirmation that no runtime was executed or modified;
* acquisition regression results;
* framework and repeated workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not publish the processing bundle.