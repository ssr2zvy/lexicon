# Http capability requirement support report

Implemented the typed required-capability declaration support for `HttpSourceContractV1` without adding runtime enforcement or optional-handler logic.

- `HttpCapability` is defined in `lexicon-core/src/protocols/http/capability.rs` as:
  `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum HttpCapability { ClientCertificateV1 }`
- Stable identifier: `HttpCapability::ClientCertificateV1.identifier() == "client-certificate-v1"`.
- Capability set representation: `HttpCapabilitySet` uses a private immutable bitmask (`bits: u8`) with `empty()`, `insert(...)`, and `contains(...)` operations. The bit positions are not exposed in the public API.
- Descriptor extension: `HttpSourceContractV1` now stores a private `required_capabilities: HttpCapabilitySet`, exposes `requires(self, capability: HttpCapability) -> Self`, and exposes `required_capabilities(&self) -> HttpCapabilitySet`.
- `requires(...)` preserves the existing acquisition handler, works in a `pub const SOURCE`, and accumulates requirements without duplicate entries.

Proof in code:

```rust
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
```

Idempotency and behavior:
- `HttpSourceContractV1::new(acquire).required_capabilities()` is empty.
- `HttpCapability::ClientCertificateV1` is declared in a const descriptor and `contains(...)` returns `true`.
- `.requires(HttpCapability::ClientCertificateV1)` called twice yields the same set as a single call.
- The acquisition function pointer remains unchanged after `.requires(...)`.

Compile-fail guard:
- Added `lexicon-core/tests/ui/requires_string.rs` to verify that a string cannot be passed to `.requires(...)`.
- Result: compile-fail test passes and rejects `requires("client-certificate-v1")` with `expected HttpCapability, found &str`.

Validation results:
- `cargo test -p lexicon-core --quiet` passed: 9 unit tests + 1 UI compile-fail suite passed.
- `cargo test --workspace --quiet` still fails in the unrelated `lexicon-cli` test `cli::tests::unrelated_preexisting_directory_remains_untouched` due a missing filesystem path / executable expectation, not from the HTTP capability change.
- `bash automation/build_bundle_install/build_bundle_install.sh` failed because the expected helper script `automation/build_bundle_install/../build_bundle_mza/mza/make-artifact.sh` is not present in this environment; this is a blocker for the official bundle/install validation path.

No capability enforcement, capability negotiation, runtime validation, or optional handler support was added; the change is limited to typed requirement declaration and tests only.
