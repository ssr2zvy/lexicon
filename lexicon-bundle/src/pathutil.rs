use std::env;
use std::path::PathBuf;

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
