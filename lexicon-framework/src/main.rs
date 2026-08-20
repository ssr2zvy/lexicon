use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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

    let workspace_root = locate_workspace_root()?;
    let source_root = workspace_root.join("lexicon-framework").join("sources");
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

fn locate_workspace_root() -> Result<PathBuf, String> {
    let mut current = env::current_dir()
        .map_err(|error| format!("failed to determine current directory: {error}"))?;

    loop {
        let candidate = current.join("Cargo.toml");
        if candidate.is_file() {
            let contents = fs::read_to_string(&candidate)
                .map_err(|error| format!("failed to read {}: {error}", candidate.display()))?;
            if contents.contains("[workspace]") {
                return Ok(current);
            }
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    Err("could not locate the workspace root from the current directory".to_string())
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
    out.push_str("lexicon-framework-core = { path = \"../../../../core\" }\n");
    out
}

fn format_get_raw_data_main(source_name: &str) -> String {
    let mut out = String::new();
    out.push_str("use lexicon_framework_core::{run_http_source, HttpAcquisition};\n\n");
    out.push_str("struct SourceImpl;\n\n");
    out.push_str("impl HttpAcquisition for SourceImpl {\n");
    out.push_str("    fn run(&self) -> Result<(), String> {\n");
    out.push_str(&format!(
        "        println!(\"{source_name}: HTTP acquisition scaffold is ready\");\n"
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