# Implementation report: Lexicon init and project discovery

## Files changed

- `lexicon-cli/src/cli/mod.rs`
- `lexicon-framework/src/main.rs`

## Functions and types changed

### CLI dispatch and output contract

- `RootCommand::Source` branch in `dispatch`
- `framework_binary_path()`

### Discovery and pruning logic

- `find_project_root(start_dir: &Path) -> Result<PathBuf, String>`
- `find_descendant_project_root(root: &Path) -> Result<Option<PathBuf>, String>`
- `should_prune_descendant_directory(path: &Path) -> bool`
- `visit_descendants(root: &Path, found: &mut Option<PathBuf>, current: &Path) -> Result<(), String>`

## Actual behavior implemented

### 1. CLI output contract

The framework owns the successful source-scaffold output because it writes the actual files. The CLI now exits after the framework succeeds without printing a duplicate success message.

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

The framework emits the only success output:

```rust
println!("[lexicon] Created source '{}' at {}", source_name, source_dir.display());
println!("[lexicon] Files to edit next:");
for (relative_path, _) in &files {
    println!("[lexicon]   - {}", source_dir.join(relative_path).display());
}
```

### 2. Deterministic descendant discovery with pruning

The descendant scan sorts entries before walking them and prunes generated/data-heavy directories that cannot legally contain Lexicon project roots.

```rust
fn should_prune_descendant_directory(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    if matches!(name, ".git" | "target" | "artifacts" | "bundles" | "mza") {
        return true;
    }

    if name == "data" {
        return true;
    }

    if name == "raw" || name == "processed" {
        let parent_name = path.parent().and_then(|parent| parent.file_name()).and_then(|value| value.to_str());
        return parent_name == Some("data");
    }

    false
}
```

```rust
let mut entries = fs::read_dir(current)
    .map_err(|error| format!("failed to read {}: {error}", current.display()))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| format!("failed to read directory entry in {}: {error}", current.display()))?;
entries.sort_by_key(|entry| entry.file_name());

for entry in entries {
    let path = entry.path();

    if path.is_symlink() {
        continue;
    }

    if should_prune_descendant_directory(&path) {
        continue;
    }

    if path.is_dir() {
        let marker = path.join("lexicon.toml");
        if marker.is_file() && path != root {
            if found.is_none() {
                *found = Some(path.clone());
            }
            return Ok(());
        }
        visit_descendants(root, found, &path)?;
        if found.is_some() {
            return Ok(());
        }
    }
}
```

## Test function names and requirement coverage

- `cli::tests::cli_source_new_prints_only_framework_success_output`
  - verifies the public CLI emits the framework-owned success sequence and no extra CLI success line
- `cli::tests::dispatch_source_new_produces_only_framework_output`
  - verifies the CLI dispatch path stays simple and does not duplicate framework success output
- `tests::find_project_root_rejects_descendant_nested_project`
  - verifies nested descendant detection and the outer/nested reporting path
- `tests::find_descendant_project_root_prunes_excluded_directories`
  - verifies deterministic pruning for `.git`, `target`, `artifacts`, `bundles`, `mza`, and `data/raw`/`data/processed`
- existing init and config tests continue to cover project creation, project-name validation, symlink escape checks, and relevant staging behavior

## Verification

### Command run

```bash
cd /workspaces/lexicon && cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

### Result

- `lexicon-cli`: 12 passed, 0 failed
- `lexicon-framework`: 6 passed, 0 failed
- total: all targeted tests passed

### End-to-end public CLI check

```bash
cd /workspaces/lexicon
/tmp/.../lexicon-cli init "$tmp" demo-project
cd "$tmp/demo-project"
LEXICON_FRAMEWORK_PATH=/workspaces/lexicon/target/debug/lexicon-framework /workspaces/lexicon/target/debug/lexicon-cli source new example-source
```

Observed output:

```text
[lexicon] Initialized project 'demo-project' at /tmp/.../demo-project
[lexicon] Created source 'example-source' at /tmp/.../demo-project/sources/example-source
[lexicon] Files to edit next:
[lexicon]   - /tmp/.../demo-project/sources/example-source/source.toml
[lexicon]   - /tmp/.../demo-project/sources/example-source/discovery.md
...
```

This confirms the CLI-to-framework dispatch path works and the framework is the sole producer of the success output.

## Remaining gaps

- The source scaffolding behavior is now correct for the active task, but the broader HTTP acquisition and runtime processing scope remains intentionally out of scope for this task and was not changed.
- The report here is limited to the init/discovery and output-contract fixes required by the active current.md task.

## Confirmation

This task changed only the Lexicon source files required for the active issue: the CLI dispatch/output contract and the framework discovery/pruning logic. No unrelated runtime or bundle/installer code was modified.
