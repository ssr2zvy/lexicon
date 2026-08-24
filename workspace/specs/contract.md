
Lexicon Unified Operator Host Framework

Status

This document defines the selected architecture for Lexicon acquisition, processing, builds, and runtime supervision. The architecture is named the Lexicon Unified Operator Host Framework.

It combines one installed control executable, a reusable framework library, a narrow Core library, Lexicon-managed native source runners, ordinary sequential Rust for source-specific behavior, in-process foreground orchestration, and same-binary operator-host supervision for background work.

The governing division is:

> Lexicon controls the supported entrypoint, build, runtime admission, HTTP-and-recording effect, and session supervision. Source implementations control arbitrary trusted Rust computation around that effect.

This is one architecture. Individual sources do not choose among alternate execution models.

1. Goals

Lexicon must:

• provide one dependable intended path for acquisition and processing;
• prevent accidental replacement of startup orchestration;
• validate source contracts at compile time;
• validate source workspaces and artifacts during the supported build;
• validate runtime identity, compatibility, and capabilities before execution;
• centralize HTTP transport, transaction recording, raw-byte preservation, metadata redaction, and session integration;
• preserve ordinary sequential Rust for arbitrary response-dependent source logic;
• retain native Cargo builds and standalone native execution;
• preserve transactional publication of acquisition and processing runtimes;
• state honestly what trusted native Rust can bypass.

Lexicon does not attempt to capability-confine trusted source developers. A managed entrypoint, Rust interface, Cargo dependency, or process boundary does not prevent linked native code from using sockets, files, subprocesses, or FFI.

2. Package boundaries

The repository has three principal packages:

```text
lexicon-cli/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── frontend.rs
    └── operator_host.rs

lexicon-framework/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── commands/
    ├── scaffold/
    ├── build/
    ├── publication/
    ├── acquisition/
    ├── processing/
    └── supervision/

lexicon-core/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── contracts/
    ├── protocols/
    │   └── http/
    ├── build/
    ├── runtime/
    ├── sessions/
    ├── raw/
    └── processing/
```

Only lexicon-cli produces an installed control executable. Its executable name is lexicon.

lexicon-framework is a reusable Rust library. It owns command semantics and operational policy.

lexicon-core is a reusable Rust library. It owns invariant-sensitive contracts and mechanics shared by the framework and managed source runtimes.

The dependency graph is:

```text
installed lexicon executable
└── lexicon-framework library
    └── lexicon-core library

generated acquisition runner
├── lexicon-core library
└── source implementation library
    ├── lexicon-core library
    └── source-specific dependencies

generated processing runner
├── lexicon-core library
└── processing implementation library
```

The framework library is not linked into source runtimes. Core is. There is no independently installed lexicon-framework executable.

3. Command routing and process model

The lexicon executable owns command-line presentation. The framework library owns project semantics and filesystem mutations.

```text
lexicon --version             → CLI presentation
lexicon init                  → framework library
lexicon source create         → framework library
lexicon source build          → framework library
lexicon build                 → framework library
lexicon data --get            → framework library
lexicon data --process        → framework library
```

The CLI parses arguments and renders typed results. It does not duplicate scaffold rules, build mechanics, runtime validation, publication behavior, or session policy.

Foreground execution

Foreground commands execute framework logic in the original lexicon process:

```text
user
→ lexicon frontend and supervisor process
→ lexicon-framework library
→ lexicon-core library
→ managed source runtime child
```

The lexicon process remains alive while acquisition or processing runs and supervises the child runtime.

Background execution

For --bg, lexicon creates a durable invocation record and re-executes its exact binary in a reserved internal role:

```text
initial lexicon frontend
→ lexicon __operator-host <invocation-reference>
→ lexicon-framework library
→ lexicon-core library
→ managed source runtime child
```

The initiating process exits only after the operator host confirms durable session ownership. The operator host owns the session lock, child lifecycle, cancellation, exit observation, and terminal reconciliation.

The operator-host invocation is versioned because a detached process may outlive an installation upgrade. It is an internal protocol, not a public framework API.

This provides an independently living supervisor only when lifetime independence is required, without installing or versioning a second framework executable.

4. Source acquisition contract

Every HTTP source exports one capability-aware, versioned Rust descriptor from its implementation library.

```rust
use std::ffi::OsString;

use lexicon_core::http::{
    AcquisitionResult,
    HttpAcquisitionContext,
    HttpCapability,
    HttpSourceContractV1,
};

pub const SOURCE: HttpSourceContractV1 =
    HttpSourceContractV1::new(acquire)
        .with_resume(resume)
        .requires(HttpCapability::ClientCertificateV1);

pub fn acquire(
    context: &mut HttpAcquisitionContext,
    args: &[OsString],
) -> AcquisitionResult<()> {
    // Ordinary sequential source-specific Rust.
    Ok(())
}
```

Only acquire is mandatory. Optional handlers and required capabilities are registered through the typed descriptor.

The compiler verifies:

• the descriptor type;
• the mandatory acquisition function signature;
• registered optional-handler signatures;
• typed capability and extension declarations;
• compatibility between source descriptor and managed runner code.

The compiler does not verify the semantic honesty of the implementation or prove that all native effects use Core.

5. Source workspace and managed runner

Each HTTP acquisition operation uses a checked-in Cargo workspace:

```text
sources/example-source/http/
├── source.toml
├── discovery.md
├── data/
│   ├── raw/
│   └── processed/
├── get-raw-data/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── sessions/
│   ├── session_status.json
│   ├── get-raw-data-impl/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   ├── lexicon-runner/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   └── runtime/
└── process-data/
    ├── sessions/
    ├── session_status.json
    ├── process-data-impl/
    └── runtime/
```

The source author edits:

• source-editable fields in source.toml;
• discovery.md;
• get-raw-data-impl/Cargo.toml for source-specific dependencies;
• get-raw-data-impl/src/lib.rs;
• additional modules under get-raw-data-impl/src/.

Lexicon generates and contractually controls:

• the acquisition workspace contract;
• lexicon-runner/Cargo.toml;
• lexicon-runner/src/main.rs;
• runner identity;
• runner template version;
• the binary selected by the supported build.

Cargo.lock is Cargo-managed and committed.

A representative managed runner is:

```rust
use std::process::ExitCode;

use lexicon_core::http::{
    runner,
    HttpSourceContractV1,
    RuntimeIdentity,
};

const IDENTITY: RuntimeIdentity =
    RuntimeIdentity::http_acquisition("example-source", 1);

const SOURCE: HttpSourceContractV1 =
    source_implementation::SOURCE;

fn main() -> ExitCode {
    runner::run(IDENTITY, &SOURCE)
}
```

The source does not provide the supported acquisition main.

“Lexicon-managed” means Lexicon generates, validates, and selects the runner through the supported build. It does not mean filesystem permissions prevent the project developer from editing it. A mismatched runner is rejected rather than silently overwritten.

The published runtime is one native executable statically linked with Core, the source implementation library, and their Rust dependencies. Lexicon exposes no dynamic Rust plugin ABI.

6. Capability and extension model

The base source contract remains small. New protocol functionality is introduced through versioned Core capabilities.

A capability may be:

• optional: a source may use the Core facility or use ordinary source-specific Rust when the effect is not required to pass through Core;
• required by a source: the descriptor declares that the source cannot run unless the selected Core/runtime supplies the capability;
• mandatory for an effect: Lexicon exposes the feature only through a Core operation, and Core’s guarantees apply to uses of that operation.

Compile time verifies descriptor and handler types. Build time verifies that the selected Core and runner provide every declared capability. Runtime admission verifies that the parent, child, descriptor, and invocation agree on the capability set before source code runs.

Capabilities are added in response to demonstrated protocol requirements. They do not form a general plugin ABI, workflow language, or policy engine.

7. Build contract

Core exposes opaque validated build states:

```text
SourceLocation
→ DiscoveredSource
→ ValidatedSource
→ ValidatedOperationWorkspace
→ ReproducibleBuildPlan
→ StagedArtifact
→ VerifiedRuntime
→ PublishedRuntime
```

Invariant-sensitive build APIs accept validated state values rather than arbitrary paths.

The framework owns command policy, diagnostics, cancellation, source selection, and paired acquisition/processing publication. Core owns invariant-sensitive mechanics.

For acquisition, the supported build:

1. Discovers the containing project.
2. Resolves the configured source directory.
3. Validates source identity, protocol, metadata, and contract version.
4. Validates the operation workspace and committed lockfile.
5. Validates the managed runner and template version.
6. Validates the source implementation library target.
7. Validates the exact compatible Core dependency.
8. Runs Cargo metadata in locked mode.
9. Constructs an exact native release build plan.
10. Builds the managed runner for the current machine.
11. Uses an isolated temporary Cargo target directory.
12. Reads Cargo JSON artifact messages.
13. Selects exactly the expected executable artifact.
14. Hashes the executable.
15. Runs a bounded runtime-information probe.
16. Validates identity, operation, protocol, Core contract, and capabilities.
17. Stages the runtime bundle.

Processing follows its corresponding contract.

Builds remain native release builds using --locked, isolated target directories, explicit package and binary targets, Cargo JSON artifact selection, staged runtime bundles, transactional publication, and rollback.

Acquisition and processing are published as one paired transaction. If either build, verification, staging, or publication fails, both previous runtime bundles survive.

Ordinary acquisition and processing never invoke Cargo. Rust and Cargo are build requirements, not runtime requirements.

8. Runtime admission

Runtime validation occurs on both sides of the source-process boundary.

Before launch, the parent validates:

• runtime.json schema;
• executable hash;
• runtime protocol version;
• source, protocol, and operation identity;
• Core contract version;
• compiled capability list;
• compatibility with the requested command.

Inside the child, linked Core validates:

• invocation-envelope version;
• project, source, protocol, and operation identity;
• session identity;
• execution mode;
• compiled source descriptor;
• required capability availability.

Only then does Core call acquire.

The contract connects across phases:

```text
compile time
    source descriptor and functions have the required Rust types

build time
    runner, source descriptor, Cargo graph, Core version,
    artifact identity, and capabilities agree

runtime
    parent and child agree on identity, invocation version,
    contract version, capabilities, operation, and session
```

A runtime that does not match the intended pattern is rejected by the supported build or runtime admission path.

9. Source-specific arguments

For:

```text
lexicon data --get example-source --protocol http -- <source-args>
```

the CLI preserves every value after -- as OsString. Neither the CLI nor framework interprets source-specific semantics.

The runner receives a reserved internal invocation envelope, followed by -- and the untouched source arguments. The source receives &[OsString] and may use Clap or another parser.

Lexicon validates its own arguments. The source validates source-specific arguments.

Raw source argument values are not persisted by default because Lexicon cannot know whether they contain credentials, signed URLs, personal data, or harmless configuration. A source may explicitly record a safe redacted summary through Core.

10. HTTP execution and raw-data contract

The supported HTTP effect API is:

```rust
let transaction = context.execute(request)?;
```

For each physical HTTP attempt submitted through Core, Core must:

1. Allocate a unique transaction identity and staging directory.
2. Finalize the effective request.
3. Persist redacted request metadata.
4. Persist the exact request-body bytes supplied to transport, when present.
5. Perform one physical HTTP exchange.
6. Persist response metadata or a transport-failure record.
7. Stream undecoded HTTP entity-body bytes to raw storage.
8. Hash while streaming.
9. Atomically finalize the transaction or leave a recognizable partial record after interruption.
10. Update session progress.
11. Return a RecordedTransaction only after durable recording completes.

The raw transaction shape is:

```text
data/raw/<timestamp>-<id>/
├── request/
│   ├── metadata.json
│   └── body
└── response/
    ├── metadata.json
    └── body
```

“Exact response body” means HTTP entity-body bytes after transfer framing and before content decoding. It does not mean TLS records, TCP packets, or HTTP/2 frames.

Core must not transparently replace stored data with decoded gzip, Brotli, or other content. A decoded reader may be offered separately.

Every physical retry attempt receives its own transaction. Every redirect exchange receives its own transaction. Neither may be invisibly collapsed below the recorder.

Source code receives a recorded transaction rather than a live unrecorded response, then performs parsing, decoding, validation, and source-specific interpretation using ordinary Rust.

11. Secret handling

Core redacts managed metadata before persistence. Mandatory sensitive metadata includes at least:

• Authorization;
• Proxy-Authorization;
• Cookie;
• Set-Cookie;
• explicitly marked sensitive headers;
• explicitly marked sensitive query parameters.

Exact arbitrary body preservation and universal body-secret removal are incompatible when credentials are themselves present in a request or response body. Core therefore does not claim generic semantic redaction of exact raw bodies.

Any future encryption, exclusion, or protected-body policy must be explicit and must not silently weaken raw-data fidelity. Source-authored files and logs remain outside Core’s redaction guarantee.

12. Sessions and supervision

In foreground mode, the original lexicon process supervises the source runtime. In background mode, the re-executed operator host supervises it.

```text
supervising lexicon process
├── select, create, or resume a session
├── acquire session locks
├── apply --abandon-past-fail
├── launch the source runtime
├── observe process exit and signals
└── reconcile abnormal termination

linked Core inside the source runtime
├── validate the invocation
├── enter running state
├── record transaction progress
├── commit checkpoints
├── record ordinary source failure
└── record normal completion

source implementation
└── decide source-specific continuation and checkpoint meaning
```

The root session_status.json is the current summary. Detailed durable history belongs under sessions/<session-id>/. Updates use atomic replacement or an equivalent transactional mechanism.

An ordinary source error is recorded by Core. A panic, abort, forced exit, or crash is observed and reconciled by the parent supervisor while completed transactions remain intact.

After machine or supervisor loss, the next invocation detects stale ownership and reconciles durable state. Core provides durable transaction and checkpoint primitives, but the source decides what resumption means for its own logic.

13. Processing

Processing remains separate from acquisition. It has its own implementation, managed runner, runtime, sessions, and status.

Processing reads protocol-scoped raw transactions and creates the source-specific SQLite database. It does not alter the acquisition raw-data contract.

The framework and Core apply corresponding build validation, runtime admission, supervision, staging, and publication guarantees to processing. Acquisition and processing runtime updates remain paired during publication.

14. Enforcement model

Compiler-enforced

• The source descriptor has the required Rust type.
• The mandatory acquisition function has the required signature.
• Registered optional handlers have the required signatures.
• Managed runner code can reference the source contract.
• Core is linked into a conforming managed runtime.

Build-tool-enforced

• The supported target uses the Lexicon-managed runner.
• A source-owned main cannot replace the selected entrypoint through the supported build.
• Runner template and workspace shape match declared versions.
• Cargo metadata and the lockfile satisfy the build contract.
• The intended executable artifact is selected and validated.
• Failed builds preserve existing runtimes.
• Acquisition and processing are published transactionally.

Runtime-enforced

• Parent and child validate invocation and runtime identity.
• Required Core capabilities exist before acquisition begins.
• Source arguments reach source code as native strings.
• Sessions are created, transitioned, and reconciled through the supported path.
• Every Core-mediated HTTP attempt receives a raw transaction.
• Core-mediated responses are recorded before source code receives them.
• Core-managed raw bytes and metadata obey the defined capture and redaction rules.

Operating-system-enforced only

Lexicon adds no sandbox beyond operating-system permissions. Source code may therefore read and write files, open raw sockets, spawn processes, use FFI, and invoke other native code when the OS permits it.

Conventional or not globally enforced

• Every HTTP request in the process uses Core.
• Every response produced by arbitrary native source code is recorded.
• Source-authored files and logs redact secrets.
• Source code avoids deliberate pre-main effects or manually substituted binaries.

These are accepted trusted-code limitations.

15. Trust and security model

Source authors are trusted project developers, not hostile third-party plugin authors.

The architecture makes the correct path obvious, structurally central, testable, and difficult to bypass accidentally. It does not execute adversarial native code safely.

A separate HTTP broker without an operating-system sandbox would add IPC and lifecycle complexity but would not prevent a source worker from opening another socket.

If hostile or independently distributed sources become a requirement, Lexicon must reopen the architecture and design genuine per-platform confinement, including network denial, constrained filesystem access, process restrictions, controlled credentials, constrained IPC, and platform-specific Linux and Windows policy. A partial sandbox is not part of this contract.

16. Versioning

The following versions remain distinct:

• project schema version;
• source manifest schema version;
• source contract version;
• runner template version;
• Core crate version;
• runtime invocation protocol version;
• raw-data schema version;
• session schema version;
• individual capability contract versions.

One number must not represent every compatibility surface.

Source implementation and runner compile together. Breaking Rust API changes are handled through explicit rebuilds and compiler diagnostics. There is no stable Rust plugin ABI.

The framework-to-runtime invocation is a small versioned compatibility surface. Unsupported runtimes are rejected with a rebuild or migration diagnostic. Already-built compatible native runtimes execute without Rust or Cargo.

17. Testing requirements

The architecture requires tests for at least:

• valid and invalid source descriptor compilation;
• optional-handler signature validation;
• capability declaration and availability checks;
• one GET producing one complete transaction;
• POST request-body preservation;
• compressed response preservation before content decoding;
• separate redirect and retry transactions;
• metadata redaction and explicitly sensitive fields;
• connection failure and truncated response state;
• source errors, panics, and abnormal exits;
• failure after thousands of transactions;
• checkpoint and resume behavior;
• stale-session reconciliation;
• non-UTF-8 Unix and Windows Unicode argument forwarding;
• parent/child identity disagreement;
• runtime hash mismatch;
• acquisition or processing build failure preserving both old runtimes;
• publication rollback;
• foreground supervision;
• background operator-host handoff.

Core should expose a test harness or transport seam that exercises the same recorder and redactor used in production. Source helpers remain ordinary Rust and use ordinary Rust tests, debuggers, stack traces, logs, and profilers.

18. Explicit non-goals

This contract does not introduce:

• WebAssembly;
• browser, JavaScript, Node.js, JVM, or Python runtimes;
• browser automation;
• a custom acquisition language;
• a stable serialized acquisition IR;
• a dynamic Rust plugin ABI;
• a universal source-argument schema;
• a general public framework IPC service;
• an internal HTTP broker;
• hostile-code sandboxing;
• automatic durable resumption of arbitrary source logic;
• a second independently installed framework executable;
• Cargo invocation during ordinary acquisition or processing.

A fixed-request TOML frontend, builder, macro, new protocol, broker, sandbox, or workflow representation may be considered only after a concrete requirement justifies its permanent cost. Any convenience frontend must adapt into the same primary contract instead of creating a second execution model.

19. Final commitments

Lexicon will ship:

1. One installed lexicon executable.
2. A reusable lexicon-framework library for command semantics and operational policy.
3. A narrow lexicon-core library for contracts and invariant-sensitive mechanics.
4. In-process framework calls for foreground commands.
5. Same-binary operator-host re-execution for background supervision.
6. One managed native acquisition runner per source.
7. One source-authored Rust implementation library per source operation.
8. A versioned capability-aware descriptor with one mandatory acquisition function.
9. Ordinary sequential Rust for source-specific decision logic.
10. Core-mediated HTTP that records each physical exchange before returning it.
11. Compile-time, build-time, and parent-and-child runtime validation.
12. Locked, isolated native release builds with Cargo JSON artifact selection.
13. Paired, staged, transactional publication of acquisition and processing runtimes.
14. No development toolchain requirement for already-built execution.
15. Honest acceptance that trusted native source code can bypass Core through unrestricted native effects.

The selected architecture becomes concrete as follows.

A source author writes acquisition logic in:

sources/example-source/http/get-raw-data/
└── get-raw-data-impl/
    ├── Cargo.toml
    └── src/
        └── lib.rs

The source does not provide the supported acquisition main.rs. Instead, it exports a typed source descriptor and an acquisition function:

use std::ffi::OsString;
use lexicon_core::http::{
    AcquisitionResult,
    HttpAcquisitionContext,
    HttpSourceContractV1,
};
pub const SOURCE: HttpSourceContractV1 =
    HttpSourceContractV1::new(acquire);
pub fn acquire(
    context: &mut HttpAcquisitionContext,
    args: &[OsString],
) -> AcquisitionResult<()> {
    context.arguments().require_empty(args)?;
    let transaction = context.execute(
        HttpRequest::get("https://example.com/data.zip")?
            .header("Accept", "application/zip")?
            .logical_key("main-dataset"),
    )?;
    // The request and response have already been recorded.
    transaction.response().require_success()?;
    Ok(())
}

The exact type of SOURCE forces the source to expose the required function with the required input and output types. A missing function, private function, asynchronous function, wrong argument type, wrong mutability, or wrong return type prevents compilation.

For example, this fails:

pub fn acquire(
    context: HttpAcquisitionContext,
) -> bool {
    true
}

It fails because it does not match the function type required by HttpSourceContractV1::new.

Lexicon generates a separate managed runner:

sources/example-source/http/get-raw-data/
└── lexicon-runner/
    ├── Cargo.toml
    └── src/
        └── main.rs

Its entrypoint is approximately:

use std::process::ExitCode;
use lexicon_core::http::{
    runner,
    HttpSourceContractV1,
    RuntimeIdentity,
};
const IDENTITY: RuntimeIdentity =
    RuntimeIdentity::http_acquisition(
        "example-source",
        1,
    );
const SOURCE: HttpSourceContractV1 =
    source_implementation::SOURCE;
fn main() -> ExitCode {
    runner::run(IDENTITY, &SOURCE)
}

This means the supported executable always enters Core first:

operating system
→ lexicon-runner/src/main.rs
→ lexicon_core::http::runner::run(...)
→ runtime validation
→ session initialization
→ source_implementation::acquire(...)

The source author cannot replace this entrypoint through the supported lexicon source build command. They can edit files because they own the project, but Lexicon validates the managed runner and rejects a changed or incompatible runner.

The acquisition workspace is:

get-raw-data/
├── Cargo.toml
├── Cargo.lock
├── get-raw-data-impl/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── lexicon-runner/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── sessions/
├── session_status.json
└── runtime/

The workspace manifest is approximately:

[workspace]
resolver = "2"
members = [
    "get-raw-data-impl",
    "lexicon-runner",
]
[workspace.dependencies]
lexicon-core = "=0.1.0"

The source implementation manifest is:

[package]
name = "example-source-get-raw-data-impl"
version = "0.1.0"
edition = "2024"
[lib]
name = "example_source_get_raw_data_impl"
path = "src/lib.rs"
[dependencies]
lexicon-core = { workspace = true }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
sha2 = "0.10"

The managed runner manifest is:

[package]
name = "example-source-get-raw-data-runner"
version = "0.1.0"
edition = "2024"
[[bin]]
name = "example-source-get-raw-data"
path = "src/main.rs"
[dependencies]
lexicon-core = { workspace = true }
source-implementation = {
    package = "example-source-get-raw-data-impl",
    path = "../get-raw-data-impl",
}

Cargo statically links the runner, Core, the source implementation, and the source’s dependencies into:

get-raw-data/runtime/example-source-get-raw-data

No .rlib or dynamically loaded Rust plugin is published.

A nontrivial source still uses ordinary sequential Rust:

use std::{
    ffi::OsString,
    fs::File,
};
use clap::Parser;
use serde::Deserialize;
use lexicon_core::http::{
    AcquisitionError,
    AcquisitionResult,
    HttpAcquisitionContext,
    HttpRequest,
    HttpSourceContractV1,
    RetryPolicy,
};
pub const SOURCE: HttpSourceContractV1 =
    HttpSourceContractV1::new(acquire);
#[derive(Parser)]
struct Args {
    #[arg(long)]
    since: Option<String>,
    #[arg(long, default_value_t = 4)]
    max_attempts: u32,
}
#[derive(Deserialize)]
struct Manifest {
    continuation: Option<String>,
    items: Vec<Item>,
}
#[derive(Deserialize)]
struct Item {
    id: String,
    url: String,
    sha256: String,
}
pub fn acquire(
    context: &mut HttpAcquisitionContext,
    raw_args: &[OsString],
) -> AcquisitionResult<()> {
    let args = Args::try_parse_from(
        std::iter::once(OsString::from("example-source"))
            .chain(raw_args.iter().cloned()),
    )
    .map_err(AcquisitionError::arguments_from)?;
    let mut continuation = None;
    loop {
        let query = QueryBody {
            since: args.since.clone(),
            continuation: continuation.clone(),
        };
        let manifest_transaction = context.execute(
            HttpRequest::post(
                "https://example.com/api/manifest",
            )?
            .json(&query)?
            .header("Accept", "application/json")?
            .sensitive_header_from_env(
                "Authorization",
                "EXAMPLE_API_TOKEN",
            )?
            .logical_key("manifest")
            .retry(
                RetryPolicy::transient()
                    .max_attempts(args.max_attempts),
            ),
        )?;
        manifest_transaction
            .response()
            .require_success()?;
        // Parsing reads the body Core has already preserved.
        let reader = File::open(
            manifest_transaction.response().body_path(),
        )
        .map_err(|error| {
            AcquisitionError::source(
                "open recorded manifest",
                error,
            )
        })?;
        let manifest: Manifest =
            serde_json::from_reader(reader)
                .map_err(|error| {
                    AcquisitionError::source(
                        "parse recorded manifest",
                        error,
                    )
                })?;
        for item in manifest.items {
            let checkpoint = format!("item/{}", item.id);
            if context.has_checkpoint(&checkpoint)? {
                continue;
            }
            let mut request =
                HttpRequest::get(&item.url)?
                    .logical_key(&checkpoint)
                    .retry(
                        RetryPolicy::transient_get()
                            .max_attempts(args.max_attempts),
                    );
            if let Some(etag) =
                context.latest_response_header(
                    &checkpoint,
                    "ETag",
                )?
            {
                request = request.header(
                    "If-None-Match",
                    etag,
                )?;
            }
            let transaction = context.execute(request)?;
            match transaction.response().status().as_u16() {
                200 => {
                    verify_sha256(
                        transaction.response().body_path(),
                        &item.sha256,
                    )?;
                }
                304 => {
                    // A previously recorded representation
                    // remains current.
                }
                status => {
                    return Err(
                        AcquisitionError::source_message(
                            format!(
                                "{} returned HTTP {}",
                                item.id,
                                status,
                            ),
                        ),
                    );
                }
            }
            // Commit only after recording and verification.
            context.commit_checkpoint(&checkpoint)?;
        }
        match manifest.continuation {
            Some(token) => continuation = Some(token),
            None => break,
        }
    }
    Ok(())
}

This remains normal Rust:

authenticate
→ POST manifest query
→ record transaction
→ parse recorded JSON
→ follow continuation token
→ iterate discovered items
→ issue conditional GETs
→ record every attempt
→ verify checksums
→ commit source checkpoints
→ continue or finish

The source does not have to encode this as start, on_response, and NextAction callbacks. Local variables, loops, early returns, helper functions, parsing libraries, and debugger breakpoints work normally.

For every call to:

context.execute(request)

Core performs this sequence:

allocate transaction ID
→ create transaction staging directory
→ finalize the effective request
→ persist redacted request metadata
→ persist request body bytes
→ perform one physical HTTP attempt
→ persist response metadata or failure state
→ stream undecoded response body to storage
→ calculate hashes
→ finalize the transaction
→ update session progress
→ return RecordedTransaction

Only after that sequence may source code inspect the response.

Consequently:

let transaction = context.execute(request)?;
parse(transaction.response().body_path())?;

means parse cannot observe a Core response before its raw transaction has been recorded.

If parsing fails or panics, the response remains preserved.

If the server returns 404, the response is recorded before require_success() reports an error.

If Core retries four times, it creates four transaction directories.

If the server redirects twice before the final response, each HTTP exchange receives its own transaction directory. Redirects and retries cannot be invisibly collapsed by the underlying HTTP client.

The runtime tree might contain:

data/raw/
├── 2026-08-24T18-30-02Z-000001/
│   ├── request/
│   │   ├── metadata.json
│   │   └── body
│   └── response/
│       ├── metadata.json
│       └── body
├── 2026-08-24T18-30-03Z-000002/
└── 2026-08-24T18-30-06Z-000003/

The stored response body contains HTTP entity bytes before content decoding. If the response declares Content-Encoding: gzip, the raw file contains the compressed bytes. Core may provide a separate decoded reader, but it does not replace the preserved body.

The build command follows a fixed route:

lexicon source build example-source --protocol http
→ lexicon-cli/src/main.rs
→ lexicon_framework::commands::source_build(...)
→ lexicon_core::build validation
→ cargo metadata --locked
→ cargo build --release --locked
→ Cargo JSON artifact selection
→ runtime identity probe
→ staged acquisition runtime
→ staged processing runtime
→ paired transactional publication

A representative Cargo invocation is:

cargo build
  --manifest-path <get-raw-data/Cargo.toml>
  --package example-source-get-raw-data-runner
  --bin example-source-get-raw-data
  --release
  --locked
  --message-format=json-render-diagnostics
  --target-dir <temporary-acquisition-target>

Lexicon selects the executable artifact for that exact package and binary target. It does not find an executable by guessing a path.

If acquisition builds but processing fails, neither existing runtime is replaced. If publishing the second runtime fails after publishing the first, Lexicon restores both previous runtime bundles.

Foreground acquisition executes as:

user
→ installed lexicon process
→ linked lexicon-framework
→ linked lexicon-core
→ published acquisition runtime child
→ child’s linked lexicon-core runner
→ source descriptor
→ source acquire function

Background acquisition executes as:

user
→ initial lexicon process
→ same lexicon executable re-executed as __operator-host
→ linked lexicon-framework
→ linked lexicon-core
→ published acquisition runtime child

There is still only one installed control executable. The background operator host is another process running the same lexicon binary, not a separately installed framework product.

Runtime validation happens twice.

The parent validates:

runtime.json
executable hash
runtime protocol
source identity
protocol identity
operation identity
Core contract version
compiled capabilities

The child’s linked Core validates:

invocation-envelope version
project identity
source identity
protocol identity
operation identity
session identity
execution mode
source descriptor
required capabilities

Only after those checks does Core call:

source_implementation::acquire(
    &mut context,
    source_arguments,
)

This is runtime enforcement of the supported invocation contract. It is not global effect confinement.

The source library can still deliberately write:

std::net::TcpStream::connect("example.com:443")?;
std::fs::write("/some/path", bytes)?;
std::process::Command::new("another-program").spawn()?;

Those operations are technically possible because the source is trusted native Rust linked into the runtime. A trait, function pointer, managed main, or Cargo dependency cannot prohibit them.

The precise guarantee is:

Every HTTP request submitted through Core is recorded.
Lexicon does not guarantee that unrestricted native source code
is incapable of performing another request outside Core.

That limitation is accepted because source authors are trusted project developers. If Lexicon later executes hostile third-party sources, it will require a different security architecture involving a broker and genuine operating-system restrictions—not merely another Rust interface or subprocess.