# Lexicon Architecture and Build Specification

Status: current, drift-reconciled baseline, 2026-08-15

## 1. Purpose

Lexicon is a generic framework for acquiring raw data from independently implemented sources and processing that raw data into source-specific SQLite datasets.

Lexicon Core does not define what the acquired data represents. Each source implementation determines:

- How its data is acquired.
- Which source-specific arguments it accepts.
- How its raw responses are interpreted.
- How its processed SQLite dataset is structured.

The framework supplies shared contracts, execution behavior, session handling, source discovery, and the global `lexicon` command.

The repository contains source code only. No compiled binaries, `.exe` files, generated archives, installers, or other build artifacts are committed.

## 2. Repository Structure

```text
lexicon/
├── Cargo.toml                  (workspace-only manifest: [workspace] + [workspace.package], no [package])
├── Cargo.lock
│
├── lexicon-bundle/             (Protocol 1 bundle crate; workspace member, inherits version/edition)
│   ├── Cargo.toml
│   ├── build.rs
│   └── src/
│       └── main.rs
│
├── lexicon-framework/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   │
│   ├── core/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── get_raw_data/
│   │       │   ├── mod.rs
│   │       │   ├── api.rs
│   │       │   ├── facade.rs
│   │       │   └── run.rs
│   │       ├── process_data/
│   │       │   ├── mod.rs
│   │       │   ├── api.rs
│   │       │   ├── facade.rs
│   │       │   └── run.rs
│   │       └── session_engine/
│   │           ├── mod.rs
│   │           └── session_engine.rs
│   │
│   └── sources/
│       └── example-source/
│           ├── data/
│           │   ├── raw/
│           │   │   └── <request-timestamp>-<id>/
│           │   │       ├── request/
│           │   │       │   ├── metadata.json
│           │   │       │   └── body
│           │   │       └── response/
│           │   │           ├── metadata.json
│           │   │           └── body
│           │   └── processed/
│           │       └── example-source.sqlite
│           │
│           ├── get-raw-data/
│           │   ├── sessions/
│           │   ├── session_status.json
│           │   └── get_raw_data_impl/
│           │       ├── Cargo.toml
│           │       ├── Cargo.lock
│           │       └── src/
│           │           └── main.rs
│           │
│           └── process-data/
│               ├── sessions/
│               ├── session_status.json
│               └── process_data_impl/
│                   ├── Cargo.toml
│                   ├── Cargo.lock
│                   ├── src/
│                   │   └── main.rs
│                   └── processing/
│                       └── ...
│
├── lexicon-cli/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   └── src/
│       └── main.rs
│
├── automation/
│   └── mza/
│       ├── Cargo.toml
│       ├── Cargo.lock
│       ├── artifacts.toml
│       ├── docs/
│       │   ├── artifact_toml/
│       │   │   ├── fields.md
│       │   │   └── example_artifacts.toml
│       │   └── protocols/
│       │       ├── cargo-bundler-v0.1.0.md
│       │       └── command-bundler-v0.1.0.md
│       └── src/
│           └── main.rs
│
├── release/
│   └── ...
│
└── README.md
```

`automation/mza/` is the build/bundle orchestrator. It remains agnostic to Lexicon-specific names, paths, and installation behavior.

`release/` is reserved for Lexicon-specific setup and release-process source code. Generated release artifacts do not belong in `release/` and are not committed.

## 3. Cargo Package Responsibilities

### 3.1 Root workspace

The repository root is a **workspace-only** Cargo manifest (`[workspace]` + `[workspace.package]`, no `[package]` section, no `src/main.rs`). It is not itself a compiled crate.

### 3.2 `lexicon-bundle`

`lexicon-bundle` is the Protocol 1 bundle crate — a dedicated workspace member (not the root package). Its `Cargo.toml` inherits `version`/`edition` from `[workspace.package]`.

For MZA's `cargo-bundler-v0.1.0` protocol, `lexicon-bundle` implements the actual Lexicon installation behavior, including such responsibilities as:

- Extracting the embedded CLI or other payloads (embedded via its own `build.rs`, see Section 13).
- Selecting installation locations.
- Adding or linking the `lexicon` command into PATH.
- Recording the Lexicon installation location.
- Supporting installation, upgrade, or uninstall behavior defined by Lexicon.

MZA itself must not contain this Lexicon-specific behavior.

### 3.3 `lexicon-framework`

`lexicon-framework` owns Lexicon Core and the source implementation structure.

Core is a Rust library. Its manifest must point its library target at `core/src/lib.rs` if that nonstandard source location is retained.

Core defines the public contracts that source implementations must satisfy. Source implementations depend on Core's public API, while Core's internal engine types remain private.

Core must not expose concrete private-engine types through public contracts.

### 3.4 `lexicon-cli`

`lexicon-cli` contains the source of the global `lexicon` command. It does not contain a committed compiled binary.

Its Cargo dependencies establish any required compile-time relationship with `lexicon-framework`. Merely placing both packages in a workspace does not create that dependency.

### 3.5 Source implementation packages

Every get-raw-data and process-data implementation is its own Rust binary package with its own `Cargo.toml`, `Cargo.lock`, and `src/main.rs`.

The Rust compiler verifies each implementation against the appropriate Lexicon Core contract.

## 4. Lexicon Command Interface

All normal interaction begins with:

```text
lexicon
```

Data operations use:

```text
lexicon data
```

Source-management operations use:

```text
lexicon source
```

### 4.1 Get raw data

```text
lexicon data --get <source-name>
```

Lexicon resolves the corresponding native implementation executable under:

```text
lexicon-framework/sources/<source-name>/get-raw-data/get_raw_data_impl/
```

Normal execution uses an already-built native executable. Cargo and rustc are not invoked during ordinary execution. The executable is a local build artifact and is never committed.

### 4.2 Process data

```text
lexicon data --process <source-name>
```

Lexicon resolves the corresponding native implementation executable under:

```text
lexicon-framework/sources/<source-name>/process-data/process_data_impl/
```

Processed output is written under:

```text
lexicon-framework/sources/<source-name>/data/processed/
```

The final processed dataset for a source is SQLite.

### 4.3 Background execution

Add `--bg`:

```text
lexicon data --get example-source --bg
lexicon data --process example-source --bg
```

### 4.4 Abandon a previous failed session

Add `--abandon-past-fail`:

```text
lexicon data --get example-source --abandon-past-fail
```

Options may be combined:

```text
lexicon data --get example-source --bg --abandon-past-fail
```

### 4.5 Source-specific arguments

Lexicon-level arguments appear before `--`. Arguments after `--` are passed directly to the selected source implementation:

```text
lexicon data --get example-source --bg -- <source-specific arguments>
```

## 5. Raw Data Contract

Raw acquisition data is stored under:

```text
lexicon-framework/sources/<source-name>/data/raw/
```

Each network transaction receives its own directory (`data/raw/<request-timestamp>-<id>/`) with separate `request/` and `response/` subdirectories.

`request/metadata.json` stores structured request metadata (timestamp, method, URL, headers, protocol info, payload size/hash). `request/body` contains the original request-body bytes when present. Authentication secrets and equivalent credentials must not be persisted unredacted in request metadata.

`response/metadata.json` stores structured response metadata (timestamp, HTTP status, headers, protocol info, payload size/hash). `response/body` contains the response payload exactly as received. The raw layer never interprets a response body based on its content type.

The initial acquisition scope is HTTP end to end. Browser automation and additional acquisition protocols are outside the initial scope.

## 6. Session Contract

Get-raw-data execution state is stored under `lexicon-framework/sources/<source-name>/get-raw-data/sessions/`, tracked in `.../get-raw-data/session_status.json`.

Process-data execution state is stored separately under `lexicon-framework/sources/<source-name>/process-data/sessions/`, tracked in `.../process-data/session_status.json`.

Session records describe execution state and are separate from raw request/response data.

## 7. Adding and Building Sources

- `lexicon source <source-name> --draft` creates the mandatory source structure (`data/raw/`, `data/processed/`, session files, `get_raw_data_impl/`, `process_data_impl/`, manifests/lockfiles, entry points, `processing/`).
- Get-raw-data behavior: `lexicon-framework/sources/<source-name>/get-raw-data/get_raw_data_impl/src/main.rs`, must satisfy the `GetRawData` contract.
- Process-data behavior: `lexicon-framework/sources/<source-name>/process-data/process_data_impl/src/main.rs`, must satisfy the `ProcessData` contract. Source-specific processing files belong under `.../process_data_impl/processing/`.
- `lexicon source <source-name> --add` locates, compiles, and verifies the source's implementation crates against Core, then links successful implementations into native executables for the current target, placed in their respective implementation directories.
- `lexicon build` rebuilds all discovered source implementations for the current OS/architecture, via directory discovery rather than a manually maintained registry.

## 8. Runtime and Build Requirements

Normal users do not require Rust, Cargo, Zig, Python, the JVM, or another language runtime to execute already-built Lexicon operations. Rust/Cargo are required only for creating, changing, validating, or rebuilding Rust source implementations. Zig-assisted linking may be used for cross-target release builds; it is not required for normal runtime use.

Release matrix (deferred until exercised end to end — the `linux-x86_64-musl` flow is validated today):

- Linux x86_64, Linux ARM64, Windows x86_64, Windows ARM64.
- macOS is not part of the Linux-host cross-release matrix; a local native macOS build may be supported separately on an appropriate macOS host.

## 9. `automation/mza`

MZA is a generic build-and-bundle orchestrator, run directly as its own compiled binary (not via `cargo run` against its own manifest — it is itself a `[[bin]]` under `automation/mza/`), against `automation/mza/artifacts.toml`.

MZA performs two ordered phases per run:

1. Build and archive ordinary artifacts (dry-run by default; only builds when invoked with `--build`).
2. Build bundles from selected completed artifacts.

MZA contains no Lexicon-specific installer behavior; that belongs entirely to the bundle-implementing crates referenced by `[[bundle]]` declarations.

## 10. Ordinary Artifact Contract

Each `[[artifact]]` declares a stable `label`, the crate to build, its `type` (`main`/`snapshot`/`custom`), and an optional `exclude` list of `[[target]]` labels it does not build for (opt-out; an artifact applies to every configured target unless excluded).

MZA resolves the artifact's package name/version from its own `Cargo.toml` — never duplicated in `artifacts.toml`.

For every applicable target, MZA (with `--locked` on every `cargo` invocation, requiring a committed `Cargo.lock`):

1. Resolves the Cargo manifest.
2. Runs `cargo zigbuild` (or `cargo build` when natively targeting macOS) for that target.
3. Locates the produced native executable.
4. Creates the artifact's `.tar.xz` archive below the configured `output_path`, using the artifact label, type, crate version, resolved name, and target triple.
5. Records the resolved `(artifact label, target label) -> absolute .tar.xz path` mapping for later bundle use.

The artifact label is also the bundle input name directly — no second alias layer.

## 11. General Bundle Contract

Each `[[bundle]]` declares a stable `label`, a bundle-implementation `crate`, a `protocol`, `inputs` (artifact labels), `type`, `output_path`, and (protocol-dependent) `build_targets`.

The bundle's package name/version are resolved from its own `Cargo.toml` — the same manifest-resolution rule as ordinary artifacts, for both protocols. MZA never infers a bundle version from its inputs.

Rules applying to every protocol:

1. One bundle execution handles exactly one target.
2. Every input artifact used in that execution must have been built for that exact target — validated at `artifacts.toml` **parse time**, before any build runs.
3. Missing target-specific inputs are configuration errors (`PARSE_INVALID_BUNDLE`), never silently skipped.
4. Input archive paths are absolute paths to completed `.tar.xz` artifacts.
5. A successful bundle execution produces exactly one final target executable.
6. MZA archives that executable as the final bundle `.tar.xz`.

Target coverage validation (`resolve_bundle_targets`):

- Without `build_targets`: all of a bundle's inputs must apply to the exact same set of `[[target]]`s (matched by target label, via each artifact's `exclude`); that shared set becomes the bundle's targets.
- With `build_targets` (an explicit list of literal target triples): each triple is matched against a `[[target]]`'s computed triple, and every input must not exclude that target's label.

MZA never combines mismatched-architecture/OS inputs into one target-specific bundle, never silently takes an intersection, and never reports a partial bundle as successful.

## 12. Protocol 1: `cargo-bundler-v0.1.0`

Use this protocol when the referenced Rust bundle crate itself becomes the final target executable (cross-compiled directly by MZA, never executed by MZA).

### 12.1 Declaration

```toml
[[bundle]]
label = "lexicon"
crate = "../.."
output_path = "../../artifacts/"
type = "custom"
protocol = "cargo-bundler-v0.1.0"
inputs = [
    "lexicon_cli",
    "lexicon_framework",
]
```

`build_targets` is not required; targets are derived from the shared set across all inputs (Section 11).

### 12.2 Bundle specification and embedded bytes

For each target, MZA writes `bundle-spec.toml` (TOML, not Rust source):

```toml
protocol = "cargo-bundler-v0.1.0"
bundle = "lexicon"
target = "x86_64-unknown-linux-musl"

[[inputs]]
label = "lexicon_cli"
archive = "/absolute/path/lexicon_cli-0.1.0-x86_64-unknown-linux-musl.tar.xz"

[[inputs]]
label = "lexicon_framework"
archive = "/absolute/path/lexicon_framework-0.1.0-x86_64-unknown-linux-musl.tar.xz"
```

MZA sets `MZA_BUNDLE_INPUTS=<absolute-path-to-bundle-spec.toml>`, then runs `cargo zigbuild --release --locked --target <triple> --manifest-path <bundle Cargo.toml>` (or `cargo build` natively on macOS).

Because `lexicon-bundle` is only ever **compiled** by MZA (for a possibly foreign target) and never **executed** by MZA, its own code cannot read `MZA_BUNDLE_INPUTS` at runtime — by the time the compiled binary runs, it's on a different machine where build-host paths are meaningless. So `lexicon-bundle/build.rs` (which *does* run natively on the build host, as part of this same `cargo zigbuild` invocation) is the contract's bridge:

1. Reads `MZA_BUNDLE_INPUTS` (falls back to an empty input list if unset, so the crate still compiles standalone outside MZA).
2. Parses the TOML; for each input, copies its `archive` file into `$OUT_DIR`.
3. Generates `$OUT_DIR/mza_bundle_inputs.rs`, containing a self-contained type/static using `include_bytes!(concat!(env!("OUT_DIR"), "/<file-name>"))` per input — real embedded bytes, not paths.

`lexicon-bundle/src/main.rs` then does:

```rust
include!(concat!(env!("OUT_DIR"), "/mza_bundle_inputs.rs"));
```

giving it `MZA_BUNDLE_INPUTS: &[MzaBundleInput]` where `archive: &'static [u8]` is the actual compiled-in archive bytes.

### 12.3 Build flow

1. Validate target coverage (Section 11), at parse time.
2. Resolve target-specific `.tar.xz` archive paths (already built as ordinary artifacts).
3. Create the uniquely scoped temp workspace (Section 14) and write `bundle-spec.toml` into it.
4. Set `MZA_BUNDLE_INPUTS`, run `cargo zigbuild --locked` for the target (`build.rs` runs as part of this).
5. Locate the resulting target executable.
6. Archive it as the final bundle `.tar.xz` (Section 15 layout).

## 13. Protocol 2: `command-bundler-v0.1.0`

Use this protocol when a Rust adapter crate runs on the build host and invokes a project-specific external bundling system (native tool, script, installer generator, etc.). The adapter crate does not become the final target executable, and is never cross-compiled.

### 13.1 Declaration

```toml
[[bundle]]
label = "example-external-bundle"
crate = "../../example-bundler"
output_path = "../../artifacts/"
type = "custom"
protocol = "command-bundler-v0.1.0"
build_targets = [
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
]
inputs = [
    "lexicon_cli",
    "lexicon_framework",
]
```

`build_targets` is **required** for this protocol — the exact set of literal target triples this bundle must be produced for. Every input artifact must provide every listed triple (validated at parse time); MZA runs the adapter crate once per triple.

The adapter crate must provide one unambiguous binary target (either a single `[[bin]]`, or `default-run` set in its `Cargo.toml`).

### 13.2 No arbitrary MZA command field

MZA does not add an arbitrary `command` field. The protocol itself defines the invocation:

```text
cargo run --release --locked --manifest-path <bundle-crate>/Cargo.toml
```

The crate's own project-specific Rust code owns the path, arguments, and invocation of whatever external bundling executable/runtime it uses. MZA does not interpret those external-tool details.

### 13.3 Bundle specification

For each target, MZA writes a target-specific `bundle-spec.toml`:

```toml
protocol = "command-bundler-v0.1.0"
bundle = "example-external-bundle"
output_path = "/tmp/mza/<run_id>/example-external-bundle/x86_64-unknown-linux-musl/output/example-external-bundle"

[bundle_target]
target = "x86_64-unknown-linux-musl"

[[bundle_target.inputs]]
label = "lexicon_cli"
archive = "/absolute/path/lexicon_cli-0.1.0-x86_64-unknown-linux-musl.tar.xz"

[[bundle_target.inputs]]
label = "lexicon_framework"
archive = "/absolute/path/lexicon_framework-0.1.0-x86_64-unknown-linux-musl.tar.xz"
```

`target` is stated once, under `[bundle_target]`, rather than repeated per input. There is no `bundle_name`/`bundle_version` field — the crate's own `Cargo.toml` is the authoritative version source, resolved by MZA directly, the same as Protocol 1.

MZA sets `MZA_BUNDLE_SPEC=<absolute-path-to-bundle-spec.toml>`. The adapter crate reads this at **runtime** (`std::env::var`/`std::fs::read_to_string`) — unlike Protocol 1, this crate genuinely executes (via `cargo run`) on the build host during MZA's process, so a live runtime env-var read works.

### 13.4 Exact output-path contract (no result manifest)

MZA already knows the bundle label, the resolved package version, the current target, and that exactly one output is permitted — so it supplies the exact `output_path` up front rather than having the crate report one back.

The adapter crate must:

1. Read `MZA_BUNDLE_SPEC`.
2. Read the target-specific input archive paths.
3. Invoke its external bundling executable/runtime, synchronously, within this same process.
4. Cause that external system to produce an executable for the specification's target.
5. Move/copy the completed executable to exactly `output_path` if the external system can't write there directly.
6. Exit `0` only once the completed output exists at that path.

MZA then verifies: exit status `0`, `output_path` exists, and it is a regular file. MZA does not inspect ELF/PE/installer headers — target correctness is trust-based, enforced structurally (via `build_targets` validation) rather than by binary inspection.

### 13.5 Host and target distinction

The adapter crate and its external bundling tool run on the build host. `cargo run` never receives `--target` for the requested output target — doing so would produce a target executable that couldn't execute on the host to drive the rest of the process. The requested output target only ever travels inside `bundle-spec.toml`.

### 13.6 Execution flow

For every `build_targets` entry: validate input coverage (parse time) → resolve target-specific `.tar.xz` paths → create the temp workspace → write the spec → set `MZA_BUNDLE_SPEC` → `cargo run --locked` the adapter crate on the host → require exit `0` → verify `output_path` is a regular file → archive it → write to the normal bundle output location (Section 15).

## 14. Temporary Build Directories

Both protocols share the same uniquely scoped temp-workspace mechanism:

```text
<system-temp>/
└── mza/
    └── <run-id>/
        └── <bundle-label>/
            └── <target>/
                ├── bundle-spec.toml
                └── output/            (Protocol 2 only, holds the produced executable before archiving)
```

`<system-temp>` is `std::env::temp_dir()` (OS standard). This scoping prevents collisions between concurrent MZA processes, multiple bundles, multiple targets, and repeated builds of the same bundle. Temporary protocol files never belong in the repository, the permanent artifact tree, or `release/`.

## 15. Permanent Output Layout

Ordinary artifacts:

```text
<output_path>/<label>/<type>/<version>/<name>-<version>-<target-triple>.tar.xz
```

Bundles (either protocol) insert the protocol id **after** `type`, and add a `<target>` directory level:

```text
<output_path>/<label>/<type>/<protocol>/<version>/<target>/<label>-<version>-<target>.tar.xz
```

Examples:

```text
artifacts/lexicon_cli/custom/0.1.0/lexicon_cli-0.1.0-x86_64-unknown-linux-musl.tar.xz

artifacts/lexicon/custom/cargo-bundler-v0.1.0/0.1.0/x86_64-unknown-linux-musl/lexicon-0.1.0-x86_64-unknown-linux-musl.tar.xz

artifacts/example-external-bundle/custom/command-bundler-v0.1.0/0.1.0/aarch64-pc-windows-gnu/example-external-bundle-0.1.0-aarch64-pc-windows-gnu.tar.xz
```

No generated file in the artifact output tree is committed to the Lexicon repository.

## 16. Error Codes and Exit Codes

MZA reports failures with stable machine-readable codes (`PARSE_INVALID_BUNDLE`, `ARTIFACT_*`, `CARGO_LOCKFILE_MISSING`, `BUNDLE_UNKNOWN_PROTOCOL`, `BUNDLE_MISSING_INPUT`, `BUNDLE_EXECUTION_FAILED`, etc.), recorded per run under an append-only run-record archive (`archive/<run-id>/{metadata,input,outcome}` under the artifacts directory).

Process exit codes distinguish failure stage: `1` if any ordinary artifact build failed, `2` if artifacts succeeded but a bundle failed — so it's always clear whether the failure happened before or during bundling.

`--locked` is used on every `cargo` invocation MZA makes (artifact builds, both bundle protocols); a missing/stale `Cargo.lock` is reported explicitly as `CARGO_LOCKFILE_MISSING` rather than surfacing as a generic cargo failure.

## 17. Installation

Installation tooling belongs under `release/` and remains pending until implemented. Once implemented, setup will:

1. Determine the operating system and CPU architecture.
2. Select or locate the appropriate completed Lexicon bundle.
3. Run the Lexicon installer executable.
4. Install or link the `lexicon` CLI into an appropriate user PATH location.
5. Record the Lexicon installation location so commands do not depend on the current working directory.
6. Report whether installation succeeded.

Architecture discovery:

- Windows: `echo $env:PROCESSOR_ARCHITECTURE` — `AMD64` → `windows-x86_64`, `ARM64` → `windows-arm64`.
- Linux: `uname -m` — `x86_64` → `linux-x86_64`, `aarch64`/`arm64` → `linux-arm64`.
- macOS: `uname -m` — `x86_64` → `macos-x86_64`, `arm64` → `macos-arm64` (detection does not imply macOS artifacts are produced by the Linux-host release pipeline).

Verify installation with `lexicon --version`.

## 18. Optional Attribution System

An optional attribution mechanism may later be exposed as `lexicon source --attribute`, creating an attribution directory for a source and possibly supporting attribution associated with individual raw requests. Its exact persistence/command behavior remain intentionally open and are not part of the required initial implementation.

## 19. Non-Negotiable Invariants

1. The repository contains source code and committed data only, never compiled executables or generated release archives.
2. Lexicon Core remains domain-agnostic.
3. Source contracts are enforced by Rust compilation.
4. Source implementations do not access Core's private engine implementation types.
5. Ordinary runtime execution does not invoke Cargo or require a development toolchain.
6. MZA remains reusable across unrelated projects and contains no Lexicon-specific bundling logic.
7. Bundle inputs are addressed directly by their artifact labels.
8. Artifact and bundle versions come from their respective Cargo manifests.
9. Every target-specific bundle contains only executable inputs built for that same target, validated at parse time.
10. Protocol 1 (`cargo-bundler-v0.1.0`) embeds its inputs' bytes via a `build.rs`-generated `include_bytes!` file, driven by a `bundle-spec.toml` MZA writes.
11. Protocol 2 (`command-bundler-v0.1.0`) uses a host-executed Rust adapter crate (`cargo run`, never cross-compiled) and an exact `output_path`; it uses no result manifest.
12. Every bundle execution (either protocol) produces exactly one final executable per target.
13. Permanent bundle paths insert the protocol after the ordinary artifact `type` segment, and add a `<target>` directory level.
14. Temporary protocol files are uniquely scoped (`mza/<run-id>/<bundle-label>/<target>/`) and never written into the permanent artifact tree.
15. Every `cargo` invocation MZA makes uses `--locked`; a missing lockfile is a distinct, explicit error.
16. Process exit codes distinguish artifact-stage failures (`1`) from bundle-stage failures (`2`).
