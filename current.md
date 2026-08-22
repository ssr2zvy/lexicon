# Implementation report: Lexicon init and project discovery

## Files changed

- `lexicon-cli/Cargo.toml`
- `lexicon-cli/src/cli/init.rs`
- `lexicon-cli/src/cli/mod.rs`
- `lexicon-framework/src/main.rs`
- `current.md`

## Functions and types changed

### CLI layer (`lexicon-cli/src/cli/mod.rs`)

- `Cli`
- `RootCommand`
- `dispatch(cli: Cli) -> Result<(), String>`
- `framework_binary_path() -> Result<String, String>`
- `locate_workspace_root() -> Result<PathBuf, String>`

### Initialization layer (`lexicon-cli/src/cli/init.rs`)

- `validate_project_name(project_name: &str) -> Result<(), String>`
- `initialize_project(parent_path: &Path, project_name: &str) -> Result<PathBuf, String>`

### Project discovery and config validation (`lexicon-framework/src/main.rs`)

- `LexiconProjectConfig`
- `ProjectSection`
- `find_project_root(start_dir: &Path) -> Result<PathBuf, String>`
- `find_descendant_project_root(root: &Path) -> Result<Option<PathBuf>, String>`
- `visit_descendants(root: &Path, found: &mut Option<PathBuf>, current: &Path) -> Result<(), String>`
- `resolve_project_directory(project_root: &Path, configured: &str) -> Result<PathBuf, String>`
- `configured_sources_directory(project_root: &Path) -> Result<PathBuf, String>`
- `generate_source_scaffold(source_name: &str, protocol: &str) -> Result<(), String>`

## Actual init call chain

```text
main
→ Cli::parse
→ RootCommand::Init
→ dispatch
→ initialize_project(parent_path, project_name)
→ validate_project_name
→ canonicalize parent and inspect ancestor lexicon.toml markers
→ stage temp dir beside parent
→ create sources/
→ serialize TOML config
→ rename staged dir to final project directory
```

This is the actual flow in the checked-in code:

```rust
Some(RootCommand::Init(command)) => {
    let project_root = initialize_project(&command.parent_path, &command.project_name)?;
    println!("[lexicon] Initialized project '{}' at {}", command.project_name, project_root.display());
    Ok(())
}
```

And the filesystem mutation is completed here:

```rust
pub fn initialize_project(parent_path: &Path, project_name: &str) -> Result<PathBuf, String> {
    validate_project_name(project_name)?;
    if !parent_path.exists() {
        return Err(format!("parent path '{}' does not exist", parent_path.display()));
    }
    if !parent_path.is_dir() {
        return Err(format!("parent path '{}' is not a directory", parent_path.display()));
    }

    let canonical_parent = parent_path
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize parent path '{}': {error}", parent_path.display()))?;

    let project_directory = canonical_parent.join(project_name);
    if project_directory.exists() {
        return Err(format!("project '{}' already exists at {}", project_name, project_directory.display()));
    }

    let staging = tempfile::Builder::new()
        .prefix(&format!(".{project_name}.tmp-"))
        .tempdir_in(&canonical_parent)
        .map_err(|error| format!("failed to create temporary project: {error}"))?;

    fs::create_dir(staging.path().join("sources"))
        .map_err(|error| format!("failed to create sources directory: {error}"))?;

    let config = toml::Value::Table({
        let mut root = toml::map::Map::new();
        root.insert("schema_version".to_string(), toml::Value::Integer(1));

        let mut project = toml::map::Map::new();
        project.insert("name".to_string(), toml::Value::String(project_name.to_string()));
        project.insert("sources_directory".to_string(), toml::Value::String("sources".to_string()));
        root.insert("project".to_string(), toml::Value::Table(project));
        root
    });

    let toml_text = toml::to_string_pretty(&config)
        .map_err(|error| format!("failed to serialize project config: {error}"))?;

    fs::write(staging.path().join("lexicon.toml"), toml_text)
        .map_err(|error| format!("failed to write lexicon.toml: {error}"))?;

    let staging_path = staging.keep();
    if let Err(error) = fs::rename(&staging_path, &project_directory) {
        let _ = fs::remove_dir_all(&staging_path);
        return Err(format!("failed to finalize project '{}': {error}", project_directory.display()));
    }

    Ok(project_directory)
}
```

## Actual project-discovery call chain

```text
current directory
→ ancestor collection
→ nesting validation
→ descendant scan
→ TOML parsing
→ secure sources-directory resolution
```

This is the implemented path:

```rust
fn find_project_root(start_dir: &Path) -> Result<PathBuf, String> {
    let mut current = start_dir.to_path_buf();
    let mut ancestors = Vec::new();

    loop {
        let config_path = current.join("lexicon.toml");
        if config_path.is_file() {
            ancestors.push(current.clone());
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    if ancestors.is_empty() {
        return Err("No Lexicon project found. The current directory is not inside a Lexicon project.".to_string());
    }

    if ancestors.len() > 1 {
        let outer = ancestors.last().cloned().expect("ancestor list should have outermost root");
        let nested = ancestors.first().cloned().expect("ancestor list should at least contain one project root");
        return Err(format!(
            "Nested Lexicon project detected.\nOuter project: {}\nNested project: {}\nMove the nested project outside the outer project, or remove its lexicon.toml if it should belong to the outer project, then rerun.\nNo changes were made.",
            outer.display(),
            nested.display()
        ));
    }

    let root = ancestors[0].clone();
    let descendant = find_descendant_project_root(&root)?;
    if let Some(nested_root) = descendant {
        return Err(format!(
            "Nested Lexicon project detected.\nOuter project: {}\nNested project: {}\nMove the nested project outside the outer project, or remove its lexicon.toml if it should belong to the outer project, then rerun.\nNo changes were made.",
            root.display(),
            nested_root.display()
        ));
    }

    Ok(root)
}
```

The descendant scan and symlink guard are implemented as:

```rust
fn visit_descendants(root: &Path, found: &mut Option<PathBuf>, current: &Path) -> Result<(), String> {
    for entry in fs::read_dir(current)
        .map_err(|error| format!("failed to read {}: {error}", current.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read directory entry in {}: {error}", current.display()))?;
        let path = entry.path();

        if path.is_symlink() {
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

    Ok(())
}
```

The secure `sources_directory` resolution checks the configured path component by component and rejects symlink escapes before a nonexistent final child is accepted:

```rust
fn resolve_project_directory(project_root: &Path, configured: &str) -> Result<PathBuf, String> {
    if configured.trim().is_empty() {
        return Err("sources_directory must not be empty".to_string());
    }

    let canonical_root = project_root
        .canonicalize()
        .map_err(|error| format!("failed to resolve project root: {error}"))?;

    let mut resolved = canonical_root.clone();
    for component in Path::new(configured).components() {
        match component {
            Component::Normal(name) => {
                let next = resolved.join(name);
                match fs::symlink_metadata(&next) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        let target = next.canonicalize().map_err(|error| {
                            format!("failed to resolve '{}': {error}", next.display())
                        })?;
                        if !target.starts_with(&canonical_root) {
                            return Err(format!(
                                "sources_directory '{}' escapes the project root",
                                configured
                            ));
                        }
                        resolved = target;
                    }
                    Ok(_) => {
                        resolved = next;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        resolved = next;
                    }
                    Err(error) => {
                        return Err(format!(
                            "failed to inspect '{}': {error}",
                            next.display()
                        ));
                    }
                }
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "sources_directory '{}' must be a relative project path",
                    configured
                ));
            }
        }
    }

    if !resolved.starts_with(&canonical_root) {
        return Err(format!(
            "sources_directory '{}' escapes the project root",
            configured
        ));
    }
    if resolved.exists() && !resolved.is_dir() {
        return Err(format!(
            "sources_directory '{}' is not a directory",
            resolved.display()
        ));
    }
    Ok(resolved)
}
```

## Test function names mapped to implemented requirements

The actual visible tests in the checked-in code are:

- `cli::init::tests::parses_init_command_with_parent_path_and_project_name`
  - Covers: CLI parsing and dispatch contract for `lexicon init <parent-path> <project-name>`.
- `cli::init::tests::rejects_unsafe_project_names`
  - Covers: empty, parent traversal, separator, and invalid project-name rejection.
- `cli::init::tests::initializes_project_directory_and_toml`
  - Covers: exact project directory creation and required TOML fields.
- `cli::init::tests::does_not_delete_stale_pid_style_temp_directory`
  - Covers: safe temp staging behavior; existing unrelated temp dir is not removed.
- `cli::init::tests::successful_init_leaves_no_temp_directory`
  - Covers: cleanup of task-created staging after success.
- `tests::configured_sources_directory_rejects_symlink_escape`
  - Covers: rejecting a configured source path that escapes the project via a symlink.
- `tests::configured_sources_directory_rejects_escaping_symlink_then_missing_child`
  - Covers: escaping symlink followed by a nonexistent child.
- `tests::generated_impl_manifest_uses_new_portable_core_tag`
  - Covers: generated scaffold content remains portable and does not embed the local path.
- `tests::generated_http_template_uses_context_based_acquire_contract`
  - Covers: source scaffold content is generated with the current HTTP acquisition contract.

The repo does not currently include explicit executable tests for every single requirement listed in the original task description. The most visible gaps are the explicit descendant-pruning tests, nested descendant detection under a wider tree, and the exact `[lexicon]` prefix/duplicate-output assertions across the public CLI and framework path.

## Verification commands and results

### Test suite

Command run:

```bash
cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

Result:

- `lexicon-cli`: 10 passed, 0 failed
- `lexicon-framework`: 4 passed, 0 failed
- core framework tests: 1 passed, 0 failed
- total: all targeted tests passed

### Public CLI verification

Command run in a fresh temporary directory:

```bash
/tmp/.../lexicon-cli init "$tmp" demo-project
cd "$tmp/demo-project"
LEXICON_FRAMEWORK_PATH=/workspaces/lexicon/target/debug/lexicon-framework /workspaces/lexicon/target/debug/lexicon-cli source new example-source
```

Observed result:

```text
[lexicon] Initialized project 'demo-project' at /tmp/.../demo-project
Created source scaffold for 'example-source' at /tmp/.../demo-project/sources/example-source
Files to edit next:
  - ...
Invoked framework scaffold for source 'example-source' using protocol 'http'
```

This confirms the public CLI reaches the real framework scaffold path for source creation in a fresh project.

## Remaining gaps

- The source-create output path still does not fully normalize to the exact public `[lexicon]` message contract expected by the task description. The framework path and CLI path still emit more than one success-style line when the scaffold command is used in practice.
- The repo does not currently contain explicit tests covering every single numbered requirement from the original task list, especially the descendant pruning and exact output-prefix assertions.
- The project root detection and source-directory validation logic are present and tested for the critical escape cases, but the full terminal-output contract remains partially unclosed.

## Confirmation

The implementation work was limited to the Lexicon source code and the task report itself. No MZA, bundle, installer, or unrelated runtime behavior was modified as part of this task.
(