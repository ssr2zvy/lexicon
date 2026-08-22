This report shows the task is not complete. The next micro-step is to fix the observable output contract before changing discovery behavior.

Required correction:

1. The framework owns successful source-scaffold output because it performs the operation.
2. Prefix every framework-generated line with [lexicon].
3. Remove the CLI’s redundant Invoked framework scaffold... message.
4. Add an executable CLI test asserting exactly one success message sequence.

Expected output:

[lexicon] Created source 'example-source' at <path>
[lexicon] Files to edit next:
[lexicon]   - <path>

The CLI dispatch should end like this:

if !status.success() {
    return Err(format!(
        "framework source scaffold step failed with exit status {status}"
    ));
}
Ok(())

It must not print another success line after the framework exits.

After that, the next correction is deterministic, pruned descendant discovery:

* Sort directory entries before traversal.
* Skip symlinks.
* Prune .git, target, artifacts, bundles, mza, data/raw, and data/processed.
* Add explicit tests for pruning and nested-marker detection.

Do not begin HTTP acquisition yet; finish these two acknowledged init/project-discovery gaps first.