use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::PathModification;

/// Makes `cli_dest` reachable as a bare command on the current PATH.
///
/// The primary, standard-practice approach on Linux is to symlink the
/// binary into a directory that's already on PATH (mirrors what most CLI
/// installers do), since that takes effect immediately with no shell restart
/// and no risk of editing the wrong startup file. Only if no writable PATH
/// directory can be found does this fall back to appending an export line
/// to a shell startup file.
#[cfg(target_os = "linux")]
pub fn ensure_linux_path(bin_dir: &Path, cli_dest: &Path) -> Result<PathModification, String> {
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

    if let Some(modification) = try_symlink_into_path(cli_dest, &path_var) {
        return Ok(modification);
    }

    ensure_linux_path_via_profile(bin_dir, entry)
}

/// Looks for a directory already on PATH that we can drop a symlink into,
/// preferring PATH's existing order (e.g. `~/.local/bin` before
/// `/usr/local/bin` when both are present).
#[cfg(target_os = "linux")]
fn try_symlink_into_path(cli_dest: &Path, path_var: &str) -> Option<PathModification> {
    let file_name = cli_dest.file_name()?;
    for dir in env::split_paths(path_var) {
        if !dir.is_dir() {
            continue;
        }
        let link_path = dir.join(file_name);
        match fs::symlink_metadata(&link_path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                if fs::read_link(&link_path).ok().as_deref() == Some(cli_dest) {
                    return Some(PathModification {
                        entry: link_path.display().to_string(),
                        modified_by_lexicon: false,
                        method: "symlink".to_string(),
                        location: link_path.display().to_string(),
                    });
                }
                continue; // something else owns this name; don't clobber it
            }
            Ok(_) => continue, // a real file already occupies that name
            Err(_) => {}
        }

        if std::os::unix::fs::symlink(cli_dest, &link_path).is_ok() {
            return Some(PathModification {
                entry: link_path.display().to_string(),
                modified_by_lexicon: true,
                method: "symlink".to_string(),
                location: link_path.display().to_string(),
            });
        }
        // Directory wasn't writable; try the next PATH entry.
    }
    None
}

/// Fallback used only when no writable PATH directory exists.
///
/// `$SHELL` is not always a reliable signal for which startup file the
/// user's next shell will actually source (e.g. it's often unset in
/// containers), so this tries the most likely candidate first, verifies it
/// by actually spawning that shell mode, and only falls through to the next
/// candidate if verification fails — rolling back the unverified edit rather
/// than leaving stray PATH exports in multiple files.
#[cfg(target_os = "linux")]
fn ensure_linux_path_via_profile(bin_dir: &Path, entry: String) -> Result<PathModification, String> {
    let home = env::var("HOME").map_err(|_| "could not determine $HOME".to_string())?;
    let export_line = format!("export PATH=\"{entry}:$PATH\"\n");
    let candidates = candidate_profiles(&home);
    let last_index = candidates.len() - 1;

    for (index, (profile_path, kind)) in candidates.into_iter().enumerate() {
        let existing = fs::read_to_string(&profile_path).unwrap_or_default();
        if existing.contains(&export_line) {
            return Ok(PathModification {
                entry,
                modified_by_lexicon: false,
                method: "profile-append".to_string(),
                location: profile_path.display().to_string(),
            });
        }

        let mut new_contents = existing.clone();
        if !new_contents.is_empty() && !new_contents.ends_with('\n') {
            new_contents.push('\n');
        }
        new_contents.push_str(crate::MARKER);
        new_contents.push('\n');
        new_contents.push_str(&export_line);
        fs::write(&profile_path, &new_contents).map_err(|err| err.to_string())?;

        if verify_path_picked_up(kind, bin_dir) {
            return Ok(PathModification {
                entry,
                modified_by_lexicon: true,
                method: "profile-append".to_string(),
                location: profile_path.display().to_string(),
            });
        }

        if index == last_index {
            // Out of candidates; keep this best-effort edit rather than leaving PATH unmodified.
            eprintln!(
                "[[LEXICON-BUNDLE]] could not verify PATH takes effect via {}; keeping the edit as a best effort",
                profile_path.display()
            );
            return Ok(PathModification {
                entry,
                modified_by_lexicon: true,
                method: "profile-append".to_string(),
                location: profile_path.display().to_string(),
            });
        }

        // This candidate didn't take effect; undo it before trying the next one.
        fs::write(&profile_path, &existing).map_err(|err| err.to_string())?;
    }

    unreachable!("candidate_profiles always returns at least one entry")
}

#[derive(Clone, Copy)]
enum ProfileKind {
    Bash,
    Zsh,
    Login,
}

/// Orders shell startup files by how likely `$SHELL` says they are to be
/// sourced, then lists the remaining common ones as fallbacks, without
/// duplicating any path.
#[cfg(target_os = "linux")]
fn candidate_profiles(home: &str) -> Vec<(PathBuf, ProfileKind)> {
    let bashrc = (PathBuf::from(home).join(".bashrc"), ProfileKind::Bash);
    let zshrc = (PathBuf::from(home).join(".zshrc"), ProfileKind::Zsh);
    let profile = (PathBuf::from(home).join(".profile"), ProfileKind::Login);

    let shell = env::var("SHELL").unwrap_or_default();
    let primary = if shell.contains("zsh") {
        zshrc.clone()
    } else if shell.contains("bash") {
        bashrc.clone()
    } else {
        profile.clone()
    };

    let mut candidates = vec![primary];
    for candidate in [bashrc, profile, zshrc] {
        if !candidates.iter().any(|(path, _)| *path == candidate.0) {
            candidates.push(candidate);
        }
    }
    candidates
}

/// Verifies a PATH edit actually takes effect by spawning the shell in the
/// startup mode that sources the given file, rather than assuming the write
/// succeeded just because it landed on disk.
#[cfg(target_os = "linux")]
fn verify_path_picked_up(kind: ProfileKind, bin_dir: &Path) -> bool {
    use std::process::{Command, Stdio};

    let (shell, args): (&str, &[&str]) = match kind {
        ProfileKind::Bash => ("bash", &["-i", "-c", "echo \"$PATH\""]),
        ProfileKind::Zsh => ("zsh", &["-i", "-c", "echo \"$PATH\""]),
        ProfileKind::Login => ("bash", &["-l", "-c", "echo \"$PATH\""]),
    };

    let output = Command::new(shell).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output();
    match output {
        Ok(out) if out.status.success() => {
            let reported_path = String::from_utf8_lossy(&out.stdout);
            env::split_paths(reported_path.trim()).any(|existing| existing == bin_dir)
        }
        _ => false,
    }
}

/// Undoes whatever `ensure_linux_path`/`ensure_windows_path` did, matched on
/// the method that was actually used at install time.
pub fn reverse_path_modification(path_modification: &PathModification) {
    match path_modification.method.as_str() {
        "symlink" => {
            let _ = fs::remove_file(&path_modification.location);
        }
        "profile-append" => reverse_profile_append(path_modification),
        _ => {}
    }
}

fn reverse_profile_append(path_modification: &PathModification) {
    let location = Path::new(&path_modification.location);
    let Ok(contents) = fs::read_to_string(location) else { return };

    let export_line = format!("export PATH=\"{}:$PATH\"", path_modification.entry);
    let filtered: Vec<&str> = contents
        .lines()
        .filter(|line| line.trim() != crate::MARKER && line.trim() != export_line)
        .collect();
    let mut new_contents = filtered.join("\n");
    if !new_contents.is_empty() {
        new_contents.push('\n');
    }
    let _ = fs::write(location, new_contents);
}

/// Adds `bin_dir` to the current user's PATH registry value if absent, then
/// broadcasts WM_SETTINGCHANGE so running processes pick up the change.
#[cfg(target_os = "windows")]
pub fn ensure_windows_path(bin_dir: &Path) -> Result<PathModification, String> {
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
