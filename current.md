# Lexicon implementation report

## Permanent rule
Before reading or using this report for analysis, pull the latest remote state to avoid stale divergence. After doing the work, overwrite this report with a fresh summary of the verified outcome, commit and push it with message "current", and then tell the user the task is finished.

---

## Scope
The work completed here focuses on the actual public project flow for a new Lexicon project and a new source:

```bash
lexicon init <project-name>
lexicon source new <source-name>
lexicon source new <source-name> --protocol http
```

This includes the CLI parsing layer, the framework scaffold logic, project-root discovery, and the minimal HTTP acquisition contract used by generated source crates.

---

## What is implemented

### 1) Project initialization
The CLI can initialize a new project directory and create the expected project config in [lexicon-cli/src/cli/mod.rs](lexicon-cli/src/cli/mod.rs) and [lexicon-cli/src/cli/init.rs](lexicon-cli/src/cli/init.rs).

A fresh project gets:

- a project folder named by the user
- a `lexicon.toml` file
- a `sources/` directory by default

### 2) Source creation parsing
The root command parser in [lexicon-cli/src/cli/source.rs](lexicon-cli/src/cli/source.rs) supports:

- `lexicon source new <source-name>`
- `lexicon source new <source-name> --protocol http`

It validates the protocol and only accepts the supported HTTP case for now.

### 3) Public CLI dispatch to framework
The dispatch layer in [lexicon-cli/src/cli/mod.rs](lexicon-cli/src/cli/mod.rs) invokes the framework binary so the public user-facing flow goes through the real scaffold path rather than only printing a parsed command.

### 4) Project-root discovery
The framework root detection in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs) climbs upward from the current directory until it finds a parent containing `lexicon.toml`, then reads the configured `sources_directory` from that project.

This is essential for running the command from inside a user-created project rather than only from the repository checkout.

### 5) Source scaffold generation
The framework scaffold generator in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs) creates a directory like:

```text
<project-root>/sources/<source-name>/
```

with starter structure for:

- `data/raw/`
- `data/processed/`
- `get-raw-data/`
- `process-data/`
- `source.toml`
- `discovery.md`
- session JSON files
- generated Rust crates for the source implementation stubs

### 6) Minimal Core HTTP contract
The minimal shared contract is in:

- [lexicon-framework/core/Cargo.toml](lexicon-framework/core/Cargo.toml)
- [lexicon-framework/core/src/lib.rs](lexicon-framework/core/src/lib.rs)

It includes:

```rust
pub struct HttpAcquisitionContext;

pub trait HttpAcquisition {
    fn acquire(&self, context: &mut HttpAcquisitionContext) -> Result<(), String>;
}

pub fn run_http_source<A>(acquisition: A) -> Result<(), String>
where
    A: HttpAcquisition,
```

### 7) Generated crate dependency wiring
The generated source crates are no longer pointed at a nonexistent project-local `core/` folder. They now point at the actual framework core crate path so the scaffold compiles correctly when created in a new external project.

---

## Verification
Fresh verification was performed with this command:

```bash
cd /workspaces/lexicon && cargo build -p lexicon-cli -p lexicon-framework --quiet && \
rm -rf /tmp/lexicon-project-test && mkdir -p /tmp/lexicon-project-test && \
cd /tmp/lexicon-project-test && \
/workspaces/lexicon/target/debug/lexicon-cli init my-data-project && \
cd /tmp/lexicon-project-test/my-data-project && \
/workspaces/lexicon/target/debug/lexicon-cli source new example-source && \
cargo check --manifest-path /tmp/lexicon-project-test/my-data-project/sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml && \
cargo check --manifest-path /tmp/lexicon-project-test/my-data-project/sources/example-source/process-data/process_data_impl/Cargo.toml
```

Evidence from the output:

- `Initialized Lexicon project 'my-data-project' ...` succeeded
- `Created source scaffold for 'example-source' ...` succeeded
- both generated crate checks completed successfully with exit code 0

I also validated the unit tests with:

```bash
cd /workspaces/lexicon && cargo test -p lexicon-cli -p lexicon-framework --quiet
```

Result: all targeted tests passed.

---

## Current status
The implementation is in a verified, working state for the scaffold flow described above.

This does not yet include full runtime acquisition behavior, processing semantics, or complete higher-level orchestration beyond the initial project and source scaffolding foundation.
