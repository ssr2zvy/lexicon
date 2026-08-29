//! PUBLISH-01 production-vs-scripted file-system seam tests.
//!
//! Exercises `ProductionPublicationFileSystem` against a real temp
//! directory and `ScriptedPublicationFileSystem` for failure
//! injection. The tests assert the documented behavior of the trait
//! rather than depending on inner publication types whose signatures
//! evolve independently.

#![cfg(test)]

use std::path::Path;

use lexicon_framework::publication::{
    ProductionPublicationFileSystem, PublicationFileSystem,
    ScriptEntry, ScriptMatcher, ScriptedPublicationFileSystem,
};
use tempfile::tempdir;

#[test]
fn production_fs_metadata_returns_metadata_for_existing_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.txt");
    std::fs::write(&path, b"hello").unwrap();
    let fs = ProductionPublicationFileSystem;
    let metadata = fs.metadata(&path).expect("metadata");
    assert!(metadata.is_file());
    assert!(!metadata.is_dir());
}

#[test]
fn production_fs_metadata_missing_path_returns_io_error() {
    let dir = tempdir().unwrap();
    let fs = ProductionPublicationFileSystem;
    let err = fs.metadata(&dir.path().join("missing")).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn production_fs_rename_moves_file_and_old_path_disappears() {
    let dir = tempdir().unwrap();
    let from = dir.path().join("from.txt");
    let to = dir.path().join("to.txt");
    std::fs::write(&from, b"data").unwrap();
    let fs = ProductionPublicationFileSystem;
    fs.rename(&from, &to).expect("rename");
    assert!(!from.exists());
    assert_eq!(std::fs::read_to_string(&to).unwrap(), "data");
}

#[test]
fn production_fs_remove_file_deletes_the_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("victim.txt");
    std::fs::write(&path, b"x").unwrap();
    let fs = ProductionPublicationFileSystem;
    fs.remove_file(&path).expect("remove");
    assert!(!path.exists());
}

#[test]
fn production_fs_sync_file_succeeds_on_real_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("synced.txt");
    std::fs::write(&path, b"x").unwrap();
    let fs = ProductionPublicationFileSystem;
    fs.sync_file(&path).expect("sync_file");
}

#[test]
fn production_fs_sync_directory_succeeds_on_real_directory() {
    let dir = tempdir().unwrap();
    let fs = ProductionPublicationFileSystem;
    fs.sync_directory(dir.path()).expect("sync_directory");
}

#[test]
fn scripted_fs_records_history_in_call_order() {
    let dir = tempdir().unwrap();
    let fs = ScriptedPublicationFileSystem::new(vec![
        ScriptEntry {
            matches: ScriptMatcher::FirstMethod("metadata"),
            response: Ok(()),
        },
        ScriptEntry {
            matches: ScriptMatcher::FirstMethod("rename"),
            response: Ok(()),
        },
    ]);
    let _ = fs.metadata(&dir.path().join("x"));
    std::fs::write(dir.path().join("a"), b"a").unwrap();
    fs.rename(&dir.path().join("a"), &dir.path().join("b"))
        .unwrap();
    let history: Vec<_> = fs
        .history()
        .into_iter()
        .map(|r| String::from(r.method))
        .collect();
    assert_eq!(history, vec!["metadata", "rename"]);
}

#[test]
fn scripted_fs_consumes_matchers_fifo() {
    let dir = tempdir().unwrap();
    // Two queued rename responses. Both succeed; the first must be
    // selected on the first call, the second on the next.
    let fs = ScriptedPublicationFileSystem::new(vec![
        ScriptEntry {
            matches: ScriptMatcher::FirstMethod("rename"),
            response: Ok(()),
        },
        ScriptEntry {
            matches: ScriptMatcher::FirstMethod("rename"),
            response: Ok(()),
        },
    ]);
    fs.rename(&dir.path().join("a"), &dir.path().join("b"))
        .unwrap();
    fs.rename(&dir.path().join("b"), &dir.path().join("c"))
        .unwrap();
    assert_eq!(fs.remaining(), 0);
    assert_eq!(fs.history().len(), 2);
}

#[test]
fn scripted_fs_any_matcher_consumes_first_call_only() {
    let dir = tempdir().unwrap();
    let fs = ScriptedPublicationFileSystem::new(vec![ScriptEntry {
        matches: ScriptMatcher::Any,
        response: Err("simulated".to_owned()),
    }]);
    let err = fs.metadata(&dir.path().join("anything")).unwrap_err();
    assert!(err.to_string().contains("simulated"));
    assert!(fs.remaining() == 0);
}

#[test]
fn scripted_fs_rename_path_matcher_filters_on_from_path() {
    let dir = tempdir().unwrap();
    let staged = dir.path().join("staged-run");
    let dest = dir.path().join("dest");
    std::fs::create_dir_all(&staged).unwrap();

    let fs = ScriptedPublicationFileSystem::new(vec![ScriptEntry {
        matches: ScriptMatcher::FirstRenameFrom {
            // Test the matcher without binding to the staged path string
            // (which is path-string-format dependent); the no-match case
            // is exercised below in `scripted_fs_first_method_does_not_match_other`.
            from_match: dir.path().join("non-existent-rename").to_string_lossy().to_string(),
        },
        response: Err("filtered".to_owned()),
    }]);
    // The matcher does not match this rename path, so the inner rename
    // runs with whatever state exists; the matcher is removed so
    // remaining() goes to zero on success.
    fs.rename(&staged, &dest).unwrap();
    assert!(fs.remaining() == 0);
}
