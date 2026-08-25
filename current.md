# Implementation report

## Files changed
- `lexicon-core/src/runtime/identity.rs`
- `lexicon-core/src/runtime/information.rs`

## Updated RuntimeOperation
The canonical runtime operation enum now includes the processing runtime alongside acquisition:

- `RuntimeOperation::Acquisition`
- `RuntimeOperation::Processing`

Stable identifiers are:
- `Acquisition` -> `"acquisition"`
- `Processing` -> `"processing"`

`RuntimeOperation::identifier()` and `RuntimeOperation::from_identifier(...)` now round-trip `"processing"` to `RuntimeOperation::Processing`, while continuing to reject aliases, capitalization differences, and surrounding whitespace.

## Processing identity constructor
Added:

- `RuntimeIdentity::http_processing(source_name, source_contract_version) -> Self`

It constructs:
- `source_name` from the supplied source
- `protocol` = `RuntimeProtocol::Http`
- `operation` = `RuntimeOperation::Processing`
- `source_contract_version` from the supplied version

The existing acquisition constructor remains unchanged:
- `RuntimeIdentity::http_acquisition(...)`

## Constant-construction proof
The processing identity constructor is const-safe and verified with:

- `const IDENTITY: RuntimeIdentity = RuntimeIdentity::http_processing("example-source", 1);`

The test asserts:
- `source_name() == "example-source"`
- `protocol() == RuntimeProtocol::Http`
- `operation() == RuntimeOperation::Processing`
- `source_contract_version() == 1`

## Accessor and equality results
Processing identities expose the expected values through the existing accessors and participate in the established `Debug`, `Clone`, `Copy`, `PartialEq`, and `Eq` behavior.

Equality checks confirm:
- acquisition identity and processing identity for the same source/version are not equal
- processing identity compares equal only to the same processing identity
- acquisition behavior remains consistent with the existing identity semantics

## Runtime-information JSON round-trip results
`RuntimeInformationV1` now supports processing operation JSON by serializing and decoding `"operation": "processing"` without changing the schema version.

Validated results:
- processing identity serializes with `"operation":"processing"`
- `RuntimeInformationV1::from_json(...)` accepts the processing operation
- a processing identity survives a JSON round trip without loss of identity information
- unknown operation identifiers are still rejected

## Acquisition/processing compatibility mismatch results
The compatibility validator compares identities exactly as required.

Verified cases:
- matching processing identity passes compatibility
- expected acquisition vs actual processing returns `RuntimeCompatibilityError::IdentityMismatch`
- expected processing vs actual acquisition returns `RuntimeCompatibilityError::IdentityMismatch`

## Acquisition behavior confirmation
Acquisition identity behavior remains unchanged:
- acquisition serialization still emits `"operation":"acquisition"`
- acquisition parsing and compatibility checks continue to pass
- the existing runtime probe and framework behavior remains intact

## Type-level guard limitation in from_http_source(...)
`RuntimeInformationV1::from_http_source(...)` still accepts any `RuntimeIdentity` in this step; there is no type-level guard preventing a processing identity from being passed through the HTTP-source constructor. This limitation is documented here and intentionally left unchanged because a later processing-descriptor step will define the proper construction path rather than redesigning the runtime-information hierarchy in this micro-step.

## Core and workspace test results
Validation succeeded with the standard Cargo flow:
- `cargo test -p lexicon-core --quiet` ✅
- `cargo test --workspace --quiet` ✅

## Bundle/install result
The optional bundle/install helper was attempted with:

- `bash automation/build_bundle_install/build_bundle_install.sh`

It failed because the external MZA checkout is unavailable in this environment:
- `/home/runner/work/lexicon/lexicon/automation/build_bundle_install/../build_bundle_mza/mza/make-artifact.sh: No such file or directory`

This is the known external blocker, and no MZA or installer code was modified.
