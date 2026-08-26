# Implementation report

## Summary
Added the typed in-memory runtime invocation envelope contract for the managed runtime boundary in `lexicon-core`.

## Changes
- Created `lexicon-core/src/runtime/invocation.rs` to define:
  - `RUNTIME_INVOCATION_PROTOCOL_VERSION = 1`
  - `RuntimeExecutionMode` with strict `run` / `resume` identifier parsing
  - `RuntimeSupervisionMode` with strict `foreground` / `background` identifier parsing
  - validated `ProjectInvocationIdentity` and `SessionInvocationIdentity`
  - `RuntimeInvocationEnvelopeV1` with constructor validation for source-contract version and runtime execution compatibility
- Added typed errors for invalid identifiers, invalid values, and invalid construction states.
- Exported the new public API through `lexicon_core::runtime` in `lexicon-core/src/runtime/mod.rs`.
- Added focused Rust tests covering valid construction, invalid component values, and invalid execution-mode combinations.

## Validation
- Ran: `cargo test --workspace --quiet`
- Result: all workspace tests passed.
