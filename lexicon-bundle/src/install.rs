use std::env;
use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tar::Archive;
use xz2::read::XzDecoder;

use crate::envpath::reverse_path_modification;
use crate::model::{Destinations, InstallState, InstallationRecord, PathModification};
use crate::pathutil::resolve_base_dir;

#[cfg(target_os = "linux")]
use crate::envpath::ensure_linux_path;
#[cfg(target_os = "windows")]
use crate::envpath::ensure_windows_path;

pub fn resolve_destinations() -> Destinations {
    let base_dir = resolve_base_dir(crate::INSTALL_BASE);
    Destinations {
        cli: base_dir.join(crate::CLI_INSTALL_PATH),
        record: base_dir.join(crate::RECORD_INSTALL_PATH),
    }
}

pub fn detect_state(dest: &Destinations) -> InstallState {
    let record_exists = dest.record.is_file();
    let cli_exists = dest.cli.is_file();
    match (record_exists, cli_exists) {
        (false, false) => InstallState::NotInstalled,
        (true, true) => InstallState::Installed,
        _ => InstallState::Damaged,
    }
}

pub fn do_install(dest: &Destinations) -> i32 {
    let staging_dir = env::temp_dir().join(format!(
        "lexicon-bundle-install-{}-{}",
        std::process::id(),
        now_unix()
    ));
    if let Err(err) = fs::create_dir_all(&staging_dir) {
        eprintln!("lexicon-bundle: failed to create staging directory: {err}");
        return 1;
    }

    let result = try_install(dest, &staging_dir);
    let _ = fs::remove_dir_all(&staging_dir);

    match result {
        Ok(path_modification) => {
            println!("[[LEXICON-BUNDLE]] Lexicon installed successfully.");
            if path_modification.method == "profile-append" {
                println!("[[LEXICON-BUNDLE]] Start a new terminal session for the `lexicon` command to become available.");
            }
            0
        }
        Err(err) => {
            eprintln!("[[LEXICON-BUNDLE]] installation failed: {err}");
            1
        }
    }
}

pub fn do_uninstall(dest: &Destinations) -> i32 {
    let record = read_record(&dest.record);

    if let Err(err) = fs::remove_file(&dest.cli) {
        if err.kind() != io::ErrorKind::NotFound {
            eprintln!(
                "lexicon-bundle: failed to remove {}: {err}",
                dest.cli.display()
            );
            return 1;
        }
    }

    if let Some(record) = &record {
        if record.path_modification.modified_by_lexicon {
            reverse_path_modification(&record.path_modification);
        }
    }

    let _ = fs::remove_file(&dest.record);
    remove_if_empty_and_lexicon_owned(dest.cli.parent());
    remove_if_empty_and_lexicon_owned(dest.record.parent());

    if dest.cli.exists() || dest.record.exists() {
        eprintln!("lexicon-bundle: uninstallation did not fully complete");
        return 1;
    }

    println!("[[LEXICON-BUNDLE]] Lexicon uninstalled successfully.");
    0
}

pub fn find_input(label: &str) -> &'static [u8] {
    crate::MZA_BUNDLE_INPUTS
        .iter()
        .find(|input| input.label == label)
        .map(|input| input.archive)
        .unwrap_or_else(|| panic!("lexicon-bundle: no embedded input with label \"{label}\""))
}

fn try_install(dest: &Destinations, staging_dir: &Path) -> Result<PathModification, String> {
    let cli_file_name = file_name_of(&dest.cli)?;

    let cli_archive = find_input(crate::CLI_ARTIFACT_LABEL);

    let staged_cli = extract_to_staging(cli_archive, staging_dir, &cli_file_name)?;

    atomic_install(&staged_cli, &dest.cli)?;

    #[cfg(unix)]
    set_executable(&dest.cli)?;

    let bin_dir = dest
        .cli
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", dest.cli.display()))?;

    #[cfg(target_os = "linux")]
    let path_modification = ensure_linux_path(bin_dir, &dest.cli)?;
    #[cfg(target_os = "windows")]
    let path_modification = ensure_windows_path(bin_dir)?;

    if !dest.cli.is_file() {
        return Err("installed CLI is missing after installation".to_string());
    }

    let record = InstallationRecord {
        schema_version: 1,
        version: env!("CARGO_PKG_VERSION").to_string(),
        target: env!("LEXICON_TARGET_TRIPLE").to_string(),
        installed_at: now_unix().to_string(),
        cli: dest.cli.display().to_string(),
        path_modification: path_modification.clone(),
    };
    write_record(&dest.record, &record)?;

    Ok(path_modification)
}

fn file_name_of(path: &Path) -> Result<String, String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{} has no file name", path.display()))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .map_err(|err| err.to_string())
}

pub fn write_record(record_path: &Path, record: &InstallationRecord) -> Result<(), String> {
    if let Some(parent) = record_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    let contents = toml::to_string_pretty(record).map_err(|err| err.to_string())?;
    fs::write(record_path, contents).map_err(|err| err.to_string())
}

pub fn read_record(record_path: &Path) -> Option<InstallationRecord> {
    let contents = fs::read_to_string(record_path).ok()?;
    toml::from_str(&contents).ok()
}

fn validate_archive(archive_bytes: &[u8]) -> Result<(), String> {
    let decoder = XzDecoder::new(Cursor::new(archive_bytes));
    let mut archive = Archive::new(decoder);
    let mut regular_count = 0;
    for entry in archive.entries().map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        match entry.header().entry_type() {
            tar::EntryType::Directory => continue,
            tar::EntryType::Regular => regular_count += 1,
            other => {
                return Err(format!(
                    "archive contains a disallowed entry type: {other:?}"
                ))
            }
        }
    }
    match regular_count {
        0 => Err("archive contains no regular file".to_string()),
        1 => Ok(()),
        _ => Err("archive contains more than one regular file".to_string()),
    }
}

/// Decompresses a tar.xz archive; rejects anything but exactly one regular
/// file (ignoring directory entries, rejecting links/specials), and extracts
/// that file into `staging_dir` under `dest_file_name` (never the archived
/// path, so archive contents can never control the install destination).
fn extract_to_staging(
    archive_bytes: &[u8],
    staging_dir: &Path,
    dest_file_name: &str,
) -> Result<PathBuf, String> {
    validate_archive(archive_bytes)?;

    let decoder = XzDecoder::new(Cursor::new(archive_bytes));
    let mut archive = Archive::new(decoder);
    for entry in archive.entries().map_err(|err| err.to_string())? {
        let mut entry = entry.map_err(|err| err.to_string())?;
        if entry.header().entry_type() == tar::EntryType::Regular {
            let staged_path = staging_dir.join(dest_file_name);
            entry.unpack(&staged_path).map_err(|err| err.to_string())?;
            return Ok(staged_path);
        }
    }
    Err("archive contains no regular file".to_string())
}

fn atomic_install(staged: &Path, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    if fs::rename(staged, dest).is_err() {
        fs::copy(staged, dest).map_err(|err| err.to_string())?;
        let _ = fs::remove_file(staged);
    }
    Ok(())
}

fn remove_if_empty_and_lexicon_owned(dir: Option<&Path>) {
    let Some(dir) = dir else { return };
    let Some(name) = dir.file_name().and_then(|name| name.to_str()) else {
        return;
    };
    if !name.eq_ignore_ascii_case("lexicon") {
        return;
    }
    if let Ok(mut entries) = fs::read_dir(dir) {
        if entries.next().is_none() {
            let _ = fs::remove_dir(dir);
        }
    }
}
