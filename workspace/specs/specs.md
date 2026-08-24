Lexicon Architecture and Build Specification

Status: current, reconciled to contract.md, 2026-08-24

1. Authority and scope

contract.md is the architectural source of truth for Lexicon acquisition, processing, source builds, runtime admission, publication, and supervision. This specification translates that architecture into concrete package, workspace, command, build, runtime, storage, and testing requirements.

If this specification conflicts with contract.md, contract.md governs and this file must be corrected.

Lexicon is a generic framework for:

1. Acquiring raw data from independently implemented sources.
2. Preserving raw request and response data.
3. Processing raw data into source-specific SQLite datasets.

Lexicon does not define what acquired data represents. Each source controls its source-specific acquisition decisions, arguments, parsing, validation, continuation logic, checkpoints, and processed SQLite schema.

HTTP is the only initially supported acquisition protocol. Browser automation and additional protocols are future work.

The governing division is:

> Lexicon controls the supported entrypoint, build, runtime admission, HTTP-and-recording effect, and session supervision. Source implementations control arbitrary trusted Rust computation around that effect.

This is one execution architecture. Individual sources do not choose alternate runner, workflow-language, subprocess-protocol, or configuration-only execution models.

2. Trust boundary

Source authors are trusted project developers, not hostile third-party plugin authors.

Lexicon makes the supported path structurally central and difficult to bypass accidentally. It does not capability-confine linked native Rust. Source code can use files, sockets, subprocesses, FFI, or other native effects whenever the operating system permits them.

Therefore:

• Core guarantees apply to effects performed through Core.
• The supported Lexicon build and runtime paths reject mismatched runners and artifacts.
• Lexicon does not claim that arbitrary native source code cannot bypass Core.
• Hostile-source support would require a separate, explicit operating-system sandbox design.

3. Principal packages

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

The root Cargo.toml is a workspace-only manifest. It contains [workspace] and shared workspace configuration but no root [package] and no root src/main.rs.

3.1 lexicon-cli

lexicon-cli is the only principal package that produces the installed control executable. The installed executable is named lexicon.

It owns:

• command-line parsing and help;
• user-facing rendering of typed framework results;
• the reserved internal operator-host entrypoint;
• process-level supervision hooks needed by the installed executable.

It does not duplicate scaffold rules, build mechanics, publication policy, runtime validation, or session semantics.

3.2 lexicon-framework

lexicon-framework is a reusable Rust library linked into lexicon-cli. It owns command semantics and operational policy, including:

• project discovery;
• source discovery and selection;
• scaffolding;
• build orchestration;
• diagnostics and cancellation policy;
• acquisition and processing command orchestration;
• staged paired publication;
• foreground and background supervision policy.

There is no independently installed or independently versioned lexicon-framework executable.

3.3 lexicon-core

lexicon-core is a narrow reusable Rust library. It owns invariant-sensitive contracts and mechanics shared by the framework and managed source runtimes, including:

• typed source descriptors and capabilities;
• opaque validated build states;
• invocation and runtime identity contracts;
• managed runner support;
• HTTP execution and transaction recording;
• raw-data and metadata contracts;
• session transition primitives;
• processing runtime contracts.

Core remains domain-agnostic and does not expose private engine types through its public source contracts.

3.4 Dependency graph

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

The framework library is not linked into source runtimes. Core is. Lexicon exposes no dynamic Rust plugin ABI.

4. Project and source workspace layout

A Lexicon project contains a project manifest and a configured source directory:

```text
telugu-lexicon/
├── lexicon.toml
└── sources/
```

Representative project configuration:

```toml
schema_version = 1

[project]
name = "telugu-lexicon"
sources_directory = "sources"
```

Each source is protocol-scoped. An HTTP source has the following acquisition and processing workspaces:

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
    ├── Cargo.toml
    ├── Cargo.lock
    ├── sessions/
    ├── session_status.json
    ├── process-data-impl/
    │   ├── Cargo.toml
    │   └── src/
    │       └── lib.rs
    ├── lexicon-runner/
    │   ├── Cargo.toml
    │   └── src/
    │       └── main.rs
    └── runtime/
```

The source author edits:

• source-editable source.toml fields;
• discovery.md;
• implementation-crate dependency manifests;
• implementation lib.rs files and their modules;
• source-specific acquisition and processing logic.

Lexicon generates and contractually controls:

• each operation workspace contract;
• each lexicon-runner/Cargo.toml;
• each lexicon-runner/src/main.rs;
• runner identity and template version;
• the runner binary selected by the supported build.

Cargo.lock is Cargo-managed, committed, and required by the supported build.

“Lexicon-managed” means Lexicon generates, validates, and selects the runner. It does not mean filesystem permissions prevent a project developer from editing generated files. A mismatched managed file is rejected rather than silently overwritten.

5. Source contracts and managed entrypoints

Every HTTP acquisition source exports one capability-aware, versioned descriptor from its implementation library:

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

• descriptor type;
• mandatory acquisition signature;
• registered optional-handler signatures;
• typed capabilities and extension declarations;
• compatibility between the descriptor and managed runner code.

The compiler does not prove semantic honesty or that every native effect uses Core.

The source implementation is a library, not the supported executable entrypoint. A representative Lexicon-managed acquisition runner is:

```rust
use std::process::ExitCode;

use lexicon_core::http::{runner, HttpSourceContractV1, RuntimeIdentity};

const IDENTITY: RuntimeIdentity =
    RuntimeIdentity::http_acquisition("example-source", 1);

const SOURCE: HttpSourceContractV1 =
    source_implementation::SOURCE;

fn main() -> ExitCode {
    runner::run(IDENTITY, &SOURCE)
}
```

The published runtime is one native executable statically linked with Core, the implementation library, and their Rust dependencies.

Processing follows the same split: a source-authored processing library plus a Lexicon-managed processing runner.

6. Capability and extension model

The base contract remains small. New protocol behavior is introduced through versioned Core capabilities.

A capability may be:

• optional;
• required by a source descriptor;
• mandatory for an effect exposed only through Core.

Compile time verifies descriptor and handler types. Build time verifies that the selected Core and runner provide all declared capabilities. Runtime admission verifies that parent, child, descriptor, and invocation agree on the capability set before source code runs.

Capabilities are added for demonstrated protocol requirements. They are not a general plugin ABI, workflow language, or policy engine.

7. Command interface and routing

All normal interaction begins with the installed lexicon executable.

```text
lexicon --version             -> CLI presentation
lexicon init                  -> framework library
lexicon source create         -> framework library
lexicon source build          -> framework library
lexicon build                 -> framework library
lexicon data --get            -> framework library
lexicon data --process        -> framework library
```

The CLI parses arguments and renders typed results. The framework owns project semantics and filesystem mutations.

Representative data commands are:

```text
lexicon data --get example-source --protocol http
lexicon data --process example-source --protocol http
lexicon data --get example-source --protocol http --bg
lexicon data --get example-source --protocol http --abandon-past-fail
```

Source-specific arguments follow --:

```text
lexicon data --get example-source --protocol http -- <source-args>
```

Every value after -- is preserved as OsString. Lexicon does not interpret source-specific semantics. The source may use Clap or another parser and is responsible for validating its own arguments.

Raw source arguments are not persisted by default because they may contain credentials, signed URLs, personal data, or other secrets. A source may explicitly record a safe redacted summary through Core.

8. Process and supervision model

8.1 Foreground execution

Foreground framework work runs in the original installed process:

```text
user
-> lexicon frontend and supervisor process
-> lexicon-framework library
-> lexicon-core library
-> managed source runtime child
```

The lexicon process remains alive while acquisition or processing runs and supervises the child.

8.2 Background execution

For --bg, the initial process creates a durable invocation record and re-executes its exact binary in a reserved internal role:

```text
initial lexicon frontend
-> lexicon __operator-host <invocation-reference>
-> lexicon-framework library
-> lexicon-core library
-> managed source runtime child
```

The initiating process exits only after the operator host confirms durable session ownership. The operator host owns the session lock, child lifecycle, cancellation, exit observation, and terminal reconciliation.

The operator-host invocation is a versioned internal protocol because the detached process may outlive an installation upgrade. It is not a public framework API.

No ordinary foreground command crosses CLI-to-framework IPC, and Lexicon installs no second framework daemon or executable.

9. Supported build contract

Core exposes opaque validated build states:

```text
SourceLocation
-> DiscoveredSource
-> ValidatedSource
-> ValidatedOperationWorkspace
-> ReproducibleBuildPlan
-> StagedArtifact
-> VerifiedRuntime
-> PublishedRuntime
```

Invariant-sensitive APIs accept validated state values rather than arbitrary paths.

For each operation, the supported build must:

1. Discover the containing project.
2. Resolve the configured source directory.
3. Validate source identity, protocol, metadata, and contract version.
4. Validate the operation workspace and committed lockfile.
5. Validate the managed runner and template version.
6. Validate the implementation library target.
7. Validate the exact compatible Core dependency.
8. Run Cargo metadata in locked mode.
9. Construct an exact native release build plan.
10. Build the managed runner for the current machine.
11. Use an isolated temporary Cargo target directory.
12. Read Cargo JSON artifact messages.
13. Select exactly the expected executable artifact.
14. Hash the executable.
15. Run a bounded runtime-information probe.
16. Validate identity, operation, protocol, Core contract, and capabilities.
17. Stage the runtime bundle.

Builds use --locked, isolated target directories, explicit package and binary targets, Cargo JSON artifact selection, staged runtime bundles, transactional publication, and rollback.

Acquisition and processing runtimes are published as one paired transaction. If either build, verification, staging, or publication fails, both previous published runtimes survive unchanged.

lexicon source build builds the selected source. lexicon build builds all discovered sources. Both use the same validated build path; neither uses a manually maintained source registry.

Ordinary acquisition and processing use already-published native runtimes and never invoke Cargo. Rust and Cargo are development/build requirements, not execution requirements.

10. Runtime bundle and admission

The supported build publishes an operation runtime below that operation’s runtime/ directory. A runtime bundle includes the native executable, runtime.json, and the integrity and compatibility metadata needed for admission. Exact on-disk version directories may evolve with the runtime schema, but publication must remain staged and atomic.

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

Only then may Core call the source handler.

The phase relationship is:

```text
compile time
    source descriptor and handlers have the required Rust types

build time
    runner, descriptor, Cargo graph, Core version,
    artifact identity, and capabilities agree

runtime
    parent and child agree on identity, invocation version,
    contract version, capabilities, operation, and session
```

An artifact that does not match the managed pattern is rejected by the supported build or runtime admission path.

11. HTTP and raw-data contract

The supported HTTP effect is:

```rust
let transaction = context.execute(request)?;
```

For every physical HTTP attempt submitted through Core, Core must:

1. Allocate a unique transaction identity and staging directory.
2. Finalize the effective request.
3. Persist redacted request metadata.
4. Persist exact request-body bytes supplied to transport, when present.
5. Perform one physical HTTP exchange.
6. Persist response metadata or a transport-failure record.
7. Stream undecoded HTTP entity-body bytes to raw storage.
8. Hash while streaming.
9. Atomically finalize the transaction or leave a recognizable partial record.
10. Update session progress.
11. Return a RecordedTransaction only after durable recording completes.

Each transaction has this shape:

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

Core must not replace stored bytes with transparently decoded gzip, Brotli, or other content. A decoded reader may be exposed separately.

Each redirect exchange and each retry attempt is a separate physical transaction. Neither may be collapsed below the recorder.

Source code receives a recorded transaction rather than an unrecorded live response, then performs parsing, decoding, validation, and interpretation using ordinary Rust.

12. Secret handling

Core redacts managed metadata before persistence. Mandatory sensitive metadata includes at least:

• Authorization;
• Proxy-Authorization;
• Cookie;
• Set-Cookie;
• explicitly marked sensitive headers;
• explicitly marked sensitive query parameters.

Exact arbitrary body preservation and universal body-secret removal are incompatible when secrets occur in request or response bodies. Core therefore does not claim generic semantic redaction of exact raw bodies.

Future encryption, exclusion, or protected-body policies must be explicit and must not silently weaken raw-data fidelity. Source-authored files and logs remain outside Core’s redaction guarantee.

13. Sessions

Acquisition and processing have separate sessions/ directories and separate root session_status.json summaries.

The root status file is the current summary. Durable detailed history belongs below sessions/<session-id>/. Status updates use atomic replacement or an equivalent transactional mechanism.

The supervising lexicon process:

• selects, creates, or resumes a session;
• acquires session locks;
• applies --abandon-past-fail;
• launches the managed runtime;
• observes exit and signals;
• reconciles abnormal termination.

Linked Core inside the runtime:

• validates the invocation;
• enters the running state;
• records transaction progress;
• commits checkpoints;
• records ordinary source failure;
• records normal completion.

The source decides source-specific continuation and checkpoint meaning.

A panic, abort, forced exit, or crash is reconciled by the parent while completed transactions remain intact. After machine or supervisor loss, the next invocation detects stale ownership and reconciles durable state.

14. Processing

Processing remains distinct from acquisition. It has its own implementation library, managed runner, runtime bundle, sessions, and status.

Processing reads protocol-scoped raw transactions and creates the source-specific SQLite database under the source’s processed-data directory. It does not alter the acquisition raw-data contract.

The same compile-time contract checks, supported build validation, runtime admission, supervision, staging, and publication guarantees apply. Acquisition and processing runtime updates remain paired during publication.

15. Enforcement model

Compiler-enforced

• Descriptor and handler types.
• Optional-handler signatures.
• Managed runner references to source contracts.
• Core linkage into conforming managed runtimes.

Build-tool-enforced

• Selection of the Lexicon-managed runner.
• Rejection of a source-owned replacement main in the supported path.
• Runner template and workspace versions.
• Cargo graph, lockfile, and Core compatibility.
• Exact executable artifact selection and verification.
• Preservation of existing runtimes after failure.
• Paired transactional publication.

Runtime-enforced

• Parent/child invocation and identity agreement.
• Required capability availability.
• Native source-argument forwarding.
• Supported session transitions and reconciliation.
• Recording of every Core-mediated HTTP attempt.
• Durable recording before Core returns a response to source code.
• Core-managed capture and redaction rules.

Not globally enforced

• That arbitrary native source code uses Core for every network effect.
• That source-authored files and logs redact secrets.
• That trusted developers never substitute binaries outside the supported path.

These are accepted trusted-code limitations.

16. Versioning

The following compatibility surfaces remain distinct:

• project schema version;
• source manifest schema version;
• source contract version;
• runner template version;
• Core crate version;
• runtime invocation protocol version;
• raw-data schema version;
• session schema version;
• individual capability contract versions.

One number must not represent every surface.

Source implementation and managed runner compile together. Breaking Rust API changes use explicit rebuilds and compiler diagnostics. There is no stable Rust plugin ABI.

The framework-to-runtime invocation is a small, versioned compatibility surface. Unsupported runtimes are rejected with a rebuild or migration diagnostic. Compatible published native runtimes execute without Rust or Cargo.

17. Installation and release bundling

The repository contains source code and committed project data only. Compiled executables, generated archives, installers, Cargo target directories, and other generated release artifacts are not committed.

Release tooling may include an ancillary lexicon-bundle package and the generic MZA build/bundle system. These are release-time mechanisms and do not change the three principal runtime package boundaries.

17.1 Installed payload

The installed control payload contains one lexicon executable. lexicon-framework and lexicon-core are statically linked libraries inside that executable; they are not separate installed executables or ordinary bundle inputs.

Project-specific managed acquisition and processing runtimes are built and published by Lexicon within their project workspaces. They are not generic framework executables.

17.2 MZA separation

MZA remains generic and contains no Lexicon-specific installation policy. A Lexicon-specific bundle crate may own extraction, installation paths, PATH integration, upgrade, and uninstall behavior.

MZA resolves package names and versions from Cargo manifests, builds with committed lockfiles and --locked, records stable machine-readable failures, and writes generated outputs only below configured artifact directories.

17.3 Ordinary artifact contract

Each ordinary artifact has a stable label, crate, type, output path, applicable targets, and optional target exclusions. MZA:

1. Resolves package name and version from the artifact’s Cargo manifest.
2. Builds for each applicable target.
3. Selects the produced native executable.
4. Archives it as .tar.xz.
5. Records the target-specific absolute archive path for bundle use.

The Lexicon installation bundle consumes the lexicon_cli executable artifact. It must not require a separate lexicon_framework executable artifact.

17.4 General bundle contract

Each bundle declares a label, implementation crate, protocol, artifact-label inputs, type, output path, and protocol-dependent targets.

For every protocol:

1. One bundle execution handles exactly one target.
2. Every input used by that execution was built for that exact target.
3. Missing target-specific inputs are configuration errors, never silent skips.
4. Input paths identify completed .tar.xz artifacts.
5. A successful execution produces exactly one final target executable.
6. MZA archives that executable as the final bundle artifact.

Without explicit build targets, inputs must have identical target sets. With explicit build targets, every requested target must exist and be supported by every input.

17.5 cargo-bundler-v0.1.0

Use this protocol when the bundle crate itself becomes the final target executable. MZA cross-compiles it and does not execute it on the build host.

For each target, MZA writes a target-specific bundle-spec.toml, sets MZA_BUNDLE_INPUTS to its absolute path, and builds the bundle crate with --locked.

The bundle crate’s build.rs runs on the build host and:

1. Reads MZA_BUNDLE_INPUTS, with an empty-input fallback for standalone compilation.
2. Parses the TOML.
3. Copies each input archive into $OUT_DIR.
4. Generates $OUT_DIR/mza_bundle_inputs.rs using include_bytes! for the copied archives.

The bundle source includes the generated file:

```rust
include!(concat!(env!("OUT_DIR"), "/mza_bundle_inputs.rs"));
```

The compiled installer therefore contains archive bytes, not build-host paths.

Representative Lexicon bundle inputs are:

```toml
protocol = "cargo-bundler-v0.1.0"
bundle = "lexicon"
target = "x86_64-unknown-linux-musl"

[[inputs]]
label = "lexicon_cli"
archive = "/absolute/path/lexicon_cli-0.1.0-x86_64-unknown-linux-musl.tar.xz"
```

17.6 command-bundler-v0.1.0

Use this protocol when a Rust adapter executes on the build host and invokes a project-specific external bundling system. The adapter does not become the target executable and is not cross-compiled.

MZA writes a target-specific bundle spec, sets MZA_BUNDLE_SPEC, and invokes the adapter with:

```text
cargo run --release --locked --manifest-path <bundle-crate>/Cargo.toml
```

The adapter must synchronously produce exactly one executable at the exact output_path supplied in the spec and exit successfully only after that regular file exists. MZA does not add an arbitrary command field or require a result manifest.

The adapter runs on the host. The requested output target travels in the bundle spec and is not passed as Cargo’s execution target for the adapter.

17.7 Temporary and permanent outputs

Protocol workspaces are uniquely scoped:

```text
<system-temp>/mza/<run-id>/<bundle-label>/<target>/
├── bundle-spec.toml
└── output/
```

Ordinary artifact layout:

```text
<output_path>/<label>/<type>/<version>/<name>-<version>-<target>.tar.xz
```

Bundle layout:

```text
<output_path>/<label>/<type>/<protocol>/<version>/<target>/<label>-<version>-<target>.tar.xz
```

Temporary protocol files and permanent generated artifacts are never committed.

17.8 Release targets

The intended cross-release matrix is Linux x86_64, Linux ARM64, Windows x86_64, and Windows ARM64. A target is supported only after its complete build, bundle, install, and execution flow is exercised. macOS requires an appropriate macOS build host and is outside the Linux-host cross-release matrix.

Normal users need no Rust, Cargo, Zig, Python, JVM, or other language runtime to execute an already-installed control executable and compatible already-published source runtimes.

18. Testing requirements

The implementation requires tests for at least:

• valid and invalid descriptor compilation;
• optional-handler signature validation;
• capability declaration and availability checks;
• one GET producing one complete transaction;
• POST request-body preservation;
• compressed response preservation before decoding;
• separate redirect and retry transactions;
• metadata redaction and explicitly sensitive fields;
• connection failure and truncated response state;
• source errors, panics, abnormal exits, and failure after many transactions;
• checkpoint and resume behavior;
• stale-session reconciliation;
• non-UTF-8 Unix and Windows Unicode argument forwarding;
• parent/child identity disagreement;
• runtime hash mismatch;
• acquisition or processing build failure preserving both old runtimes;
• publication rollback;
• foreground supervision;
• background operator-host handoff;
• MZA target-coverage validation and exact bundle input selection;
• confirmation that the Lexicon installer payload contains no separate framework executable.

Core should expose a test harness or transport seam that exercises the same recorder and redactor used in production. Source helpers remain ordinary Rust and use ordinary Rust tests, debuggers, stack traces, logs, and profilers.

19. Explicit non-goals

The architecture does not introduce:

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

A fixed-request TOML frontend, builder, macro, new protocol, broker, sandbox, or workflow representation may be considered only after a concrete requirement justifies its permanent cost. Any convenience frontend must adapt into the same primary contract rather than create a second execution model.

20. Non-negotiable invariants

1. Only lexicon-cli produces the installed lexicon control executable.
2. lexicon-framework is an in-process reusable library, not an independently installed framework process.
3. lexicon-core is a narrow reusable library shared with managed source runtimes.
4. Source implementations are libraries linked behind Lexicon-managed runner entrypoints.
5. Source contracts are enforced by Rust compilation.
6. Supported builds validate workspace shape, lockfile, runner, Cargo graph, artifact identity, and capabilities.
7. Parent and child both validate runtime identity and compatibility before source logic runs.
8. Foreground framework calls stay in the original lexicon process.
9. Background supervision uses re-execution of the exact lexicon binary in a reserved operator-host role.
10. Every Core-mediated physical HTTP exchange is durably recorded before source code receives it.
11. Core preserves undecoded HTTP entity-body bytes and redacts managed metadata.
12. Acquisition and processing runtimes are staged and published as a paired transaction with rollback.
13. Ordinary data execution never invokes Cargo or requires a development toolchain.
14. The repository never commits compiled executables or generated release archives.
15. MZA remains reusable and contains no Lexicon-specific installation policy.
16. A Lexicon installation bundle contains the installed CLI payload, not a separate framework executable.
17. Trusted native source code can bypass Core through unrestricted native effects; Lexicon does not claim otherwise.