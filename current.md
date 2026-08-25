# Runtime manifest implementation report

## Files created and changed
- `lexicon-framework/src/build/runtime_manifest.rs` — added the in-memory manifest contract, validation, deterministic JSON encoding/decoding, strict SHA-256 parsing, and unit tests.
- `lexicon-framework/src/build/mod.rs` — exported the manifest API and types from the build module.

## Manifest schema version
- `RUNTIME_MANIFEST_SCHEMA_VERSION: u32 = 1`
- This value applies only to the runtime manifest document and remains distinct from the Core runtime-information schema and the other protocol/source contract versions.

## Exact JSON structure
```json
{
  "schema_version": 1,
  "artifact": {
    "executable": "example-source-get-raw-data",
    "size": 123456,
    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
  },
  "runtime_information": {
    "schema_version": 1,
    "identity": {
      "source": "example-source",
      "protocol": "http",
      "operation": "acquisition",
      "source_contract_version": 1
    },
    "descriptor": {
      "contract_version": 1,
      "required_capabilities": []
    },
    "runtime": {
      "available_capabilities": []
    }
  }
}
```

The nested `runtime_information` object is delegated to the existing Core `RuntimeInformationV1` document and is serialized/decoded via `RuntimeInformationV1::to_json()` / `RuntimeInformationV1::from_json(...)` without a second framework-side schema.

## Manifest construction API
- `RuntimeManifestV1::from_verified_http_runtime(executable_name: &str, verified: &VerifiedHttpRuntime) -> Result<RuntimeManifestV1, RuntimeManifestConstructionError>`
- Construction accepts only a verified HTTP runtime and copies the artifact size and SHA-256 from `VerifiedHttpRuntime` without rehashing or executing the candidate.
- The constructor uses the admitted runtime information already produced by the verification/probe flow and never independently accepts caller-supplied size, digest, or runtime metadata.

## Executable-name validation
- Allowed examples: `example-source-get-raw-data`, `example-source-get-raw-data.exe`
- Rejected cases:
  - empty string
  - `.` and `..`
  - `/`, `\`, or NUL bytes
  - absolute paths
  - parent traversal components
  - drive-prefix paths such as `C:\...` and `C:/...`
  - any colon-containing name (drive style)
- This ensures the manifest stores only a runtime-bundle relative filename, never a temporary build path.

## Digest parsing behavior
- Added `ExecutableSha256` with `ExecutableSha256::from_hex(value: &str) -> Result<ExecutableSha256, ExecutableSha256ParseError>`.
- Accepted values are exactly 64 characters long and lowercase hexadecimal only: `0-9` and `a-f`.
- Rejected values include uppercase hex, whitespace, prefixes like `sha256:`, non-hex characters, and incorrect length.

## Encoding and decoding APIs
- `RuntimeManifestV1::to_json() -> Result<String, RuntimeManifestEncodingError>`
- `RuntimeManifestV1::from_json(input: &str) -> Result<Self, RuntimeManifestDecodingError>`
- These operate entirely in memory and do not write the manifest to disk.
- Encoding uses the canonical Core runtime-information JSON as the nested document and emits JSON without an added trailing newline.
- Decoding rejects invalid JSON, duplicate keys, unknown fields, missing fields, unknown manifest schema versions, invalid executable names, zero executable sizes, malformed SHA-256 values, and malformed nested runtime information.

## Typed errors
- `RuntimeManifestConstructionError`:
  - `InvalidExecutableName`
- `RuntimeManifestEncodingError`:
  - `RuntimeInformation(RuntimeInformationEncodingError)`
  - `Serialization(String)`
- `RuntimeManifestDecodingError`:
  - `Json(String)`
  - `UnknownSchemaVersion(u32)`
  - `InvalidExecutableName`
  - `InvalidExecutableSize(u64)`
  - `InvalidSha256(String)`
  - `MalformedRuntimeInformation(RuntimeInformationDecodingError)`
- `ExecutableSha256ParseError`:
  - `InvalidLength(usize)`
  - `InvalidCharacter { index: usize, value: char }`

## Nested Core runtime-information delegation
- The framework does not define a second Rust model for Core identity/descriptor/runtime capability metadata.
- `RuntimeManifestV1` stores `RuntimeInformationV1` directly and delegates serialization/decoding through the Core APIs.
- The nested runtime_information object is taken from `RuntimeInformationV1::to_json()` and converted back through `RuntimeInformationV1::from_json(...)` during decode.

## Round-trip and validation results
- Verified runtime → manifest construction: passed
- Executable size matches `VerifiedHttpRuntime`: passed
- SHA-256 matches `VerifiedHttpRuntime`: passed
- Runtime information matches admitted probe result: passed
- JSON round trip equality: passed
- Manifest decode rejection checks: passed for all malformed inputs including:
  - invalid JSON
  - unknown fields
  - missing fields
  - unknown manifest schema versions
  - zero executable size
  - short SHA-256 values
  - uppercase SHA-256 values
  - non-hex SHA-256 values
  - malformed nested runtime information
  - duplicate JSON keys
  - invalid executable names

## No file written or published
- No `runtime.json` file was written to disk.
- No runtime bundle directory was created.
- No executable staging or publication step was performed.
- No runtime bundle was written or published.

## Test results
Executed successfully:
- `cargo test -p lexicon-framework --quiet` → 77 passed, 0 failed
- `cargo test --workspace --quiet` → workspace tests passed

## Bundle/install result
- The external MZA dependency was not available in this environment, so the bundle/install helper was not run.
- No MZA or installer code was modified.
