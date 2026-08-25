Current implementation request: relocate Core into the top-level lexicon-core package

Objective

Perform the next mechanical architecture step:

current
lexicon-framework/core/
└── package: lexicon-framework-core
required
lexicon-core/
└── package: lexicon-core

This task establishes the three principal package boundary:

lexicon-cli
└── lexicon-framework
    └── lexicon-core

Do not redesign the source contract or generated source workspaces yet. This step must relocate and rename the existing Core crate while preserving its current behavior.

Required repository structure

The relevant root structure must become:

Cargo.toml
├── lexicon-cli/
├── lexicon-framework/
├── lexicon-core/
└── lexicon-bundle/

Create:

lexicon-core/
├── Cargo.toml
└── src/
    └── lib.rs

Remove:

lexicon-framework/core/

There must not be two checked-in copies of Core.

Root workspace changes

Update the root Cargo.toml workspace members.

Replace:

"lexicon-framework/core"

with:

"lexicon-core"

The root must remain a workspace-only manifest. Do not add a root package or root src/main.rs.

Update the root Cargo.lock through the repository’s established lockfile workflow.

lexicon-core package identity

The relocated crate must use:

[package]
name = "lexicon-core"
version = "0.1.2"
edition = "2024"
[lib]
path = "src/lib.rs"

Rust code imports the crate as:

lexicon_core

The existing Core implementation must move without semantic redesign. Preserve:

HttpAcquisitionContext
HttpAcquisition
run_http_source

Preserve their current behavior and existing tests.

Do not introduce HttpSourceContractV1, capabilities, runtime identity, HTTP recording, or managed-runner support in this task.

lexicon-framework dependency

Add the top-level Core crate as a direct dependency of lexicon-framework:

[dependencies]
lexicon-core = { path = "../lexicon-core" }

The resulting Cargo dependency graph must show:

lexicon-cli
→ lexicon-framework
→ lexicon-core

lexicon-framework remains a library-only package. Do not restore its deleted executable target.

It is acceptable that the framework does not yet exercise the complete future Core API. This task establishes the correct package dependency before that API is expanded.

Temporary legacy source-template compatibility

The currently generated source crates still use the previous source-owned executable contract and fetch:

lexicon-framework-core
tag v0.1.2

Do not migrate those generated templates in this task.

Specifically, do not yet change:

* generated implementation crates from main.rs to lib.rs;
* their existing lexicon-framework-core Git dependency;
* their existing trait implementation;
* their existing source-owned entrypoint;
* the current source build target selection.

Those references intentionally remain as temporary compatibility with the already-released v0.1.2 Core contract.

They will be removed in the next source-runtime migration, where all of the following must change together:

source implementation library
+ lexicon-core dependency
+ typed source descriptor
+ Lexicon-managed runner
+ supported build target

Do not leave generated sources partially converted between the old and new contracts.

Command behavior that must remain unchanged

The following commands must continue working:

lexicon init . telugu-lexicon
lexicon source create example-source --protocol http
lexicon source build example-source --protocol http

Preserve:

* in-process CLI-to-framework routing;
* current project initialization;
* current source scaffolding;
* current locked source builds;
* Cargo JSON artifact selection;
* isolated temporary target directories;
* randomized runtime staging;
* paired acquisition and processing publication;
* rollback and runtime preservation;
* existing CLI output and error behavior.

Bundle and installer behavior

Do not change the established bundle architecture.

The release route remains:

MZA
→ builds lexicon_cli
→ cargo-bundler-v0.1.0
→ compiles lexicon-bundle installer
→ installer installs lexicon

Requirements:

* lexicon-bundle remains a binary installer crate.
* cargo-bundler-v0.1.0 remains the active protocol.
* mza_artifacts.toml continues to use lexicon_cli as the bundle’s only ordinary artifact input.
* No lexicon_framework artifact may return.
* lexicon-core is a linked library, not an ordinary MZA artifact or installed executable.
* The installed payload remains the single lexicon control executable.

Only update bundle or lockfile automation where required by the Core package relocation. Do not redesign installation behavior.

Required tests

Update tests and package references so the workspace reports the Core package as:

lexicon-core

Tests must prove:

* lexicon-core is a top-level workspace member;
* lexicon-framework/core no longer exists;
* the package name is lexicon-core;
* the Rust crate name is lexicon_core;
* lexicon-framework directly depends on lexicon-core;
* existing Core behavior and tests remain unchanged;
* lexicon-framework remains library-only;
* lexicon remains the only installed control executable;
* current source creation and source building still work;
* the Protocol 1 bundle and installer remain functional.

Required validation

Run the required workflow:

bash automation/build_bundle_install/build_bundle_install.sh

Using the installed CLI, execute:

lexicon --version
lexicon init . telugu-lexicon
cd telugu-lexicon
lexicon source create example-source --protocol http
lexicon source build example-source --protocol http

Verify that both current runtimes are still published:

sources/example-source/http/get-raw-data/runtime/example-source-get-raw-data
sources/example-source/http/process-data/runtime/example-source-process-data

Also inspect Cargo metadata and verify:

lexicon-cli
→ lexicon-framework
→ lexicon-core

Explicit exclusions

Do not implement:

* HttpSourceContractV1;
* capability declarations;
* optional handlers;
* source implementation lib.rs;
* acquisition workspace manifests;
* generated lexicon-runner;
* processing managed runners;
* runtime identity;
* runtime.json;
* runtime-information probes;
* opaque validated build states;
* context.execute;
* raw HTTP recording;
* session redesign;
* foreground runtime supervision;
* __operator-host;
* data --get;
* data --process;
* a new MZA protocol;
* changes to the Lexicon installer architecture.

Completion report

After completion, replace current.md with a focused implementation report containing:

* the old and new Core locations;
* the old and new Cargo package names;
* root workspace changes;
* the new framework-to-Core dependency;
* files moved, created, deleted, and updated;
* confirmation that Core behavior was not redesigned;
* the exact remaining legacy lexicon-framework-core template references and why they remain temporarily;
* test results;
* Cargo dependency-graph evidence;
* installed CLI command results;
* source-create and source-build results;
* bundle/install validation;
* any remaining blocker.

Then stop. Do not begin the typed descriptor or managed-runner migration.