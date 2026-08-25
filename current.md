Current implementation request: runtime-information compatibility validation

Objective

Add the pure Core validation that compares probed RuntimeInformationV1 against the runtime identity expected by Lexicon.

This step defines the compatibility decision that the future framework-side build probe and runtime-admission paths will call.

Do not launch a subprocess, generate a runner, or change source scaffolding.

Required validation phases

A structurally decoded runtime-information document is not necessarily compatible.

Compatibility requires:

1. The reported runtime identity exactly matches the identity Lexicon expected.
2. The descriptor contract version matches the source contract version declared by that identity.
3. Every capability required by the descriptor exists in the runtime’s available capability set.

These checks occur after JSON decoding.

Typed compatibility error

Add a canonical error type under lexicon_core::runtime, equivalent to:

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCompatibilityError {
    IdentityMismatch {
        expected: RuntimeIdentity,
        actual: RuntimeIdentity,
    },
    DescriptorContractVersionMismatch {
        identity_version: u32,
        descriptor_version: u32,
    },
    MissingCapabilities(MissingHttpCapabilities),
}

Equivalent naming is acceptable, but the three failure categories must remain distinguishable.

Implement:

std::fmt::Display
std::error::Error

Do not return String.

Do not combine structural JSON errors with compatibility errors. JSON decoding continues to return RuntimeInformationDecodingError.

Compatibility API

Provide:

impl RuntimeInformationV1 {
    pub const fn validate_compatibility(
        &self,
        expected_identity: RuntimeIdentity,
    ) -> Result<(), RuntimeCompatibilityError>;
}

If current compiler limitations prevent the complete function from being const, keep all underlying comparisons const-friendly and document exactly why the public validator cannot be const. Do not redesign the types merely to force const evaluation.

Validation order

Perform checks in this deterministic order.

1. Runtime identity

Compare:

self.identity()

against:

expected_identity

The complete identity must match, including:

* source identity;
* protocol;
* operation;
* source contract version.

A mismatch returns:

RuntimeCompatibilityError::IdentityMismatch

Do not accept aliases, partial matches, or “close enough” identities.

2. Descriptor contract version

After identity equality succeeds, require:

self.descriptor_contract_version()
    == self.identity().source_contract_version()

A mismatch returns:

RuntimeCompatibilityError::DescriptorContractVersionMismatch

This is where the independently preserved identity and descriptor versions are first required to agree.

JSON decoding must continue accepting them independently.

3. Capabilities

Call the existing capability validation.

Missing capabilities return:

RuntimeCompatibilityError::MissingCapabilities(...)

The error must preserve the complete missing capability set.

Successful result

Return:

Ok(())

only when all three checks succeed.

The validator must not:

* invoke acquire;
* invoke resume;
* create an acquisition context;
* perform HTTP;
* inspect files;
* inspect Cargo metadata;
* mutate the information object;
* add or infer available capabilities.

Error access

Callers must be able to inspect the typed values carried by every error variant.

Do not reduce errors to formatted messages before returning them.

The Display implementation should produce concise diagnostics suitable for later framework wrapping, but no printing should occur inside Core.

Probe behavior remains unchanged

The existing runtime-information probe must still report structurally valid information even when that information is incompatible.

For example, the probe may successfully emit:

identity source contract version: 1
descriptor contract version: 1
required capabilities: client-certificate-v1
available capabilities: empty

Then:

RuntimeInformationV1::from_json(...)

succeeds, while:

validate_compatibility(...)

returns MissingCapabilities.

The probe itself must not call the new compatibility validator.

Required tests

Add focused tests proving:

1. Matching identity, contract version, and capabilities return Ok(()).
2. A different source identity returns IdentityMismatch.
3. A different source contract version in the expected identity returns IdentityMismatch.
4. The error preserves both expected and actual identities.
5. A descriptor contract version differing from the reported identity version returns DescriptorContractVersionMismatch.
6. The version-mismatch error preserves both version values.
7. A missing required capability returns MissingCapabilities.
8. The missing-capability error preserves the complete missing set.
9. Extra available capabilities do not cause rejection.
10. No required capabilities with an empty available set succeeds.
11. Validation order is identity before descriptor version.
12. Validation order is descriptor version before capabilities.
13. Structurally valid incompatible JSON still decodes successfully.
14. Compatibility validation after decoding returns the expected typed error.
15. The runtime-information probe can emit incompatible information successfully.
16. The probe does not perform compatibility validation.
17. Compatibility validation does not invoke acquire.
18. Compatibility validation does not invoke resume.
19. Compatibility validation does not mutate runtime information.
20. Existing runtime-information, capability, and probe tests continue to pass.

If the current enums only contain one protocol or operation variant, do not add fake variants solely to test mismatch behavior.

Public exports

Expose the compatibility error through:

lexicon_core::runtime::RuntimeCompatibilityError

Re-export it through the HTTP namespace only if that matches the established RuntimeInformationV1 re-export convention:

lexicon_core::http::RuntimeCompatibilityError

Both paths must refer to the same canonical type.

Preserve existing external behavior

Do not change:

* source scaffolding;
* source implementation crates;
* source create;
* source build;
* Cargo invocation;
* runtime publication;
* CLI behavior;
* probe argument or probe JSON;
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

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If the known MZA dependency remains unavailable, report it separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* parent-side subprocess probing;
* probe timeout enforcement;
* Cargo build integration;
* build-time artifact verification;
* generated runners;
* runner::run;
* runner main.rs;
* source workspace migration;
* invocation envelopes;
* acquisition or resume execution;
* child runtime admission;
* runtime.json;
* executable hashing;
* publication changes;
* HTTP transport;
* raw recording;
* sessions;
* supervision;
* __operator-host;
* processing runtime compatibility.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* the exact compatibility API;
* the compatibility error representation;
* validation order;
* successful compatibility results;
* each typed incompatibility result;
* proof that structural decoding remains separate from compatibility validation;
* proof that the probe still reports incompatible information;
* proof that handlers were not invoked;
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not add parent-side probing or managed runners.