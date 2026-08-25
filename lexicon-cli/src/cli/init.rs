use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "init", about = "Create a new Lexicon project root.")]
pub struct InitCommand {
    #[arg(value_name = "PARENT_PATH")]
    pub parent_path: PathBuf,

    #[arg(value_name = "PROJECT_NAME")]
    pub project_name: String,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::InitCommand;
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
            let result =
                lexicon_framework::commands::init(&std::path::PathBuf::from("/tmp"), bad_name);
            assert!(result.is_err(), "expected invalid project name: {bad_name}");
        }
    }

    #[test]
    fn initializes_project_directory_and_toml() {
        let parent = std::env::temp_dir().join(format!("lexicon-init-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(&parent).unwrap();

        let result = lexicon_framework::commands::init(&parent, "example-project").unwrap();
        let project_dir = result.project_directory;

        assert_eq!(project_dir, parent.join("example-project"));
        assert!(project_dir.join("sources").is_dir());

        let contents = fs::read_to_string(project_dir.join("lexicon.toml")).unwrap();
        assert!(contents.contains("schema_version = 1"));
        assert!(contents.contains("[project]"));
        assert!(contents.contains("name = \"example-project\""));
        assert!(contents.contains("sources_directory = \"sources\""));

        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn does_not_delete_stale_pid_style_temp_directory() {
        let parent =
            std::env::temp_dir().join(format!("lexicon-init-stale-temp-{}", std::process::id()));
        let stale = parent.join(".example-project.tmp-12345");
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(&stale).unwrap();
        fs::create_dir_all(&parent).unwrap();

        let _ = lexicon_framework::commands::init(&parent, "example-project").unwrap();

        assert!(stale.exists(), "stale temp dir should not be removed");
        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn successful_init_leaves_no_temp_directory() {
        let parent =
            std::env::temp_dir().join(format!("lexicon-init-temp-clean-{}", std::process::id()));
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(&parent).unwrap();

        let result = lexicon_framework::commands::init(&parent, "clean-project").unwrap();
        let project_dir = result.project_directory;
        let temp_dirs = fs::read_dir(&parent)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".clean-project.tmp-")
            })
            .collect::<Vec<_>>();

        assert!(project_dir.exists());
        assert!(
            temp_dirs.is_empty(),
            "no temp directories should remain after successful init"
        );
        let _ = fs::remove_dir_all(parent);
    }
}
