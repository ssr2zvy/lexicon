# Implementation report

Implemented the typed processing source contract in `lexicon-core`.

## What changed
- Added the new `lexicon_core::processing` module and exported it from the crate root.
- Created the core processing contract types:
  - `ProcessingContext` with private representation and a crate-private test constructor
  - `ProcessingError` and `ProcessingResult<T>` with minimal `Display` / `Error` support
  - `ProcessDataFn = fn(&mut ProcessingContext, &[OsString]) -> ProcessingResult<()>`
  - `ProcessingSourceContractV1` with `CONTRACT_VERSION`, `new(...)`, and `process_handler()`
- Kept the descriptor allocation-free, const-constructible, and free of dynamic registries or capability metadata.
- Added compile-fail UI coverage covering invalid handler signatures and a public-constant/private-handler positive case.
- Added positive behavior tests proving the descriptor is copyable, callable, context-aware, const-friendly, and safe to construct/copy without invoking the handler.
- Added Unix non-UTF-8 argument coverage using the native `OsString` path.

## Validation
- Ran `cargo test --workspace --quiet`
- Result: all workspace tests passed.
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