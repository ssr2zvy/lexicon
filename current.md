One correction remains: the pruning logic incorrectly prunes every directory named data.

This block must be removed:

if name == "data" {
    return true;
}

Otherwise Lexicon will miss a nested project at:

project/data/another-project/lexicon.toml

Only these paths were supposed to be pruned:

data/raw/
data/processed/

Correct function:

fn should_prune_descendant_directory(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    if matches!(name, ".git" | "target" | "artifacts" | "bundles" | "mza") {
        return true;
    }
    if matches!(name, "raw" | "processed") {
        return path
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            == Some("data");
    }
    false
}

Add a regression test proving both behaviors simultaneously:

* data/raw/lexicon.toml is ignored.
* data/processed/lexicon.toml is ignored.
* data/nested-project/lexicon.toml is detected.

The existing pruning test is insufficient because pruning the entire data/ directory falsely makes the raw and processed assertions pass. After this correction and test pass, the init/discovery task is complete.