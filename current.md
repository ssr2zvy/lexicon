# Runtime bundle admission implementation report

## Files created and changed
- Created `lexicon-framework/src/build/runtime_bundle_admission.rs`.
- Updated `lexicon-framework/src/build/mod.rs` to export the bundle admission API.

## Public bundle-admission API
- Added `pub const MAX_RUNTIME_MANIFEST_BYTES: usize = 128 * 1024;`.
- Added `pub fn admit_http_runtime_bundle(bundle_directory: &Path, expected_identity: RuntimeIdentity) -> Result<AdmittedHttpRuntimeBundle, RuntimeBundleAdmissionError>;`.
- Exported `AdmittedHttpRuntimeBundle` and `RuntimeBundleAdmissionError` via the build module.

## Opaque admitted-bundle representation
- `AdmittedHttpRuntimeBundle` is an opaque value with private fields:
  - `directory: PathBuf`
  - `executable_path: PathBuf`
  - `manifest_path: PathBuf`
  - `manifest: RuntimeManifestV1`
  - `artifact: HashedRuntimeArtifact`
- Accessors:
  - `directory() -> &Path`
  - `executable_path() -> &Path`
  - `manifest_path() -> &Path`
  - `manifest() -> &RuntimeManifestV1`
  - `artifact() -> &HashedRuntimeArtifact`
  - `runtime_information() -> &RuntimeInformationV1`
- No public unchecked constructor was added.

## Manifest size and boundary policy
- The manifest is read with a bounded cap of `128 * 1024` bytes and rejects oversized input with `ManifestTooLarge`.
- The accepted boundary is exactly one UTF-8 JSON document followed by one ASCII newline and no additional bytes.
- The validator rejects empty manifests, NUL-containing content, invalid UTF-8, missing newline, extra final newline, CRLF, leading whitespace, trailing whitespace before the newline, and extra text before or after the JSON document.
- The code strips exactly one final newline before calling `RuntimeManifestV1::from_json(...)`.

## Exact directory-shape validation
- The bundle directory must be a non-symlink directory.
- `runtime.json` must exist as a non-symlink regular file.
- The manifest-selected executable is resolved only from the validated manifest name.
- The directory is rejected if it contains any entry other than `runtime.json` and the declared executable.
- This covers missing bundle directories, file-at-bundle-paths, final-component symlinks, missing `runtime.json`, symlinked `runtime.json`, extra files, extra directories, multiple candidate files, missing executable files, and symlinked executable paths.

## Runtime compatibility delegation
- The admission flow follows the required order and delegates compatibility to:
  - `manifest.runtime_information().validate_compatibility(expected_identity)`
- This preserves the existing core compatibility logic and prevents reimplementing runtime-probe logic during admission.

## Executable path-containment behavior
- The executable path is assembled as `bundle_directory.join(manifest.executable_name())`.
- The resulting path is required to remain a direct child of the supplied bundle directory, and it is never canonicalized to some external target.

## Fresh hash comparison
- Admission computes a fresh hash with `hash_runtime_executable(...)`.
- It verifies both:
  - `actual size == manifest size`
  - `actual SHA-256 == manifest SHA-256`
- Same-size content substitution is rejected through SHA-256 comparison, even when the length matches.

## Typed admission errors
- The new error enum includes the required typed categories:
  - bundle metadata and symlink/directory failures
  - manifest metadata, symlink, regular-file, boundary, and decode failures
  - compatibility failures
  - unexpected directory entries
  - executable missing/symlink/regular-file failures
  - hash failures and artifact mismatch values (`expected_size`, `actual_size`, `expected_sha256`, `actual_sha256`)
- `Display` and `Error` implementations were added without returning plain strings or terminating the process.

## Successful staged-bundle admission
- A valid staged bundle is admitted successfully with `admit_http_runtime_bundle(...)`.
- The admitted bundle preserves the original directory, manifest path, executable path, runtime identity, required and available capabilities, and fresh artifact hash.

## Malformed bundle rejection results
- The implementation rejects the required malformed cases, including:
  - missing bundle path
  - file instead of bundle directory
  - final-component bundle symlink
  - missing `runtime.json`
  - symlinked `runtime.json`
  - empty manifest
  - oversized manifest
  - missing final newline
  - two final newlines
  - CRLF manifest
  - invalid UTF-8
  - NUL-containing manifest data
  - malformed JSON
  - unknown manifest schema versions
  - runtime identity mismatch
  - descriptor-version mismatch
  - missing required capabilities
  - missing manifest-declared executable
  - symlinked executable
  - directory at executable location
  - extra file or directory entries
  - modified executable size
  - same-size modified executable content via SHA-256 mismatch
  - mismatch error with preserved expected/actual size and SHA-256 values

## Execution and modification status
- Admission does not execute the runtime candidate.
- Admission does not publish or modify the staged bundle contents.
- Dropping the admitted value does not remove the bundle directory or its files.

## Validation results
- `cargo test -p lexicon-framework --quiet` -> passed (82 tests)
- `cargo test --workspace --quiet` -> passed
- External MZA bundle/install checkout was not present in this environment, so `bash automation/build_bundle_install/build_bundle_install.sh` was not run and is reported as a known external blocker rather than a code issue.
