Current implementation request: add typed required-capability declarations

Objective

Extend HttpSourceContractV1 with one small feature:

.requires(HttpCapability::ClientCertificateV1)

This task only lets a source descriptor declare required capabilities.

Do not implement capability availability checking, runner support, build validation, or runtime admission yet.

Required API

This must compile as a constant:

use std::ffi::OsString;
use lexicon_core::http::{
    AcquisitionResult,
    HttpAcquisitionContext,
    HttpCapability,
    HttpSourceContractV1,
};
pub const SOURCE: HttpSourceContractV1 =
    HttpSourceContractV1::new(acquire)
        .requires(HttpCapability::ClientCertificateV1);
fn acquire(
    context: &mut HttpAcquisitionContext,
    args: &[OsString],
) -> AcquisitionResult<()> {
    let _ = context;
    let _ = args;
    Ok(())
}

A descriptor without requirements must continue compiling:

pub const SOURCE: HttpSourceContractV1 =
    HttpSourceContractV1::new(acquire);

Capability type

Add:

lexicon-core/src/protocols/http/capability.rs

Define:

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpCapability {
    ClientCertificateV1,
}

Expose it as:

lexicon_core::http::HttpCapability

Do not accept arbitrary strings as capabilities.

Provide a stable machine-readable identifier:

impl HttpCapability {
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::ClientCertificateV1 =>
                "client-certificate-v1",
        }
    }
}

Do not implement client-certificate behavior. This variant is only a typed capability identity.

Capability set

Add a small immutable value type:

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpCapabilitySet {
    // private representation
}

It must provide:

impl HttpCapabilitySet {
    pub const fn empty() -> Self;
    pub const fn contains(
        self,
        capability: HttpCapability,
    ) -> bool;
}

It may use a private bitset representation so descriptor construction remains valid in a const.

Do not expose raw bit positions as part of the public API.

Adding the same capability twice must be idempotent rather than an error.

Descriptor extension

Extend HttpSourceContractV1 with a private required-capability set.

Add:

impl HttpSourceContractV1 {
    pub const fn requires(
        self,
        capability: HttpCapability,
    ) -> Self;
    pub const fn required_capabilities(
        &self,
    ) -> HttpCapabilitySet;
}

Requirements:

* requires must work in a pub const SOURCE;
* it must preserve the existing acquisition handler;
* repeated calls must accumulate requirements;
* repeated declaration of the same capability must not duplicate it;
* the capability storage must remain typed;
* descriptor fields must remain non-public.

Tests

Add tests proving:

1. A descriptor with no .requires(...) has an empty requirement set.
2. ClientCertificateV1 can be declared in a constant descriptor.
3. The returned set contains ClientCertificateV1.
4. Calling .requires(ClientCertificateV1) twice is idempotent.
5. Adding a capability does not replace or corrupt the acquisition handler.
6. Existing descriptor and compile-fail tests still pass.
7. A string cannot be passed to .requires(...).

The invalid string case must be a real compile-fail test:

HttpSourceContractV1::new(acquire)
    .requires("client-certificate-v1");

Do not test build-time or runtime availability because those mechanisms do not exist yet.

Preserve existing behavior

Do not change:

* AcquisitionError;
* AcquisitionResult;
* the mandatory acquisition function signature;
* historical HttpAcquisition;
* historical run_http_source;
* source scaffolding;
* source implementation crates;
* source builds;
* runtime publication;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installed paths.

Explicit exclusions

Do not implement:

* actual client-certificate loading;
* TLS configuration;
* capability providers;
* capability negotiation;
* parent capability validation;
* child capability validation;
* capability serialization;
* runtime.json;
* runtime probing;
* optional handlers;
* with_resume;
* source implementation libraries;
* managed runners;
* build-state types;
* invocation envelopes;
* HTTP execution or recording;
* sessions or supervision.

Validation

Run:

cargo test --workspace --quiet

Run the official validator:

bash automation/build_bundle_install/build_bundle_install.sh

Verify that the existing CLI, source scaffolding, source builds, Protocol 1 bundle, and installer remain unchanged.

Completion report

Replace current.md with a focused report containing:

* the HttpCapability definition;
* its stable identifier;
* the capability-set representation;
* the .requires(...) implementation;
* proof that it works in pub const SOURCE;
* idempotency test results;
* compile-fail string test result;
* confirmation that the acquisition handler remains unchanged;
* workspace and official validation results;
* any blocker.

Then stop. Do not implement capability enforcement or optional handlers.