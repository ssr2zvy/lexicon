use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct LexiconProjectConfig {
    schema_version: Option<u32>,
    project: Option<ProjectSection>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SourceTomlDocument {
    schema_version: u32,
    source: SourceTomlSection,
}

#[derive(Debug, Serialize, Deserialize)]
struct SourceTomlSection {
    name: String,
    protocol: String,
}

#[derive(Debug, Deserialize)]
struct ProjectSection {
    name: Option<String>,
    sources_directory: Option<String>,
}

pub mod commands {
    use super::*;
    use std::path::{Path, PathBuf};

    #[derive(Debug)]
    pub struct InitResult {
        pub project_directory: PathBuf,
    }

    #[derive(Debug)]
    pub struct SourceCreateResult {
        pub source_name: String,
        pub protocol: String,
        pub protocol_dir: PathBuf,
        pub created_files: Vec<PathBuf>,
    }

    #[derive(Debug)]
    pub struct SourceBuildResult {
        pub source_name: String,
        pub protocol: String,
        pub get_runtime: PathBuf,
        pub process_runtime: PathBuf,
    }

    pub fn init(parent_path: &Path, project_name: &str) -> Result<InitResult, String> {
        let project_directory = initialize_project(parent_path, project_name)?;
        Ok(InitResult { project_directory })
    }

    pub fn source_create(source_name: &str, protocol: &str) -> Result<SourceCreateResult, String> {
        generate_source_scaffold(source_name, protocol)
    }

    pub fn source_build(source_name: &str, protocol: &str) -> Result<SourceBuildResult, String> {
        build_source(source_name, protocol)
    }
}

fn validate_project_name(project_name: &str) -> Result<(), String> {
    if project_name.trim().is_empty() {
        return Err("project name cannot be empty".to_string());
    }

    if project_name == "." || project_name == ".." {
        return Err(format!(
            "invalid project name '{}': use a simple directory name",
            project_name
        ));
    }

    let path = Path::new(project_name);
    if path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                Component::RootDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "invalid project name '{}': use a single directory name without separators or parent traversal",
            project_name
        ));
    }

    if path.components().any(|c| matches!(c, Component::CurDir)) {
        return Err(format!(
            "invalid project name '{}': use a single directory name without separators or parent traversal",
            project_name
        ));
    }

    if project_name.contains(['/', '\\']) {
        return Err(format!(
            "invalid project name '{}': use a single directory name without separators or parent traversal",
            project_name
        ));
    }

    Ok(())
}

fn initialize_project(parent_path: &Path, project_name: &str) -> Result<PathBuf, String> {
    validate_project_name(project_name)?;

    if !parent_path.exists() {
        return Err(format!(
            "parent path '{}' does not exist",
            parent_path.display()
        ));
    }
    if !parent_path.is_dir() {
        return Err(format!(
            "parent path '{}' is not a directory",
            parent_path.display()
        ));
    }

    let canonical_parent = parent_path.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize parent path '{}': {error}",
            parent_path.display()
        )
    })?;

    let mut existing_marker = None;
    for ancestor in canonical_parent.ancestors() {
        let marker = ancestor.join("lexicon.toml");
        if marker.is_file() {
            existing_marker = Some(ancestor.to_path_buf());
            break;
        }
    }

    if let Some(marker_root) = existing_marker {
        return Err(format!(
            "Nested Lexicon project detected.\nOuter project: {}\nNested project: {}\nMove the nested project outside the outer project, or remove its lexicon.toml if it should belong to the outer project, then rerun.\nNo changes were made.",
            marker_root.display(),
            canonical_parent.join(project_name).display()
        ));
    }

    let project_directory = canonical_parent.join(project_name);
    if project_directory.exists() {
        return Err(format!(
            "project '{}' already exists at {}",
            project_name,
            project_directory.display()
        ));
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
        project.insert(
            "name".to_string(),
            toml::Value::String(project_name.to_string()),
        );
        project.insert(
            "sources_directory".to_string(),
            toml::Value::String("sources".to_string()),
        );
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
        return Err(format!(
            "failed to finalize project '{}': {error}",
            project_directory.display()
        ));
    }

    Ok(project_directory)
}

fn generate_source_scaffold(
    source_name: &str,
    protocol: &str,
) -> Result<commands::SourceCreateResult, String> {
    validate_source_name(source_name)?;
    validate_protocol(protocol)?;

    let project_root = find_project_root(
        &env::current_dir()
            .map_err(|error| format!("failed to determine current directory: {error}"))?,
    )?;
    let source_root = configured_sources_directory(&project_root)?;
    let source_dir = source_root.join(source_name);
    let protocol_dir = source_dir.join(protocol);

    if source_dir.exists() {
        return Err(format!(
            "source '{}' already exists at {}",
            source_name,
            source_dir.display()
        ));
    }

    fs::create_dir_all(&source_root)
        .map_err(|error| format!("failed to create {}: {error}", source_root.display()))?;

    let staging = tempfile::Builder::new()
        .prefix(&format!("{source_name}-"))
        .tempdir_in(&source_root)
        .map_err(|error| {
            format!(
                "failed to create staging directory in {}: {error}",
                source_root.display()
            )
        })?;
    let staging_path = staging.path().to_path_buf();

    let directories = [
        Path::new("data/raw"),
        Path::new("data/processed"),
        Path::new("get-raw-data/sessions"),
        Path::new("get-raw-data/get-raw-data-impl/src"),
        Path::new("get-raw-data/runtime"),
        Path::new("process-data/sessions"),
        Path::new("process-data/process-data-impl/src"),
        Path::new("process-data/process-data-impl/processing"),
        Path::new("process-data/runtime"),
    ];

    for directory in &directories {
        let path = staging_path.join(directory);
        fs::create_dir_all(&path)
            .map_err(|error| format!("failed to create directory {}: {error}", path.display()))?;
    }

    let files = [
        ("source.toml", format_source_toml(source_name, protocol)),
        ("discovery.md", format_discovery_markdown(source_name)),
        (
            "data/raw/.gitkeep",
            "# generated by lexicon source create\n".to_string(),
        ),
        (
            "data/processed/.gitkeep",
            "# generated by lexicon source create\n".to_string(),
        ),
        (
            "get-raw-data/session_status.json",
            format_session_status_json(source_name, "get-raw-data"),
        ),
        (
            "process-data/session_status.json",
            format_session_status_json(source_name, "process-data"),
        ),
        (
            "get-raw-data/get-raw-data-impl/Cargo.toml",
            format_impl_cargo_toml(&format!("{source_name}-get-raw-data")),
        ),
        (
            "get-raw-data/get-raw-data-impl/Cargo.lock",
            format_cargo_lockfile(),
        ),
        (
            "get-raw-data/get-raw-data-impl/src/main.rs",
            format_get_raw_data_main(source_name),
        ),
        (
            "get-raw-data/runtime/.gitignore",
            "*\n!.gitignore\n".to_string(),
        ),
        (
            "process-data/process-data-impl/Cargo.toml",
            format_impl_cargo_toml(&format!("{source_name}-process-data")),
        ),
        (
            "process-data/process-data-impl/Cargo.lock",
            format_cargo_lockfile(),
        ),
        (
            "process-data/process-data-impl/src/main.rs",
            format_process_data_main(source_name),
        ),
        (
            "process-data/runtime/.gitignore",
            "*\n!.gitignore\n".to_string(),
        ),
    ];

    for (relative_path, contents) in &files {
        let path = staging_path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create parent directory for {}: {error}",
                    path.display()
                )
            })?;
        }
        fs::write(&path, contents)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }

    finalize_source_staging(staging, &protocol_dir)?;

    let output_files = [
        "source.toml",
        "discovery.md",
        "get-raw-data/get-raw-data-impl/src/main.rs",
        "process-data/process-data-impl/src/main.rs",
    ];
    let created_files: Vec<PathBuf> = output_files.iter().map(|f| protocol_dir.join(f)).collect();

    Ok(commands::SourceCreateResult {
        source_name: source_name.to_string(),
        protocol: protocol.to_string(),
        protocol_dir,
        created_files,
    })
}

fn build_source(source_name: &str, protocol: &str) -> Result<commands::SourceBuildResult, String> {
    validate_source_name(source_name)?;
    validate_protocol(protocol)?;

    let project_root = find_project_root(
        &env::current_dir()
            .map_err(|error| format!("failed to determine current directory: {error}"))?,
    )?;
    let sources_root = configured_sources_directory(&project_root)?;
    let source_root = sources_root.join(source_name);
    let protocol_root = source_root.join(protocol);

    if !source_root.exists() {
        return Err(format!("source '{}' does not exist", source_name));
    }
    if !source_root.is_dir() {
        return Err(format!("source '{}' does not exist", source_name));
    }
    if !protocol_root.exists() {
        return Err(format!(
            "protocol '{}' does not exist for source '{}'",
            protocol, source_name
        ));
    }
    if !protocol_root.is_dir() {
        return Err(format!(
            "protocol '{}' does not exist for source '{}'",
            protocol, source_name
        ));
    }

    let source_toml = protocol_root.join("source.toml");
    let _source_doc = load_source_metadata(&source_toml, source_name, protocol)?;
    let get_manifest = protocol_root.join("get-raw-data/get-raw-data-impl/Cargo.toml");
    let process_manifest = protocol_root.join("process-data/process-data-impl/Cargo.toml");
    if !get_manifest.is_file() {
        return Err("missing get-raw-data implementation manifest".to_owned());
    }
    if !process_manifest.is_file() {
        return Err("missing process-data implementation manifest".to_owned());
    }

    let get_executable = build_single_crate(&get_manifest, "get-raw-data")?;
    let process_executable = build_single_crate(&process_manifest, "process-data")?;

    let get_runtime_dir = protocol_root.join("get-raw-data/runtime");
    let process_runtime_dir = protocol_root.join("process-data/runtime");
    fs::create_dir_all(&get_runtime_dir)
        .map_err(|error| format!("failed to create {}: {error}", get_runtime_dir.display()))?;
    fs::create_dir_all(&process_runtime_dir).map_err(|error| {
        format!(
            "failed to create {}: {error}",
            process_runtime_dir.display()
        )
    })?;

    let get_staged = stage_runtime_file(&get_runtime_dir, &get_executable.path, "get-raw-data")?;
    let process_staged = stage_runtime_file(
        &process_runtime_dir,
        &process_executable.path,
        "process-data",
    )?;

    let mut get_backup = None;
    let mut process_backup = None;
    let get_final = get_runtime_dir.join(
        get_executable
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .as_ref(),
    );
    let process_final = process_runtime_dir.join(
        process_executable
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .as_ref(),
    );

    if get_final.exists() {
        get_backup = Some(move_to_backup(&get_final)?);
    }
    if process_final.exists() {
        process_backup = Some(move_to_backup(&process_final)?);
    }

    publish_runtime_transaction(
        &get_staged,
        &process_staged,
        &get_final,
        &process_final,
        get_backup.as_ref(),
        process_backup.as_ref(),
        |src, dst| fs::rename(src, dst),
        |path| fs::remove_file(path),
    )?;

    if let Some(path) = get_backup.as_ref() {
        let _ = fs::remove_file(path);
    }
    if let Some(path) = process_backup.as_ref() {
        let _ = fs::remove_file(path);
    }

    Ok(commands::SourceBuildResult {
        source_name: source_name.to_string(),
        protocol: protocol.to_string(),
        get_runtime: get_final
            .canonicalize()
            .unwrap_or_else(|_| get_final.clone()),
        process_runtime: process_final
            .canonicalize()
            .unwrap_or_else(|_| process_final.clone()),
    })
}

fn load_source_metadata(
    path: &Path,
    expected_name: &str,
    expected_protocol: &str,
) -> Result<SourceTomlDocument, String> {
    if !path.is_file() {
        return Err("source metadata does not match the requested source and protocol".to_owned());
    }

    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let parsed: SourceTomlDocument = toml::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;

    if parsed.schema_version != 1 {
        return Err("unsupported schema version".to_owned());
    }
    if parsed.source.name != expected_name {
        return Err("source metadata does not match the requested source and protocol".to_owned());
    }
    if parsed.source.protocol != expected_protocol {
        return Err("source metadata does not match the requested source and protocol".to_owned());
    }
    Ok(parsed)
}

pub struct BuiltExecutable {
    pub path: PathBuf,
    pub _target_dir: tempfile::TempDir,
}

pub fn build_single_crate(
    manifest_path: &Path,
    operation_name: &str,
) -> Result<BuiltExecutable, String> {
    let manifest = manifest_path
        .canonicalize()
        .map_err(|error| format!("failed to resolve {}: {error}", manifest_path.display()))?;
    ensure_lockfile_for_manifest(&manifest)?;

    let tempdir = tempfile::Builder::new()
        .prefix(&format!("lexicon-{operation_name}-build-"))
        .tempdir()
        .map_err(|error| format!("failed to create temporary build directory: {error}"))?;
    let target_dir = tempdir.path().to_path_buf();

    let output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&target_dir)
        .arg("--message-format=json-render-diagnostics")
        .output()
        .map_err(|_| {
            "[lexicon] ERROR: source build requires Cargo and a Rust development toolchain"
                .to_owned()
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            eprintln!("{stderr}");
        }
        return Err(format!("{} implementation build failed", operation_name));
    }

    let executable = select_executable_from_cargo_json(
        &String::from_utf8_lossy(&output.stdout),
        operation_name,
    )?;
    if !executable.is_file() {
        return Err(format!("{} implementation build failed", operation_name));
    }

    Ok(BuiltExecutable {
        path: executable,
        _target_dir: tempdir,
    })
}

fn ensure_lockfile_for_manifest(manifest_path: &Path) -> Result<(), String> {
    let status = Command::new("cargo")
        .arg("generate-lockfile")
        .arg("--manifest-path")
        .arg(manifest_path)
        .status()
        .map_err(|error| {
            format!(
                "failed to generate lockfile for {}: {error}",
                manifest_path.display()
            )
        })?;

    if !status.success() {
        return Err(format!(
            "failed to generate Cargo lockfile for {}",
            manifest_path.display()
        ));
    }
    Ok(())
}

pub fn select_executable_from_cargo_json(
    cargo_output: &str,
    operation_name: &str,
) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();

    for line in cargo_output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };

        if value.get("reason").and_then(|item| item.as_str()) != Some("compiler-artifact") {
            continue;
        }

        let target = value.get("target").cloned().unwrap_or_default();
        let kinds = target
            .get("kind")
            .and_then(|item| item.as_array())
            .cloned()
            .unwrap_or_default();
        let is_bin = kinds.iter().any(|item| item.as_str() == Some("bin"));
        let is_lib = kinds.iter().any(|item| item.as_str() == Some("lib"));
        if !is_bin || is_lib {
            continue;
        }

        let target_name = target
            .get("name")
            .and_then(|item| item.as_str())
            .unwrap_or("");
        let package_id = value
            .get("package_id")
            .and_then(|item| item.as_str())
            .unwrap_or("");
        let matches_requested =
            target_name.contains(operation_name) || package_id.contains(operation_name);
        if !matches_requested {
            continue;
        }

        let executable = value.get("executable").and_then(|item| item.as_str());
        let Some(path) = executable else {
            return Err(format!("{} implementation build failed", operation_name));
        };
        let candidate = PathBuf::from(path);
        if candidate.file_name().is_some() && !candidate.ends_with(".d") {
            candidates.push(candidate);
        }
    }

    match candidates.len() {
        0 => Err(format!("{} implementation build failed", operation_name)),
        1 => Ok(candidates[0].clone()),
        _ => Err(format!(
            "{} implementation build failed: multiple executable artifacts matched {}",
            operation_name, operation_name
        )),
    }
}

pub fn stage_runtime_file(
    runtime_dir: &Path,
    source_executable: &Path,
    operation_name: &str,
) -> Result<PathBuf, String> {
    let executable_name = source_executable
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| {
            source_executable
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or(operation_name)
        });

    let staging_file = tempfile::Builder::new()
        .prefix(&format!(".{executable_name}.staging-"))
        .tempfile_in(runtime_dir)
        .map_err(|error| {
            format!(
                "failed to create staging file in {}: {error}",
                runtime_dir.display()
            )
        })?;
    let staged = staging_file.path().to_path_buf();

    fs::copy(source_executable, &staged)
        .map_err(|error| format!("failed to stage {}: {error}", runtime_dir.display()))?;
    let metadata = fs::metadata(&staged)
        .map_err(|error| format!("failed to inspect staged {}: {error}", staged.display()))?;
    if !metadata.is_file() {
        let _ = fs::remove_file(&staged);
        return Err(format!("{} implementation build failed", operation_name));
    }

    let _ = staging_file
        .persist(&staged)
        .map_err(|error| format!("failed to persist staged {}: {error}", staged.display()));
    Ok(staged)
}

pub fn move_to_backup(path: &Path) -> Result<PathBuf, String> {
    let unique = format!(
        ".backup-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let backup = path.parent().unwrap().join(unique);
    fs::rename(path, &backup)
        .map_err(|error| format!("failed to create backup for {}: {error}", path.display()))?;
    Ok(backup)
}

pub fn publish_runtime_transaction<F, R>(
    get_staged: &Path,
    process_staged: &Path,
    get_final: &Path,
    process_final: &Path,
    get_backup: Option<&PathBuf>,
    process_backup: Option<&PathBuf>,
    rename: F,
    remove: R,
) -> Result<(), String>
where
    F: Fn(&Path, &Path) -> std::io::Result<()>,
    R: Fn(&Path) -> std::io::Result<()>,
{
    if let Err(error) = rename(get_staged, get_final) {
        restore_runtime_after_failure(
            get_final,
            get_backup,
            get_staged,
            process_staged,
            process_final,
            process_backup,
            &remove,
        )?;
        return Err(format!(
            "source runtime publication failed; previous runtimes were restored: {error}"
        ));
    }
    if let Err(error) = rename(process_staged, process_final) {
        let _ = remove(get_final);
        restore_runtime_after_failure(
            get_final,
            get_backup,
            get_staged,
            process_staged,
            process_final,
            process_backup,
            &remove,
        )?;
        return Err(format!(
            "source runtime publication failed; previous runtimes were restored: {error}"
        ));
    }
    Ok(())
}

fn restore_runtime_after_failure<F>(
    get_final: &Path,
    get_backup: Option<&PathBuf>,
    get_staged: &Path,
    process_staged: &Path,
    process_final: &Path,
    process_backup: Option<&PathBuf>,
    remove: &F,
) -> Result<(), String>
where
    F: Fn(&Path) -> std::io::Result<()>,
{
    let _ = remove(get_staged);
    let _ = remove(process_staged);
    let _ = remove(get_final);
    let _ = remove(process_final);
    if let Some(path) = get_backup {
        fs::rename(path, get_final)
            .map_err(|error| format!("failed to restore get runtime: {error}"))?;
    }
    if let Some(path) = process_backup {
        fs::rename(path, process_final)
            .map_err(|error| format!("failed to restore process runtime: {error}"))?;
    }
    Ok(())
}

fn finalize_source_staging(staging: tempfile::TempDir, source_dir: &Path) -> Result<(), String> {
    let staging_path = staging.path().to_path_buf();
    let source_parent = source_dir.parent().ok_or_else(|| {
        format!(
            "failed to resolve parent directory for {}",
            source_dir.display()
        )
    })?;

    if !source_parent.exists() {
        fs::create_dir_all(source_parent)
            .map_err(|error| format!("failed to create {}: {error}", source_parent.display()))?;
    }

    let rename_result = fs::rename(&staging_path, source_dir);

    if let Err(error) = rename_result {
        let _ = fs::remove_dir_all(&staging_path);
        let _ = fs::remove_dir(source_parent);
        drop(staging);
        return Err(format!(
            "failed to rename {} to {}: {error}",
            staging_path.display(),
            source_dir.display()
        ));
    }

    drop(staging);
    Ok(())
}

fn validate_source_name(source_name: &str) -> Result<(), String> {
    if source_name.trim().is_empty() {
        return Err("source name cannot be empty".to_string());
    }
    if source_name == "." || source_name == ".." {
        return Err(format!(
            "invalid source name '{}': use a simple source identifier",
            source_name
        ));
    }
    if Path::new(source_name).is_absolute() {
        return Err(format!(
            "invalid source name '{}': source names must be relative and not absolute",
            source_name
        ));
    }
    if source_name.contains(['/', '\\'])
        || source_name
            .split(['/', '\\'])
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(format!(
            "invalid source name '{}': source names must be a single path segment",
            source_name
        ));
    }
    Ok(())
}

fn validate_protocol(protocol: &str) -> Result<(), String> {
    if protocol.eq_ignore_ascii_case("http") {
        Ok(())
    } else {
        Err(format!(
            "unsupported protocol '{}'; only 'http' is currently supported",
            protocol
        ))
    }
}

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
        return Err(
            "No Lexicon project found. The current directory is not inside a Lexicon project."
                .to_string(),
        );
    }

    if ancestors.len() > 1 {
        let outer = ancestors
            .last()
            .cloned()
            .expect("ancestor list should have outermost root");
        let nested = ancestors
            .first()
            .cloned()
            .expect("ancestor list should at least contain one project root");
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

fn find_descendant_project_root(root: &Path) -> Result<Option<PathBuf>, String> {
    let mut found = None;
    visit_descendants(root, &mut found, root)?;
    Ok(found)
}

// Lexicon project roots are only legal under user-managed project trees. Generated and
// data-heavy directories are pruned before descendant discovery to avoid walking unbounded
// build or cache trees and to keep nested-root detection deterministic.
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

fn visit_descendants(
    root: &Path,
    found: &mut Option<PathBuf>,
    current: &Path,
) -> Result<(), String> {
    let mut entries = fs::read_dir(current)
        .map_err(|error| format!("failed to read {}: {error}", current.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "failed to read directory entry in {}: {error}",
                current.display()
            )
        })?;
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

    Ok(())
}

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
                        return Err(format!("failed to inspect '{}': {error}", next.display()));
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

fn configured_sources_directory(project_root: &Path) -> Result<PathBuf, String> {
    let config_path = project_root.join("lexicon.toml");
    let contents = fs::read_to_string(&config_path)
        .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
    let parsed: LexiconProjectConfig = toml::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))?;

    if parsed.schema_version != Some(1) {
        return Err(format!(
            "unsupported schema_version in {}: expected 1 but found {:?}",
            config_path.display(),
            parsed.schema_version
        ));
    }

    let project = parsed
        .project
        .as_ref()
        .ok_or_else(|| format!("missing [project] section in {}", config_path.display()))?;
    let project_name = project
        .name
        .as_deref()
        .ok_or_else(|| format!("missing project.name in {}", config_path.display()))?
        .trim();
    if project_name.is_empty()
        || project_name == "."
        || project_name == ".."
        || project_name.contains(['/', '\\'])
    {
        return Err(format!(
            "invalid project.name '{}' in {}",
            project_name,
            config_path.display()
        ));
    }

    let configured = project.sources_directory.as_deref().unwrap_or("sources");

    let path = Path::new(configured);
    if path.is_absolute() {
        return Err(format!(
            "invalid sources_directory '{}' in {}: must be a relative path",
            configured,
            config_path.display()
        ));
    }
    if path.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!(
            "invalid sources_directory '{}' in {}: must remain within the project root",
            configured,
            config_path.display()
        ));
    }

    resolve_project_directory(project_root, configured)
}

fn format_source_toml(source_name: &str, protocol: &str) -> String {
    let document = SourceTomlDocument {
        schema_version: 1,
        source: SourceTomlSection {
            name: source_name.to_owned(),
            protocol: protocol.to_owned(),
        },
    };

    toml::to_string_pretty(&document)
        .unwrap_or_else(|error| panic!("failed to serialize source.toml: {error}"))
}

fn format_discovery_markdown(source_name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {source_name}\n\n"));
    out.push_str("## Source description\n\n");
    out.push_str("Describe the source and the data it produces.\n\n");
    out.push_str("## Discovery method\n\n");
    out.push_str("Document how this source was discovered and why it belongs in this project.\n\n");
    out.push_str("## Acquisition endpoint or location\n\n");
    out.push_str("Record the upstream endpoint, dataset, or location used for acquisition.\n\n");
    out.push_str("## Why HTTP is the correct acquisition protocol\n\n");
    out.push_str("Explain why HTTP is the correct protocol for this source and how it matches the project contract.\n\n");
    out.push_str("## Required authentication or access conditions\n\n");
    out.push_str("List any required credentials, access restrictions, or network constraints.\n\n");
    out.push_str("## Attribution and usage notes\n\n");
    out.push_str("Capture attribution, licensing, and usage guidance for this source.\n\n");
    out.push_str("## Operational observations\n\n");
    out.push_str("Record operational notes, expected cadence, and troubleshooting observations.\n");
    out
}

fn format_impl_cargo_toml(package_name: &str) -> String {
    let mut out = String::new();
    out.push_str("[package]\n");
    out.push_str(&format!("name = \"{package_name}\"\n"));
    out.push_str("version = \"0.1.0\"\n");
    out.push_str("edition = \"2024\"\n\n");
    out.push_str("[dependencies]\n");
    out.push_str("lexicon-framework-core = {\n");
    out.push_str("    git = \"https://github.com/ssr2zvy/lexicon\",\n");
    out.push_str("    tag = \"v0.1.2\"\n");
    out.push_str("}\n");
    out
}

fn to_pascal_case(source_name: &str) -> String {
    let mut out = String::new();
    let mut capitalize_next = true;

    for ch in source_name.chars() {
        if ch == '-' || ch == '_' || ch == ' ' {
            capitalize_next = true;
            continue;
        }
        if capitalize_next {
            out.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            out.extend(ch.to_lowercase());
        }
    }

    if out.is_empty() {
        "Source".to_string()
    } else {
        out
    }
}

fn format_get_raw_data_main(source_name: &str) -> String {
    let source_type = to_pascal_case(source_name);
    let mut out = String::new();
    out.push_str("use lexicon_framework_core::{\n");
    out.push_str("    run_http_source,\n");
    out.push_str("    HttpAcquisition,\n");
    out.push_str("    HttpAcquisitionContext,\n");
    out.push_str("};\n\n");
    out.push_str(&format!("struct {source_type};\n\n"));
    out.push_str(&format!("impl HttpAcquisition for {source_type} {{\n"));
    out.push_str(&format!(
        "    fn acquire(&self, context: &mut HttpAcquisitionContext) -> Result<(), String> {{\n"
    ));
    out.push_str("        let _ = context;\n");
    out.push_str("        todo!(\"implement HTTP acquisition\")\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");
    out.push_str("fn main() {\n");
    out.push_str(&format!("    let source = {source_type};\n"));
    out.push_str("    if let Err(error) = run_http_source(source) {\n");
    out.push_str("        eprintln!(\"[lexicon] ERROR: {error}\");\n");
    out.push_str("        std::process::exit(1);\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

fn format_process_data_main(source_name: &str) -> String {
    let mut out = String::new();
    out.push_str("fn main() {\n");
    out.push_str(&format!(
        "    println!(\"{source_name}: process-data scaffold is ready\");\n"
    ));
    out.push_str(
        "    println!(\"Edit the implementation to turn raw data into the source-specific SQLite dataset.\");\n",
    );
    out.push_str("}\n");
    out
}

fn format_session_status_json(source_name: &str, stage: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"schema_version\": 1,\n");
    out.push_str(&format!("  \"source_id\": \"{source_name}\",\n"));
    out.push_str(&format!("  \"stage\": \"{stage}\",\n"));
    out.push_str("  \"status\": \"initialized\",\n");
    out.push_str("  \"last_updated\": null\n");
    out.push_str("}\n");
    out
}

fn format_cargo_lockfile() -> String {
    let mut out = String::new();
    out.push_str("# This file is automatically @generated by Cargo.\n");
    out.push_str("# It is not intended for manual editing.\n");
    out.push_str("version = 3\n");
    out
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::path::PathBuf;

    use super::commands::{init, source_create};
    use super::{
        build_single_crate, configured_sources_directory, finalize_source_staging,
        find_descendant_project_root, find_project_root, format_get_raw_data_main,
        format_impl_cargo_toml, format_source_toml, move_to_backup, publish_runtime_transaction,
        restore_runtime_after_failure, select_executable_from_cargo_json, stage_runtime_file,
        validate_protocol, validate_source_name,
    };

    #[test]
    fn generated_source_toml_matches_required_contract() {
        let source = format_source_toml("example-source", "http");

        assert!(source.contains("schema_version = 1"));
        assert!(source.contains("[source]"));
        assert!(source.contains("name = \"example-source\""));
        assert!(source.contains("protocol = \"http\""));
        assert!(!source.contains("id = \"example-source\""));
    }

    #[test]
    fn validate_source_name_and_protocol_require_safe_values() {
        assert!(validate_source_name("example-source").is_ok());
        assert!(validate_source_name("/bad").is_err());
        assert!(validate_source_name(".").is_err());
        assert!(validate_source_name("..").is_err());
        assert!(validate_protocol("http").is_ok());
        assert!(validate_protocol("browser").is_err());
    }

    #[test]
    fn generated_impl_manifest_uses_new_portable_core_tag() {
        let manifest = format_impl_cargo_toml("example-source-get-raw-data");

        assert!(manifest.contains("name = \"example-source-get-raw-data\""));
        assert!(manifest.contains("git = \"https://github.com/ssr2zvy/lexicon\""));
        assert!(manifest.contains("tag = \"v0.1.2\""));
        assert!(!manifest.contains("tag = \"v0.1.0\""));
        assert!(!manifest.contains("/workspaces/lexicon"));
    }

    #[test]
    fn generated_http_template_uses_context_based_acquire_contract() {
        let source = format_get_raw_data_main("example-source");

        assert!(source.contains("HttpAcquisitionContext"));
        assert!(source.contains("fn acquire(&self, context: &mut HttpAcquisitionContext)"));
        assert!(source.contains("if let Err(error) = run_http_source(source)"));
        assert!(source.contains("let source = ExampleSource;"));
    }

    #[test]
    fn configured_sources_directory_rejects_symlink_escape() {
        let root =
            std::env::temp_dir().join(format!("lexicon-sources-symlink-{}", std::process::id()));
        let outside =
            std::env::temp_dir().join(format!("lexicon-sources-outside-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();

        let symlink_path = root.join("sources");
        std::os::unix::fs::symlink(&outside, &symlink_path).unwrap();
        fs::write(
            root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();

        let result = configured_sources_directory(&root);
        assert!(result.is_err(), "symlink escape should be rejected");

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn configured_sources_directory_rejects_escaping_symlink_then_missing_child() {
        let root = std::env::temp_dir().join(format!(
            "lexicon-sources-escape-child-{}",
            std::process::id()
        ));
        let outside = std::env::temp_dir().join(format!(
            "lexicon-sources-escape-outside-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();

        let link = root.join("link");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        fs::write(
            root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"link/nonexistent-child\"\n",
        )
        .unwrap();

        let result = configured_sources_directory(&root);
        assert!(
            result.is_err(),
            "escaped symlink path with missing child should be rejected"
        );

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn find_project_root_rejects_descendant_nested_project() {
        let root = std::env::temp_dir().join(format!("lexicon-nested-root-{}", std::process::id()));
        let nested = root.join("tools/inner");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"outer\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();
        fs::write(
            nested.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"inner\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();

        let result = find_project_root(&root);
        assert!(
            result.is_err(),
            "nested descendant project should be rejected"
        );
        let text = result.unwrap_err();
        assert!(text.contains("Outer project:"));
        assert!(text.contains("Nested project:"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn find_descendant_project_root_prunes_excluded_directories() {
        let root = std::env::temp_dir().join(format!("lexicon-prune-root-{}", std::process::id()));
        let raw = root.join("data/raw");
        let processed = root.join("data/processed");
        let nested = root.join("data/nested-project");
        let _ = fs::remove_dir_all(&root);

        fs::create_dir_all(&raw).unwrap();
        fs::create_dir_all(&processed).unwrap();
        fs::create_dir_all(&nested).unwrap();

        fs::write(
            root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"outer\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();
        fs::write(
            raw.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"raw\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();
        fs::write(
            processed.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"processed\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();
        fs::write(
            nested.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"nested\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();

        let result = find_descendant_project_root(&root).unwrap();
        assert_eq!(
            result,
            Some(nested),
            "data/raw and data/processed must be ignored while a real nested project under data/ is still detected"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn generated_discovery_markdown_contains_required_prompts() {
        let markdown = super::format_discovery_markdown("example-source");
        let required = [
            "# example-source",
            "## Source description",
            "Describe the source and the data it produces.",
            "## Discovery method",
            "Document how this source was discovered and why it belongs in this project.",
            "## Acquisition endpoint or location",
            "Record the upstream endpoint, dataset, or location used for acquisition.",
            "## Why HTTP is the correct acquisition protocol",
            "Explain why HTTP is the correct protocol for this source and how it matches the project contract.",
            "## Required authentication or access conditions",
            "List any required credentials, access restrictions, or network constraints.",
            "## Attribution and usage notes",
            "Capture attribution, licensing, and usage guidance for this source.",
            "## Operational observations",
            "Record operational notes, expected cadence, and troubleshooting observations.",
        ];

        for fragment in &required {
            assert!(
                markdown.contains(fragment),
                "discovery.md is missing required prompt: {fragment}\n---\n{markdown}"
            );
        }
    }

    #[test]
    fn selects_correct_binary_artifact_from_compiler_json() {
        let output = r#"{"reason":"compiler-artifact","package_id":"example-source-get-raw-data 0.1.0 (path+file:///tmp/example-source/http/get-raw-data/get-raw-data-impl)","target":{"kind":["bin"],"name":"example-source-get-raw-data"},"executable":"/tmp/example-source/http/get-raw-data/runtime/example-source-get-raw-data"}
{"reason":"compiler-artifact","target":{"kind":["lib"],"name":"example-source_get_raw_data"},"executable":"/tmp/lib.so"}
{"reason":"compiler-artifact","target":{"kind":["bin"],"name":"other-binary"},"executable":"/tmp/other-binary"}
"#;

        let result = select_executable_from_cargo_json(output, "get-raw-data").unwrap();
        assert_eq!(
            result,
            PathBuf::from(
                "/tmp/example-source/http/get-raw-data/runtime/example-source-get-raw-data"
            )
        );
    }

    #[test]
    fn ignores_unrelated_compiler_artifact_json() {
        let output = r#"{"reason":"compiler-artifact","package_id":"other-package 0.1.0 (path+file:///tmp/other)","target":{"kind":["bin"],"name":"other-binary"},"executable":"/tmp/other-binary"}
{"reason":"compiler-artifact","package_id":"example-source-process-data 0.1.0 (path+file:///tmp/example-source/http/process-data/process-data-impl)","target":{"kind":["bin"],"name":"example-source-process-data"},"executable":"/tmp/example-source-process-data"}
{"reason":"compiler-artifact","target":{"kind":["test"],"name":"test-suite"},"executable":"/tmp/test-suite"}
{"reason":"compiler-artifact","target":{"kind":["example"],"name":"example"},"executable":"/tmp/example"}
{"reason":"compiler-artifact","target":{"kind":["custom-build"],"name":"build-script"},"executable":"/tmp/build-script"}
"#;

        let result = select_executable_from_cargo_json(output, "get-raw-data").unwrap_err();
        assert!(result.contains("implementation build failed"));
    }

    #[test]
    fn rejects_missing_executable_in_compiler_artifact_json() {
        let output = r#"{"reason":"compiler-artifact","package_id":"example-source-get-raw-data 0.1.0 (path+file:///tmp/example-source/http/get-raw-data/get-raw-data-impl)","target":{"kind":["bin"],"name":"example-source-get-raw-data"}}
"#;

        let result = select_executable_from_cargo_json(output, "get-raw-data");
        assert!(
            result.is_err(),
            "missing executable path must fail the build"
        );
    }

    #[test]
    fn rejects_ambiguous_executable_selection_in_compiler_artifact_json() {
        let output = r#"{"reason":"compiler-artifact","package_id":"example-source-get-raw-data 0.1.0 (path+file:///tmp/example-source/http/get-raw-data/get-raw-data-impl)","target":{"kind":["bin"],"name":"example-source-get-raw-data"},"executable":"/tmp/first"}
{"reason":"compiler-artifact","package_id":"example-source-get-raw-data 0.1.0 (path+file:///tmp/example-source/http/get-raw-data/get-raw-data-impl)","target":{"kind":["bin"],"name":"example-source-get-raw-data"},"executable":"/tmp/second"}
"#;

        let result = select_executable_from_cargo_json(output, "get-raw-data");
        assert!(
            result.is_err(),
            "multiple matching executable artifacts must fail deterministically"
        );
    }

    #[test]
    fn stage_runtime_file_uses_randomized_unique_suffixes_in_runtime_directory() {
        use std::os::unix::fs::PermissionsExt;

        let root =
            std::env::temp_dir().join(format!("lexicon-runtime-staging-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let executable = root.join("example-source-process-data");
        fs::write(&executable, "binary\n").unwrap();
        let permissions = std::fs::Permissions::from_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();

        let stale_pid_path = root.join(format!(
            ".example-source-process-data.staging-{}",
            std::process::id()
        ));
        fs::write(&stale_pid_path, "stale-value\n").unwrap();

        let first = stage_runtime_file(&root, &executable, "process-data").unwrap();
        let second = stage_runtime_file(&root, &executable, "process-data").unwrap();

        assert_ne!(first, second, "randomized staging paths must differ");
        assert!(first.starts_with(&root));
        assert!(second.starts_with(&root));
        assert!(
            first
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".example-source-process-data.staging-")
        );
        assert!(
            second
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".example-source-process-data.staging-")
        );
        assert_ne!(
            first.file_name().unwrap().to_string_lossy().as_ref(),
            format!(
                ".example-source-process-data.staging-{}",
                std::process::id()
            )
        );
        assert!(
            stale_pid_path.exists(),
            "the stale PID-style file must remain untouched"
        );
        assert_eq!(
            fs::read_to_string(&stale_pid_path).unwrap(),
            "stale-value\n"
        );

        let _ = fs::remove_file(&first);
        let _ = fs::remove_file(&second);
        let _ = fs::remove_file(&stale_pid_path);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn build_single_crate_keeps_the_built_executable_available_after_return() {
        let root =
            std::env::temp_dir().join(format!("lexicon-build-artifact-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();

        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"temporary-build-check\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n",
        )
        .unwrap();
        fs::write(
            root.join("src/main.rs"),
            "fn main() { println!(\"ok\"); }\n",
        )
        .unwrap();

        let manifest = root.join("Cargo.toml");
        let artifact = build_single_crate(&manifest, "temporary-build-check").unwrap();

        assert!(artifact.path.is_file());
        assert!(artifact.path.exists());
        assert!(artifact.path.metadata().unwrap().is_file());
        assert!(
            artifact
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains("temporary-build-check")
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn publication_transaction_publishes_both_executables_successfully() {
        let root =
            std::env::temp_dir().join(format!("lexicon-publish-success-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let get_final = root.join("get-raw-data");
        let process_final = root.join("process-data");
        let get_staged = root.join(".get.staging");
        let process_staged = root.join(".process.staging");
        fs::write(&get_final, "old-get\n").unwrap();
        fs::write(&process_final, "old-process\n").unwrap();
        fs::write(&get_staged, "new-get\n").unwrap();
        fs::write(&process_staged, "new-process\n").unwrap();

        let get_backup = Some(move_to_backup(&get_final).unwrap());
        let process_backup = Some(move_to_backup(&process_final).unwrap());
        let result = publish_runtime_transaction(
            &get_staged,
            &process_staged,
            &get_final,
            &process_final,
            get_backup.as_ref(),
            process_backup.as_ref(),
            |src, dst| fs::rename(src, dst),
            |path| fs::remove_file(path),
        );

        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&get_final).unwrap(), "new-get\n");
        assert_eq!(fs::read_to_string(&process_final).unwrap(), "new-process\n");
        assert!(!get_staged.exists());
        assert!(!process_staged.exists());
        fs::remove_file(get_backup.as_ref().unwrap()).unwrap();
        fs::remove_file(process_backup.as_ref().unwrap()).unwrap();
        assert!(!get_backup.as_ref().unwrap().exists());
        assert!(!process_backup.as_ref().unwrap().exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn publication_transaction_backs_up_existing_executables() {
        let root =
            std::env::temp_dir().join(format!("lexicon-publish-backup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let get_final = root.join("get-raw-data");
        let process_final = root.join("process-data");
        fs::write(&get_final, "old-get\n").unwrap();
        fs::write(&process_final, "old-process\n").unwrap();

        let get_backup = move_to_backup(&get_final).unwrap();
        let process_backup = move_to_backup(&process_final).unwrap();

        assert!(get_backup.exists());
        assert!(process_backup.exists());
        assert_eq!(fs::read_to_string(&get_backup).unwrap(), "old-get\n");
        assert_eq!(
            fs::read_to_string(&process_backup).unwrap(),
            "old-process\n"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn framework_init_returns_typed_result_not_exit() {
        let parent = std::env::temp_dir().join(format!("lexicon-fw-init-{}", std::process::id()));
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(&parent).unwrap();

        let result = init(&parent, "my-project");
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.project_directory, parent.join("my-project"));
        assert!(info.project_directory.join("lexicon.toml").is_file());

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn framework_init_fails_with_error_not_exit_for_bad_name() {
        let parent =
            std::env::temp_dir().join(format!("lexicon-fw-init-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(&parent).unwrap();

        let result = init(&parent, "../evil");
        assert!(result.is_err(), "bad project name should return Err");

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn framework_source_create_fails_with_error_not_exit_for_bad_protocol() {
        let temp = std::env::temp_dir().join(format!("lexicon-fw-sc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();
        fs::write(
            temp.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();

        let orig_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp).unwrap();
        let result = source_create("example-source", "browser");
        std::env::set_current_dir(&orig_dir).unwrap();

        assert!(result.is_err(), "unsupported protocol should return Err");
        assert!(
            result.unwrap_err().contains("unsupported protocol"),
            "error must describe the unsupported protocol"
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn publication_failure_in_second_publish_restores_the_first_runtime() {
        let root = std::env::temp_dir().join(format!(
            "lexicon-publish-second-fail-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let get_final = root.join("get-raw-data");
        let process_final = root.join("process-data");
        let get_staged = root.join(".get.staging");
        let process_staged = root.join(".process.staging");
        fs::write(&get_final, "old-get\n").unwrap();
        fs::write(&process_final, "old-process\n").unwrap();
        let get_backup = Some(move_to_backup(&get_final).unwrap());
        let process_backup = Some(move_to_backup(&process_final).unwrap());
        fs::write(&get_final, "old-get\n").unwrap();
        fs::write(&process_final, "old-process\n").unwrap();
        fs::write(&get_staged, "new-get\n").unwrap();
        fs::write(&process_staged, "new-process\n").unwrap();

        let result = publish_runtime_transaction(
            &get_staged,
            &process_staged,
            &get_final,
            &process_final,
            get_backup.as_ref(),
            process_backup.as_ref(),
            |src, dst| {
                if src == process_staged {
                    Err(io::Error::new(
                        io::ErrorKind::Other,
                        "simulated second publish failure",
                    ))
                } else {
                    fs::rename(src, dst)
                }
            },
            |path| fs::remove_file(path),
        );

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&get_final).unwrap(), "old-get\n");
        assert_eq!(fs::read_to_string(&process_final).unwrap(), "old-process\n");
        assert!(!get_staged.exists());
        assert!(!process_staged.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn publication_failure_restores_both_previous_runtime_executables() {
        let root = std::env::temp_dir().join(format!(
            "lexicon-publish-both-restore-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let get_final = root.join("get-raw-data");
        let process_final = root.join("process-data");
        let get_staged = root.join(".get.staging");
        let process_staged = root.join(".process.staging");
        fs::write(&get_final, "old-get\n").unwrap();
        fs::write(&process_final, "old-process\n").unwrap();
        fs::write(&get_staged, "new-get\n").unwrap();
        fs::write(&process_staged, "new-process\n").unwrap();

        let get_backup = Some(move_to_backup(&get_final).unwrap());
        let process_backup = Some(move_to_backup(&process_final).unwrap());
        let result = publish_runtime_transaction(
            &get_staged,
            &process_staged,
            &get_final,
            &process_final,
            get_backup.as_ref(),
            process_backup.as_ref(),
            |src, dst| {
                if src == get_staged {
                    Err(io::Error::new(
                        io::ErrorKind::Other,
                        "simulated first publish failure",
                    ))
                } else {
                    fs::rename(src, dst)
                }
            },
            |path| fs::remove_file(path),
        );

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&get_final).unwrap(), "old-get\n");
        assert_eq!(fs::read_to_string(&process_final).unwrap(), "old-process\n");
        assert!(!get_staged.exists());
        assert!(!process_staged.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn transaction_cleanup_removes_staged_files_after_failure() {
        let root =
            std::env::temp_dir().join(format!("lexicon-staged-cleanup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let get_final = root.join("get-raw-data");
        let process_final = root.join("process-data");
        let get_staged = root.join(".get.staging");
        let process_staged = root.join(".process.staging");
        fs::write(&get_final, "old-get\n").unwrap();
        fs::write(&process_final, "old-process\n").unwrap();
        fs::write(&get_staged, "new-get\n").unwrap();
        fs::write(&process_staged, "new-process\n").unwrap();

        let _ = restore_runtime_after_failure(
            &get_final,
            Some(&move_to_backup(&get_final).unwrap()),
            &get_staged,
            &process_staged,
            &process_final,
            Some(&move_to_backup(&process_final).unwrap()),
            &|path| fs::remove_file(path),
        );

        assert!(!get_staged.exists());
        assert!(!process_staged.exists());
        assert!(get_final.exists());
        assert!(process_final.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn transaction_cleanup_removes_backup_files_after_success() {
        let root =
            std::env::temp_dir().join(format!("lexicon-backup-cleanup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let get_final = root.join("get-raw-data");
        let process_final = root.join("process-data");
        fs::write(&get_final, "old-get\n").unwrap();
        fs::write(&process_final, "old-process\n").unwrap();

        let get_backup = move_to_backup(&get_final).unwrap();
        let process_backup = move_to_backup(&process_final).unwrap();
        fs::write(&get_final, "new-get\n").unwrap();
        fs::write(&process_final, "new-process\n").unwrap();

        assert!(get_backup.exists());
        assert!(process_backup.exists());
        fs::remove_file(&get_backup).unwrap();
        fs::remove_file(&process_backup).unwrap();
        assert!(!get_backup.exists());
        assert!(!process_backup.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unrelated_runtime_files_remain_untouched() {
        let root =
            std::env::temp_dir().join(format!("lexicon-unrelated-keep-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let get_final = root.join("get-raw-data");
        let process_final = root.join("process-data");
        let unrelated = root.join("notes.txt");
        fs::write(&get_final, "old-get\n").unwrap();
        fs::write(&process_final, "old-process\n").unwrap();
        fs::write(&unrelated, "keep-me\n").unwrap();

        let result = restore_runtime_after_failure(
            &get_final,
            Some(&move_to_backup(&get_final).unwrap()),
            &root.join(".get.staging"),
            &root.join(".process.staging"),
            &process_final,
            Some(&move_to_backup(&process_final).unwrap()),
            &|path| fs::remove_file(path),
        );

        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&unrelated).unwrap(), "keep-me\n");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn gitignore_file_remains_untouched_after_runtime_restore() {
        let root =
            std::env::temp_dir().join(format!("lexicon-gitignore-restore-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let get_directory = root.join("get-raw-data");
        let process_directory = root.join("process-data");
        fs::create_dir_all(&get_directory).unwrap();
        fs::create_dir_all(&process_directory).unwrap();
        let get_file = get_directory.join("get-raw-data");
        let process_file = process_directory.join("process-data");
        fs::write(&get_file, "old-get\n").unwrap();
        fs::write(&process_file, "old-process\n").unwrap();
        fs::write(get_directory.join(".gitignore"), "*\n!.gitignore\n").unwrap();
        fs::write(process_directory.join(".gitignore"), "*\n!.gitignore\n").unwrap();

        let get_backup = Some(move_to_backup(&get_file).unwrap());
        let process_backup = Some(move_to_backup(&process_file).unwrap());
        fs::write(&get_file, "new-get\n").unwrap();
        fs::write(&process_file, "new-process\n").unwrap();
        fs::write(get_directory.join(".get.staging"), "staged\n").unwrap();
        fs::write(process_directory.join(".process.staging"), "staged\n").unwrap();

        let restore = restore_runtime_after_failure(
            &get_file,
            get_backup.as_ref(),
            &get_directory.join(".get.staging"),
            &process_directory.join(".process.staging"),
            &process_file,
            process_backup.as_ref(),
            &|path| fs::remove_file(path),
        );

        assert!(restore.is_ok());
        assert_eq!(
            fs::read_to_string(get_directory.join(".gitignore")).unwrap(),
            "*\n!.gitignore\n"
        );
        assert_eq!(
            fs::read_to_string(process_directory.join(".gitignore")).unwrap(),
            "*\n!.gitignore\n"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn finalize_source_staging_cleans_up_tempdir_when_rename_fails() {
        let root =
            std::env::temp_dir().join(format!("lexicon-stage-cleanup-{}", std::process::id()));
        let sources_dir = root.join("sources");
        let source_dir = sources_dir.join("example-source");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&sources_dir).unwrap();
        fs::write(
            root.join("lexicon.toml"),
            "schema_version = 1\n[project]\nname = \"demo\"\nsources_directory = \"sources\"\n",
        )
        .unwrap();

        let staging = tempfile::Builder::new()
            .prefix("example-source-")
            .tempdir_in(&sources_dir)
            .unwrap();
        let staging_path = staging.path().to_path_buf();
        fs::write(staging_path.join("source.toml"), "schema_version = 1\n").unwrap();
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("existing.txt"), "preserve-me\n").unwrap();

        let result = finalize_source_staging(staging, &source_dir);

        assert!(
            result.is_err(),
            "rename should fail when the final directory already exists"
        );
        assert!(
            !staging_path.exists(),
            "staging directory must be removed on rename failure"
        );
        assert!(
            source_dir.join("existing.txt").exists(),
            "existing content must remain untouched"
        );

        let _ = fs::remove_dir_all(&root);
    }
}
