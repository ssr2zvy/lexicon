Current implementation request: in-memory processing runtime information

Objective

Add the typed in-memory runtime-information model connecting:

* RuntimeIdentity::http_processing(...);
* ProcessingSourceContractV1;
* the processing descriptor contract version.

This step defines what a future processing runtime probe will report, without adding JSON, probing, runners, execution, manifests, staging, or publication.

Required module

Create:

lexicon-core/src/processing/runtime_information.rs

Export the public API through:

lexicon_core::processing

Do not reuse the HTTP acquisition RuntimeInformationV1 as the processing information type.

Why processing needs its own type

The existing acquisition information includes acquisition-specific metadata such as:

* required HTTP capabilities;
* available HTTP capabilities;
* resume-handler registration.

Those fields do not belong to the current processing descriptor.

Define a distinct processing type rather than representing processing with empty HTTP capability sets or a false HTTP resume flag.

Processing information type

Define:

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessingRuntimeInformationV1 {
    identity: RuntimeIdentity,
    descriptor_contract_version: u32,
}

Keep fields private.

Provide accessors:

impl ProcessingRuntimeInformationV1 {
    pub const fn identity(
        &self,
    ) -> RuntimeIdentity;
    pub const fn descriptor_contract_version(
        &self,
    ) -> u32;
}

The type must remain allocation-free and copyable.

Construction error

Define:

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingRuntimeInformationConstructionError {
    WrongProtocol {
        actual: RuntimeProtocol,
    },
    WrongOperation {
        actual: RuntimeOperation,
    },
    IdentityContractVersionMismatch {
        identity_version: u32,
        descriptor_version: u32,
    },
}

Equivalent naming is acceptable, but all three failures must remain distinguishable.

Implement:

std::fmt::Display
std::error::Error

Do not return String.

Construction API

Provide:

impl ProcessingRuntimeInformationV1 {
    pub fn from_processing_source(
        identity: RuntimeIdentity,
        source: &ProcessingSourceContractV1,
    ) -> Result<
        Self,
        ProcessingRuntimeInformationConstructionError,
    >;
}

The source reference enforces that construction is tied to the typed processing descriptor.

The function must validate, in this order:

1. identity.protocol() == RuntimeProtocol::Http
2. identity.operation() == RuntimeOperation::Processing
3. identity.source_contract_version() == ProcessingSourceContractV1::CONTRACT_VERSION

Only then construct the information object.

The function must not invoke:

source.process_handler()

The descriptor reference is used for type linkage, not execution.

Compatibility validation

Provide:

impl ProcessingRuntimeInformationV1 {
    pub fn validate_compatibility(
        &self,
        expected_identity: RuntimeIdentity,
    ) -> Result<(), ProcessingRuntimeCompatibilityError>;
}

Define:

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingRuntimeCompatibilityError {
    IdentityMismatch {
        expected: RuntimeIdentity,
        actual: RuntimeIdentity,
    },
    DescriptorContractVersionMismatch {
        identity_version: u32,
        descriptor_version: u32,
    },
}

Implement Display and Error.

Validation order:

1. exact identity equality;
2. descriptor contract version equals the actual identity’s source contract version.

There is no processing capability check in this version.

Type safety requirement

These must be impossible through the supported constructor:

acquisition identity + processing descriptor
non-HTTP identity + processing descriptor
processing identity version 2 + ProcessingSourceContractV1

Do not silently construct inconsistent processing information and defer all errors until later.

No JSON yet

Do not add:

to_json()
from_json()

Do not add a processing runtime-information schema version yet.

The next micro-step will define the serialized processing probe document after this in-memory model is stable.

Required tests

Add tests proving:

1. A valid processing identity and descriptor construct successfully.
2. The information preserves the source identity.
3. The information protocol is HTTP.
4. The information operation is Processing.
5. The descriptor contract version is 1.
6. The type is Copy.
7. Construction does not invoke the process handler.
8. An acquisition identity returns WrongOperation.
9. An identity contract version other than 1 returns IdentityContractVersionMismatch.
10. Validation against the same processing identity succeeds.
11. Validation against another source returns IdentityMismatch.
12. Validation against an acquisition identity returns IdentityMismatch.
13. Descriptor-version disagreement returns DescriptorContractVersionMismatch.
14. Construction and validation do not mutate the descriptor.
15. Construction and validation do not invoke the process handler.
16. A private process handler behind public SOURCE works.
17. Native source arguments are not involved in information construction.
18. Existing processing descriptor tests remain unchanged.
19. Existing acquisition runtime-information behavior remains unchanged.
20. Existing framework tests remain unchanged.
21. All workspace tests pass.

If only HTTP currently exists in RuntimeProtocol, do not add a fake protocol solely to test WrongProtocol. Test that branch only when a real second protocol exists, while retaining the typed variant for future use.

Preserve existing behavior

Do not change:

* HTTP acquisition runtime-information representation;
* acquisition runtime-information JSON;
* acquisition probe behavior;
* processing descriptor signature;
* runtime identity identifiers;
* framework probing;
* hashing;
* verification;
* manifests;
* staging;
* bundle admission;
* reversible publication;
* source scaffolding;
* source create;
* source build;
* Cargo invocation;
* legacy publication;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-framework remains library-only.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Explicit exclusions

Do not implement:

* processing runtime-information JSON;
* processing probe arguments;
* processing probe handler;
* framework processing probe admission;
* processing subprocess probing;
* processing executable verification;
* processing manifests;
* processing staging;
* processing bundle admission;
* paired publication;
* processing runner;
* processing main.rs;
* processing execution;
* raw-transaction discovery;
* SQLite behavior;
* processing sessions;
* source workspace migration;
* acquisition managed runners;
* runner::run;
* invocation envelopes;
* HTTP transport;
* raw recording;
* supervision;
* __operator-host.

Validation

Run:

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If it remains unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the exact processing runtime-information representation;
* construction API;
* construction error representation;
* validation order;
* compatibility error representation;
* successful construction and validation results;
* each rejected inconsistent identity case;
* proof that the processing handler was not invoked;
* confirmation that acquisition runtime information was unchanged;
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not serialize or probe processing runtime information.