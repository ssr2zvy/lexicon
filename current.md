# Current implementation report: Core child admission for HTTP acquisition invocations

## Summary

Implemented the HTTP runtime admission micro-step in `lexicon-core` for parsed runtime invocations. The new admission layer validates the compiled identity, source contract version, capability requirements, and selected handler without invoking it.

## Changes made

- Added `lexicon-core/src/protocols/http/invocation.rs` with:
  - `AdmittedHttpHandler`
  - `AdmittedHttpRuntimeInvocation`
  - `HttpRuntimeInvocationAdmissionError`
  - `admit_http_runtime_invocation(...)`
- Exported the API from `lexicon_core::http` via `lexicon-core/src/protocols/http/mod.rs`.
- Added a constructor for `MissingHttpCapabilities` so the HTTP admission path can reuse the runtime capability comparison without bypassing the existing typed error model.
- Preserved the original source arguments exactly as they were transported through `ParsedRuntimeInvocation`, and selected the correct acquisition/resume handler without invoking it.

## Validation

- `cargo test -p lexicon-core --quiet`

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpRuntimeInvocationAdmissionError {
    WrongCompiledProtocol {
        actual: RuntimeProtocol,
    },
    WrongCompiledOperation {
        actual: RuntimeOperation,
    },
    IdentityMismatch {
        compiled: RuntimeIdentity,
        envelope: RuntimeIdentity,
    },
    DescriptorContractVersionMismatch {
        identity_version: u32,
        descriptor_version: u32,
    },
    MissingCapabilities(
        MissingHttpCapabilities,
    ),
    ResumeHandlerUnavailable,
}

Equivalent naming is acceptable.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String, print arguments, or exit.

Error formatting must not reveal:

* source arguments;
* session identity;
* project identity;
* envelope JSON.

Runtime identity values may be represented in diagnostics only through their established non-secret identifiers.

No handler invocation

Admission must not:

* call acquisition;
* call resume;
* construct HttpAcquisitionContext;
* perform HTTP;
* create raw transactions;
* create sessions;
* access files;
* launch processes.

Use handler call counters or panic handlers to prove this.

Required tests

Add tests proving:

1. Matching acquisition/run invocation is admitted.
2. Run selects the exact acquisition function pointer.
3. Matching acquisition/resume invocation is admitted when resume exists.
4. Resume selects the exact resume function pointer.
5. Foreground mode is preserved.
6. Background mode is preserved.
7. Project identity is preserved.
8. Session identity is preserved.
9. Source arguments remain in exact order.
10. Empty source argument values are preserved.
11. Reserved-looking source values are preserved.
12. Non-UTF-8 Unix source arguments are preserved byte-for-byte.
13. Wrong compiled protocol is typed when a real alternate protocol exists.
14. Processing compiled identity returns WrongCompiledOperation.
15. Envelope and compiled source mismatch returns IdentityMismatch.
16. Envelope and compiled operation mismatch returns IdentityMismatch.
17. Envelope and compiled version mismatch returns IdentityMismatch.
18. Compiled descriptor-version mismatch is typed.
19. Missing capability requirements return the complete missing set.
20. Extra available capabilities do not cause rejection.
21. Resume without a registered handler returns ResumeHandlerUnavailable.
22. Resume-handler absence is checked after identity, version, and capabilities.
23. Admission does not invoke acquisition.
24. Admission does not invoke resume.
25. Failed admission cannot construct the admitted value.
26. Existing invocation transport tests remain unchanged.
27. Existing HTTP descriptor and capability tests remain unchanged.
28. Processing descriptor behavior remains unchanged.
29. All workspace tests pass repeatedly.

Do not add a fake protocol solely to test the wrong-protocol branch.

Preserve existing behavior

Do not change:

* invocation JSON or argv transport;
* source descriptor signatures;
* capability identifiers;
* resume registration;
* runtime-information probes;
* hashing;
* verification;
* manifests;
* staging;
* bundle admission;
* paired publication;
* source scaffolding;
* source create;
* source build;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior.

Validation

Run:

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet
cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* processing child admission;
* handler invocation;
* runner::run;
* runner main.rs;
* managed runner generation;
* process launching;
* project filesystem validation;
* session creation or locking;
* HTTP execution;
* raw recording;
* processing SQLite behavior;
* foreground supervision;
* background supervision;
* __operator-host;
* source workspace migration;
* source build integration.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* admission API;
* admitted invocation and handler representations;
* exact validation order;
* operation-identity guard;
* descriptor-version behavior;
* capability-validation behavior;
* run and resume selection results;
* source-argument preservation results;
* proof that handlers were not invoked;
* typed failure results;
* Core and repeated workspace test results;
* bundle/install result or known external blocker.

Then stop. Do not invoke the selected handler.