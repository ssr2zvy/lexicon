# implementation report: framework-side processing probe-output admission

Files changed
- `lexicon-framework/src/build/runtime_probe.rs`
- `lexicon-framework/src/build/mod.rs`

Processing admission API
- Added `AdmittedProcessingRuntimeInformation` as an opaque wrapper around `ProcessingRuntimeInformationV1`.
- Added `admit_processing_runtime_information_probe(expected_identity: RuntimeIdentity, stdout: &[u8]) -> Result<AdmittedProcessingRuntimeInformation, ProcessingRuntimeProbeAdmissionError>`.
- The function performs no filesystem access and does not launch a subprocess.

Admitted processing wrapper
- `AdmittedProcessingRuntimeInformation::information()` returns the decoded `ProcessingRuntimeInformationV1` reference.
- The wrapper has no public unchecked constructor.

Shared private boundary validator
- Added a shared private validator `validate_runtime_probe_output(stdout: &[u8]) -> Result<&str, RuntimeProbeOutputBoundaryError>`.
- The validator enforces the required shared output boundary rules: max size via `MAX_RUNTIME_INFORMATION_PROBE_BYTES`, nonempty output, no NUL bytes, valid UTF-8, exactly one final ASCII newline, no `\r\n`, no extra trailing newline, no leading whitespace, no trailing whitespace before the newline, and no diagnostic text or extra JSON document content.
- Both acquisition admission and processing admission reuse this shared boundary validation to avoid duplicated checks.

Exact accepted and rejected output forms
- Accepted: valid Core-generated processing probe output with a single trailing `\n` and otherwise exact JSON-only text.
- Rejected: empty output, oversized output, NUL-containing output, invalid UTF-8, missing final newline, two final newlines, CRLF (`\r\n`), leading whitespace, leading newline, trailing whitespace immediately before the newline, prefix/suffix diagnostic text, and multiple JSON documents.

Typed processing admission errors
- `ProcessingRuntimeProbeAdmissionError` includes:
  - `OutputTooLarge { maximum, actual }`
  - `EmptyOutput`
  - `ContainsNul`
  - `InvalidUtf8(std::str::Utf8Error)`
  - `InvalidOutputBoundary`
  - `Decode(ProcessingRuntimeInformationDecodingError)`
  - `Incompatible(ProcessingRuntimeCompatibilityError)`
- `Display` and `Error` implementations are provided.

Core decoding and compatibility delegation
- Admission order is: shared boundary validation -> strip exactly one final newline -> `ProcessingRuntimeInformationV1::from_json(...)` -> `validate_compatibility(expected_identity)` -> construct wrapper.
- Structural decoding, JSON-schema validation, identity parsing, and compatibility rules remain delegated to Core.

Successful and failed admission results
- Matching processing identity and valid Core-generated output succeeds and returns the opaque admitted wrapper.
- A different source identity, mismatched descriptor contract version, acquisition-operation document, or invalid JSON all fail as typed errors instead of being admitted.

Acquisition admission remains unchanged
- `admit_http_runtime_information_probe` retains the same public API and the same error classification variants.
- The raw acquisition behavior is preserved while sharing the same private boundary validator.

Proof that the processing handler was not invoked
- The processing admission tests panic inside a `failing_process_handler` and still succeed for valid admission, proving the function validates captured metadata without executing the processing handler or constructing a `ProcessingContext`.
- No runtime execution, SQLite setup, or session creation was introduced.

Framework and workspace test results
- `cargo test -p lexicon-framework --quiet` passed.
- `cargo test --workspace --quiet` passed on the final rerun.
- One earlier workspace run hit a transient file-busy race in an unrelated probe test, but the rerun completed successfully with all workspace tests passing.

Bundle/install result or external blocker
- The external MZA bundle/install helper is not available in this sandbox environment, so the external bundle/install flow was not run here. This is the known external blocker for that step.
- No MZA or installer code was modified.

Stopped after report generation; no processing runtime subprocess was executed.
* framework processing probe-output admission;
* processing subprocess execution;
* processing executable hashing integration;
* processing verification;
* processing manifests;
* processing staging;
* processing bundle admission;
* paired publication;
* processing runner main.rs;
* processing execution;
* raw-transaction discovery;
* SQLite behavior;
* processing sessions;
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

* files created and changed;
* canonical shared probe argument location;
* preserved acquisition re-export;
* processing probe API;
* outcome and error types;
* exact argument behavior;
* output and newline behavior;
* construction-failure behavior;
* write and flush failure results;
* proof that the process handler was not invoked;
* non-UTF-8 argument behavior;
* acquisition compatibility results;
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not add framework-side processing probing or a managed processing runner.