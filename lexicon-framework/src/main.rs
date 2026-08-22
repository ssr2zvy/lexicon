use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct LexiconProjectConfig {
    schema_version: Option<u32>,
    project: Option<ProjectSection>,
}

#[derive(Debug, Deserialize)]
struct ProjectSection {
    name: Option<String>,
    sources_directory: Option<String>,
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        println!("lexicon-framework: no command given");
        return;
    }

    if args.len() == 1 && args[0] == "test" {
        println!("test::: installation successful and routed through the cli to the framework");
        return;
    }

    if args.len() == 3 && args[0] == "source" && args[1] == "new" {
        if let Err(error) = generate_source_scaffold(&args[2], "http") {
            eprintln!("lexicon-framework: {error}");
            std::process::exit(1);
        }
        return;
    }

    if args.len() == 5 && args[0] == "source" && args[1] == "new" && args[3] == "--protocol" {
        if let Err(error) = generate_source_scaffold(&args[2], &args[4]) {
            eprintln!("lexicon-framework: {error}");
            std::process::exit(1);
        }
        return;
    }

    if args.len() == 4 && args[0] == "source" && args[1] == "new" {
        let protocol_flag = &args[3];
        if let Some(protocol) = protocol_flag.strip_prefix("--protocol=") {
            if let Err(error) = generate_source_scaffold(&args[2], protocol) {
                eprintln!("lexicon-framework: {error}");
                std::process::exit(1);
            }
            return;
        }
    }

    let joined = args.join(" ");
    eprintln!("lexicon-framework: unknown command \"{joined}\"");
    std::process::exit(1);
}

fn generate_source_scaffold(source_name: &str, protocol: &str) -> Result<(), String> {
    validate_source_name(source_name)?;
    validate_protocol(protocol)?;

    let project_root = find_project_root(&env::current_dir().map_err(|error| {
        format!("failed to determine current directory: {error}")
    })?)?;
    let source_root = configured_sources_directory(&project_root)?;
    let source_dir = source_root.join(source_name);

    if source_dir.exists() {
        return Err(format!(
            "source '{}' already exists at {}",
            source_name,
            source_dir.display()
        ));
    }

    fs::create_dir_all(&source_root).map_err(|error| {
        format!("failed to create {}: {error}", source_root.display())
    })?;

    let directories = [
        Path::new("data/raw"),
        Path::new("data/processed"),
        Path::new("get-raw-data/sessions"),
        Path::new("get-raw-data/get_raw_data_impl/src"),
        Path::new("process-data/sessions"),
        Path::new("process-data/process_data_impl/src"),
        Path::new("process-data/process_data_impl/processing"),
    ];

    for directory in &directories {
        let path = source_dir.join(directory);
        fs::create_dir_all(&path).map_err(|error| {
            format!("failed to create directory {}: {error}", path.display())
        })?;
    }

    let files = [
        ("source.toml", format_source_toml(source_name, protocol)),
        ("discovery.md", format_discovery_markdown(source_name)),
        (
            "get-raw-data/session_status.json",
            format_session_status_json(source_name, "get-raw-data"),
        ),
        (
            "process-data/session_status.json",
            format_session_status_json(source_name, "process-data"),
        ),
        (
            "get-raw-data/get_raw_data_impl/Cargo.toml",
            format_impl_cargo_toml(&format!("{source_name}-get-raw-data")),
        ),
        (
            "get-raw-data/get_raw_data_impl/Cargo.lock",
            format_cargo_lockfile(),
        ),
        (
            "get-raw-data/get_raw_data_impl/src/main.rs",
            format_get_raw_data_main(source_name),
        ),
        (
            "process-data/process_data_impl/Cargo.toml",
            format_impl_cargo_toml(&format!("{source_name}-process-data")),
        ),
        (
            "process-data/process_data_impl/Cargo.lock",
            format_cargo_lockfile(),
        ),
        (
            "process-data/process_data_impl/src/main.rs",
            format_process_data_main(source_name),
        ),
    ];

    for (relative_path, contents) in &files {
        let path = source_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("failed to create parent directory for {}: {error}", path.display())
            })?;
        }
        fs::write(&path, contents).map_err(|error| {
            format!("failed to write {}: {error}", path.display())
        })?;
    }

    println!("Created source scaffold for '{}' at {}", source_name, source_dir.display());
    println!("Files to edit next:");
    for (relative_path, _) in &files {
        println!("  - {}", source_dir.join(relative_path).display());
    }

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
    if source_name.contains(['/', '\\']) {
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
            "unsupported protocol '{}'; only 'http' is currently supported for source creation",
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

fn find_descendant_project_root(root: &Path) -> Result<Option<PathBuf>, String> {
    let mut found = None;
    visit_descendants(root, &mut found, root)?;
    Ok(found)
}

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
    if project_name.is_empty() || project_name == "." || project_name == ".." || project_name.contains(['/', '\\']) {
        return Err(format!("invalid project.name '{}' in {}", project_name, config_path.display()));
    }

    let configured = project
        .sources_directory
        .as_deref()
        .unwrap_or("sources");

    let path = Path::new(configured);
    if path.is_absolute() {
        return Err(format!(
            "invalid sources_directory '{}' in {}: must be a relative path",
            configured,
            config_path.display()
        ));
    }
    if path.components().any(|component| matches!(component, Component::ParentDir | Component::RootDir)) {
        return Err(format!(
            "invalid sources_directory '{}' in {}: must remain within the project root",
            configured,
            config_path.display()
        ));
    }

    let resolved = project_root.join(path);
    let canonical_project_root = project_root.canonicalize().unwrap_or_else(|_| project_root.to_path_buf());
    match resolved.strip_prefix(&canonical_project_root) {
        Ok(_) => Ok(resolved),
        Err(_) => Err(format!(
            "invalid sources_directory '{}' in {}: resolves outside the project root",
            configured,
            config_path.display()
        )),
    }
}

fn format_source_toml(source_name: &str, protocol: &str) -> String {
    let mut out = String::new();
    out.push_str("schema_version = 1\n\n");
    out.push_str(&format!("id = \"{source_name}\"\n\n"));
    out.push_str("[acquisition]\n");
    out.push_str(&format!("protocol = \"{protocol}\"\n"));
    out.push_str("access_pattern = \"unspecified\"\n\n");
    out.push_str("[discovery]\n");
    out.push_str("documentation = \"discovery.md\"\n");
    out
}

fn format_discovery_markdown(source_name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {source_name}\n\n"));
    out.push_str(&format!(
        "This source scaffold was created by `lexicon source new {source_name}`.\n\n"
    ));
    out.push_str(
        "Edit this document to describe the source, its acquisition rules, and the data it produces.\n",
    );
    out.push_str(
        "The source implementation is intentionally scaffolded without a real runtime implementation yet.\n",
    );
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

fn format_get_raw_data_main(source_name: &str) -> String {
    let mut out = String::new();
    out.push_str("use lexicon_framework_core::{run_http_source, HttpAcquisition, HttpAcquisitionContext};\n\n");
    out.push_str("struct SourceImpl;\n\n");
    out.push_str("impl HttpAcquisition for SourceImpl {\n");
    out.push_str(
        "    fn acquire(&self, context: &mut HttpAcquisitionContext) -> Result<(), String> {\n",
    );
    out.push_str(&format!(
        "        let _ = context;\n        println!(\"{source_name}: HTTP acquisition scaffold is ready\");\n"
    ));
    out.push_str("        Ok(())\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");
    out.push_str("fn main() {\n");
    out.push_str("    let source = SourceImpl;\n");
    out.push_str("    if let Err(error) = run_http_source(source) {\n");
    out.push_str(&format!(
        "        eprintln!(\"{source_name}: HTTP acquisition failed: {{error}}\");\n"
    ));
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

fn format_cargo_lockfile() -> String {
    let mut out = String::new();
    out.push_str("# This file is automatically @generated by Cargo.\n");
    out.push_str("# It is not intended for manual editing.\n");
    out.push_str("version = 3\n");
    out
}

#[cfg(test)]
mod tests {
    use super::{format_get_raw_data_main, format_impl_cargo_toml};

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
    }
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