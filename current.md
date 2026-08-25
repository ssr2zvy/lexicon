Current implementation request: add the compiled HTTP acquisition runtime identity

Objective

Add the immutable identity value that a future Lexicon-managed acquisition runner will compile into its executable:

const IDENTITY: RuntimeIdentity =
    RuntimeIdentity::http_acquisition(
        "example-source",
        1,
    );

This task defines and tests the identity value only.

Do not generate a runner, execute a descriptor, serialize identity, or implement runtime admission yet.

Required module structure

Add:

lexicon-core/src/runtime/
├── mod.rs
└── identity.rs

Expose the canonical type through:

lexicon_core::runtime::RuntimeIdentity

Also re-export it through the HTTP namespace so the future runner import from the contract remains valid:

lexicon_core::http::RuntimeIdentity

Both paths must refer to the same type, not duplicate identity structures.

Protocol and operation types

Define typed protocol and operation identities:

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
)]
#[non_exhaustive]
pub enum RuntimeProtocol {
    Http,
}
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
)]
#[non_exhaustive]
pub enum RuntimeOperation {
    Acquisition,
}

Provide stable identifiers:

impl RuntimeProtocol {
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Http => "http",
        }
    }
}
impl RuntimeOperation {
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Acquisition =>
                "acquisition",
        }
    }
}

Do not use arbitrary protocol or operation strings inside RuntimeIdentity.

Do not add processing yet.

RuntimeIdentity

Define:

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
)]
pub struct RuntimeIdentity {
    source_name: &'static str,
    protocol: RuntimeProtocol,
    operation: RuntimeOperation,
    source_contract_version: u32,
}

All fields must remain private.

Provide the constructor:

impl RuntimeIdentity {
    pub const fn http_acquisition(
        source_name: &'static str,
        source_contract_version: u32,
    ) -> Self {
        // ...
    }
}

It must assign:

source_name             → supplied source name
protocol                → RuntimeProtocol::Http
operation               → RuntimeOperation::Acquisition
source_contract_version → supplied contract version

The second argument in:

RuntimeIdentity::http_acquisition(
    "example-source",
    1,
)

means HTTP source-contract version 1. It is not the Core crate version, runner-template version, runtime-invocation version, project-schema version, or capability version.

Accessors

Provide const accessors:

impl RuntimeIdentity {
    pub const fn source_name(
        &self,
    ) -> &'static str;
    pub const fn protocol(
        &self,
    ) -> RuntimeProtocol;
    pub const fn operation(
        &self,
    ) -> RuntimeOperation;
    pub const fn source_contract_version(
        &self,
    ) -> u32;
}

Do not expose public field mutation.

Constant construction

This must compile:

use lexicon_core::http::RuntimeIdentity;
pub const IDENTITY: RuntimeIdentity =
    RuntimeIdentity::http_acquisition(
        "example-source",
        1,
    );

The value must remain suitable for direct inclusion in a generated runner’s main.rs.

Do not require heap allocation, runtime initialization, lazy statics, a registry, or serialization.

Tests

Add focused tests proving:

1. RuntimeIdentity::http_acquisition(...) works in a pub const.
2. source_name() returns "example-source".
3. protocol() returns RuntimeProtocol::Http.
4. operation() returns RuntimeOperation::Acquisition.
5. source_contract_version() returns 1.
6. RuntimeProtocol::Http.identifier() returns "http".
7. RuntimeOperation::Acquisition.identifier() returns "acquisition".
8. lexicon_core::runtime::RuntimeIdentity and lexicon_core::http::RuntimeIdentity are the same type.
9. Two identities with the same fields compare equal.
10. Different source names or contract versions compare unequal.

Preserve existing descriptor behavior

Do not change:

* HttpSourceContractV1;
* HttpAcquireFn;
* HttpResumeFn;
* .with_resume(...);
* HttpCapability;
* HttpCapabilitySet;
* .requires(...);
* AcquisitionError;
* AcquisitionResult;
* historical HttpAcquisition;
* historical run_http_source.

The new identity is not yet stored inside HttpSourceContractV1. The descriptor defines source behavior; the runner identity defines the artifact Lexicon intended to build.

No validation yet

Do not validate:

* source-name syntax;
* supported contract versions;
* runner-template versions;
* Core versions;
* capabilities;
* project identity;
* session identity;
* invocation envelopes;
* parent/child agreement.

Those checks require later build and runtime-admission layers.

This task only creates the typed compiled identity value that those layers will consume.

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
* installed paths.

Validation

Run:

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet

Run the official validator if the external MZA checkout is available:

bash automation/build_bundle_install/build_bundle_install.sh

If the known missing-MZA blocker remains, report it separately. Do not expand this task into MZA dependency management.

Explicit exclusions

Do not implement:

* a generated runner;
* runner::run;
* identity serialization;
* runtime-information output;
* runtime.json;
* executable hashing;
* parent admission;
* child admission;
* invocation envelopes;
* capability availability;
* descriptor invocation;
* source-library scaffolding;
* processing identity;
* HTTP execution;
* raw recording;
* sessions;
* supervision;
* __operator-host.

Completion report

Replace current.md with a focused report containing:

* files created and changed;
* the exact RuntimeIdentity representation;
* the protocol and operation enums;
* stable identifiers;
* constructor and accessor APIs;
* constant-construction proof;
* equality test results;
* confirmation that descriptor behavior was untouched;
* Core and workspace test results;
* official validation result or the known external-MZA blocker.

Then stop. Do not generate or execute a managed runner.