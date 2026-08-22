The main init flow is correct. These are the exact changes still needed and why.

1. Correct sources_directory containment

Current logic checks the unresolved path text:

let resolved = project_root.join(path);
resolved.strip_prefix(&canonical_project_root)

That does not detect a symlink escape.

Example:

project/sources → /somewhere/outside-project

The textual path still begins with project/, so the current check accepts it even though writes go outside the project.

Change it so the resolved filesystem location is checked against the canonical project root. Also reject:

* Empty paths.
* Absolute paths.
* ...
* Windows path prefixes such as C:.
* Existing symlinks that escape the project root.

This prevents a configured sources directory from writing outside the project.

2. Make initialization atomic

Current flow creates:

project/sources/

before writing lexicon.toml.

If writing the TOML fails, a partially initialized project remains. There is also a race between:

project_directory.exists()

and:

create_dir_all(...)

Change the flow to:

1. Create a uniquely named temporary directory beside the intended project.
2. Create sources/ and lexicon.toml inside it.
3. Rename the completed temporary directory to the final project name.
4. Remove only the temporary directory if initialization fails.

The final project should either exist completely or not exist at all.

3. Bound the descendant project scan

visit_descendants currently traverses every directory below the project, including potentially:

.git/
target/
artifacts/
data/raw/
data/processed/

As acquired data grows, every Lexicon command could scan thousands or millions of files and directories just to detect another lexicon.toml.

Keep recursive nested-project detection, but define directories that cannot contain valid Lexicon projects and prune them from traversal. Do not follow symlinks.

Also make the result deterministic—either sort directory entries or collect and sort all nested project paths before reporting them.

4. Remove duplicate init validation

The project name is validated in both:

dispatch()
initialize_project()

initialize_project() must remain authoritative because it performs the filesystem operation and could be called from somewhere other than this dispatch branch.

The separate dispatch validation is redundant. Remove it or make Clap use the same validator while still retaining validation inside initialize_project().

5. Update the stale CLI description

This text is no longer true:

This parser validates the command interface ... without invoking framework behavior.

The source command now launches lexicon-framework. Update the description to reflect actual dispatch behavior.

6. Add the [lexicon] output contract

Operational messages should become:

[lexicon] Initialized project 'telugu-data'.
[lexicon] Created source 'example-source'.
[lexicon] WARNING: Nested project detected.
[lexicon] ERROR: No Lexicon project was found.

Specifically:

* Prefix CLI success messages.
* Prefix framework success messages.
* Prefix the final error printed by main().
* Prefix nested-project and configuration errors.
* Use stdout for normal messages.
* Use stderr for warnings and errors.
* Do not prefix the same framework output again when it passes through the CLI.

7. Add named tests for the actual requirements

“11 tests passed” is insufficient evidence without identifying what they cover. Add or identify tests for:

* Correct <parent-path>/<project-name> creation.
* Exact TOML fields.
* Ancestor nesting.
* Descendant nesting.
* Existing target rejection.
* Project movement portability.
* Symlink escape through sources_directory.
* Absolute and parent-traversing source paths.
* Failure without a partial project remaining.
* Recursive-scan exclusions.
* [lexicon] success and error formatting.

The two critical correctness changes are secure sources_directory resolution and atomic initialization. The remaining items make discovery scalable, output consistent, and verification credible.