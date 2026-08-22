# Implementation report: Lexicon init and project discovery

## Files changed

- `lexicon-cli/src/cli/mod.rs`
- `lexicon-framework/src/main.rs`

## Functions and types changed

### CLI output contract

- `dispatch(cli: Cli) -> Result<(), String>`
- `RootCommand::Source` branch in `dispatch`
- `framework_binary_path() -> Result<String, String>`

### Discovery and pruning logic

- `find_project_root(start_dir: &Path) -> Result<PathBuf, String>`
- `find_descendant_project_root(root: &Path) -> Result<Option<PathBuf>, String>`
- `should_prune_descendant_directory(path: &Path) -> bool`
- `visit_descendants(root: &Path, found: &mut Option<PathBuf>, current: &Path) -> Result<(), String>`

## Actual init flow

```text
main
→ Cli::parse
→ RootCommand::Init
→ dispatch
→ initialize_project(parent_path, project_name)
→ validate_project_name
→ canonicalize the parent directory
→ reject nested existing project markers
→ create staging dir beside target
→ create sources/
→ write lexicon.toml
→ rename staged dir into final project path
```

The CLI dispatch path that was fixed is:

```rust
Some(RootCommand::Init(command)) => {
    let project_root = initialize_project(&command.parent_path, &command.project_name)?;
    println!("[lexicon] Initialized project '{}' at {}", command.project_name, project_root.display());
    Ok(())
}
```

The actual init implementation stages a temporary directory with `tempfile`, writes the TOML, and renames it into the final project path.

## Actual project-discovery flow

```text
current directory
→ ancestor collection
→ nesting validation
→ descendant scan
→ TOML parsing
→ secure sources-directory resolution
```

The final descendant-pruning logic is:

```rust
fn should_prune_descendant_directory(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    if matches!(name, ".git" | "target" | "artifacts" | "bundles" | "mza") {
        return true;
    }

    if matches!(name, "raw" | "processed") {
        return path
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            == Some("data");
    }

    false
}
```

This ensures:

- `data/raw/` is ignored
- `data/processed/` is ignored
- `data/nested-project/lexicon.toml` is still detected as a real nested project

## Output contract fix

The framework owns the scaffold output, and the CLI no longer prints a duplicate success message after the framework exits.

```rust
Some(RootCommand::Source(command)) => {
    match command.action {
        SourceAction::New(new_command) => {
            let framework_path = framework_binary_path()?;
            let status = Command::new(framework_path)
                .args([
                    "source",
                    "new",
                    &new_command.source_name,
                    "--protocol",
                    &new_command.protocol,
                ])
                .status()
                .map_err(|error| format!("failed to execute framework binary: {error}"))?;
            if !status.success() {
                return Err(format!(
                    "framework source scaffold step failed with exit status {}",
                    status
                ));
            }
            Ok(())
        }
    }
}
```

The framework emits the success lines itself:

```rust
println!("[lexicon] Created source '{}' at {}", source_name, source_dir.display());
println!("[lexicon] Files to edit next:");
for (relative_path, _) in &files {
    println!("[lexicon]   - {}", source_dir.join(relative_path).display());
}
```

## Test function names and requirement coverage

- `cli::tests::cli_source_new_prints_only_framework_success_output`
  - verifies the public CLI emits only the framework-owned success output and no duplicate CLI message
- `cli::tests::dispatch_source_new_produces_only_framework_output`
  - verifies the CLI dispatch path remains simple and does not add a second success message
- `tests::find_project_root_rejects_descendant_nested_project`
  - verifies nested-descendant detection and outer/nested reporting
- `tests::find_descendant_project_root_prunes_excluded_directories`
  - verifies `data/raw` and `data/processed` are ignored while `data/nested-project` is still detected
- existing `lexicon-cli` init tests continue to cover project-name validation, staging behavior, and TOML generation
- existing `lexicon-framework` config tests cover symlink-escape safeguards

## Verification

### Command run

```bash
cd /workspaces/lexicon && cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

### Result

- `lexicon-cli`: 12 passed, 0 failed
- `lexicon-framework`: 6 passed, 0 failed
- total: all targeted tests passed

### Public CLI verification

```bash
pushd /workspaces/lexicon >/dev/null
/tmp/.../lexicon-cli init "$tmp" demo-project
cd "$tmp/demo-project"
LEXICON_FRAMEWORK_PATH=/workspaces/lexicon/target/debug/lexicon-framework /workspaces/lexicon/target/debug/lexicon-cli source new example-source
popd >/dev/null
```

Observed public output:

```text
[lexicon] Initialized project 'demo-project' at /tmp/.../demo-project
[lexicon] Created source 'example-source' at /tmp/.../demo-project/sources/example-source
[lexicon] Files to edit next:
[lexicon]   - /tmp/.../demo-project/sources/example-source/source.toml
[lexicon]   - /tmp/.../demo-project/sources/example-source/discovery.md
...
```

This confirms the public CLI dispatch path reaches the real framework scaffold and emits the expected `[lexicon]` prefix contract.

## Remaining gaps

- This task is complete for the init/discovery and output-contract scope described by the active task.
- HTTP acquisition and runtime processing remain intentionally out of scope and were not changed.

## Confirmation

Only the Lexicon source files required for this task were changed, and the final report now reflects the verified state of the implementation.
