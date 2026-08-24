use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PathModification {
    pub entry: String,
    pub modified_by_lexicon: bool,
    pub method: String,
    pub location: String,
}

#[derive(Serialize, Deserialize)]
pub struct InstallationRecord {
    pub schema_version: u32,
    pub version: String,
    pub target: String,
    pub installed_at: String,
    pub cli: String,
    pub path_modification: PathModification,
}

pub enum InstallState {
    NotInstalled,
    Installed,
    Damaged,
}

pub struct Destinations {
    pub cli: PathBuf,
    pub record: PathBuf,
}
