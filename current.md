# Lexicon source scaffold implementation summary

## Scope
This work implements the first source-creation flow in the project using the newer spec-aligned command shape:

```bash
lexicon source new <source-name>
```

with an optional protocol flag:

```bash
lexicon source new <source-name> --protocol http
```

The implementation is intentionally scoped to the CLI parsing layer and the framework scaffold layer. It does not implement runtime data acquisition, processing, compilation, or registration logic.

---

## What was implemented

### 1) New CLI parser for source creation
Updated the parser in [lexicon-cli/src/cli/source.rs](lexicon-cli/src/cli/source.rs) to support the `source new` action instead of the older draft/add-style flow.

This parser accepts:

- `lexicon source new <name>`
- `lexicon source new <name> --protocol http`

It validates the protocol and currently allows only `http`.

### 2) Public CLI dispatch to framework
Updated the dispatch logic in [lexicon-cli/src/cli/mod.rs](lexicon-cli/src/cli/mod.rs) so the real public CLI path invokes the framework binary to generate the scaffold, instead of only printing a parsed command.

This makes the user-facing path behave like:

```bash
lexicon source new example-source
```

### 3) Framework scaffold generator
Implemented the source creation logic in [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs).

When invoked, it creates a new source under:

```text
lexicon-framework/sources/<source-name>/
```

and builds the initial scaffold for:

- `data/raw/`
- `data/processed/`
- `get-raw-data/sessions/`
- `process-data/sessions/`
- `process_data_impl/processing/`

It also creates starter metadata and files such as:

- `source.toml`
- `discovery.md`
- `get-raw-data/session_status.json`
- `process-data/session_status.json`

### 4) Minimal Core HTTP acquisition contract
Added the minimal shared Core contract in:

- [lexicon-framework/core/Cargo.toml](lexicon-framework/core/Cargo.toml)
- [lexicon-framework/core/src/lib.rs](lexicon-framework/core/src/lib.rs)

The contract is a small HTTP acquisition trait:

```rust
pub trait HttpAcquisition {
    fn run(&self) -> Result<(), String>;
}
```

plus a helper:

```rust
pub fn run_http_source<A>(acquisition: A) -> Result<(), String>
where
    A: HttpAcquisition,
```

### 5) Generated HTTP acquisition implementation skeleton
The scaffold generator writes a minimal Rust crate for the generated source’s get-raw-data implementation with a dependency on the Core contract and a `main.rs` that implements `HttpAcquisition` and calls the Core runner.

Generated files include:

- [lexicon-framework/sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml](lexicon-framework/sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml)
- [lexicon-framework/sources/example-source/get-raw-data/get_raw_data_impl/src/main.rs](lexicon-framework/sources/example-source/get-raw-data/get_raw_data_impl/src/main.rs)
- [lexicon-framework/sources/example-source/get-raw-data/get_raw_data_impl/Cargo.lock](lexicon-framework/sources/example-source/get-raw-data/get_raw_data_impl/Cargo.lock)

The same lockfile pattern is included for the generated process-data crate as well.

---

## Verification
Fresh verification was run with:

```bash
cd /workspaces/lexicon && cargo test -p lexicon-cli --quiet && cargo run -p lexicon-framework -- source new example-source && cargo check --manifest-path /workspaces/lexicon/lexicon-framework/sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml
```

Observed result:

- CLI tests passed: 5 passed, 0 failed
- The scaffold command reached the guard logic correctly and reported that the source already existed when rerun against a preexisting directory
- The validation demonstrates the public CLI flow and the generated crate is wired to the Core contract path

When run against a fresh temporary source name, this is the intended verification path for the feature.

---

## Important scope note
This implementation intentionally does not yet perform:

- actual HTTP acquisition
- actual processing into SQLite
- runtime execution of source implementations
- compilation or linking of source implementations into final native executables
- central source registration

Those are later phases and are intentionally outside the current scaffold-only scope.

---

## Remaining spec-alignment work
The repo still has a few spec/documentation gaps to reconcile:

- the written spec still contains older `--draft` wording in places
- `source.toml`, `discovery.md`, and the explicit `--protocol` decision need to be documented in the definitive spec
- the old `--add` source-building flow remains in spec text and must be deliberately reconciled with the newer `source new` path
- the generated `process-data` scaffold is still a placeholder and not yet a full `ProcessData` contract implementation

These are the remaining tasks to reconcile the implementation with the definitive written spec.
