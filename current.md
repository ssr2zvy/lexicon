# Current task: complete `lexicon source new` with a required acquisition protocol
## Objective
Implement and verify this public command:
```bash
lexicon source new <source-name> --protocol <protocol>

Both <source-name> and --protocol <protocol> are required.

The only supported protocol in the current implementation is:

http

This task creates a compilable source-development scaffold. It does not perform HTTP requests, compile final source executables, process data, or register the source.

Required command behavior

The following command must succeed:

lexicon source new example-source --protocol http

The following command must be rejected by Clap because --protocol is missing:

lexicon source new example-source

The following command must be rejected because the protocol is unsupported:

lexicon source new example-source --protocol browser

Protocol validation must occur before any source files or directories are created.

Required execution flow

The public execution path must be:

lexicon CLI
→ parse `source new`
→ require source name
→ require `--protocol`
→ invoke lexicon-framework with the source name and protocol
→ framework locates the containing Lexicon project
→ framework reads and validates lexicon.toml
→ framework resolves sources_directory
→ framework validates the source name
→ framework validates the protocol
→ framework rejects an existing source
→ framework creates the source in a temporary staging directory
→ framework writes the complete protocol-specific scaffold
→ framework renames the staging directory to the final source directory
→ framework prints the completed source path and files to edit

The framework owns the successful scaffold output. The CLI must not print a duplicate success message after the framework exits successfully.

CLI parsing contract

The source-new command must represent the protocol as a required named option, not an optional value with a default.

The Clap shape should be equivalent to:

pub struct NewSourceCommand {
    #[arg(value_name = "SOURCE_NAME")]
    pub source_name: String,
    #[arg(long, value_name = "PROTOCOL", required = true)]
    pub protocol: String,
}

If the field is a plain String with no default_value and no Option, Clap may already infer that it is required. Preserve whichever form is clearest, but there must be no default protocol and no successful parse when --protocol is omitted.

--protocol remains a named option even though it is required. The -- prefix identifies the value by name; it does not mean the value is optional.

Project discovery

The framework must begin from the current working directory and use the completed project-discovery behavior:

1. Walk upward to locate the containing lexicon.toml.
2. Reject ambiguous nested Lexicon project layouts.
3. Parse and validate lexicon.toml.
4. Resolve [project].sources_directory safely relative to the project root.
5. Preserve the existing symlink-escape protections.
6. Preserve the existing deterministic descendant scan and pruning behavior.

Do not redesign the completed init or project-discovery implementation in this task.

Validation

Validate the source name before creating anything.

The source name must:

* Not be empty.
* Be one safe directory-name component.
* Not be . or ...
* Not be absolute.
* Not contain / or \.
* Not contain parent traversal.

Validate the protocol before creating anything.

For this task:

http

is the only accepted value.

Do not silently normalize, substitute, or default an unsupported or missing protocol to http.

If the final source directory already exists, return an error and leave it completely unchanged.

Required generated structure

For:

lexicon source new example-source --protocol http

create the source under the project’s configured sources directory:

<project-root>/
└── sources/
    └── example-source/
        ├── source.toml
        ├── discovery.md
        ├── data/
        │   ├── raw/
        │   └── processed/
        ├── get-raw-data/
        │   ├── sessions/
        │   ├── session_status.json
        │   └── get_raw_data_impl/
        │       ├── Cargo.toml
        │       ├── Cargo.lock
        │       └── src/
        │           └── main.rs
        └── process-data/
            ├── sessions/
            ├── session_status.json
            └── process_data_impl/
                ├── Cargo.toml
                ├── Cargo.lock
                ├── src/
                │   └── main.rs
                └── processing/

The common source layout remains stable. The selected protocol controls the acquisition-specific metadata, dependencies, contract, and generated implementation template under get_raw_data_impl.

Future protocols may generate different acquisition-specific contents. Do not implement future protocols in this task.

source.toml

Generate:

schema_version = 1
[source]
name = "example-source"
protocol = "http"

The actual source name supplied by the user must be serialized safely through TOML serialization. Do not construct TOML through unsafe string interpolation.

discovery.md

Generate an initial documentation template for recording how the source was discovered and how its acquisition method was identified.

It should provide headings or prompts for at least:

* Source description.
* Discovery method.
* Acquisition endpoint or location.
* Why HTTP is the correct acquisition protocol.
* Required authentication or access conditions.
* Attribution and usage notes.
* Operational observations.

Do not invent source-specific values.

HTTP acquisition crate

For --protocol http, generate a Rust binary crate under:

get-raw-data/get_raw_data_impl/

Its generated source must:

1. Depend on the existing portable Lexicon Core dependency mechanism.
2. Use the existing released/tagged Core dependency configuration.
3. Not embed a machine-local absolute repository path.
4. Define a source implementation type.
5. Implement the current context-based HttpAcquisition contract.
6. Call the existing Core runner from main.

The generated implementation must be equivalent in behavior to:

use lexicon_framework_core::{
    run_http_source,
    HttpAcquisition,
    HttpAcquisitionContext,
};
struct ExampleSource;
impl HttpAcquisition for ExampleSource {
    fn acquire(
        &self,
        _context: &mut HttpAcquisitionContext,
    ) -> Result<(), String> {
        todo!("implement HTTP acquisition")
    }
}
fn main() {
    if let Err(error) = run_http_source(ExampleSource) {
        eprintln!("[lexicon] ERROR: {error}");
        std::process::exit(1);
    }
}

Adapt identifiers to the source name using the project’s existing identifier-generation behavior.

Do not implement actual HTTP requests in this task.

Process-data crate

Generate the existing process-data binary crate skeleton under:

process-data/process_data_impl/

Processing remains independent from the acquisition protocol.

Preserve the existing portable dependency behavior and current process-data template. Do not implement SQLite processing in this task.

Atomic source creation

Source creation must be atomic.

Required behavior:

1. Resolve the final path:

<sources_directory>/<source-name>

2. Reject the operation if the final path already exists.
3. Create a uniquely named temporary staging directory inside the configured sources directory.
4. Generate the complete source scaffold inside the staging directory.
5. Rename the completed staging directory to the final source path.
6. Remove only the staging directory created by the current operation if generation fails.
7. Never delete or overwrite a preexisting source or unrelated temporary directory.
8. Leave no task-created staging directory after success.

Use a random tempfile-managed staging directory rather than a predictable PID-only path.

Output contract

Successful public output must be produced by the framework and use the [lexicon] prefix:

[lexicon] Created source 'example-source' at <absolute-source-path>
[lexicon] Files to edit next:
[lexicon]   - <absolute-source-path>/source.toml
[lexicon]   - <absolute-source-path>/discovery.md
[lexicon]   - <absolute-source-path>/get-raw-data/get_raw_data_impl/src/main.rs
[lexicon]   - <absolute-source-path>/process-data/process_data_impl/src/main.rs

Every Lexicon-owned human-readable output line must begin with [lexicon].

The CLI must not print another success line after the framework completes.

Failures must return a nonzero exit status and produce a clear [lexicon] ERROR: message through the established top-level error path. Avoid printing the same error in both the framework and CLI if the framework was successfully launched and already reported it.

Required tests

Add or update executable tests covering all of the following:

1. lexicon source new example-source --protocol http parses successfully.
2. Omitting --protocol is rejected by Clap.
3. Omitting the protocol value is rejected by Clap.
4. An unsupported protocol is rejected before filesystem mutation.
5. An unsafe source name is rejected before filesystem mutation.
6. Running outside a Lexicon project fails without creating files.
7. A valid HTTP source produces the complete required directory structure.
8. source.toml contains the correct schema version, source name, and protocol.
9. The HTTP implementation template uses the context-based acquire contract.
10. Generated manifests contain no machine-local absolute repository paths.
11. Both generated Rust crates pass cargo check.
12. An existing source directory is not overwritten or modified.
13. A failed generation leaves no task-created staging directory.
14. A successful generation leaves no staging directory.
15. The public CLI reaches the real framework scaffold behavior.
16. Public successful output contains the required [lexicon] lines.
17. The CLI does not print a duplicate success message.

Tests must exercise behavior rather than merely search the source code for expected strings where an executable test is practical.

End-to-end verification

Create a fresh temporary parent directory and run the actual public flow:

lexicon init <temporary-parent> demo-project
cd <temporary-parent>/demo-project
lexicon source new example-source --protocol http

Then verify:

cargo check --manifest-path \
    sources/example-source/get-raw-data/get_raw_data_impl/Cargo.toml
cargo check --manifest-path \
    sources/example-source/process-data/process_data_impl/Cargo.toml

Also execute the missing-protocol case:

lexicon source new missing-protocol-source

It must fail through Clap and must not create:

sources/missing-protocol-source/

Execute the unsupported-protocol case:

lexicon source new unsupported-source --protocol browser

It must fail and must not create:

sources/unsupported-source/

Scope exclusions

Do not implement or modify:

* Actual HTTP network acquisition.
* Raw request/response transaction recording.
* SQLite processing.
* lexicon source add.
* lexicon build.
* Runtime launching of compiled source implementations.
* MZA.
* Bundling.
* Installation or uninstallation.
* Update behavior.
* Unrelated init/project-discovery behavior.

Required implementation report

After implementation and verification, replace current.md with a function-level report containing:

* Exact files changed.
* Exact functions and types changed.
* The final CLI parsing definition.
* The exact CLI-to-framework call chain.
* The framework validation and atomic-generation flow.
* The generated source.toml.
* The generated HTTP contract implementation shape.
* Test function names mapped to each requirement.
* Exact verification commands.
* Exact test results.
* Any remaining gap or blocker.

Do not report the task as complete unless the required protocol is enforced, the scaffold is created atomically, both generated crates compile, and the end-to-end public CLI test succeeds.