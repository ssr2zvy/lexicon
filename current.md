# Current implementation task: finish Lexicon project initialization and discovery

## Purpose

Finish and verify the Lexicon source-code behavior for project initialization, project discovery, configuration-path containment, nested-project rejection, and consistent terminal output.

This file is an implementation instruction, not a task ledger. The permanent Git/worktree procedure belongs in `instructions.md` and must not be repeated in the implementation report.

## Scope

Work only in the Lexicon source code and tests that control:

- `lexicon init <parent-path> <project-name>` parsing and dispatch.
- Project-directory creation.
- `lexicon.toml` generation and parsing.
- Project-root discovery.
- Nested-project detection.
- `sources_directory` validation and resolution.
- Human-facing `[lexicon]` output for these flows.

Do not modify MZA, bundle protocols, installer behavior, HTTP transaction recording, source executable compilation, `data --get`, `data --process`, or SQLite processing in this task.

## Required public behavior

### 1. Initialization command

The public command is:

```bash
lexicon init <parent-path> <project-name>
```

Example:

```bash
lexicon init /projects telugu-data
```

It creates:

```text
/projects/telugu-data/
├── lexicon.toml
└── sources/
```

`<parent-path>` is the existing directory that will contain the project. `<project-name>` is both the new folder name and the logical project name.

### 2. Generated configuration

The generated configuration must have this logical content:

```toml
schema_version = 1

[project]
name = "telugu-data"
sources_directory = "sources"
```

Requirements:

- Serialize TOML safely; do not interpolate unescaped values manually.
- Do not store the absolute project path.
- Resolve `sources_directory` relative to the directory containing `lexicon.toml`.
- Moving the complete project directory must leave the configuration valid.
- Source-specific protocol, discovery, API, processing, and attribution information does not belong in this file.

## Required source-code flow

### 3. CLI parsing and dispatch

The executable flow must be:

```text
main
→ Cli::parse
→ RootCommand::Init
→ init dispatch branch
→ initialize_project(parent_path, project_name)
```

Requirements:

- Parse `parent_path` as `PathBuf`.
- Parse `project_name` as a single project name, not as a second path.
- Keep authoritative project-name validation inside `initialize_project` or the lowest filesystem-mutating function.
- Do not perform redundant validation in dispatch unless Clap uses the same validator for earlier user feedback.
- Update any stale CLI description claiming that dispatch never invokes framework behavior.

### 4. Safe and atomic initialization

`initialize_project` must:

1. Validate the project name.
2. Reject empty names, `.`, `..`, absolute names, path separators, parent traversal, and platform path prefixes.
3. Require the parent path to exist and be a directory.
4. Canonicalize the parent path; canonicalization failure is an error, never a textual fallback.
5. Inspect all ancestors for an existing `lexicon.toml`.
6. Reject initialization beneath an existing Lexicon project.
7. Compute `project_directory = canonical_parent / project_name`.
8. Reject an existing target project path.
9. Create a randomly named, automatically managed staging directory beside the target.
10. Create `sources/` and serialize/write `lexicon.toml` inside the staging directory.
11. Rename the completed staging directory into the final target path.
12. Leave no final project and no task-created staging directory after failure.

Do not use a predictable PID-only staging path. Do not delete a preexisting directory that this invocation did not create. Prefer a `tempfile`-managed directory or an equivalent ownership-safe mechanism.

The result must be all-or-nothing:

```text
complete project exists
```

or:

```text
final project does not exist
```

### 5. Project-root discovery and nesting rejection

Starting from the current working directory:

1. Inspect every ancestor for `lexicon.toml`.
2. If none exists, return a clear outside-project error.
3. If more than one ancestor contains a marker, report the outer and nested projects and stop.
4. If exactly one marker exists, treat it as the candidate project root.
5. Recursively inspect descendants for additional `lexicon.toml` files.
6. Do not follow directory symlinks.
7. Do not merge nested project sources into the outer project.
8. If a nested marker exists, stop before performing the requested operation.

The descendant traversal must not walk indefinitely through generated or data-heavy trees. Prune directories that cannot legally contain Lexicon project roots:

```text
.git/
target/
artifacts/
bundles/
mza/
data/raw/
data/processed/
```

Document this exclusion rule in the source code and test it. Use deterministic traversal or sort discovered nested paths before choosing what to report.

The nesting error must identify both paths and perform no automatic deletion or movement:

```text
[lexicon] ERROR: Nested Lexicon project detected.
[lexicon] Outer project: /projects/outer
[lexicon] Nested project: /projects/outer/tools/inner
[lexicon] Move the nested project outside the outer project, or remove its
[lexicon] lexicon.toml if it should belong to the outer project, then rerun.
[lexicon] No changes were made.
```

### 6. Secure `sources_directory` resolution

Configuration parsing must require and validate:

- Supported `schema_version`.
- A nonempty valid `project.name`.
- A nonempty relative `project.sources_directory`.

Reject `sources_directory` values containing:

- Absolute paths.
- Root components.
- Parent traversal.
- Platform path prefixes.
- Existing symlinks that resolve outside the canonical project root.

Do not use this unsafe pattern:

```rust
candidate
    .canonicalize()
    .unwrap_or_else(|_| candidate.to_path_buf())
```

It permits an escape such as:

```text
project/escaping-link → /outside
sources_directory = "escaping-link/nonexistent-child"
```

Resolve existing components one at a time. If an existing component is a symlink, canonicalize it immediately and ensure it remains under the canonical project root before processing later components. A nonexistent final component may be returned only after every existing parent component has been proven to remain inside the project.

If the final path already exists, require it to be a directory.

## Terminal-output contract

### 7. Human-facing messages

Use the exact marker:

```text
[lexicon]
```

Examples:

```text
[lexicon] Initialized project 'telugu-data' at /projects/telugu-data
[lexicon] Created source 'example-source' at /projects/telugu-data/sources/example-source
[lexicon] WARNING: Nested project detected.
[lexicon] ERROR: No Lexicon project was found.
```

Rules:

- Normal operational output goes to stdout.
- Warnings and errors go to stderr.
- Both `lexicon-cli` and `lexicon-framework` present themselves publicly as `[lexicon]`.
- Do not expose `[lexicon-framework]` as a separate public identity.
- Do not double-prefix output forwarded from the framework.
- Do not print two success messages for one completed source-scaffold operation.
- Clap-generated `--help` and `--version`, machine-readable output, and direct source-executable passthrough may remain unprefixed.

## Required tests

Add or verify named executable tests for:

1. Parsing `lexicon init <parent-path> <project-name>`.
2. Creating exactly `<parent-path>/<project-name>`.
3. Generating the required TOML fields.
4. Rejecting unsafe project names.
5. Rejecting nonexistent and non-directory parents.
6. Rejecting an existing target.
7. Rejecting initialization beneath an existing project.
8. Leaving no final or staging directory after an induced initialization failure.
9. Not deleting a preexisting staging-like directory.
10. Discovering a project from a deeply nested working directory.
11. Detecting multiple ancestor project markers.
12. Detecting a descendant nested project.
13. Not following directory symlinks during descendant discovery.
14. Pruning the explicitly excluded generated/data directories.
15. Rejecting empty, absolute, prefixed, and parent-traversing `sources_directory` values.
16. Rejecting an escaping symlink followed by a nonexistent child.
17. Rejecting a configured source path that is an existing regular file.
18. Moving a completed project and rediscovering it successfully.
19. Prefixing operational success and error output with `[lexicon]`.
20. Avoiding duplicate CLI/framework success output.

A total test count is not sufficient evidence. The implementation report must identify the test function names and what requirement each test reaches.

## Verification

At minimum run:

```bash
cargo test -p lexicon-cli -p lexicon-framework -- --nocapture
```

Also execute the real public CLI against a fresh temporary parent directory and verify:

```text
lexicon init <temporary-parent> <project-name>
cd <temporary-parent>/<project-name>
lexicon source new example-source
```

The public CLI verification must reach the real framework scaffold path. A framework-only command does not prove CLI-to-framework dispatch.

## Required implementation report

Replace this file with a focused source-code report after implementation. Do not include the Git/worktree protocol, task-ledger prose, release rules, or unrelated MZA/bundle discussion.

The report must contain:

1. Exact files changed.
2. Exact functions and types changed.
3. The actual init call chain:

   ```text
   main
   → parser
   → command enum
   → dispatch
   → initialization
   → staging
   → TOML serialization
   → final rename
   ```

4. The actual project-discovery call chain:

   ```text
   current directory
   → ancestor collection
   → nesting validation
   → descendant scan
   → TOML parsing
   → secure sources-directory resolution
   ```

5. Relevant source-code blocks showing the implemented behavior.
6. Test function names mapped to requirements.
7. Exact verification commands and results.
8. Any remaining gaps stated plainly.
9. Confirmation that only this task's Lexicon source code was changed.

Stop after completing and verifying this scope. Do not proceed to source compilation, executable placement, runtime launching, HTTP transaction recording, or processing behavior in this task.
(