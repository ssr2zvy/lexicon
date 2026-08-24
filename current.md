Current implementation request: replace CLI-to-framework IPC with an in-process framework library

Objective

Implement the first architectural migration required by workspace/specs/contract.md and workspace/specs/specs.md:

current control flow
installed lexicon CLI
→ separately installed lexicon-framework executable
required control flow
installed lexicon executable
→ statically linked lexicon-framework library

After this task, lexicon must be the only installed Lexicon control executable.

The lexicon-bundle package remains a separate installer executable. MZA continues using cargo-bundler-v0.1.0 to compile lexicon-bundle into the final target-specific installer with the archived lexicon CLI embedded inside it.

The distinction is:

release artifact
lexicon-bundle installer executable
→ installs
lexicon control executable
removed
lexicon-framework executable

Existing init, source create, and source build behavior must continue working through direct Rust library calls.

This task establishes the control-plane boundary only. Do not yet implement the new source descriptor, Lexicon-managed source runners, runtime admission, HTTP recording, or operator-host supervision.

1. Convert lexicon-framework into a library-only package

lexicon-framework must stop producing an executable.

Required shape:

lexicon-framework/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── commands/
    ├── scaffold/
    ├── build/
    └── publication/

Requirements:

* Remove the lexicon-framework binary target.
* Remove lexicon-framework/src/main.rs after its behavior has been migrated.
* Expose framework operations through library APIs.
* Move command parsing out of the framework.
* The framework must receive typed inputs rather than process argument strings.
* The framework library must not call std::process::exit.
* The framework library must not print user-facing success or error messages directly.
* Framework functions must return typed success values or typed errors for the CLI to render.
* Cargo diagnostics needed by source build must be returned as structured error information or emitted through a typed reporting interface.

Representative API boundaries may be:

lexicon_framework::commands::init(...)
lexicon_framework::commands::source_create(...)
lexicon_framework::commands::source_build(...)

The exact internal types may be refined, but command semantics must remain in lexicon-framework.

2. Route the CLI directly into the framework library

Add lexicon-framework as a normal dependency of lexicon-cli.

The route must become:

lexicon-cli/src/main.rs
→ CLI parsing
→ direct lexicon_framework function call
→ typed result
→ CLI rendering

Remove all machinery associated with locating or launching a framework executable, including:

* the public --framework-path option;
* LEXICON_FRAMEWORK_PATH;
* the remembered framework-path state file;
* framework_binary_path;
* read_framework_path;
* write_framework_path;
* FRAMEWORK_FROM_CLI;
* Command::new(framework_path);
* tests that require or resolve a framework executable.

The CLI must not spawn itself or another executable for ordinary foreground framework commands.

3. Move project initialization semantics into the framework

The operational implementation of lexicon init currently resides under lexicon-cli.

Move project creation, validation, nested-project rejection, TOML writing, staging, and atomic finalization into lexicon-framework.

The CLI may retain the Clap argument definitions, but it must call the framework library to perform the operation.

The CLI remains responsible for rendering:

[lexicon] Initialized project '<project-name>' at <absolute-path>

The framework must return the typed information required to render that line.

4. Preserve existing source behavior

The following commands must continue to work:

lexicon init
lexicon source create
lexicon source build

Preserve the existing behavior of:

* project discovery;
* source-name and protocol validation;
* source scaffolding;
* locked native Cargo builds;
* isolated temporary target directories;
* Cargo JSON executable selection;
* randomized same-filesystem staging;
* paired runtime publication;
* rollback;
* existing runtime preservation after failure;
* existing success and error wording unless the new CLI/framework division requires a strictly mechanical rendering change.

source create and source build must now execute framework logic in the original lexicon process.

Do not redesign the generated source crates in this task. The existing trait and source-owned executable scaffold may remain temporarily so this migration does not combine two architectural boundaries.

5. Preserve the lexicon-bundle installer layer

lexicon-bundle remains a binary crate and remains the Lexicon-specific installation layer.

It continues to own:

* extraction of the embedded CLI archive;
* installation-path selection;
* installation of the lexicon executable;
* PATH integration;
* installation records;
* upgrades;
* uninstall behavior;
* platform-specific installation behavior.

It must not be converted into a library.

The package remains conceptually:

lexicon-bundle/
├── Cargo.toml
├── build.rs
└── src/
    └── main.rs

MZA continues to build it using:

cargo-bundler-v0.1.0

For each target, MZA must still:

1. Build the ordinary lexicon_cli artifact for that target.
2. Archive the CLI artifact as .tar.xz.
3. Write the target-specific bundle-spec.toml.
4. Set MZA_BUNDLE_INPUTS to that specification.
5. Cross-compile the lexicon-bundle crate for the target.
6. Allow lexicon-bundle/build.rs to copy and embed the CLI archive through $OUT_DIR.
7. Produce the target-specific lexicon-bundle installer executable.
8. Archive that installer executable as the final bundle artifact.

The resulting relationship is:

MZA
→ builds lexicon_cli archive
→ supplies archive to cargo-bundler-v0.1.0
→ compiles lexicon-bundle installer
→ installer contains lexicon_cli archive
→ installer installs lexicon

6. Update mza_artifacts.toml without removing the bundle

Update:

automation/build_bundle_mza/mza_artifacts.toml

The existing separate ordinary framework artifact must be removed.

Conceptually, change the ordinary artifacts from:

lexicon_cli
lexicon_framework

to:

lexicon_cli

The Lexicon bundle declaration must remain.

Its implementation crate must remain lexicon-bundle, its protocol must remain cargo-bundler-v0.1.0, and its input artifact labels must contain only:

lexicon_cli

The resulting conceptual configuration is:

[[artifacts]]
label = "lexicon_cli"
crate = "lexicon-cli"
type = "executable"
[[bundles]]
label = "lexicon"
crate = "lexicon-bundle"
protocol = "cargo-bundler-v0.1.0"
artifact_labels = ["lexicon_cli"]
type = "installer"

Use the repository’s actual MZA schema and field names rather than copying this conceptual example blindly.

Do not:

* delete the Lexicon bundle declaration;
* replace Protocol 1;
* invoke the installer on the build host;
* convert lexicon-bundle into an ordinary artifact;
* embed build-host paths in the installer;
* bypass MZA_BUNDLE_INPUTS;
* add Lexicon-specific installation policy to MZA.

7. Update the installation contract

Update lexicon-install.toml so the installed payload contains:

lexicon

and does not contain:

lexicon-framework

Remove:

* the framework artifact label;
* Linux framework installation paths;
* Windows framework installation paths;
* framework entries in installation records;
* framework upgrade logic;
* framework uninstall logic.

Keep:

* the CLI artifact label;
* Linux and Windows CLI installation paths;
* installation records;
* PATH integration;
* upgrade behavior;
* uninstall behavior;
* every installation rule still required by lexicon-bundle.

The installer executable itself remains the delivery artifact. “One installed control executable” does not mean that the installer binary ceases to exist.

8. Update build, bundle, and install automation

Reconcile at least:

automation/build_bundle_mza/mza_artifacts.toml
automation/build_bundle_install/update_lock_file.sh
automation/build_bundle_install/
lexicon-install.toml
lexicon-bundle/

Requirements:

* MZA builds the lexicon_cli ordinary artifact.
* MZA no longer builds a lexicon_framework ordinary artifact.
* The Protocol 1 bundle consumes exactly the target-matching lexicon_cli archive.
* MZA still compiles lexicon-bundle into the target installer.
* The final installer still installs, upgrades, and uninstalls lexicon.
* Lockfile updating remains correct for the root workspace and lexicon-bundle.
* No automation expects a framework executable or framework archive.

Do not replace the removed framework executable with a wrapper, symlink, copied CLI, or compatibility executable.

9. Required command verification

Run the required workflow:

bash automation/build_bundle_install/build_bundle_install.sh

The workflow must still prove the complete route:

build lexicon_cli
→ archive lexicon_cli
→ run MZA cargo-bundler-v0.1.0
→ compile lexicon-bundle installer
→ archive installer
→ extract installer
→ execute installer
→ install lexicon

Using the installed result, verify without --framework-path or LEXICON_FRAMEWORK_PATH:

lexicon --version
lexicon --help
lexicon init . telugu-lexicon
cd telugu-lexicon
lexicon source create example-source --protocol http
lexicon source build example-source --protocol http

Verify that:

* lexicon --help does not expose --framework-path;
* project initialization succeeds;
* source creation succeeds;
* source building succeeds;
* both acquisition and processing runtimes are published;
* the Protocol 1 lexicon-bundle installer is still produced;
* the installer embeds the target-matching lexicon_cli archive;
* the bundle contains no lexicon_framework input archive;
* the installed payload contains lexicon;
* the installed payload contains no lexicon-framework executable;
* no framework entry appears in the installation record;
* no framework-path state file is created;
* unsupported protocols still produce one neutral [lexicon] ERROR: line;
* existing rollback and runtime-preservation tests still pass.

10. Required tests

Add or update tests proving:

* CLI commands call framework library functions directly;
* framework command functions return typed results;
* framework failures return errors rather than exiting the process;
* the CLI renders framework errors exactly once;
* init filesystem semantics are owned by the framework package;
* source create and source build require no framework executable;
* mza_artifacts.toml retains the Lexicon Protocol 1 bundle;
* the bundle has lexicon_cli as its only ordinary artifact input;
* lexicon-bundle remains a binary target;
* the generated installer contains the CLI archive;
* install, upgrade, and uninstall operate on lexicon;
* install, upgrade, and uninstall do not reference a framework executable;
* existing source-build staging and transactional-publication coverage remains intact.

11. Explicit exclusions

Do not implement in this task:

* relocation or renaming of lexicon-framework/core;
* HttpSourceContractV1;
* source implementation lib.rs conversion;
* generated lexicon-runner;
* capability descriptors;
* opaque build-state types;
* runtime.json;
* runtime-information probing;
* parent/child runtime admission;
* context.execute;
* raw HTTP transaction recording;
* foreground runtime supervision;
* __operator-host;
* data --get or data --process execution;
* processing-contract redesign;
* a new MZA bundling protocol;
* removal or replacement of lexicon-bundle.

Those source-runtime changes belong to the next implementation request, where Core relocation, the typed descriptor, the implementation-library workspace, and the Lexicon-managed runner can be introduced together.

Completion report

After implementation, replace current.md with a focused report containing:

* the package and dependency changes;
* the removed framework executable and IPC paths;
* the new direct command routes;
* the framework result and error types;
* the changes to mza_artifacts.toml;
* confirmation that lexicon-bundle remains a binary installer;
* confirmation that cargo-bundler-v0.1.0 remains the active protocol;
* the exact Protocol 1 input artifact list;
* the installation-layout changes;
* the exact validation commands and results;
* evidence that init, source create, and source build work through the installed lexicon executable;
* evidence that the installer is still produced;
* evidence that no separate framework executable is built, bundled, installed, or recorded;
* any remaining blocker.

Then stop. Do not continue into the managed-runner or Core-contract migration.