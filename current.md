# Implementation report

Implemented the compiled HTTP acquisition runtime identity without changing the existing HTTP descriptor behavior.

What changed
- Added the new runtime module at `lexicon-core/src/runtime/mod.rs` and `lexicon-core/src/runtime/identity.rs`.
- Defined the canonical `RuntimeProtocol`, `RuntimeOperation`, and `RuntimeIdentity` types with private fields, const constructors, and const accessors.
- Kept the design zero-allocation and const-friendly so it remains suitable for generated runner `main.rs` code.
- Re-exported the canonical type through both `lexicon_core::runtime::RuntimeIdentity` and `lexicon_core::http::RuntimeIdentity` so both paths point to the same type.
- Added focused tests covering const construction, accessors, identifier values, equality semantics, and the re-export equivalence.

Validation
- Ran: `cargo test --workspace --quiet`
- Result: all workspace tests passed.

Files touched
- `lexicon-core/src/lib.rs`
- `lexicon-core/src/protocols/http/mod.rs`
- `lexicon-core/src/runtime/mod.rs`
- `lexicon-core/src/runtime/identity.rs`

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