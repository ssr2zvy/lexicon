Implementation report

Completed the HTTP processing admission micro-step in `lexicon-core`.

What changed
- Added `lexicon-core/src/processing/invocation.rs` with the processing admission API and typed error handling.
- Exported the public processing admission API through `lexicon_core::processing`.
- Implemented `AdmittedProcessingHandler`, `AdmittedProcessingRuntimeInvocation`, and `admit_processing_runtime_invocation`.
- Preserved the exact envelope, supervision mode, project/session identity, and source arguments while selecting the registered processing function pointer without invoking it.
- Enforced the required validation order: protocol, operation, identity, descriptor contract version, and execution mode.
- Added focused tests covering successful admission, pointer identity, invalid combinations, resume rejection, and error sanitization.

Validation
- Ran: `cargo test -p lexicon-core processing --quiet`
- Result: passed (87/87 processing-related tests succeeded)

Notes
- Full workspace tests were intentionally skipped, as requested.
