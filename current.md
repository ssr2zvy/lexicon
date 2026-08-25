Current implementation request: add the mandatory HTTP source descriptor contract

Objective

Implement the first compile-time source-contract slice in lexicon-core.

Add:

HttpSourceContractV1

Its constructor must accept exactly one mandatory acquisition function with this signature:

fn(
    &mut HttpAcquisitionContext,
    &[OsString],
) -> AcquisitionResult<()>

This task defines and tests the typed descriptor only.

Do not change source scaffolding, source builds, executable entrypoints, or runtime behavior yet.

Required public API

The following imports must compile:

use lexicon_core::http::{
    AcquisitionError,
    AcquisitionResult,
    HttpAcquisitionContext,
    HttpSourceContractV1,
};

A valid source contract must compile:

use std::ffi::OsString;
use lexicon_core::http::{
    AcquisitionResult,
    HttpAcquisitionContext,
    HttpSourceContractV1,
};
pub const SOURCE: HttpSourceContractV1 =
    HttpSourceContractV1::new(acquire);
pub fn acquire(
    context: &mut HttpAcquisitionContext,
    args: &[OsString],
) -> AcquisitionResult<()> {
    let _ = context;
    let _ = args;
    Ok(())
}

Required module structure

Introduce the HTTP protocol module under Core:

lexicon-core/
└── src/
    ├── lib.rs
    └── protocols/
        ├── mod.rs
        └── http/
            ├── mod.rs
            ├── contract.rs
            └── error.rs

Expose it at:

lexicon_core::http

For example, lib.rs may contain:

pub mod protocols;
pub use protocols::http;

Do not create empty placeholder modules for unrelated future functionality.

Acquisition result and error

Define:

pub type AcquisitionResult<T> =
    Result<T, AcquisitionError>;

Provide a minimal owned AcquisitionError suitable for source failures.

It must:

* implement Debug;
* implement Display;
* implement std::error::Error;
* preserve a human-readable error message;
* provide a straightforward constructor from a message.

Do not design the complete future error taxonomy in this task.

A representative API is:

#[derive(Debug)]
pub struct AcquisitionError {
    message: String,
}
impl AcquisitionError {
    pub fn source_message(
        message: impl Into<String>,
    ) -> Self {
        // ...
    }
}

Do not add HTTP status, transport, recording, session, retry, or capability variants yet.

Descriptor representation

Define the mandatory handler type:

pub type HttpAcquireFn = fn(
    &mut HttpAcquisitionContext,
    &[OsString],
) -> AcquisitionResult<()>;

Define the versioned descriptor:

#[derive(Clone, Copy)]
pub struct HttpSourceContractV1 {
    acquire: HttpAcquireFn,
}

Its constructor must be:

impl HttpSourceContractV1 {
    pub const fn new(
        acquire: HttpAcquireFn,
    ) -> Self {
        // ...
    }
}

Requirements:

* the handler field must not be publicly writable;
* callers must construct the descriptor through new;
* new must be usable in a public constant;
* the descriptor must contain a real typed function pointer;
* do not erase the handler behind dyn Any;
* do not use a string function name;
* do not use a macro-generated registry;
* do not serialize the handler;
* do not introduce a dynamic plugin ABI.

The descriptor may expose a narrowly scoped handler accessor or invocation method if needed for direct testing. Do not implement the managed runner yet.

Compile-time guarantees

Rust compilation must reject descriptor construction when the handler is:

* missing;
* asynchronous;
* given HttpAcquisitionContext by value;
* given an immutable context reference;
* missing the source-argument slice;
* given the wrong argument type;
* given parameters in the wrong order;
* returning bool;
* returning Result<(), String>;
* returning any type other than AcquisitionResult<()>.

Rust does not enforce parameter variable names. These are equivalent:

pub fn acquire(
    context: &mut HttpAcquisitionContext,
    args: &[OsString],
) -> AcquisitionResult<()>
pub fn acquire(
    first: &mut HttpAcquisitionContext,
    second: &[OsString],
) -> AcquisitionResult<()>

Do not claim otherwise.

A handler does not technically have to be public when a public SOURCE descriptor contains its function pointer. Do not add a false compile-fail test claiming Rust rejects a private handler. Generated source templates may still make acquire public later as a Lexicon convention.

Tests

Add positive tests proving:

* a correctly typed function constructs HttpSourceContractV1;
* the descriptor can be declared as pub const SOURCE;
* the descriptor retains the supplied handler;
* invoking the retained handler receives the same mutable context and native argument slice;
* AcquisitionError preserves and displays its message.

Add compile-fail coverage proving rejection of at least:

1. no handler argument;
2. async handler;
3. context by value;
4. immutable context;
5. missing &[OsString];
6. wrong source-argument type;
7. reversed parameters;
8. bool return;
9. Result<(), String> return.

Use Rust compile-fail documentation tests or another focused compile-test mechanism. Do not create tests that merely compare source text or search for type names.

Preserve the historical API temporarily

The existing API must continue compiling and behaving as before:

lexicon_core::{
    HttpAcquisition,
    HttpAcquisitionContext,
    run_http_source,
}

It is acceptable to move its implementation internally and re-export it from the root, but do not change its behavior or signatures in this task.

The new path must also expose the context:

lexicon_core::http::HttpAcquisitionContext

Do not remove the historical trait until generated sources have migrated to the new descriptor and managed runner.

No source-template migration

Do not change the currently generated source crates.

They must continue using the historical compatibility contract during this step.

Do not modify:

get-raw-data-impl/src/main.rs
process-data-impl/src/main.rs
source create
source build

Do not generate SOURCE yet. This task only ensures that Core possesses and tests the descriptor type required by the following migration.

Bundle and installation preservation

Preserve:

external pinned MZA
→ lexicon_cli artifact
→ cargo-bundler-v0.1.0
→ lexicon-bundle installer
→ installed lexicon

Do not change:

* mza_artifacts.toml;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-core remains a linked Rust library, not an MZA artifact or installed executable.

Required validation

Run:

cargo test --workspace --quiet

Run the required official validator:

bash automation/build_bundle_install/build_bundle_install.sh

Verify that:

* all positive descriptor tests pass;
* all invalid handler examples fail compilation;
* existing historical Core tests pass;
* existing framework and CLI tests pass;
* lexicon source create still creates the historical scaffold;
* lexicon source build still publishes both existing runtimes;
* the Protocol 1 installer still succeeds;
* the installed payload remains only lexicon.

Explicit exclusions

Do not implement:

* optional handlers;
* with_resume;
* HttpCapability;
* requires;
* capability lists;
* implementation-library scaffolding;
* acquisition workspaces;
* lexicon-runner;
* managed main.rs;
* runtime identity;
* runtime-information probes;
* validated build states;
* runtime.json;
* invocation envelopes;
* context.execute;
* HTTP transport;
* raw transaction recording;
* session changes;
* supervision;
* __operator-host;
* acquisition execution;
* processing-contract changes.

These belong to later micro-steps.

Completion report

After completion, replace current.md with a focused report containing:

* files created and changed;
* the exact public API;
* the descriptor’s internal representation;
* the exact mandatory handler type;
* positive test results;
* every compile-fail case and its result;
* confirmation that public SOURCE works in a constant;
* confirmation that private handler visibility was not falsely treated as a type requirement;
* historical API compatibility results;
* workspace test results;
* bundle/install validation;
* any remaining blocker.

Then stop. Do not migrate source scaffolding or generate a managed runner.