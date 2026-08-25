Current implementation request: add the optional typed resume handler

Objective

Extend HttpSourceContractV1 with one optional handler:

.with_resume(resume)

This task only registers and retains a correctly typed resume function.

Do not implement session resumption, handler selection, runner behavior, or runtime invocation yet.

Required public API

Add:

pub type HttpResumeFn = fn(
    &mut HttpAcquisitionContext,
    &[OsString],
) -> AcquisitionResult<()>;

Expose it as:

lexicon_core::http::HttpResumeFn

Extend HttpSourceContractV1 with:

impl HttpSourceContractV1 {
    pub const fn with_resume(
        self,
        resume: HttpResumeFn,
    ) -> Self;
    pub const fn resume_handler(
        &self,
    ) -> Option<HttpResumeFn>;
}

A descriptor with a resume handler must compile as a constant:

use std::ffi::OsString;
use lexicon_core::http::{
    AcquisitionResult,
    HttpAcquisitionContext,
    HttpCapability,
    HttpSourceContractV1,
};
pub const SOURCE: HttpSourceContractV1 =
    HttpSourceContractV1::new(acquire)
        .with_resume(resume)
        .requires(
            HttpCapability::ClientCertificateV1,
        );
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

Descriptor representation

Add one private field to HttpSourceContractV1:

resume: Option<HttpResumeFn>

HttpSourceContractV1::new(acquire) must initialize it to:

None

.with_resume(resume) must return a descriptor containing:

Some(resume)

Requirements:

* the method must remain usable in pub const SOURCE;
* registering resume must preserve the mandatory acquisition handler;
* registering resume must preserve all required capabilities;
* a descriptor without .with_resume(...) must continue working unchanged;
* the handler must be stored as a typed function pointer;
* do not use strings, dynamic dispatch, serialization, or a registry.

If .with_resume(...) is called more than once, the later call may replace the earlier handler, following ordinary builder-method behavior. Do not add a new error mechanism solely for duplicate registration.

Compile-time guarantees

Rust must reject resume functions with the wrong signature, including:

* asynchronous resume functions;
* context passed by value;
* immutable context;
* missing &[OsString];
* wrong argument type;
* reversed parameters;
* bool return;
* Result<(), String> return.

The compiler verifies the function shape. It does not verify that the function performs genuine resume behavior.

Tests

Add positive tests proving:

1. HttpSourceContractV1::new(acquire).resume_handler() returns None.
2. .with_resume(resume) works inside pub const SOURCE.
3. resume_handler() returns Some(...) after registration.
4. The retained resume function can be invoked with a mutable context and &[OsString].
5. Registering resume does not replace or corrupt the acquisition handler.
6. Registering resume preserves ClientCertificateV1 in the required-capability set.
7. A second .with_resume(...) call deterministically replaces the first handler.
8. Existing descriptor and capability tests remain unchanged and pass.

Add focused compile-fail coverage for at least:

async fn resume(...)
fn resume(
    context: HttpAcquisitionContext,
    args: &[OsString],
) -> AcquisitionResult<()>
fn resume(
    context: &mut HttpAcquisitionContext,
    args: &[OsString],
) -> Result<(), String>

The compile-fail tests must fail specifically at:

.with_resume(resume)

Preserve existing behavior

Do not change:

* the mandatory acquire signature;
* HttpAcquireFn;
* AcquisitionError;
* AcquisitionResult;
* HttpCapability;
* HttpCapabilitySet;
* .requires(...);
* historical HttpAcquisition;
* historical run_http_source;
* source scaffolding;
* source builds;
* runtime publication;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle.

No resume semantics yet

This task does not decide:

* when a session is resumable;
* whether acquire or resume is selected;
* what checkpoint is supplied;
* how stale sessions are reconciled;
* how source arguments are restored;
* how the runner invokes resume;
* how resume success or failure changes session state.

It only makes the optional handler part of the typed source descriptor.

Validation

Run:

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet

Run the official validator if its external MZA dependency is available:

bash automation/build_bundle_install/build_bundle_install.sh

If the previously reported unrelated CLI or missing-MZA failures still exist, report them separately and prove that the focused lexicon-core tests pass. Do not broaden this task into repairing unrelated validation infrastructure.

Explicit exclusions

Do not implement:

* runtime resume selection;
* checkpoints;
* session persistence;
* session reconciliation;
* source argument persistence;
* capability enforcement;
* capability negotiation;
* additional optional handlers;
* source implementation libraries;
* managed runners;
* runtime identity;
* runtime probing;
* runtime.json;
* invocation envelopes;
* HTTP execution or recording;
* foreground supervision;
* __operator-host;
* processing changes.

Completion report

Replace current.md with a focused report containing:

* the exact HttpResumeFn type;
* the new descriptor field;
* .with_resume(...);
* resume_handler();
* proof of constant construction;
* proof that acquire and capability requirements are preserved;
* duplicate-registration behavior;
* positive test results;
* compile-fail test results;
* workspace and official validation results;
* any unrelated pre-existing blocker.

Then stop. Do not implement runtime resume behavior or managed runners.