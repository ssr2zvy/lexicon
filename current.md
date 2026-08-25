# Implementation report

Files created and changed:
- `lexicon-core/src/runtime/information.rs`
- `lexicon-core/src/runtime/mod.rs`
- `lexicon-core/src/protocols/http/mod.rs`
- `lexicon-core/src/protocols/http/contract.rs`
- `current.md`

Exact runtime metadata representation:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeInformationV1 {
    identity: RuntimeIdentity,
    descriptor_contract_version: u32,
    required_capabilities: HttpCapabilitySet,
    resume_handler_registered: bool,
}
```

Descriptor contract version:

```rust
impl HttpSourceContractV1 {
    pub const CONTRACT_VERSION: u32 = 1;
}
```

Constructor and accessor APIs:

```rust
impl RuntimeInformationV1 {
    pub const fn from_http_source(
        identity: RuntimeIdentity,
        source: &HttpSourceContractV1,
    ) -> Self {
        Self {
            identity,
            descriptor_contract_version: HttpSourceContractV1::CONTRACT_VERSION,
            required_capabilities: source.required_capabilities(),
            resume_handler_registered: source.resume_handler().is_some(),
        }
    }

    pub const fn identity(&self) -> RuntimeIdentity;
    pub const fn descriptor_contract_version(&self) -> u32;
    pub const fn required_capabilities(&self) -> HttpCapabilitySet;
    pub const fn resume_handler_registered(&self) -> bool;
}
```

Constant-construction proof:
- `RuntimeInformationV1::from_http_source(...)` is a `const fn` and was validated with a compile-time constant construction test.
- The data model remains allocation-free and stores only the captured identity, a u32 descriptor version, the capability set value, and a bool registration flag.

Required-capability results:
- Empty descriptors produce `HttpCapabilitySet::empty()`.
- Declaring `.requires(HttpCapability::ClientCertificateV1)` retains the capability in the metadata snapshot.

Resume-registration results:
- `resume_handler_registered() == false` when no `.with_resume(...)` handler is attached.
- `resume_handler_registered() == true` when a resume handler is attached.

Proof that handlers were not invoked:
- Runtime metadata construction only reads `source.required_capabilities()` and `source.resume_handler().is_some()`.
- The tests use `panic!`-based acquire/resume handlers to ensure `from_http_source` does not call either handler.
- Construction succeeded without triggering those panic branches.

Proof that mismatched identity and descriptor versions remain independently representable:
- `RuntimeIdentity::http_acquisition("example-source", 2)` was constructed alongside a descriptor whose contract version is `HttpSourceContractV1::CONTRACT_VERSION == 1`.
- `RuntimeInformationV1` recorded both independently without rejecting the mismatch.

Re-export equivalence:
- `lexicon_core::runtime::RuntimeInformationV1` and `lexicon_core::http::RuntimeInformationV1` resolve to the same Rust type.
- The test verifies that both export paths compare equal and are identical concrete types.

Validation results:
- `cargo test -p lexicon-core --quiet` -> passed (`33` unit tests + `1` trybuild UI test passed)
- `cargo test --workspace --quiet` -> passed across the workspace

Bundle/install result:
- Attempted: `bash automation/build_bundle_install/build_bundle_install.sh`
- Result: failed because the external MZA dependency is still unavailable in this environment:
  `bash: /home/runner/work/lexicon/lexicon/automation/build_bundle_mza/mza/make-artifact.sh: No such file or directory`
- This is the known external-MZA blocker, and no dependency-management expansion was undertaken as part of this task.

Implementation status: complete for the requested in-memory runtime-information metadata step, without adding serialization, probing, validation, or managed-runner execution.
