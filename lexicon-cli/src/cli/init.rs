use std::fs;
use std::path::{Component, Path, PathBuf};

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "init", about = "Create a new Lexicon project root.")]
pub struct InitCommand {
    #[arg(value_name = "PARENT_PATH")]
    pub parent_path: PathBuf,

    #[arg(value_name = "PROJECT_NAME")]
    pub project_name: String,
}

pub fn validate_project_name(project_name: &str) -> Result<(), String> {
    if project_name.trim().is_empty() {
        return Err("project name cannot be empty".to_string());
    }

    if project_name == "." || project_name == ".." {
        return Err(format!("invalid project name '{}': use a simple directory name", project_name));
    }

    let path = Path::new(project_name);
    if path.is_absolute() || path.components().any(|component| matches!(component, Component::RootDir | Component::ParentDir | Component::Prefix(_))) {
        return Err(format!("invalid project name '{}': use a single directory name without separators or parent traversal", project_name));
    }

    if path.components().any(|component| matches!(component, Component::CurDir)) {
        return Err(format!("invalid project name '{}': use a single directory name without separators or parent traversal", project_name));
    }

    if project_name.contains(['/', '\\']) {
        return Err(format!("invalid project name '{}': use a single directory name without separators or parent traversal", project_name));
    }

    Ok(())
}

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
        return Err(format!("project '{}' already exists at {}", project_name, project_directory.display()));
    }

    fs::create_dir_all(project_directory.join("sources"))
        .map_err(|error| format!("failed to create project directory '{}': {error}", project_directory.display()))?;

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

    fs::write(project_directory.join("lexicon.toml"), toml_text)
        .map_err(|error| format!("failed to write {}: {error}", project_directory.join("lexicon.toml").display()))?;

    Ok(project_directory)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{initialize_project, validate_project_name};
    use crate::cli::{Cli, RootCommand};
    use clap::Parser;

    #[test]
    fn parses_init_command_with_parent_path_and_project_name() {
        let cli = Cli::try_parse_from(["lexicon", "init", "/tmp", "my-data-project"])
            .expect("lexicon init should parse with parent path and project name");

        match cli.command {
            Some(RootCommand::Init(command)) => {
                assert_eq!(command.parent_path, std::path::PathBuf::from("/tmp"));
                assert_eq!(command.project_name, "my-data-project");
            }
            other => panic!("expected Init subcommand, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unsafe_project_names() {
        for bad_name in [".", "..", "bad/name", "bad\\name", "../evil"] {
            let result = validate_project_name(bad_name);
            assert!(result.is_err(), "expected invalid project name: {bad_name}");
        }
    }

    #[test]
    fn initializes_project_directory_and_toml() {
        let parent = std::env::temp_dir().join(format!("lexicon-init-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(&parent).unwrap();

        let project_dir = initialize_project(&parent, "example-project").unwrap();

        assert_eq!(project_dir, parent.join("example-project"));
        assert!(project_dir.join("sources").is_dir());

        let contents = fs::read_to_string(project_dir.join("lexicon.toml")).unwrap();
        assert!(contents.contains("schema_version = 1"));
        assert!(contents.contains("[project]"));
        assert!(contents.contains("name = \"example-project\""));
        assert!(contents.contains("sources_directory = \"sources\""));

        let _ = fs::remove_dir_all(parent);
    }
}
