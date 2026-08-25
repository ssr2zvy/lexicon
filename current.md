Current implementation request: disk-based HTTP runtime bundle admission

Objective

Add the framework operation that validates a staged or published HTTP runtime bundle from disk before it can be selected for execution or publication.

The operation must verify:

1. the exact bundle directory shape;
2. the runtime.json file boundary and schema;
3. runtime identity and capability compatibility;
4. the executable’s size and SHA-256 against the manifest.

Do not execute the runtime, publish the staged bundle, or integrate admission into a CLI command yet.

Architectural position

The completed build-side flow is:

VerifiedHttpRuntime
→ StagedHttpRuntimeBundle
   ├── executable
   └── runtime.json

This micro-step adds:

runtime bundle directory
→ structural filesystem validation
→ runtime.json decoding
→ compatibility validation
→ executable hash verification
→ AdmittedHttpRuntimeBundle

The same admission function can later validate both staged and published bundles.

Required module

Create:

lexicon-framework/src/build/runtime_bundle_admission.rs

Export its public API through:

lexicon-framework/src/build/mod.rs

lexicon-framework remains library-only.

Manifest size limit

Define:

pub const MAX_RUNTIME_MANIFEST_BYTES: usize =
    128 * 1024;

This limit includes the required final newline.

Read the manifest with bounded memory. Do not use an unbounded whole-file read.

Admitted bundle type

Define an opaque type:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedHttpRuntimeBundle {
    directory: PathBuf,
    executable_path: PathBuf,
    manifest_path: PathBuf,
    manifest: RuntimeManifestV1,
    artifact: HashedRuntimeArtifact,
}

Provide:

impl AdmittedHttpRuntimeBundle {
    pub fn directory(&self) -> &Path;
    pub fn executable_path(&self) -> &Path;
    pub fn manifest_path(&self) -> &Path;
    pub fn manifest(&self) -> &RuntimeManifestV1;
    pub fn artifact(&self) -> &HashedRuntimeArtifact;
    pub fn runtime_information(
        &self,
    ) -> &RuntimeInformationV1;
}

Do not provide a public unchecked constructor.

A value must only exist after all filesystem, manifest, compatibility, and digest checks succeed.

Public admission API

Provide:

pub fn admit_http_runtime_bundle(
    bundle_directory: &Path,
    expected_identity: RuntimeIdentity,
) -> Result<
    AdmittedHttpRuntimeBundle,
    RuntimeBundleAdmissionError,
>;

This function must not execute the runtime.

Exact accepted directory shape

The bundle directory must contain exactly:

<bundle-directory>/
├── <manifest-declared-executable>
└── runtime.json

Reject:

* a missing bundle directory;
* a bundle path that is not a directory;
* a final-component symlink for the bundle directory;
* a missing runtime.json;
* a symlinked runtime.json;
* subdirectories;
* additional files;
* multiple executable candidates;
* a manifest-declared executable that is missing;
* a manifest-declared executable that is a symlink;
* any filesystem entry not named runtime.json or the declared executable.

Do not guess which file is the executable.

The manifest selects the executable filename.

Manifest file boundary

The staged manifest contains:

<RuntimeManifestV1 JSON>\n

Require exactly:

* one UTF-8 JSON document;
* followed by one ASCII \n;
* with no additional bytes.

Reject:

* empty manifest;
* oversized manifest;
* NUL bytes;
* invalid UTF-8;
* missing final newline;
* multiple final newlines;
* \r\n;
* leading whitespace;
* trailing whitespace before the newline;
* text before or after the JSON document.

Remove exactly one final newline before calling:

RuntimeManifestV1::from_json(...)

Do not generally trim the file.

Required admission order

Perform checks in this deterministic order:

1. Inspect the bundle path using symlink-aware metadata.
2. Require a non-symlink directory.
3. Locate and validate runtime.json as a non-symlink regular file.
4. Enforce the manifest byte limit while reading.
5. Validate the exact manifest file boundary.
6. Decode with:

RuntimeManifestV1::from_json(...)

7. Validate nested runtime compatibility using:

manifest
    .runtime_information()
    .validate_compatibility(expected_identity)

8. Resolve the executable using only the validated manifest filename.
9. Require the executable to be a non-symlink regular file.
10. Enumerate the directory and reject every unexpected entry.
11. Hash the executable using:

hash_runtime_executable(...)

12. Require:

actual size == manifest size
actual SHA-256 == manifest SHA-256

13. Return AdmittedHttpRuntimeBundle.

Do not duplicate Core compatibility logic.

Path containment

The executable path must be formed as:

bundle_directory.join(
    manifest.executable_name(),
)

The existing manifest filename validation prevents absolute paths and traversal.

Still ensure the resulting executable is a direct child of the supplied bundle directory.

Do not canonicalize it to an arbitrary external target.

Digest mismatch

If the actual artifact differs from the manifest, return a typed mismatch containing:

* expected size;
* actual size;
* expected SHA-256;
* actual SHA-256.

A same-size content substitution must be detected through SHA-256.

Typed error

Define an error equivalent to:

#[derive(Debug)]
pub enum RuntimeBundleAdmissionError {
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
        RuntimeManifestDecodingError,
    ),
    Incompatible(
        RuntimeCompatibilityError,
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

Add separate NUL and UTF-8 variants if those distinctions are not represented cleanly by InvalidManifestBoundary.

Callers must at least distinguish:

* bundle shape failures;
* manifest I/O and boundary failures;
* manifest decoding failures;
* runtime compatibility failures;
* unexpected entries;
* executable failures;
* artifact mismatch.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String, print diagnostics, or terminate the process.

No probing during admission

Do not execute:

probe_http_runtime_information(...)

Disk admission relies on:

* the runtime information recorded during supported build verification;
* the recorded executable digest;
* a fresh digest of the executable currently on disk.

The child will perform its own invocation validation later when normal execution is implemented.

Required tests

Add tests proving:

1. A successfully staged bundle is admitted.
2. The admitted directory matches the supplied directory.
3. The executable path comes from the manifest.
4. The manifest path is exactly runtime.json.
5. Runtime identity remains accessible.
6. Required and available capabilities remain accessible.
7. A missing bundle path is rejected.
8. A bundle path that is a file is rejected.
9. A final-component bundle symlink is rejected where supported.
10. Missing runtime.json is rejected.
11. A symlinked runtime.json is rejected.
12. An empty manifest is rejected.
13. An oversized manifest is rejected.
14. A manifest without a final newline is rejected.
15. A manifest with two final newlines is rejected.
16. A manifest using \r\n is rejected.
17. Invalid UTF-8 is rejected.
18. NUL-containing manifest data is rejected.
19. Malformed manifest JSON is rejected.
20. Unknown manifest schema versions are rejected.
21. Runtime identity mismatch is rejected.
22. Descriptor-version mismatch is rejected.
23. Missing required capabilities are rejected.
24. A missing manifest-declared executable is rejected.
25. A symlinked executable is rejected.
26. A directory at the executable path is rejected.
27. An extra file is rejected.
28. An extra directory is rejected.
29. Modified executable size is rejected.
30. Same-size modified executable bytes are rejected through SHA-256.
31. The mismatch error preserves expected and actual values.
32. Admission does not execute the candidate.
33. Admission does not modify the bundle.
34. Dropping the admitted value does not delete the bundle.
35. Existing staging, manifest, verification, probe, and hashing tests pass.
36. All workspace tests pass.

Use a successfully created StagedHttpRuntimeBundle as the valid test fixture rather than manually recreating the bundle contract.

When testing ownership, keep the staged bundle owner alive while admitting its directory.

Preserve existing behavior

Do not change:

* Core runtime-information behavior;
* runtime probing;
* executable hashing;
* candidate verification;
* runtime manifest schema;
* staging behavior or cleanup ownership;
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

* publication of staged bundles;
* replacement of existing bundles;
* backup or rollback;
* paired acquisition/processing publication;
* integration with source build;
* runtime execution;
* re-probing during admission;
* invocation envelopes;
* managed-runner generation;
* runner main.rs;
* runner::run;
* source workspace migration;
* acquisition or resume execution;
* HTTP transport;
* raw recording;
* sessions;
* supervision;
* __operator-host;
* processing-runtime admission.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the public bundle-admission API;
* the opaque admitted-bundle representation;
* manifest size and boundary policy;
* exact directory-shape validation;
* runtime compatibility delegation;
* executable path-containment behavior;
* fresh hash comparison;
* typed admission errors;
* successful staged-bundle admission;
* every malformed bundle rejection result;
* confirmation that admission did not execute or modify the runtime;
* framework and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not publish or execute the admitted bundle.