# Implementation report

## Summary
Added the typed in-memory processing runtime information model to `lexicon-core` without changing the existing acquisition runtime behavior or the framework runtime probe flow.

## Changes made
- Created `lexicon-core/src/processing/runtime_information.rs`.
- Added `ProcessingRuntimeInformationV1` as a copyable, allocation-free value type with private fields and accessors.
- Added construction validation tied to `RuntimeIdentity` and `ProcessingSourceContractV1`:
  - HTTP-only protocol requirement
  - processing operation requirement
  - source contract version must match `ProcessingSourceContractV1::CONTRACT_VERSION`
- Added explicit error types for construction and compatibility checks:
  - `ProcessingRuntimeInformationConstructionError`
  - `ProcessingRuntimeCompatibilityError`
- Exported the public API through `lexicon_core::processing`.
- Added focused tests covering successful construction, identity preservation, protocol and operation validation, version mismatches, compatibility checks, no-handler invocation, descriptor immutability, and private handler access behind the public source.

## Validation
Ran:
```bash
cargo test --workspace --quiet
```
Result: all workspace tests passed successfully.

## Notes
This change intentionally does not add JSON serialization, runtime probing, manifests, staging, publication, or runner logic. It only defines the in-memory processing runtime information required for a later serialized probe document step.
