# Implementation report

Implemented the HTTP source descriptor contract in `lexicon-core` and preserved the compatibility API for the existing source runner.

## Summary

The merged change adds the first typed HTTP acquisition contract under Core without changing the current source scaffolding or runtime execution model yet.

It introduces:

- `lexicon_core::http` as the new public protocol namespace
- `AcquisitionResult<T>` and a minimal `AcquisitionError` implementation
- `HttpAcquireFn` and `HttpSourceContractV1` as a typed, compile-time-safe descriptor
- a `const`-friendly constructor that stores a real function pointer instead of a dynamic registry
- compatibility re-exports for the historical root API:
  - `lexicon_core::HttpAcquisition`
  - `lexicon_core::HttpAcquisitionContext`
  - `lexicon_core::run_http_source`

## Files added or updated

- `lexicon-core/src/lib.rs`
- `lexicon-core/src/protocols/mod.rs`
- `lexicon-core/src/protocols/http/mod.rs`
- `lexicon-core/src/protocols/http/contract.rs`
- `lexicon-core/src/protocols/http/error.rs`
- `lexicon-core/tests/contract_ui.rs`
- `lexicon-core/tests/ui/*.rs`
- `lexicon-core/Cargo.toml`
- `Cargo.lock`

## What the contract does

The new descriptor enforces a typed acquisition function shape:

- `fn(&mut HttpAcquisitionContext, &[OsString]) -> AcquisitionResult<()>`

This ensures compile-time rejection for malformed handlers such as:

- missing handler arguments
- async handlers
- pass-by-value context
- immutable context
- missing `&[OsString]`
- wrong argument type
- reversed parameters
- bool returns
- `Result<(), String>` returns

The descriptor is intentionally a simple typed pointer-based contract; it does not add a dynamic plugin ABI, registry, or serialization layer.

## Validation

This change was validated through the workspace Rust test suite using the standard Cargo path:

- `cargo test --workspace --quiet`

The merged implementation keeps the historical API intact while exposing the new HTTP contract path for future migration work.

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