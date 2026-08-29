//! PUBLISH-01 filesystem seam between production delegation and tests.
//!
//! The audit demands that production code call every file-system
//! operation through a typed seam so tests can script failure
//! sequences without re-implementing the publication pipeline.
//! `ProductionPublicationFileSystem` delegates to real OS calls;
//! `ScriptedPublicationFileSystem` lets a test fail the exact Nth
//! operation the audit cared about.

use std::fs;
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Abstraction over the file-system effects the publication pipeline
/// may invoke. Production callers accept any clonable implementation;
/// tests inject [`ScriptedPublicationFileSystem`] instead.
pub trait PublicationFileSystem: Send + Sync {
    fn metadata(&self, path: &Path) -> io::Result<fs::Metadata>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn sync_file(&self, path: &Path) -> io::Result<()>;
    fn sync_directory(&self, path: &Path) -> io::Result<()>;
    /// Sleep before retrying a transient failure (Windows ETXTBSY-style
    /// races). Production pauses; the fake can advance its clock.
    fn sleep_before_retry(&self, delay: Duration);
}

/// Production implementation that invokes real OS calls.
pub struct ProductionPublicationFileSystem;

impl PublicationFileSystem for ProductionPublicationFileSystem {
    fn metadata(&self, path: &Path) -> io::Result<fs::Metadata> {
        path.metadata()
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn sync_file(&self, path: &Path) -> io::Result<()> {
        let file = fs::OpenOptions::new().read(true).open(path)?;
        file.sync_all()
    }

    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        let dir = fs::File::open(path)?;
        dir.sync_all()
    }

    fn sleep_before_retry(&self, delay: Duration) {
        std::thread::sleep(delay);
    }
}

/// Scripted test implementation. The audit's PROCESS-01 / PUBLISH-01 lists
/// specific Nth-operation failure modes. Calls are dispatched in FIFO
/// order, and each script entry's first matching call consumes the entry.
pub struct ScriptedPublicationFileSystem {
    inner: Mutex<ScriptedInner>,
}

struct ScriptedInner {
    history: Vec<CallRecord>,
    script: Vec<ScriptEntry>,
}

#[derive(Clone, Debug)]
pub struct CallRecord {
    pub method: &'static str,
    pub from: Option<PathBuf>,
    pub to: Option<PathBuf>,
    pub outcome: Result<(), String>,
}

#[derive(Clone, Debug)]
pub struct ScriptEntry {
    pub matches: ScriptMatcher,
    pub response: Result<(), String>,
}

/// Filter that decides which calls a script entry applies to.
#[derive(Clone, Debug)]
pub enum ScriptMatcher {
    /// Always match the next call regardless of method or path. The
    /// catch-all entry used to script "no further calls".
    Any,
    /// Match the first call whose method name equals this string and
    /// whose `from` path matches the supplied string. Useful for "the
    /// second rename fails".
    FirstRenameFrom { from_match: String },
    /// Match the first call whose method name equals this string. Used
    /// for "the Nth sync fails" scripting.
    FirstMethod(&'static str),
}

impl ScriptedPublicationFileSystem {
    pub fn new(script: Vec<ScriptEntry>) -> Self {
        Self {
            inner: Mutex::new(ScriptedInner {
                history: Vec::new(),
                script,
            }),
        }
    }

    /// Quick-start: a script that fails only when called once, then
    /// succeeds thereafter. This is the minimum lethal injection.
    pub fn fail_once(method: &'static str, message: &str) -> Self {
        Self::new(vec![ScriptEntry {
            matches: ScriptMatcher::FirstMethod(method),
            response: Err(message.to_owned()),
        }])
    }

    /// Pop the first matching scripted entry.
    fn next_outcome(
        &self,
        method: &'static str,
        from: Option<&Path>,
        to: Option<&Path>,
    ) -> Result<(), String> {
        let mut guard = self.inner.lock().expect("poisoning");
        if let Some(index) = guard.script.iter().position(|entry| match entry.matches {
            ScriptMatcher::Any => true,
            ScriptMatcher::FirstMethod(m) => m == &method,
            ScriptMatcher::FirstRenameFrom { from_match } => {
                method == "rename"
                    && from.is_some_and(|p| p.to_string_lossy() == *from_match)
            }
        }) {
            let entry = guard.script.remove(index);
            let _ = to;
            let outcome = entry.response.clone();
            guard.history.push(CallRecord {
                method,
                from: from.map(|p| p.to_path_buf()),
                to: to.map(|p| p.to_path_buf()),
                outcome: outcome.clone(),
            });
            outcome
        } else {
            Ok(())
        }
    }

    /// Snapshot of recorded calls, oldest first.
    pub fn history(&self) -> Vec<CallRecord> {
        self.inner.lock().expect("poisoning").history.clone()
    }

    /// Remaining unconsumed scripted entries.
    pub fn remaining(&self) -> usize {
        self.inner.lock().expect("poisoning").script.len()
    }
}

impl PublicationFileSystem for ScriptedPublicationFileSystem {
    fn metadata(&self, path: &Path) -> io::Result<fs::Metadata> {
        let outcome = self.next_outcome("metadata", Some(path), None);
        outcome.map_err(io_error)?;
        path.metadata()
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let outcome = self.next_outcome("rename", Some(from), Some(to));
        outcome.map_err(io_error)?;
        fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        let outcome = self.next_outcome("remove_file", Some(path), None);
        outcome.map_err(io_error)?;
        fs::remove_file(path)
    }

    fn sync_file(&self, path: &Path) -> io::Result<()> {
        let outcome = self.next_outcome("sync_file", Some(path), None);
        outcome.map_err(io_error)?;
        let file = fs::OpenOptions::new().read(true).open(path)?;
        file.sync_all()
    }

    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        let outcome = self.next_outcome("sync_directory", Some(path), None);
        outcome.map_err(io_error)?;
        let dir = fs::File::open(path)?;
        dir.sync_all()
    }

    fn sleep_before_retry(&self, delay: Duration) {
        // Test intent: keep the timeline short. We honor a sleep of < 5s
        // by playing it real; longer durations no-op so debug-mode
        // scripts don't depend on time.
        if delay < Duration::from_secs(5) {
            std::thread::sleep(delay);
        }
    }
}

fn io_error(message: String) -> io::Error {
    io::Error::new(ErrorKind::Other, message)
}

/// Convenience helper for tests: build a canonical file-system snapshot
/// at `root` containing `relative` paths, all empty. Used to seed staged
/// factories before exercising publication paths.
pub fn seed_empty_staging(root: &Path, relative: &[&str]) -> io::Result<()> {
    fs::create_dir_all(root)?;
    for entry in relative {
        let target = root.join(entry);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        if !target.exists() {
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&target)?;
        }
    }
    Ok(())
}

/// Helper used by tests: ensure a script's history matches an expected
/// list of (method, outcome_ok). The audit's PROCESS-01 / PUBLISH-01
/// tests exercise failure modes whose scripts we ship below.
pub fn assert_history_methods(history: &[CallRecord], expected: &[&'static str]) {
    assert_eq!(
        history.len(),
        expected.len(),
        "expected {} calls got {} history {:?}",
        expected.len(),
        history.len(),
        history.iter().map(|r| r.method).collect::<Vec<_>>()
    );
    for (i, (record, method)) in history.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            record.method, *method,
            "history[{i}] expected {method} got {}",
            record.method
        );
        assert!(
            record.outcome.is_ok(),
            "history[{i}] method {method} should succeed; got {:?}",
            record.outcome
        );
    }
}

/// Wall clock for tests that want a stable monotonic anchor.
pub fn monotonic_anchor() -> SystemTime {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(UNIX_EPOCH, |d| UNIX_EPOCH + d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn production_helper_succeeds_against_real_filesystem() {
        let dir = tempdir().expect("tempdir");
        let src = dir.path().join("src.txt");
        let dst = dir.path().join("dst.txt");
        std::fs::write(&src, b"data").expect("write");
        let fs = ProductionPublicationFileSystem;
        fs.rename(&src, &dst).expect("rename");
        assert!(!src.exists());
        assert_eq!(std::fs::read(&dst).expect("read"), b"data");
    }

    #[test]
    fn scripted_fs_returns_first_script_outcome_and_then_succeeds() {
        let dir = tempdir().expect("tempdir");
        let src = dir.path().join("a.txt");
        let dst = dir.path().join("b.txt");
        std::fs::write(&src, b"x").expect("write");

        // First call (metadata) fails with our scripted message.
        let mut script = ScriptedPublicationFileSystem::new(vec![ScriptEntry {
            matches: ScriptMatcher::FirstMethod("metadata"),
            response: Err("simulated_os_failure".to_owned()),
        }]);
        let outcome = script.metadata(&src);
        assert!(outcome.is_err());

        // Subsequent calls succeed; we reach a regular rename next.
        script.rename(&src, &dst).expect("rename on second try");
        assert!(!src.exists());
        assert!(dst.exists());
        assert!(script.history().len() == 2);
    }

    #[test]
    fn scripted_fs_records_history_in_call_order() {
        let dir = tempdir().expect("tempdir");
        let fs = ScriptedPublicationFileSystem::fail_once("sync_file", "won't sync");
        let _ = fs.metadata(&dir.path().join("missing"));
        let history = fs.history();
        assert_eq!(
            history.first().map(|r| r.method),
            Some("metadata")
        );
    }

    #[test]
    fn empty_staging_helper_seeds_files() {
        let dir = tempdir().expect("tempdir");
        seed_empty_staging(dir.path(), &["a.txt", "nested/b.txt"]).expect("seed");
        assert!(dir.path().join("a.txt").is_file());
        assert!(dir.path().join("nested").is_file());
        assert!(dir.path().join("nested/b.txt").is_file());
    }

    #[test]
    fn fail_once_helper_surfaces_failure_on_every_path() {
        let dir = tempdir().expect("tempdir");
        let fs = ScriptedPublicationFileSystem::fail_once("sync_file", "fsync refused");
        let err = fs.sync_file(&dir.path().join("ghost.txt")).unwrap_err();
        assert!(err.to_string().contains("fsync refused"));
        assert_eq!(fs.remaining(), 0);
    }
}
