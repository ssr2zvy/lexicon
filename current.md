Current implementation request: typed processing source descriptor

Objective

Add the first compile-time processing source contract to lexicon-core.

A processing implementation library must export a typed descriptor containing one mandatory processing function.

This step defines types only. Do not add processing execution, SQLite operations, runners, probes, manifests, staging, or publication.

Required public namespace

Expose the new API through:

lexicon_core::processing

Use the appropriate existing or new files under:

lexicon-core/src/processing/
├── mod.rs
├── contract.rs
├── context.rs
└── error.rs

Equivalent internal file organization is acceptable.

Processing result and error

Define:

pub type ProcessingResult<T> =
    Result<T, ProcessingError>;

Add a minimal typed error:

#[derive(Debug)]
pub struct ProcessingError {
    // Private representation.
}

Provide only the minimal construction and trait support needed for the descriptor tests:

std::fmt::Display
std::error::Error

Do not design detailed SQLite, session, checkpoint, parsing, or filesystem error categories yet.

Do not use:

Result<T, String>

as the processing contract.

Processing context

Define:

pub struct ProcessingContext {
    // Private fields.
}

The context must be a Core-owned type with private representation.

Provide the smallest test construction mechanism needed inside lexicon-core, such as a crate-private or test-only constructor.

Do not expose a public empty constructor implying that source code controls context creation.

Do not add paths, database handles, sessions, transaction readers, or SQLite behavior yet.

Mandatory processing function type

Define the exact function-pointer type:

pub type ProcessDataFn = fn(
    context: &mut ProcessingContext,
    args: &[OsString],
) -> ProcessingResult<()>;

The contract is synchronous ordinary Rust.

Do not use:

* async fn;
* boxed futures;
* dynamic trait objects;
* a serialized workflow;
* callbacks for individual raw transactions;
* a plugin ABI.

Versioned descriptor

Define:

#[derive(Clone, Copy)]
pub struct ProcessingSourceContractV1 {
    process: ProcessDataFn,
}

The field must remain private.

Provide:

impl ProcessingSourceContractV1 {
    pub const CONTRACT_VERSION: u32 = 1;
    pub const fn new(
        process: ProcessDataFn,
    ) -> Self;
    pub const fn process_handler(
        &self,
    ) -> ProcessDataFn;
}

The descriptor must:

* store a real typed function pointer;
* be allocation-free;
* support construction in a pub const;
* contain no dynamic registry;
* contain no serialization;
* contain no source instance or trait object.

Required source shape

This must compile:

use std::ffi::OsString;
use lexicon_core::processing::{
    ProcessingContext,
    ProcessingResult,
    ProcessingSourceContractV1,
};
pub const SOURCE: ProcessingSourceContractV1 =
    ProcessingSourceContractV1::new(process);
pub fn process(
    context: &mut ProcessingContext,
    args: &[OsString],
) -> ProcessingResult<()> {
    let _ = context;
    let _ = args;
    Ok(())
}

The symbol name SOURCE is a managed-runner convention to be used later.

Rust type enforcement comes from the declared descriptor type and new(...) parameter.

Required compile-time enforcement

Add UI compile-fail coverage proving rejection of processing handlers with:

1. no parameters;
2. missing args;
3. context passed by value;
4. immutable context reference;
5. wrong context type;
6. wrong argument type;
7. mutable argument slice;
8. reversed parameters;
9. bool return;
10. Result<(), String> return;
11. async function;
12. closure requiring captured state;
13. function returning ProcessingResult<bool>.

The failures must occur when the malformed function is passed to:

ProcessingSourceContractV1::new(...)

Use the project’s existing compile-fail test approach.

Visibility semantics

Do not falsely claim Rust requires the handler itself to be public.

This is valid inside one implementation crate:

pub const SOURCE: ProcessingSourceContractV1 =
    ProcessingSourceContractV1::new(process);
fn process(
    context: &mut ProcessingContext,
    args: &[OsString],
) -> ProcessingResult<()> {
    Ok(())
}

The managed runner accesses the public descriptor, not the function symbol directly.

Add a positive test proving a public constant may contain a private handler.

Descriptor behavior tests

Add tests proving:

1. CONTRACT_VERSION == 1.
2. A valid processing function constructs the descriptor.
3. The descriptor works in a constant.
4. The descriptor is Copy.
5. process_handler() returns the stored function.
6. The retained function pointer is callable.
7. The handler receives mutable ProcessingContext.
8. The handler receives &[OsString].
9. Nonempty native arguments reach the handler unchanged.
10. Constructing the descriptor does not invoke the handler.
11. Copying the descriptor does not invoke the handler.
12. A private handler works behind a public descriptor constant.
13. Acquisition descriptor behavior remains unchanged.
14. Processing runtime identity behavior remains unchanged.
15. All workspace tests pass.

Add non-UTF-8 argument coverage on Unix if it can reuse the existing native-argument test pattern without introducing runtime execution.

No acquisition capability reuse

Do not add HttpCapabilitySet to the processing descriptor.

Do not add:

* .requires(...);
* .with_resume(...);
* processing capabilities;
* optional handlers.

Those require demonstrated processing requirements and belong to later steps.

No descriptor/runtime-information connection yet

Do not add:

RuntimeInformationV1::from_processing_source(...)

in this step.

Do not fix the existing lack of a type-level operation guard in from_http_source(...) yet.

The next processing micro-step can define processing runtime information and the correct operation-specific construction path.

Preserve existing behavior

Do not change:

* HTTP acquisition descriptor behavior;
* acquisition capabilities;
* acquisition resume handler;
* runtime identity JSON;
* Core probe behavior;
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
* existing publication;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-framework remains library-only.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-core --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If it remains unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* processing runtime-information construction;
* processing runtime probe;
* processing compatibility specialization;
* processing verification;
* processing manifest construction;
* processing staging;
* processing bundle admission;
* paired publication;
* processing runner;
* processing main.rs;
* processing execution;
* raw-transaction discovery;
* SQLite creation;
* processing sessions;
* checkpoints;
* source workspace migration;
* managed acquisition runners;
* runner::run;
* invocation envelopes;
* HTTP execution;
* raw recording;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* exact public processing API;
* descriptor representation;
* exact mandatory function type;
* contract-version constant;
* constant-construction proof;
* positive descriptor tests;
* every compile-fail case and result;
* confirmation that a private handler works behind public SOURCE;
* confirmation that construction does not invoke the handler;
* acquisition compatibility results;
* Core and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not connect the descriptor to runtime information or generate a processing runner.