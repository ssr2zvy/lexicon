# Implementation report

Implemented the bounded runtime information probe work for both HTTP acquisition and processing candidates.

## What changed
- Added a shared private transport, `execute_runtime_information_probe`, that owns process creation, reserved probe argument injection, null stdin, concurrent bounded stdout/stderr draining, timeout handling, cleanup, exit-status validation, and deterministic error precedence.
- Kept the existing acquisition API path intact while routing it through the shared transport and preserving the established `probe_http_runtime_information` result/error contract.
- Added the processing probe admission and execution flow:
  - `AdmittedProcessingRuntimeInformation`
  - `ProcessingRuntimeProbeAdmissionError`
  - `ProcessingRuntimeProbeExecutionError`
  - `admit_processing_runtime_information_probe(...)`
  - `probe_processing_runtime_information(...)`
- Exported the processing APIs via `lexicon-framework/src/build/mod.rs`.
- Corrected the bounded stream drainer so it continues draining while discarding overflow instead of stopping early, preventing pipe deadlocks.

## Validation
- Ran: `cargo test --workspace --quiet`
- Result: all workspace tests passed.
