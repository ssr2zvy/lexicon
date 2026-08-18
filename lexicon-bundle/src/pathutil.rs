use std::env;
use std::path::{Path, PathBuf};

pub fn resolve_base_dir(base: &str) -> PathBuf {
    match base {
        "home" => {
            let home = env::var("HOME")
                .or_else(|_| env::var("USERPROFILE"))
                .unwrap_or_else(|_| panic!("lexicon-bundle: neither HOME nor USERPROFILE is set"));
            PathBuf::from(home)
        }
        "user_program_files" => {
            let local_app_data = env::var("LOCALAPPDATA")
                .unwrap_or_else(|_| panic!("lexicon-bundle: LOCALAPPDATA is not set"));
            PathBuf::from(local_app_data).join("Programs")
        }
        other => panic!("lexicon-bundle: unknown install base \"{other}\""),
    }
}

pub fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

pub fn segments(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|segment| !segment.is_empty()).collect()
}

pub fn relative_path(from_dir: &[&str], to: &[&str]) -> String {
    let common = from_dir.iter().zip(to.iter()).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = Vec::new();
    for _ in common..from_dir.len() {
        parts.push("..");
    }
    parts.extend_from_slice(&to[common..]);
    parts.join("/")
}
