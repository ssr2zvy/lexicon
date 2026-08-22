

⸻

Implement the revised lexicon init and project-discovery contract.

1. Command shape

Change initialization to:

lexicon init <parent-path> <project-name>

Example:

lexicon init /projects telugu-data

must create:

/projects/telugu-data/
├── lexicon.toml
└── sources/

<parent-path> identifies the directory that will contain the new project folder. <project-name> supplies both the new folder name and the logical project name.

2. Generated lexicon.toml

Generate exactly this logical structure:

schema_version = 1
[project]
name = "telugu-data"
sources_directory = "sources"

Requirements:

* Serialize TOML safely rather than interpolating unescaped user input.
* sources_directory must remain relative to the lexicon.toml location.
* Do not record the parent path or another absolute project path.
* Moving the complete project directory must not invalidate its configuration.

3. Init flow

The implementation flow must be:

1. Clap parses <parent-path> as a PathBuf and <project-name> as a string.
2. Validate the project name as a safe single directory name.
3. Reject names containing /, \, .., or absolute-path syntax.
4. Verify that <parent-path> exists and is a directory.
5. Canonicalize the parent path.
6. Calculate:

project_directory = parent_path / project_name

7. Refuse to overwrite an existing target.
8. Search upward from the parent path for an existing lexicon.toml.
9. If one exists, reject initialization because the new project would be nested.
10. Create the project directory.
11. Create sources/.
12. Write lexicon.toml.
13. Report the absolute initialized project path.

A failure must not delete, modify, or “fix” another project automatically.

4. Project-root discovery

Replace “stop at the first marker” with full nesting validation:

1. Starting from the current directory, inspect every ancestor for lexicon.toml.
2. If more than one ancestor contains it, report nested projects and stop.
3. If none contains it, report that the command is outside a Lexicon project.
4. If exactly one exists, treat it as the candidate active project.
5. Recursively search downward from that root for additional lexicon.toml files.
6. Do not follow directory symlinks.
7. If another marker is found, report the outer and nested project paths and stop.
8. Do not combine their sources or continue the requested operation.
9. If no nesting exists, parse the root configuration and continue.

The error should resemble:

Nested Lexicon project detected.
Outer project: /projects/outer
Nested project: /projects/outer/tools/inner
Move the nested project outside the outer project, or remove its
lexicon.toml if it should belong to the outer project, then rerun.
No changes were made.

5. Configuration validation

When reading lexicon.toml, validate:

* schema_version is supported.
* project.name is present and valid.
* project.sources_directory is relative.
* It contains no parent traversal.
* Resolving it cannot escape the project root.

Then resolve:

sources_path = project_root / sources_directory

6. Required tests

Add tests covering:

* Parsing both init arguments.
* Correct target path construction.
* Exact generated TOML fields.
* Rejection of unsafe project names.
* Rejection of a nonexistent/non-directory parent.
* Rejection when the target already exists.
* Rejection when initializing beneath an existing Lexicon project.
* Discovery from a deeply nested working directory.
* Detection of two ancestor project markers.
* Detection of a descendant nested project.
* Rejection of absolute or escaping sources_directory.
* Moving a completed project directory and successfully rediscovering it afterward.
* Confirmation that errors leave existing files unchanged.

Required return report

Return an exact function-level flow, using the real function and type names after implementation:

CLI parse
→ command enum variant
→ dispatch function
→ init function
→ path/name validation
→ nesting detection
→ directory creation
→ TOML serialization

Also return the project-discovery flow:

current directory
→ ancestor collection
→ active-root selection
→ descendant scan
→ TOML parsing
→ sources-directory resolution

For every step, identify:

* The source file.
* Function/type name.
* Input.
* Output or error.
* Tests that exercise it.

Include the exact verification commands and results. Do not describe a flow as implemented unless an executable test reaches it.