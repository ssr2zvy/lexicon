Task ledger and release rule

Rule: from now on, task updates are recorded in this file and the repository change pushed is only this file. Local implementation work may remain uncommitted until explicitly requested, but this note is the single committed record of progress.

Status: the two correctness issues called out in the earlier task note were fixed in the local implementation and verified with the project tests.

Verified command:

cargo test -p lexicon-cli -p lexicon-framework -- --nocapture

Fresh result:

- lexicon-cli: 10 passed, 0 failed
- lexicon-framework core: 1 passed, 0 failed
- lexicon-framework main: 4 passed, 0 failed

What was fixed in the code:

1. Symlink containment
   - The unsafe fallback that used the textual path after canonicalize failed was replaced with a component-by-component path resolution routine.
   - A path like "escaping-symlink/nonexistent-child" is now rejected before it can escape the project root.

2. Temporary initialization safety
   - The project initializer now uses a random tempfile-managed staging directory instead of a predictable ".{project_name}.tmp-<pid>" path.
   - It no longer removes a preexisting unrelated directory and it leaves no temporary directory behind after a successful init.

3. Current working delta
   - The repository currently has local implementation changes in the Rust code and Cargo lockfile, but they are intentionally left in the working tree for the current task flow.
   - The committed task record is kept in this file only, so the note is the authoritative change log for the latest response while preserving the live implementation edits.

This file is therefore the single task-tracking source for future updates, and future commits for this workflow should only include this file unless a different instruction explicitly overrides the rule.

