# Typed resume-handler implementation report

Implemented the optional typed resume handler for `HttpSourceContractV1` without adding resume runtime behavior.

- Exact type:
  `pub type HttpResumeFn = fn(&mut HttpAcquisitionContext, &[OsString]) -> AcquisitionResult<()>;`
- Descriptor field:
  `resume: Option<HttpResumeFn>`
- Builder API:
  `pub const fn with_resume(self, resume: HttpResumeFn) -> Self`
  `pub const fn resume_handler(&self) -> Option<HttpResumeFn>`
- Descriptor representation:
  `HttpSourceContractV1::new(acquire)` initializes `resume` to `None`.
  `with_resume(resume)` stores `Some(resume)` and preserves the existing `acquire` handler and `required_capabilities` set.

Constant-construction proof:

```rust
use std::ffi::OsString;
use lexicon_core::http::{
    AcquisitionResult,
    HttpAcquisitionContext,
    HttpCapability,
    HttpSourceContractV1,
};

pub const SOURCE: HttpSourceContractV1 = HttpSourceContractV1::new(acquire)
    .with_resume(resume)
    .requires(HttpCapability::ClientCertificateV1);

fn acquire(
    context: &mut HttpAcquisitionContext,
    args: &[OsString],
) -> AcquisitionResult<()> {
    let _ = context;
    let _ = args;
    Ok(())
}

fn resume(
    context: &mut HttpAcquisitionContext,
    args: &[OsString],
) -> AcquisitionResult<()> {
    let _ = context;
    let _ = args;
    Ok(())
}
```

This remains valid as a `const`, and the stored acquisition handler remains the same while the capability requirement is preserved.

Duplicate-registration behavior:
- Later `.with_resume(...)` calls replace earlier ones using ordinary builder semantics.
- No new error mechanism was introduced.

Positive coverage added in `lexicon-core/src/protocols/http/contract.rs`:
1. `HttpSourceContractV1::new(acquire).resume_handler()` is `None`.
2. `.with_resume(resume)` works in `const SOURCE`.
3. `resume_handler()` returns `Some(...)` after registration.
4. The retained resume function is callable with a mutable `HttpAcquisitionContext` and `&[OsString]`.
5. Registration does not replace or corrupt the acquisition handler.
6. `ClientCertificateV1` remains in the required capability set.
7. A second `.with_resume(...)` call replaces the first handler deterministically.
8. Existing capability and descriptor tests remain unchanged.

Compile-fail coverage:
- Added `lexicon-core/tests/ui/invalid_resume_handlers.rs`.
- It verifies type rejection for the unsupported signatures including async resume, by-value context, immutable context, missing `&[OsString]`, wrong argument type, reversed parameters, bool return, and `Result<(), String>`.
- The failures are triggered specifically at `.with_resume(resume)`.

Validation results:
- `cargo test -p lexicon-core --quiet` — passed.
- `cargo test --workspace --quiet` — passed.
- `bash automation/build_bundle_install/build_bundle_install.sh` — failed because the external MZA dependency is not present in this environment: `/home/runner/work/lexicon/lexicon/automation/build_bundle_install/../build_bundle_mza/mza/make-artifact.sh` does not exist.

This task intentionally does not add runtime resume selection, persistence, checkpointing, runner logic, or session reconciliation; it only registers the optional typed resume handler on the descriptor.
