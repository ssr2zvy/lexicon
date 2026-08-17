use std::env;
use std::fs;
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tar::Archive;
use xz2::read::XzDecoder;

// Protocol-agnostic part of cargo-bundler-v0.1.0: MZA_BUNDLE_INPUTS is
// captured at compile time by build.rs and embedded here via `include!`.
include!(concat!(env!("OUT_DIR"), "/mza_bundle_inputs.rs"));
// Lexicon-specific install layout, resolved at compile time from
// lexicon-install.toml (see lexicon-bundle/build.rs).
include!(concat!(env!("OUT_DIR"), "/lexicon_install_layout.rs"));

const MARKER: &str = "# Added by the lexicon installer";

#[derive(Serialize, Deserialize, Default, Clone)]
struct PathModification {
    entry: String,
    modified_by_lexicon: bool,
    method: String,
    location: String,
}

#[derive(Serialize, Deserialize)]
struct InstallationRecord {
    schema_version: u32,
    version: String,
    target: String,
    installed_at: String,
    cli: String,
    framework: String,
    path_modification: PathModification,
}

enum InstallState {
    NotInstalled,
    Installed,
    Damaged,
}

struct Destinations {
    cli: PathBuf,
    framework: PathBuf,
    record: PathBuf,
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    std::process::exit(dispatch(&args));
}

fn dispatch(args: &[String]) -> i32 {
    let dest = resolve_destinations();
    let state = detect_state(&dest);

    match args.first().map(String::as_str) {
        None => default_flow(state, &dest),
        Some("install") => install_command(state, &dest),
        Some("uninstall") => uninstall_command(state, &dest),
        Some("update") => {
            println!("Update is not implemented.");
            0
        }
        Some("repair") => {
            println!("Repair is not implemented.");
            0
        }
        Some("--help") | Some("-h") => {
            print_help();
            0
        }
        Some(other) => {
            eprintln!("lexicon-bundle: unknown argument \"{other}\"");
            print_help();
            2
        }
    }
}

fn print_help() {
    println!("Usage:");
    println!("  lexicon-bundle");
    println!("  lexicon-bundle install");
    println!("  lexicon-bundle uninstall");
    println!("  lexicon-bundle update    (not implemented)");
    println!("  lexicon-bundle repair    (not implemented)");
    println!("  lexicon-bundle --help");
}

fn default_flow(state: InstallState, dest: &Destinations) -> i32 {
    match state {
        InstallState::NotInstalled => {
            println!("Lexicon is not installed.");
            print_destinations(dest);
            if prompt_default_yes("Install Lexicon?") {
                do_install(dest)
            } else {
                println!("Installation cancelled.");
                0
            }
        }
        other => show_maintenance_menu(other, dest),
    }
}

fn install_command(state: InstallState, dest: &Destinations) -> i32 {
    match state {
        InstallState::NotInstalled => {
            println!("Lexicon is not installed.");
            print_destinations(dest);
            if prompt_default_yes("Install Lexicon?") {
                do_install(dest)
            } else {
                println!("Installation cancelled.");
                0
            }
        }
        other => show_maintenance_menu(other, dest),
    }
}

fn uninstall_command(state: InstallState, dest: &Destinations) -> i32 {
    match state {
        InstallState::NotInstalled => {
            println!("Lexicon is not installed.");
            0
        }
        _ => run_uninstall_flow(dest),
    }
}

fn show_maintenance_menu(state: InstallState, dest: &Destinations) -> i32 {
    loop {
        match state {
            InstallState::Damaged => println!("Lexicon has a damaged or incomplete installation.\n"),
            _ => println!("Lexicon is installed.\n"),
        }
        println!("1. Update (not implemented)");
        println!("2. Repair (not implemented)");
        println!("3. Uninstall");
        println!("4. Cancel");

        match read_line("Select an option: ").trim() {
            "1" => println!("Update is not implemented."),
            "2" => println!("Repair is not implemented."),
            "3" => return run_uninstall_flow(dest),
            "4" => return 0,
            _ => eprintln!("Invalid selection."),
        }
    }
}

fn run_uninstall_flow(dest: &Destinations) -> i32 {
    print_destinations(dest);
    if prompt_default_no("Uninstall Lexicon?") {
        do_uninstall(dest)
    } else {
        println!("Uninstallation cancelled.");
        0
    }
}

fn print_destinations(dest: &Destinations) {
    println!("  CLI:       {}", dest.cli.display());
    println!("  Framework: {}", dest.framework.display());
}

fn read_line(prompt: &str) -> String {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    input
}

fn prompt_default_yes(question: &str) -> bool {
    let answer = read_line(&format!("{question} [Y/n] ")).trim().to_lowercase();
    answer.is_empty() || answer == "y" || answer == "yes"
}

fn prompt_default_no(question: &str) -> bool {
    let answer = read_line(&format!("{question} [y/N] ")).trim().to_lowercase();
    answer == "y" || answer == "yes"
}

fn resolve_destinations() -> Destinations {
    let base_dir = resolve_base_dir(INSTALL_BASE);
    Destinations {
        cli: base_dir.join(CLI_INSTALL_PATH),
        framework: base_dir.join(FRAMEWORK_INSTALL_PATH),
        record: base_dir.join(RECORD_INSTALL_PATH),
    }
}

fn detect_state(dest: &Destinations) -> InstallState {
    let record_exists = dest.record.is_file();
    let executables_exist = dest.cli.is_file() && dest.framework.is_file();
    match (record_exists, executables_exist) {
        (false, false) => InstallState::NotInstalled,
        (true, true) => InstallState::Installed,
        _ => InstallState::Damaged,
    }
}

fn resolve_base_dir(base: &str) -> PathBuf {
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

fn find_input(label: &str) -> &'static [u8] {
    MZA_BUNDLE_INPUTS
        .iter()
        .find(|input| input.label == label)
        .map(|input| input.archive)
        .unwrap_or_else(|| panic!("lexicon-bundle: no embedded input with label \"{label}\""))
}

// ---- Installation ----

fn do_install(dest: &Destinations) -> i32 {
    let staging_dir = env::temp_dir().join(format!("lexicon-bundle-install-{}-{}", std::process::id(), now_unix()));
    if let Err(err) = fs::create_dir_all(&staging_dir) {
        eprintln!("lexicon-bundle: failed to create staging directory: {err}");
        return 1;
    }

    let result = try_install(dest, &staging_dir);
    let _ = fs::remove_dir_all(&staging_dir);

    match result {
        Ok(path_modification) => {
            println!("Lexicon installed successfully.");
            if !path_modification.method.is_empty() {
                println!(
                    "Start a new terminal session for the `lexicon` command to become available."
                );
            }
            0
        }
        Err(err) => {
            eprintln!("lexicon-bundle: installation failed: {err}");
            1
        }
    }
}

fn try_install(dest: &Destinations, staging_dir: &Path) -> Result<PathModification, String> {
    let cli_file_name = file_name_of(&dest.cli)?;
    let framework_file_name = file_name_of(&dest.framework)?;

    let cli_archive = find_input(CLI_ARTIFACT_LABEL);
    let framework_archive = find_input(FRAMEWORK_ARTIFACT_LABEL);

    let staged_cli = extract_to_staging(cli_archive, staging_dir, &cli_file_name)?;
    let staged_framework = extract_to_staging(framework_archive, staging_dir, &framework_file_name)?;

    atomic_install(&staged_cli, &dest.cli)?;
    atomic_install(&staged_framework, &dest.framework)?;

    #[cfg(unix)]
    {
        set_executable(&dest.cli)?;
        set_executable(&dest.framework)?;
    }

    let bin_dir = dest
        .cli
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", dest.cli.display()))?;

    #[cfg(target_os = "linux")]
    let path_modification = ensure_linux_path(bin_dir)?;
    #[cfg(target_os = "windows")]
    let path_modification = ensure_windows_path(bin_dir)?;

    if !dest.cli.is_file() || !dest.framework.is_file() {
        return Err("installed files are missing after installation".to_string());
    }
    if !verify_framework_reachable(&dest.cli, &dest.framework) {
        return Err("the installed CLI cannot reach the installed framework".to_string());
    }

    let record = InstallationRecord {
        schema_version: 1,
        version: env!("CARGO_PKG_VERSION").to_string(),
        target: env!("LEXICON_TARGET_TRIPLE").to_string(),
        installed_at: now_unix().to_string(),
        cli: dest.cli.display().to_string(),
        framework: dest.framework.display().to_string(),
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
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|err| err.to_string())
}

fn write_record(record_path: &Path, record: &InstallationRecord) -> Result<(), String> {
    if let Some(parent) = record_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    let contents = toml::to_string_pretty(record).map_err(|err| err.to_string())?;
    fs::write(record_path, contents).map_err(|err| err.to_string())
}

fn read_record(record_path: &Path) -> Option<InstallationRecord> {
    let contents = fs::read_to_string(record_path).ok()?;
    toml::from_str(&contents).ok()
}

/// Verifies the framework path relative to the CLI's own directory (the same
/// scheme lexicon-cli/build.rs compiles into the installed CLI) resolves to
/// the framework file actually installed.
fn verify_framework_reachable(cli_dest: &Path, framework_dest: &Path) -> bool {
    let Some(bin_dir) = cli_dest.parent() else { return false };
    let mut cli_parent_segments = segments(CLI_INSTALL_PATH);
    cli_parent_segments.pop();
    let framework_segments = segments(FRAMEWORK_INSTALL_PATH);
    let relative = relative_path(&cli_parent_segments, &framework_segments);
    normalize(&bin_dir.join(relative)) == normalize(framework_dest)
}

/// Lexically collapses `.` and `..` components without touching the
/// filesystem, since the joined path may not exist yet to `canonicalize()`.
fn normalize(path: &Path) -> PathBuf {
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

fn segments(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|segment| !segment.is_empty()).collect()
}

fn relative_path(from_dir: &[&str], to: &[&str]) -> String {
    let common = from_dir.iter().zip(to.iter()).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = Vec::new();
    for _ in common..from_dir.len() {
        parts.push("..");
    }
    parts.extend_from_slice(&to[common..]);
    parts.join("/")
}

/// Decompresses a tar.xz archive; rejects anything but exactly one regular
/// file (ignoring directory entries, rejecting links/specials), and extracts
/// that file into `staging_dir` under `dest_file_name` (never the archived
/// path, so archive contents can never control the install destination).
fn extract_to_staging(archive_bytes: &[u8], staging_dir: &Path, dest_file_name: &str) -> Result<PathBuf, String> {
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

fn validate_archive(archive_bytes: &[u8]) -> Result<(), String> {
    let decoder = XzDecoder::new(Cursor::new(archive_bytes));
    let mut archive = Archive::new(decoder);
    let mut regular_count = 0;
    for entry in archive.entries().map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        match entry.header().entry_type() {
            tar::EntryType::Directory => continue,
            tar::EntryType::Regular => regular_count += 1,
            other => return Err(format!("archive contains a disallowed entry type: {other:?}")),
        }
    }
    match regular_count {
        0 => Err("archive contains no regular file".to_string()),
        1 => Ok(()),
        _ => Err("archive contains more than one regular file".to_string()),
    }
}

fn atomic_install(staged: &Path, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    if fs::rename(staged, dest).is_err() {
        // Staging and destination may be on different filesystems.
        fs::copy(staged, dest).map_err(|err| err.to_string())?;
        let _ = fs::remove_file(staged);
    }
    Ok(())
}

// ---- Uninstallation ----

fn do_uninstall(dest: &Destinations) -> i32 {
    let record = read_record(&dest.record);

    if let Err(err) = fs::remove_file(&dest.cli) {
        if err.kind() != io::ErrorKind::NotFound {
            eprintln!("lexicon-bundle: failed to remove {}: {err}", dest.cli.display());
            return 1;
        }
    }
    if let Err(err) = fs::remove_file(&dest.framework) {
        if err.kind() != io::ErrorKind::NotFound {
            eprintln!("lexicon-bundle: failed to remove {}: {err}", dest.framework.display());
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
    remove_if_empty_and_lexicon_owned(dest.framework.parent());
    remove_if_empty_and_lexicon_owned(dest.record.parent());

    if dest.cli.exists() || dest.framework.exists() || dest.record.exists() {
        eprintln!("lexicon-bundle: uninstallation did not fully complete");
        return 1;
    }

    println!("Lexicon uninstalled successfully.");
    0
}

/// Only removes directories Lexicon itself owns (named "lexicon", matching
/// the installed layout), and only when empty; never touches shared
/// directories like the user's home, .local, .local/bin, or Programs.
fn remove_if_empty_and_lexicon_owned(dir: Option<&Path>) {
    let Some(dir) = dir else { return };
    let Some(name) = dir.file_name().and_then(|name| name.to_str()) else { return };
    if !name.eq_ignore_ascii_case("lexicon") {
        return;
    }
    if let Ok(mut entries) = fs::read_dir(dir) {
        if entries.next().is_none() {
            let _ = fs::remove_dir(dir);
        }
    }
}

fn reverse_path_modification(path_modification: &PathModification) {
    let location = Path::new(&path_modification.location);
    let Ok(contents) = fs::read_to_string(location) else { return };

    let export_line = format!("export PATH=\"{}:$PATH\"", path_modification.entry);
    let filtered: Vec<&str> = contents
        .lines()
        .filter(|line| line.trim() != MARKER && line.trim() != export_line)
        .collect();
    let mut new_contents = filtered.join("\n");
    if !new_contents.is_empty() {
        new_contents.push('\n');
    }
    let _ = fs::write(location, new_contents);
}

/// Ensures `bin_dir` is on PATH for future shells; if it isn't, adds a marked,
/// idempotent entry to the user's shell startup file.
#[cfg(target_os = "linux")]
fn ensure_linux_path(bin_dir: &Path) -> Result<PathModification, String> {
    let entry = bin_dir.display().to_string();
    let path_var = env::var("PATH").unwrap_or_default();
    if env::split_paths(&path_var).any(|existing| existing == bin_dir) {
        return Ok(PathModification {
            entry,
            modified_by_lexicon: false,
            method: String::new(),
            location: String::new(),
        });
    }

    let home = env::var("HOME").map_err(|_| "could not determine $HOME".to_string())?;
    let profile_path = choose_profile_path(&home);
    let export_line = format!("export PATH=\"{entry}:$PATH\"\n");
    let existing = fs::read_to_string(&profile_path).unwrap_or_default();
    if existing.contains(&export_line) {
        return Ok(PathModification {
            entry,
            modified_by_lexicon: false,
            method: "profile-append".to_string(),
            location: profile_path.display().to_string(),
        });
    }

    let mut new_contents = existing;
    if !new_contents.is_empty() && !new_contents.ends_with('\n') {
        new_contents.push('\n');
    }
    new_contents.push_str(MARKER);
    new_contents.push('\n');
    new_contents.push_str(&export_line);
    fs::write(&profile_path, new_contents).map_err(|err| err.to_string())?;

    Ok(PathModification {
        entry,
        modified_by_lexicon: true,
        method: "profile-append".to_string(),
        location: profile_path.display().to_string(),
    })
}

#[cfg(target_os = "linux")]
fn choose_profile_path(home: &str) -> PathBuf {
    let shell = env::var("SHELL").unwrap_or_default();
    if shell.contains("zsh") {
        PathBuf::from(home).join(".zshrc")
    } else if shell.contains("bash") {
        PathBuf::from(home).join(".bashrc")
    } else {
        PathBuf::from(home).join(".profile")
    }
}

/// Adds `bin_dir` to the current user's PATH registry value if absent, then
/// broadcasts WM_SETTINGCHANGE so running processes pick up the change.
#[cfg(target_os = "windows")]
fn ensure_windows_path(bin_dir: &Path) -> Result<PathModification, String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let entry = bin_dir.display().to_string();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .map_err(|err| format!("failed to open HKCU\\Environment: {err}"))?;

    let current: String = env_key.get_value("Path").unwrap_or_default();
    let already_present = current.split(';').any(|existing| existing.eq_ignore_ascii_case(&entry));
    if already_present {
        return Ok(PathModification {
            entry,
            modified_by_lexicon: false,
            method: "registry".to_string(),
            location: "HKCU\\Environment\\Path".to_string(),
        });
    }

    let new_path = if current.is_empty() { entry.clone() } else { format!("{current};{entry}") };
    env_key.set_value("Path", &new_path).map_err(|err| format!("failed to update user PATH: {err}"))?;

    broadcast_environment_change();

    Ok(PathModification {
        entry,
        modified_by_lexicon: true,
        method: "registry".to_string(),
        location: "HKCU\\Environment\\Path".to_string(),
    })
}

#[cfg(target_os = "windows")]
fn broadcast_environment_change() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };

    let param: Vec<u16> = OsStr::new("Environment").encode_wide().chain(std::iter::once(0)).collect();
    let mut result: usize = 0;
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            param.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            &mut result as *mut usize,
        );
    }
}
