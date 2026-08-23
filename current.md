# Implementation report for the source-build task

## Scope and outcome

Implemented the requested `lexicon source build <source-name> --protocol <protocol>` flow, aligned with the repository specification in [instructions.md](instructions.md). The CLI now requires a source name and a `--protocol` flag, forwards the command to the framework, and the framework validates the Lexicon project, source metadata, and protocol-scoped source layout before compiling and publishing both implementation crates.

## Changed files

- [lexicon-cli/src/cli/source.rs](lexicon-cli/src/cli/source.rs)
- [lexicon-cli/src/cli/mod.rs](lexicon-cli/src/cli/mod.rs)
- [lexicon-framework/src/main.rs](lexicon-framework/src/main.rs)
- [lexicon-framework/Cargo.toml](lexicon-framework/Cargo.toml)
- [current.md](current.md)

## Final CLI parser and behavior

The parser now exposes:

- `SourceAction::Build(BuildSourceCommand)`
- `BuildSourceCommand { source_name: String, protocol: String }`

The command is accepted only in the form:

```bash
lexicon source build example-source --protocol http
```

and rejected when:

- the source name is missing
- `--protocol` is missing
- the protocol value is missing
- the protocol is unsupported
- the deprecated `source add` command is used

The CLI dispatch sends the framework this exact sequence:

```text
source
build
<source-name>
--protocol
<protocol>
```

The CLI no longer emits a duplicate success message after the framework finishes.

## Framework validation flow

The framework now:

- finds the containing project via `lexicon.toml`
- resolves the configured `sources_directory`
- validates the source name and protocol
- ensures the source root and protocol directory exist
- loads and validates `source.toml`
- rejects mismatched source/protocol metadata
- rejects unsupported schema versions
- rejects symlink/path traversal escapes outside the project root
- rejects missing implementation manifests before publication

## Scaffold reconciliation

The scaffold generation flow was updated to produce the protocol-scoped structure with the required crate names:

- `get-raw-data/get-raw-data-impl`
- `process-data/process-data-impl`

It also generates the required runtime sibling directories and `.gitignore` files:

```gitignore
*
!.gitignore
```

## Cargo build behavior

The build path uses the required cargo invocation pattern:

```bash
cargo build --release --locked --manifest-path <manifest> --target-dir <temp-dir> --message-format=json-render-diagnostics
```

It uses isolated temporary target directories and parses Cargo JSON `compiler-artifact` output to identify the runtime executable. It rejects missing or ambiguous output and emits the required Lexicon-owned toolchain error when Cargo is unavailable.

## Publication and rollback

The runtime publication logic:

- builds both crates before altering runtime directories
- stages both executables in the runtime directory
- preserves any existing runtime executable with backups
- performs atomic same-filesystem moves when possible
- restores previous files and removes staged artifacts during failure
- only prints success after both successful publications

Final runtime outputs are placed in the protocol-scoped runtime directories for each operation.

## Verification evidence

I validated the implementation with the repo’s relevant Rust test suite and the required build script.

1. Rust validation command:

```bash
cd /workspaces/lexicon && pushd /workspaces/lexicon >/dev/null && cargo test -p lexicon-cli -p lexicon-framework -- --nocapture ; popd >/dev/null
```

Result: all relevant tests passed.

2. Repo-required validation command:

```bash
cd /workspaces/lexicon && pushd /workspaces/lexicon >/dev/null && bash ./automation/build_bundle_install/build_bundle_install.sh ; popd >/dev/null
```

Result: build, bundle, and install completed successfully.

## Test totals

- `lexicon-cli`: 24 passed, 0 failed
- `lexicon-framework` library tests: 1 passed, 0 failed
- `lexicon-framework` binary tests: 10 passed, 0 failed
- total relevant tests: 35 passed, 0 failed

## Final notes

- `source create` remains functional and uses the protocol-scoped scaffold.
- `source add` remains rejected.
- Root `lexicon build` remains unchanged.
- No stale runtime staging or backup artifacts from the verification run remain in the repo tree.
