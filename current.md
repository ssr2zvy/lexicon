## Permanent rule
Before reading or using this report for analysis, pull the latest remote state to avoid stale divergence. After verifying the implementation, overwrite this report with a fresh summary of the verified outcome, commit it with message "current", and push the branch.

---

## Scope
This work covers the public CLI and scaffold flow that the spec calls for, specifically:

```bash
lexicon init <project-name>
lexicon source new <source-name>
lexicon source new <source-name> --protocol http
```

The implemented portion is limited to parsing, project-root discovery, and scaffold generation on the CLI/framework side. It does not add the full runtime data-processing or orchestration behavior beyond the scaffolded foundations.

---

## What was implemented exactly

### 1) Root CLI parsing
The root parser in [lexicon-cli/src/cli/mod.rs](lexicon-cli/src/cli/mod.rs) supports the public command surface:

- `lexicon init <project-name>`
- `lexicon source new <source-name>`
- `lexicon source new <source-name> --protocol http`
- `lexicon data --get <source>` and `lexicon data --process <source>`
- `lexicon build`

The actual new-source parsing lives in [lexicon-cli/src/cli/source.rs](lexicon-cli/src/cli/source.rs), and the new project initializer lives in [lexicon-cli/src/cli/init.rs](lexicon-cli/src/cli/init.rs).

### 2) Project initialization flow
The CLI init command creates a project directory, a `lexicon.toml` file, and a default `sources/` directory. The dispatch layer then reports success only after the project directory and config are created.

### 3) Project-root discovery
The framework in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs) climbs upward from the current working directory until it finds a parent containing `lexicon.toml`, then reads the configured `sources_directory` value. This allows `lexicon source new` to work from inside a user-created project instead of only from the repository checkout.

### 4) Source scaffold generation
The framework creates a template project structure under the configured sources directory, including:

- `source.toml`
- `discovery.md`
- session status JSON files
- `get-raw-data/` and `process-data/` directories
- generated Rust crates for the implementation stubs

This scaffold is produced by the generation logic in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs).

### 5) Minimal shared HTTP acquisition contract
The shared core contract in [lexicon-framework/core/src/lib.rs](lexicon-framework/core/src/lib.rs) defines the minimal trait-based HTTP acquisition boundary:

```rust
pub struct HttpAcquisitionContext;

pub trait HttpAcquisition {
    fn acquire(
        &self,
        context: &mut HttpAcquisitionContext,
    ) -> Result<(), String>;
}

pub fn run_http_source<A>(acquisition: A) -> Result<(), String>
where
    A: HttpAcquisition,
{
    let mut context = HttpAcquisitionContext;
    acquisition.acquire(&mut context)
}
```

The corresponding crate metadata is in [lexicon-framework/core/Cargo.toml](lexicon-framework/core/Cargo.toml).

### 6) Portable generated dependency wiring
The scaffold generator writes manifests that depend on the shared core crate via a portable git-tagged dependency instead of a machine-local absolute path:

```toml
[dependencies]
lexicon-framework-core = {
    git = "https://github.com/ssr2zvy/lexicon",
    tag = "v0.1.1"
}
```

This is the exact fix for the portability issue: generated crates no longer embed `/workspaces/lexicon` paths and can be checked out and built in a fresh project under `/tmp`.

### 7) Repo hygiene updates
The ignore rules in [.gitignore](.gitignore) were updated to cover nested generated directories such as `**/bundles/`, `**/mza/`, `**/artifacts/`, and `**/target/` anywhere in the repository tree.

---

## Verification performed
I verified this with fresh commands on the real repo and on a newly-created external project:

```bash
cd /workspaces/lexicon && cargo test -p lexicon-cli -p lexicon-framework --quiet
cd /workspaces/lexicon && cargo build -p lexicon-cli -p lexicon-framework --quiet
rm -rf /tmp/lexicon-portable-test && mkdir -p /tmp/lexicon-portable-test
cd /tmp/lexicon-portable-test
/workspaces/lexicon/target/debug/lexicon-cli init my-data-project
cd /tmp/lexicon-portable-test/my-data-project
/workspaces/lexicon/target/debug/lexicon-cli source new example-source
cargo check --manifest-path /tmp/lexicon-portable-test/my-data-project/sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml
cargo check --manifest-path /tmp/lexicon-portable-test/my-data-project/sources/example-source/process-data/process_data_impl/Cargo.toml
```

Evidence from the successful output:

- `Initialized Lexicon project 'my-data-project' ...` succeeded
- `Created source scaffold for 'example-source' ...` succeeded
- both generated source crates finished `cargo check` successfully
- the generated manifests contained the git-tagged dependency and no `/workspaces/lexicon` path

---

## Current status
The verified implementation is in place for the CLI parsing and project/source scaffold flow described above. This does not add the full runtime acquisition behavior or the later orchestration build flow beyond the scaffold foundation that the project currently requires.
