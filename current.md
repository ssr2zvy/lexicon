Current implementation request: in-memory runtime information

Objective

Add the smallest in-memory runtime-information model connecting the implemented RuntimeIdentity and HttpSourceContractV1.

This step defines the information a future managed-runner probe will report. It must not implement runner execution, serialization, probing, artifact validation, or runtime admission.

Required implementation

1. Define the descriptor contract version

Add an associated constant to HttpSourceContractV1:

impl HttpSourceContractV1 {
    pub const CONTRACT_VERSION: u32 = 1;
}

This is the version of the Rust source descriptor contract. It remains distinct from:

* Core crate version;
* runner-template version;
* runtime-invocation protocol version;
* raw-data schema version;
* session schema version.

Do not derive this value from the crate package version.

2. Add the runtime-information type

Create:

lexicon-core/src/runtime/information.rs

Define:

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeInformationV1 {
    identity: RuntimeIdentity,
    descriptor_contract_version: u32,
    required_capabilities: HttpCapabilitySet,
    resume_handler_registered: bool,
}

The fields must remain private.

The type must be allocation-free and suitable for constant construction.

3. Construct information from an HTTP descriptor

Provide:

impl RuntimeInformationV1 {
    pub const fn from_http_source(
        identity: RuntimeIdentity,
        source: &HttpSourceContractV1,
    ) -> Self;
}

It must:

* preserve the supplied RuntimeIdentity;
* set descriptor_contract_version to HttpSourceContractV1::CONTRACT_VERSION;
* copy the descriptor’s required capability set;
* report whether a resume handler is registered;
* not invoke either handler;
* not retain handler function pointers;
* not retain the source descriptor reference.

The runtime information represents metadata only.

4. Add constant accessors

Provide:

pub const fn identity(&self) -> RuntimeIdentity;
pub const fn descriptor_contract_version(&self) -> u32;
pub const fn required_capabilities(
    &self,
) -> HttpCapabilitySet;
pub const fn resume_handler_registered(&self) -> bool;

These accessors must expose values without exposing mutable internal state.

5. Export the canonical type

Export the type through:

lexicon_core::runtime::RuntimeInformationV1

Also re-export that same canonical type through:

lexicon_core::http::RuntimeInformationV1

The two paths must not define separate wrapper types.

Important semantic distinction

required_capabilities describes what the source descriptor requires.

It does not describe:

* capabilities available from the selected Core build;
* capabilities supported by the parent CLI;
* capabilities admitted for a particular invocation;
* capabilities proven to be compatible.

Capability availability and requirement matching belong to a later step.

Likewise, construction must not reject disagreement between:

identity.source_contract_version()

and:

HttpSourceContractV1::CONTRACT_VERSION

Both values must remain independently observable so a later validation layer can detect and diagnose disagreement.

Required tests

Add focused tests proving:

1. RuntimeInformationV1 can be constructed in a constant.
2. The supplied runtime identity is preserved exactly.
3. The descriptor contract version is 1.
4. An empty descriptor produces an empty required-capability set.
5. ClientCertificateV1 is retained when declared through .requires(...).
6. resume_handler_registered() is false without .with_resume(...).
7. resume_handler_registered() is true with .with_resume(...).
8. Constructing runtime information does not invoke acquire.
9. Constructing runtime information does not invoke resume.
10. Identity contract version 2 and descriptor contract version 1 can coexist in the information object without construction failing.
11. The runtime and http export paths refer to the same Rust type.
12. All existing descriptor, capability, resume-handler, and identity tests remain unchanged and pass.

Preserve existing external behavior

Do not change:

* source scaffolding;
* source implementation crates;
* source create;
* source build;
* runtime publication;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, also run:

bash automation/build_bundle_install/build_bundle_install.sh

If the known missing-MZA dependency remains, report it separately. Do not expand this task into MZA dependency management.

Explicit exclusions

Do not implement:

* serde derives;
* JSON or other serialization;
* a runtime-information wire schema;
* runtime-information command-line output;
* a runtime-information probe;
* runner::run;
* a generated runner;
* descriptor invocation;
* runtime capability availability;
* capability negotiation or matching;
* identity compatibility validation;
* runtime admission;
* invocation envelopes;
* runtime.json;
* executable hashing;
* build artifact verification;
* source-library scaffolding;
* acquisition workspace migration;
* processing runtime information;
* HTTP execution;
* raw transaction recording;
* sessions;
* supervision;
* __operator-host.

These belong to later micro-steps.

Completion report

After completion, replace current.md with a focused implementation report containing:

* files created and changed;
* the exact RuntimeInformationV1 representation;
* the descriptor contract-version constant;
* constructor and accessor APIs;
* constant-construction proof;
* required-capability results;
* resume-registration results;
* proof that handlers were not invoked;
* proof that mismatched identity and descriptor versions remain independently representable;
* re-export equivalence;
* Core and workspace test results;
* official bundle/install result or the known external-MZA blocker.

Then stop. Do not add serialization, probing, validation, or managed-runner execution.