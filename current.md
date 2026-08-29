# Current master milestone: close every known Contract V1 and Specification V1 conformance gap

## 1. Authority, baseline, and verdict

This file is the complete replacement for `current.md`.

It applies to the exact audited repository state:

```text
repository: https://github.com/ssr2zvy/lexicon
branch: main
commit: 0494cd751114312028aced50cc62ef80a0fd3157
audited MZA repository: https://github.com/ssr2zvy/mza
audited MZA commit: d2c2406ed9f83d2de4c7a38fbf1ac3a568d1e410
```

The conformance claim at the Lexicon baseline is rejected.

The baseline commit changes only `current.md`. GitHub reports no combined commit statuses and no associated workflow runs for that exact commit. The repository therefore contains neither implementation changes at that commit nor durable CI evidence supporting its completion report.

Six specialist domains received exact GitHub code packets during review: Core identity/builds, foreground process control, background handoff, HTTP/processing, MZA/release/supply chain, and the full specification matrix. Several sessions returned useful findings before execution quota prevented complete final reports. This document incorporates only findings independently confirmed against the committed source; it does not invent missing votes or claim unanimity.

This milestone closes all gaps found in the repository-wide audit at once, but it does so through ordered internal gates. A later gate may be implemented on the same branch, but it must not be declared complete while an earlier invariant on which it depends remains open.

Passing an individual gate is not permission to call the repository conformant. The sole completion event is a clean, exact-commit verification of every gate in Â§18.

### Meaning of âall gapsâ

âAll gapsâ means every contradiction, missing implementation, weak assertion, and missing evidence identified by the audit of `contract.md`, `specs.md`, `status.md`, production source, test source, release automation, and the selected MZA revision. It is not a claim that unknown defects cannot exist. Any newly discovered contract gap is added to this ledger before the master milestone can close.

### External blocker

Full MZA conformance cannot be implemented honestly against MZA commit `d2c2406...` alone. That revision defines artifact construction inputs, but exposes no installer-runtime library target or installer entrypoint for Lexicon to invoke. Lexicon must not invent such an API.

The master milestone therefore remains open until one of these occurs:

1. MZA publishes and pins a Protocol 1 installer API that actually owns install, upgrade, uninstall, command registration, and platform integration; or
2. the committee deliberately amends `contract.md` to remove that requirement.

The second option is a contract decision, not an implementation shortcut.

## 2. Architectural decisions that do not change

The implementation must preserve these settled boundaries:

- one installed user command named `lexicon`;
- Lexicon-owned managed acquisition and processing runners;
- source libraries export typed descriptors and handlers;
- Core owns mediated HTTP execution, recording, session machinery, and processing input admission;
- one Unified Operator Host supervises one top-level `lexicon data` invocation;
- `--bg` selects supervision mode for that invocation;
- source-owned fan-out state remains source-owned, normally under `get-raw-data/state/`;
- Core does not acquire a generic job-queue schema or schedule source work items;
- source-specific arguments remain native `OsString` values;
- release construction and installation use the selected MZA protocol instead of a second Lexicon installer;
- the trusted-native execution model remains in force; hostile-source sandboxing is not silently added to Contract V1.

## 3. Required execution order

| Gate | Diff groups | Exit condition |
|---|---|---|
| 0 | DOC, MATRIX | False completion claims are removed; normative contradictions are resolved. |
| 1 | HTTP | The present secret-persistence defect is closed and its adversarial suite passes. |
| 2 | CLI, NAME, SCAFFOLD, COREID, BUILD | Installed identity, exact scaffold, immutable Core admission, and build selection are proven from the real executable. |
| 3 | HANDOFF | Background transfer is reserved, fenced, owned, and proven with the real operator host. |
| 4 | FOREGROUND | Cancellation controls and reaps the complete process tree on Unix and Windows. |
| 5 | PROCESS, CHECKPOINT, PUBLISH | Processing, checkpoint, and paired-publication crash/failure properties are proven. |
| 6 | MZA, RELEASE | A pinned real MZA API constructs and installs all supported release artifacts without lock regeneration. |
| 7 | SUPPLY, CI | Release policy and exact-commit Linux/Windows evidence are durable. |
| 8 | FINAL | Every mapped requirement has real evidence and the clean exact SHA passes all commands. |

No merge order may expose unredacted secrets, weaken dependency admission, or publish a partially compatible runtime pair.

## 4. Diff ledger

| ID | Required change | Primary files | Gate |
|---|---|---|---|
| DOC-01 | Replace false completion report with this ledger. | `current.md` | 0 |
| DOC-02 | Resolve private-handler test contradiction. | `workspace/specs/specs.md`, `lexicon-core/tests/` | 0 |
| DOC-03 | Describe the real split invocation transport. | `workspace/specs/specs.md` | 0 |
| DOC-04 | Align MZA generated-input include with the selected immutable protocol. | `workspace/specs/specs.md` | 0 |
| MATRIX-01 | Rebuild the conformance matrix from actual test identifiers. | `workspace/specs/status.md`, `workspace/specs/conformance.toml` | 0 |
| HTTP-01 | Use one mandatory-header sensitivity rule in recording and admission. | `lexicon-core/src/protocols/http/transaction/{recorder,metadata}.rs` | 1 |
| HTTP-02 | Strip secrets across exact URL origins, including IP literals. | `lexicon-core/src/protocols/http/context.rs` | 1 |
| HTTP-03 | Strengthen attempt, redirect, retry, truncation, and durability tests. | `lexicon-core/src/protocols/http/runner.rs`, new test support | 1 |
| CLI-01 | Produce a binary named `lexicon` and preserve typed exit causes. | `lexicon-cli/Cargo.toml`, `lexicon-cli/src/main.rs` | 2 |
| NAME-01 | Replace permissive path-name checks with one typed safe-name grammar. | `lexicon-framework/src/lib.rs`, new `identity/name.rs` | 2 |
| SCAFFOLD-01 | Generate the exact required source tree and initial status files. | `lexicon-framework/src/lib.rs`, `lexicon-core/src/session/model.rs` | 2 |
| SCAFFOLD-02 | Make project/source publication durable and failure-atomic. | new `lexicon-framework/src/fs/durable.rs`, scaffold callers | 2 |
| COREID-01 | Embed exact Core URL and revision and fail closed on dirty/mismatched source. | `lexicon-framework/build.rs`, generated identity module | 2 |
| COREID-02 | Admit only the exact generated Core dependency and resolved source. | `lexicon-framework/src/lib.rs` or new build admission module | 2 |
| COREID-03 | Remove production legacy schema/path entrypoints. | `lexicon-core/src/lib.rs`, HTTP/session context modules | 2 |
| COREID-04 | Prove copied installed-CLI operation outside the checkout with Git unavailable. | `lexicon-cli/tests/installed_core_identity.rs` | 2 |
| BUILD-01 | Validate one exact release executable under the isolated target. | `lexicon-framework/src/lib.rs` or `src/build/` | 2 |
| BUILD-02 | Prove paired build and publication failure behavior. | `lexicon-framework/src/publication/`, integration tests | 2/5 |
| HANDOFF-01 | Add durable successor reservation, epoch, digest, expiry, and state. | new `lexicon-core/src/session/handoff.rs`, store/error modules | 3 |
| HANDOFF-02 | Fence ordinary acquisition and stale reconciliation. | Core lease/store and framework coordinator | 3 |
| HANDOFF-03 | Separate `Ready` from `Owned`; return only after ownership. | `lexicon-framework/src/data/background.rs`, supervision envelope | 3 |
| HANDOFF-04 | Exercise the actual hidden operator host under forced interleavings. | new multiprocess integration suite | 3 |
| FOREGROUND-01 | Add cross-platform process-tree supervision and cancellation. | new `lexicon-framework/src/process/`, foreground runner | 4 |
| FOREGROUND-02 | Report cancellation-specific durable failure and CLI status. | data errors/outcomes, session model, CLI | 4 |
| PROCESS-01 | Prove admitted transaction enumeration and processing rollback. | Core processing modules and tests | 5 |
| CHECKPOINT-01 | Prove checkpoints are backed by completed durable transactions. | Core checkpoint modules and tests | 5 |
| PUBLISH-01 | Prove paired publication and Windows replacement rollback. | framework publication modules and tests | 5 |
| MZA-01 | Pin exact MZA source and consume its real Protocol 1 format. | `.gitmodules`, `automation/build_bundle_mza/`, bundle adapter | 6 |
| MZA-02 | Invoke a real upstream-owned installer API; delete the stub/print adapter. | `lexicon-bundle/` | 6 |
| RELEASE-01 | Stop regenerating lockfiles and build every supported target locked. | release scripts/configuration | 6 |
| RELEASE-02 | Remove obsolete local installer orchestration and update every documented entrypoint. | release automation, `README.md`, `instructions.md`, container files | 6 |
| SUPPLY-01 | Establish official-release dependency/build policy. | new `workspace/specs/release-policy.md` | 7 |
| CI-01 | Add native Linux/Windows exact-commit conformance workflows. | `.github/workflows/conformance.yml` | 7 |
| CI-02 | Publish durable machine-readable verification evidence. | `verification/`, CI artifact | 7 |

## 5. Gate 0 â correct the normative and evidence documents

### DOC-01 â `current.md`

Replace the file completely with this document. Do not retain the current âImplementation Completeâ header, the `442 tests` total, or temporary Windows log paths.

### DOC-02 â `workspace/specs/specs.md`

The Â§44 âprivate handler compile-failâ requirement contradicts the descriptor architecture and the repositoryâs valid `private_handler_works_behind_public_descriptor_constant` behavior. Rust permits a public descriptor constant to contain a pointer to a private function; consumers need the descriptor, not direct visibility of the handler.

Replace that required case with:

```markdown
* private acquisition handler behind a public `SOURCE` descriptor: compile-pass;
* private processing handlers behind a public `SOURCE` descriptor: compile-pass;
* missing public `SOURCE` descriptor in a managed source library: runner-link or compile failure;
```

Add:

```text
lexicon-core/tests/ui-pass/private_acquisition_handler.rs
lexicon-core/tests/ui-pass/private_processing_handlers.rs
lexicon-core/tests/managed_runner_contract.rs
```

The compile-pass files must instantiate the public descriptor with private functions. The managed-runner test must create a source library without `pub const SOURCE`, generate the real runner, invoke Cargo, and assert failure at the actual boundary. Do not relabel `ui/missing_handler.rs`; it currently proves a constructor arity error, not a missing exported descriptor.

### DOC-03 â `workspace/specs/specs.md`

Replace wording that says the JSON runtime envelope itself contains source arguments. The normative transport is:

```markdown
The logical runtime invocation consists of three separately validated channels:

1. a schema-versioned identity envelope containing project, source, protocol,
   operation, session, runtime identity, and execution mode;
2. the private `LEXICON_RUNTIME_CONTEXT_V1` environment document containing
   validated native paths and session context; and
3. trailing native OS arguments delivered as `OsString` values.

No source-specific argument is serialized through UTF-8 JSON. The runner rejects
identity or path disagreement between the envelope, private context, executable
manifest, and durable session.
```

This resolves the text without weakening native argument preservation.

### DOC-04 â `workspace/specs/specs.md`

The current Â§41 example says:

```rust
include!(env!("MZA_BUNDLE_INPUTS"));
```

At audited MZA commit `d2c2406...`, `MZA_BUNDLE_INPUTS` is the build-host TOML specification path, not generated Rust. Replace the normative example with:

```rust
include!(concat!(env!("OUT_DIR"), "/mza_bundle_inputs.rs"));
```

State explicitly that the bundleâs `build.rs` reads the TOML path from `MZA_BUNDLE_INPUTS`, copies the selected archives into `OUT_DIR`, and generates `mza_bundle_inputs.rs`. Link the exact accepted MZA commit. If a later accepted MZA revision changes the protocol, update the spec and adapter together; do not mix revisions.

### MATRIX-01 â `workspace/specs/status.md`

Rewrite every row into this exact shape:

```markdown
### <requirement id and title>

* Contract/spec authority: `<section>`
* Implementation: `<file>:<symbol>`
* Automated evidence: `<exact test target>::<exact test name>` or `none`
* Required environment: `<platform/toolchain>`
* Durable evidence: `<workflow + artifact/run URL>` or `none`
* Status: `not implemented | partial | implemented, unverified | conformant`
* Open gap: `<precise remaining assertion>`
```

Rules:

- a production function is not an automated test;
- prose such as âpaired admissionâ is not a test identifier;
- an early-returning test is not a skip and not platform evidence;
- a Linux container run cannot prove a native Windows path;
- temporary files outside the repository are not durable evidence;
- a row is `conformant` only for the exact commit identified by a green durable run;
- item 58 remains `partial` until HANDOFF-04 passes;
- foreground cancellation remains `partial` until FOREGROUND-01/02 pass;
- installed/no-Git Core identity remains `partial` until COREID-01/02 tests pass;
- MZA remains `not implemented` until MZA-02 invokes a real upstream entrypoint.

Add `workspace/specs/conformance.toml`:

```toml
schema_version = 1

[[requirement]]
id = "contract-30-background-continuous-ownership"
authority = "workspace/specs/specs.md#30"
implementation = [
  "lexicon-core/src/session/handoff.rs",
  "lexicon-framework/src/data/background.rs",
]
tests = [
  "background_handoff::real_operator_host_claims_reserved_handoff",
  "background_handoff::contender_cannot_steal_any_transfer_state",
]
platforms = ["linux-x86_64", "windows-x86_64"]
```

Populate one entry for every status row. Create workspace member `automation/conformance-matrix` with package name `lexicon-conformance-matrix`. Its `check` subcommand loads `workspace/specs/conformance.toml`, obtains exact identifiers from `cargo test --workspace -- --list`, and rejects missing test names, duplicate requirement IDs, duplicate platform entries, empty implementation lists, or `conformant` rows without durable evidence. Trybuild fixtures map to their actual harness test, with the fixture path recorded separately.

The checker command is:

```bash
cargo run --locked -p lexicon-conformance-matrix -- \
  check workspace/specs/conformance.toml
```

## 6. Gate 1 â close HTTP secret and recording defects first

### HTTP-01 â shared sensitivity policy

Add `lexicon-core/src/protocols/http/sensitivity.rs`:

```rust
use std::collections::HashSet;

pub(crate) const MANDATORY_SENSITIVE_HEADERS: [&str; 4] = [
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
];

pub(crate) fn is_mandatory_sensitive_header(name: &str) -> bool {
    MANDATORY_SENSITIVE_HEADERS
        .iter()
        .any(|required| name.eq_ignore_ascii_case(required))
}

pub(crate) fn must_redact_header(name: &str, explicit: &HashSet<String>) -> bool {
    is_mandatory_sensitive_header(name)
        || explicit.iter().any(|candidate| name.eq_ignore_ascii_case(candidate))
}
```

Export it privately from `lexicon-core/src/protocols/http/mod.rs`:

```rust
mod sensitivity;
```

In `transaction/recorder.rs`, delete the two direction-specific hard-coded `matches!` expressions. Both request and response recording must call the shared rule. Derive the case-insensitive explicit-sensitive name set from finalized request headers and retain it for response recording, so a source marking `X-Api-Key` protects that name in either direction.

The required request decision is:

```rust
let redact = crate::protocols::http::sensitivity::must_redact_header(
    &header.name,
    explicit_sensitive_names,
);
```

The required response decision is:

```rust
let redact = crate::protocols::http::sensitivity::must_redact_header(
    name,
    explicit_sensitive_names,
);
```

In `transaction/metadata.rs`, replace the current rule that rejects `StoredHeaderValue::Redacted` for non-mandatory names. Admission must enforce:

```rust
match &header.value {
    StoredHeaderValue::Redacted => Ok(()),
    StoredHeaderValue::Utf8(_) | StoredHeaderValue::Base64(_) => {
        if is_mandatory_sensitive_header(&header.name) {
            Err(TransactionAdmissionError::SensitiveHeaderNotRedacted {
                name: header.name.clone(),
            })
        } else {
            Ok(())
        }
    }
}
```

This is intentionally asymmetric: mandatory secrets must be redacted; custom explicitly sensitive headers are allowed to be structurally redacted.

Add unit and end-to-end cases for all four names, mixed casing, request direction, response direction, and custom `X-Api-Key`. The test must decode metadata through `admit_transaction_from_disk`; substring checks are insufficient.

### HTTP-02 â exact origin comparison

In `lexicon-core/src/protocols/http/context.rs`, replace the current `domain()` comparison with exact origin comparison:

```rust
fn same_origin(left: &url::Url, right: &url::Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}
```

Then use:

```rust
let cross_origin = !same_origin(&request.url, &next_url);
```

When `cross_origin` is true, remove:

- every mandatory sensitive header;
- every header marked explicitly sensitive;
- `Host`, so the transport calculates the next authority correctly.

Do not compare `Url::domain()`: IP literals return no registrable domain and can be incorrectly treated as same-origin.

Add a two-server test using distinct loopback IP literals or ports. The second server must record received headers and prove that all mandatory and explicit secrets are absent.

### HTTP-03 â test exact durable behavior

Add `lexicon-core/src/protocols/http/test_support/scripted_server.rs` and export it only under `#[cfg(test)]`. It must support deterministic scripted responses, redirects, connection close after N body bytes, response headers, and an attempt counter.

Replace or strengthen the current named tests in `protocols/http/runner.rs` so they assert:

```text
compressed_response_preserves_exact_wire_bytes_and_hash
redirect_chain_persists_each_attempt_with_parent_identity
retry_policy_persists_exactly_three_distinct_attempts
connection_failure_persists_finalized_failure_metadata
truncated_body_preserves_partial_bytes_and_incomplete_marker
all_mandatory_and_explicit_headers_are_structurally_redacted
sensitive_query_never_appears_in_any_durable_or_diagnostic_text
cross_origin_redirect_strips_secrets_for_ip_literal_hosts
execute_returns_only_after_transaction_directory_sync
```

Every attempt assertion must enumerate transaction directories and decode their typed metadata. Verify exact indices, parents, terminal state, raw body bytes, byte count, SHA-256, and error classification.

Add a private durability observer/failpoint to the transaction publisher. Production defaults to no observer. The observer emits ordered events only after each successful file sync, atomic replacement, and parent-directory sync. The final test blocks the last directory sync and proves `execute` has not returned. This proves program ordering; it must not be described as proof against every storage-device failure mode.

## 7. Gate 2 â installed command, scaffold, Core identity, and builds

### CLI-01 â `lexicon-cli/Cargo.toml`

Append this literal target declaration:

```toml
[[bin]]
name = "lexicon"
path = "src/main.rs"
```

The installed artifact, `CARGO_BIN_EXE_lexicon`, MZA artifact selector, README commands, and integration tests must all use `lexicon`, never the package-default `lexicon-cli` name.

### CLI-01 â `lexicon-cli/src/main.rs`

Add `lexicon-cli/src/lib.rs`:

```rust
pub mod cli;

pub use cli::{Cli, CliError, dispatch};
```

Remove `mod cli;` from `main.rs` and replace the unconditional exit-1 wrapper with:

```rust
use clap::Parser;
use std::process::ExitCode;
use lexicon_cli::{Cli, dispatch};

fn main() -> ExitCode {
    match dispatch(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[lexicon] ERROR: {error}");
            error.exit_code()
        }
    }
}
```

In `lexicon-cli/src/cli/mod.rs`, change `dispatch` from `Result<(), String>` to `Result<(), CliError>`. Add typed variants for foreground execution, background execution, source creation/build, project initialization, aggregate build, operator-host execution, protocol validation, and help rendering. Do not convert framework errors to strings at their call sites. A final `Message(String)` variant may cover preexisting noncritical string APIs during the refactor, but cancellation and ownership failures must retain their concrete variants.

The error type exposes:

```rust
impl CliError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Interrupted { unix_signal: Some(2), .. } => ExitCode::from(130),
            Self::Interrupted { unix_signal: Some(15), .. } => ExitCode::from(143),
            Self::Interrupted { .. } => ExitCode::from(1),
            _ => ExitCode::from(1),
        }
    }
}
```

Windows cancellation uses a documented nonzero status derived from the console event outcome; it must not be mislabeled as Unix signal 2.

### NAME-01 â new typed identity module

Create `lexicon-framework/src/identity/name.rs`:

```rust
use std::{fmt, str::FromStr};

const MAX_MANAGED_NAME_BYTES: usize = 63;
const RESERVED: &[&str] = &[
    "lexicon", "http", "data", "state", "runtime", "sessions",
    "get-raw-data", "process-data", "con", "prn", "aux", "nul",
    "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9",
    "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ManagedName(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedNameError {
    Empty,
    TooLong,
    InvalidGrammar,
    Reserved,
}

impl ManagedName {
    pub fn parse(value: &str) -> Result<Self, ManagedNameError> {
        if value.is_empty() {
            return Err(ManagedNameError::Empty);
        }
        if value.len() > MAX_MANAGED_NAME_BYTES {
            return Err(ManagedNameError::TooLong);
        }
        let bytes = value.as_bytes();
        let edge_ok = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
        if !edge_ok(bytes[0]) || !edge_ok(bytes[bytes.len() - 1]) {
            return Err(ManagedNameError::InvalidGrammar);
        }
        if !bytes.iter().all(|byte| edge_ok(*byte) || *byte == b'-') {
            return Err(ManagedNameError::InvalidGrammar);
        }
        if RESERVED.contains(&value) {
            return Err(ManagedNameError::Reserved);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ManagedName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ManagedName {
    type Err = ManagedNameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}
```

Add `lexicon-framework/src/identity/mod.rs` containing `pub(crate) mod name;`, and declare `mod identity;` in `lib.rs`.

Replace `validate_source_name` and `validate_project_name` call sites with early parsing into `ManagedName`. Filesystem joins, TOML interpolation, package names, binary names, and Rust templates must receive `&ManagedName`, not unchecked `&str`.

Add table-driven tests for empty, dot names, separators, absolute paths, uppercase, whitespace, controls, quotes, TOML/Rust metacharacters, leading/trailing hyphens, Windows device names, reserved layout names, length 63, and length 64. Run the same cases on Linux and Windows.

### SCAFFOLD-01 â exact source tree

In `lexicon-framework/src/lib.rs`, add the omitted directory to the `directories` array:

```rust
Path::new("http/get-raw-data/state"),
```

Add these three entries to the staged files:

```rust
(
    "http/get-raw-data/state/.gitkeep",
    "# source-owned durable acquisition state\n".to_owned(),
),
(
    "http/get-raw-data/session_status.json",
    initial_acquisition_status_json(&project, &source)?,
),
(
    "http/process-data/session_status.json",
    initial_processing_status_json(&project, &source)?,
),
```

Do not hand-format those JSON documents. In `lexicon-core/src/session/model.rs`, add a typed constructor:

```rust
pub struct NewSessionStatus {
    pub project: ProjectIdentity,
    pub runtime: OwnedRuntimeIdentity,
    pub operation: SessionOperation,
}

impl SessionStatusV1 {
    pub fn initial(new: NewSessionStatus, clock: &dyn SessionClock) -> Self {
        Self {
            schema_version: SESSION_SCHEMA_VERSION,
            project: new.project,
            runtime: new.runtime,
            operation: new.operation,
            current_session: None,
            current_state: None,
            revision: 0,
            updated_at: clock.now(),
        }
    }
}
```

Expose the existing canonical session-status encoder through a public or crate-shared typed API and use it for scaffold creation. The initial files must decode through the production decoder and agree with project, source, protocol, and operation.

The exact required new-source shape is:

```text
sources/<source>/http/
  source.toml
  discovery.md
  data/raw/.gitkeep
  data/processed/.gitkeep
  get-raw-data/
    Cargo.toml
    Cargo.lock
    session_status.json
    state/.gitkeep
    sessions/
    get-raw-data-impl/Cargo.toml
    get-raw-data-impl/src/lib.rs
    lexicon-runner/Cargo.toml
    lexicon-runner/src/main.rs
    runtime/.gitignore
  process-data/
    Cargo.toml
    Cargo.lock
    session_status.json
    sessions/
    process-data-impl/Cargo.toml
    process-data-impl/src/lib.rs
    lexicon-runner/Cargo.toml
    lexicon-runner/src/main.rs
    runtime/.gitignore
```

Update `SourceCreateResult.created_files` to include all three new files.

### SCAFFOLD-02 â durable file publication

Create `lexicon-framework/src/fs/durable.rs`:

```rust
use std::{fs, io::Write, path::Path};

pub(crate) fn write_new_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "file has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

pub(crate) enum DirectorySyncOutcome {
    Synced,
    UnsupportedByPlatform,
}

pub(crate) fn sync_directory(path: &Path) -> std::io::Result<DirectorySyncOutcome> {
    #[cfg(unix)]
    {
        fs::File::open(path)?.sync_all()?;
        Ok(DirectorySyncOutcome::Synced)
    }
    #[cfg(windows)]
    {
        crate::fs::windows::flush_directory(path)
    }
}
```

Also create `lexicon-framework/src/fs/mod.rs`:

```rust
pub(crate) mod durable;
#[cfg(windows)]
mod windows;
```

Declare `mod fs;` from `lexicon-framework/src/lib.rs` and add `lexicon-framework/src/fs/windows.rs` for the platform implementation described below.

Add a Windows implementation that uses write-through replacement and attempts an opened directory handle with `FILE_FLAG_BACKUP_SEMANTICS` and `FlushFileBuffers`. If Windows/filesystem semantics document directory flush as unsupported, return `DirectorySyncOutcome::UnsupportedByPlatform`, record that outcome in diagnostics, and rely on the strongest supported file flush plus write-through atomic replacement. Do not silently pretend a directory flush occurred. This follows the specificationâs âwhere supportedâ rule.

Replace scaffold `fs::write` calls with `write_new_file`. After both `cargo generate-lockfile` calls succeed:

1. sync every generated lockfile;
2. sync every staged directory bottom-up;
3. rename the staging directory to the final source path;
4. sync the final source parent directory.

Apply the same pattern to project initialization. On any error, the final project/source path must not exist. The temporary staging directory may be automatically removed only after failure details have been captured.

Add filesystem failpoint tests for every write, file sync, directory sync, lock generation, and final rename. Each test asserts no final partial tree is visible.

### COREID-01 â fail-closed embedded identity

Replace `lexicon-framework/build.rs` with an implementation that emits both exact identity components:

```rust
const EXPECTED_CORE_GIT_URL: &str = "https://github.com/ssr2zvy/lexicon";

fn main() {
    println!("cargo:rerun-if-env-changed=LEXICON_EMBEDDED_CORE_GIT_URL");
    println!("cargo:rerun-if-env-changed=LEXICON_EMBEDDED_CORE_GIT_REV");
    println!("cargo:rerun-if-changed=../lexicon-core");

    let url = resolve_core_url().expect("resolve immutable Core Git URL");
    let revision = resolve_core_revision().expect("resolve immutable Core Git revision");
    validate_url(&url).expect("validate immutable Core Git URL");
    validate_revision(&revision).expect("validate immutable Core Git revision");
    verify_compiled_core_tree(&revision).expect("compiled lexicon-core must match embedded revision");

    println!("cargo:rustc-env=LEXICON_EMBEDDED_CORE_GIT_URL={url}");
    println!("cargo:rustc-env=LEXICON_EMBEDDED_CORE_GIT_REV={revision}");
}
```

Required resolution policy:

- if either environment variable is provided, both are required;
- the URL must normalize to exactly the approved repository identity;
- revision is exactly 40 lowercase hexadecimal characters and not all zero;
- otherwise obtain `remote.origin.url` and `rev-parse HEAD` from the build checkout;
- verify `git diff-index --quiet <revision> -- lexicon-core` and require `git ls-files --others --exclude-standard -- lexicon-core` to produce no entries, rejecting staged, unstaged, or untracked files under `lexicon-core`;
- a release source archive without Git must receive both values from the pinned release process and carry a verified source-manifest hash;
- no zero, package version, branch, tag, or âunknownâ fallback exists.

Generate `OUT_DIR/embedded_core_identity.rs` and include it from a small framework identity module:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedCoreIdentity {
    pub git_url: &'static str,
    pub git_revision: &'static str,
}

pub const EMBEDDED_CORE_IDENTITY: EmbeddedCoreIdentity = EmbeddedCoreIdentity {
    git_url: env!("LEXICON_EMBEDDED_CORE_GIT_URL"),
    git_revision: env!("LEXICON_EMBEDDED_CORE_GIT_REV"),
};
```

Generated workspace manifests must interpolate both fields from this one constant.

### COREID-02 â exact manifest and resolved dependency admission

Move dependency validation out of the monolithic `lexicon-framework/src/lib.rs` into `lexicon-framework/src/build/core_dependency.rs`.

The manifest validator must require the exact table:

```toml
lexicon_core = {
  package = "lexicon-core",
  git = "https://github.com/ssr2zvy/lexicon",
  rev = "<embedded 40-character revision>"
}
```

The validator must compare the tableâs key set exactly with:

```rust
const REQUIRED_KEYS: [&str; 3] = ["git", "package", "rev"];
```

Reject `path`, `version`, `branch`, `tag`, `registry`, alternate Git URLs, extra features, or any other key. The two managed runner manifests must still use only `lexicon_core = { workspace = true }`.

After lockfile generation, invoke:

```text
cargo metadata --locked --format-version 1 --manifest-path <workspace>/Cargo.toml
```

Do not use `--no-deps` for final identity admission. Locate the resolved `lexicon-core` package and verify its `source` URL, requested revision, resolved commit fragment, and package identity agree with `EMBEDDED_CORE_IDENTITY`. Reject multiple matching packages.

Add negative tests for every forbidden key/source form and a lockfile whose resolved commit disagrees with the manifest revision.

### COREID-03 â remove production legacy entrypoints

Delete the public legacy surface from `lexicon-core/src/lib.rs`:

```text
trait HttpAcquisition
run_http_source
```

Delete `HttpAcquisitionContext::from_env_legacy` and `SessionDataPaths::from_legacy_parts` from production code. Keep no fallback to `LEXICON_SOURCE_DIRECTORY`.

Change the acquisition state accessor in `protocols/http/context.rs` from:

```rust
pub fn source_state_directory(&self) -> Option<&Path>
```

to:

```rust
pub fn source_state_directory(&self) -> &Path {
    self.paths
        .source_state_directory()
        .expect("validated acquisition context always has source state")
}
```

Prefer strengthening `SessionDataPaths` into separate acquisition and processing path types so the invariant is represented by the type system rather than `expect`. Update all current `.unwrap()` test and production call sites. Processing must not gain an acquisition-state directory.

Delete legacy tests or move them behind a non-default, explicitly documented transition feature. Full Contract V1 conformance is tested with that feature disabled and its APIs absent.

### COREID-04 â real installed/no-Git test

Create `lexicon-cli/tests/installed_core_identity.rs`. It must:

1. obtain `env!("CARGO_BIN_EXE_lexicon")`;
2. copy it to a fresh directory outside the checkout;
3. run it from a second fresh working directory;
4. place Cargo and rustc in a controlled `PATH` but omit Git, or install a `git` shim that exits nonzero and records any invocation;
5. remove `CARGO_MANIFEST_DIR`, `GIT_DIR`, `GIT_WORK_TREE`, and checkout-specific environment;
6. run the actual process `lexicon init ...` and then `lexicon source create ...`;
7. parse both generated workspace manifests and assert the exact embedded URL/revision table;
8. run `cargo metadata --locked` and `cargo build --release --locked` for acquisition and processing;
9. assert the Git shim was never invoked by Lexicon at runtime and no generated path references the original checkout.

Set Cargoâs Git-fetch behavior explicitly so the test result is deterministic. If the controlled cache already contains the exact dependency, run offline. If the test permits Cargoâs built-in fetcher, ensure `net.git-fetch-with-cli = false`; the absence of a standalone Git executable must remain real.

The strongest CI variant makes the original checkout unreadable to the test process. The present in-process dispatch tests may remain as unit tests but must be renamed; they are not installed/no-Git evidence.

### BUILD-01 â exact Cargo artifact selection

Replace untyped `serde_json::Value` filtering with:

```rust
#[derive(serde::Deserialize)]
struct CompilerArtifact {
    reason: String,
    package_id: String,
    target: CargoTarget,
    profile: CargoProfile,
    executable: Option<std::path::PathBuf>,
}

#[derive(serde::Deserialize)]
struct CargoTarget {
    kind: Vec<String>,
    crate_types: Vec<String>,
    name: String,
}

#[derive(serde::Deserialize)]
struct CargoProfile {
    test: bool,
}
```

Change selection to accept `expected_target_directory: &Path`. It must require exactly one record satisfying:

```rust
artifact.reason == "compiler-artifact"
    && artifact.package_id == expected_package_id
    && artifact.target.kind == ["bin"]
    && artifact.target.crate_types == ["bin"]
    && artifact.target.name == expected_binary_name
    && !artifact.profile.test
    && artifact.executable.is_some()
```

Canonicalize the executable and isolated target directory, require the executable to be under `<target>/release/`, require the platform executable suffix, and require it to be a regular file. The build command must contain `--release --locked --message-format=json-render-diagnostics` and no `--target`, thereby building for the current host.

Reject debug paths, external paths, symlink escapes, missing executables, multiple matches, libraries, test profiles, wrong package IDs, wrong binary names, and malformed JSON.

### BUILD-02 â lock generation and build command seams

Introduce a typed command executor shared by lock generation, metadata, and build:

```rust
pub(crate) trait CargoExecutor {
    fn run(&self, invocation: &CargoInvocation) -> Result<CargoOutput, CargoExecutionError>;
}

pub(crate) enum CargoInvocation {
    GenerateLockfile { manifest: PathBuf },
    MetadataLocked { manifest: PathBuf },
    BuildReleaseLocked { manifest: PathBuf, target_dir: PathBuf },
}
```

Production converts only these typed variants to exact Cargo arguments. Tests assert source creation can generate lockfiles but never selects `BuildReleaseLocked`; therefore `lexicon source create` resolves dependencies without compiling source code.

Add real `build_source` integration tests for acquisition build failure, processing build failure, unchanged lockfiles, isolated target directories, exact artifacts, and preservation of the previously published pair. Publication-specific failure injection is completed under PUBLISH-01.

## 8. Gate 3 â atomic, fenced background supervision transfer

### Current code that must disappear

The baseline sequence is encoded at:

```text
lexicon-framework/src/data/background.rs:402-411   writes ACK
lexicon-framework/src/data/background.rs:413-428   tries to acquire afterward
lexicon-framework/src/data/background.rs:335       initiator releases and returns
lexicon-framework/src/session/coordinator.rs:85    drops physical lease
lexicon-framework/src/session/coordinator.rs:543   test documents race window
```

Delete the readiness-only `HandoffIntentDocumentV1` and `HandoffAcknowledgementDocumentV1`, the public `release_for_handoff` primitive, the fake shell/cmd acknowledgement executor, and the known-race test. No final code comment may call the race an accepted limitation.

### HANDOFF-01 â Core handoff model

Add dependencies to `lexicon-core/Cargo.toml` and `lexicon-framework/Cargo.toml` using exact versions selected and locked by the implementation:

```toml
hmac = "0.12"
getrandom = "0.3"
```

Create `lexicon-core/src/session/handoff.rs`. Its public model must be equivalent to this literal API:

```rust
use crate::session::{SessionIdentity, SessionTimestamp};

pub const HANDOFF_SCHEMA_VERSION: u32 = 1;
pub const HANDOFF_TOKEN_BYTES: usize = 32;
pub const HANDOFF_DIGEST_BYTES: usize = 32;

pub struct HandoffToken([u8; HANDOFF_TOKEN_BYTES]);

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct HandoffTokenDigest([u8; HANDOFF_DIGEST_BYTES]);

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorIdentityV1 {
    pub instance_nonce: String,
    pub process_id: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum HandoffAuthorityStateV1 {
    Reserved {
        expected_instance_nonce: String,
    },
    Ready {
        operator: OperatorIdentityV1,
    },
    Owned {
        operator: OperatorIdentityV1,
        owned_at: SessionTimestamp,
    },
    Revoked {
        reason: HandoffRevocationReason,
        revoked_at: SessionTimestamp,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffRevocationReason {
    SpawnFailed,
    ChildExited,
    ReadyTimedOut,
    OwnershipTimedOut,
    AuthenticationFailed,
    Expired,
    InitiatorRecovery,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffReservationDocumentV1 {
    pub schema_version: u32,
    pub session_id: String,
    pub epoch: u64,
    pub token_digest: HandoffTokenDigest,
    pub created_at: SessionTimestamp,
    pub expires_at: SessionTimestamp,
    pub authority: HandoffAuthorityStateV1,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandoffReservationRequest {
    pub token_digest: HandoffTokenDigest,
    pub expected_instance_nonce: String,
    pub expires_at: SessionTimestamp,
}

#[derive(Debug)]
pub struct HandoffClaimProof {
    pub session: SessionIdentity,
    pub epoch: u64,
    pub token: HandoffToken,
    pub operator: OperatorIdentityV1,
}
```

Required implementation rules:

- generate `HandoffToken` from the OS CSPRNG; UUID v4 text is not the secret;
- implement `Debug` for the token as `HandoffToken([REDACTED])`;
- never implement `Display`, `Serialize`, or `Clone` for the plaintext token;
- compute the persisted digest as `HMAC-SHA-256(token, domain-separator || session-identity || epoch)`;
- verify it in constant time through `Mac::verify_slice`, never with ordinary byte equality;
- validate nonce length/grammar, nonzero epoch, expiration ordering, schema version, identity, state transition, and monotonically increasing revision;
- bound the handoff document and acknowledgement sizes before allocation/deserialization;
- the durable document stores only `HandoffTokenDigest`.

Export typed items from `lexicon-core/src/session/mod.rs`.

### HANDOFF-01 â durable paths and store operations

In `lexicon-core/src/session/store.rs`, add `SessionOperationRoot::handoff_path(session)` returning:

```text
<operation-root>/sessions/<session-id>/handoff.json
```

Add these operations:

```rust
impl SessionStore {
    pub fn reserve_handoff(
        &self,
        lease: &SessionLease,
        session: &SessionIdentity,
        expected_session_revision: u64,
        request: HandoffReservationRequest,
    ) -> Result<HandoffReservationDocumentV1, SessionHandoffError>;

    pub fn load_handoff(
        &self,
        session: &SessionIdentity,
    ) -> Result<Option<HandoffReservationDocumentV1>, SessionHandoffError>;

    pub fn mark_handoff_ready(
        &self,
        session: &SessionIdentity,
        epoch: u64,
        token: &HandoffToken,
        operator: OperatorIdentityV1,
        expected_revision: u64,
    ) -> Result<HandoffReservationDocumentV1, SessionHandoffError>;

    pub fn claim_handoff(
        &self,
        lease: &SessionLease,
        proof: HandoffClaimProof,
        expected_revision: u64,
    ) -> Result<HandoffReservationDocumentV1, SessionHandoffError>;

    pub fn revoke_handoff(
        &self,
        lease: &SessionLease,
        session: &SessionIdentity,
        epoch: u64,
        reason: HandoffRevocationReason,
        expected_revision: u64,
    ) -> Result<HandoffReservationDocumentV1, SessionHandoffError>;
}
```

`reserve_handoff`, `claim_handoff`, and `revoke_handoff` require proof of the exact session lease. Reuse the session storeâs staged write, file sync, atomic replace, and directory sync primitive; do not call plain `std::fs::write`.

`mark_handoff_ready` is the one transition performed before the successor can own the physical session lease. It must authenticate the secret, use an independent atomic compare-and-replace discipline on the handoff document, and be legal only from the matching `Reserved` state. An alternative private authenticated control channel may replace this method, but the durable reservation still exists and `Ready` is never sufficient for CLI success.

Add `SessionHandoffError` variants for:

```rust
MissingReservation
AlreadyReserved { epoch: u64 }
ReservedForDifferentSuccessor { epoch: u64 }
SchemaVersion(u32)
Malformed(String)
IdentityMismatch
AuthenticationFailed
OperatorMismatch
RevisionConflict { expected: u64, actual: u64 }
StaleEpoch { expected: u64, actual: u64 }
Expired { epoch: u64 }
Revoked { epoch: u64 }
AlreadyClaimed { epoch: u64 }
InvalidTransition
LeaseRequired(SessionLeaseError)
Persistence(SessionStoreError)
```

Errors and debug output must never contain the secret token or unredacted source arguments.

### HANDOFF-02 â fence every ordinary path

Add this variant to `SessionLeaseError` or, preferably, surface the dedicated handoff error through the coordinator:

```rust
ReservedForSuccessor {
    session: SessionIdentity,
    epoch: u64,
    expires_at: SessionTimestamp,
}
```

After any ordinary caller acquires the physical lock and before it reads, transitions, reconciles, abandons, or replaces a session, it must inspect `handoff.json`.

The following paths must call one common guard:

```text
SessionStore::prepare
SessionStore::load/reconcile stale owner path
SessionCoordinator::prepare_run
SessionCoordinator::prepare_resume
SessionCoordinator::resume_prepared_launch (replace this API)
abandonment
root-summary repair when it could alter the reserved session
```

The guard semantics are:

```rust
match reservation.authority {
    Reserved { .. } | Ready { .. } if now < reservation.expires_at => {
        Err(SessionHandoffError::ReservedForDifferentSuccessor { .. })
    }
    Owned { .. } => Err(SessionHandoffError::AlreadyClaimed { .. }),
    Revoked { .. } => continue_with_normal_policy(),
    Reserved { .. } | Ready { .. } => {
        atomically_revoke_expired_epoch_while_holding_lease()?;
        continue_with_stale_reconciliation()
    }
}
```

Only `claim_handoff` receives the proof that bypasses the ordinary guard. An ordinary contender may briefly acquire the OS lock after the initiator releases it, but it must release immediately without changing the session or root summary. It cannot satisfy success, abandon, fail, resume, or steal the reserved session.

### HANDOFF-02 â framework ownership types

In `lexicon-framework/src/session/coordinator.rs`, remove `PreparedSessionLaunch::release_for_handoff` and replace it with move-only owners:

```rust
pub struct ReservedSessionHandoff {
    prepared: PreparedSessionLaunch,
    reservation: HandoffReservationDocumentV1,
    token: HandoffToken,
}

pub struct TransferableSessionHandoff {
    record: SessionRecordV1,
    reservation: HandoffReservationDocumentV1,
    token: HandoffToken,
    operation_root: PathBuf,
}

impl PreparedSessionLaunch {
    pub fn reserve_handoff(
        self,
        request: HandoffReservationRequest,
    ) -> Result<ReservedSessionHandoff, SessionCoordinationError>;
}

impl ReservedSessionHandoff {
    pub fn release_after_authenticated_ready(
        self,
        operator: &OperatorIdentityV1,
    ) -> Result<TransferableSessionHandoff, SessionCoordinationError>;

    pub fn fail_before_release(
        self,
        reason: HandoffRevocationReason,
        failure: SafeSessionFailure,
    ) -> Result<SessionRecordV1, SessionCoordinationError>;
}
```

The only code path able to drop the initiatorâs physical lease is `release_after_authenticated_ready`, after confirming matching durable reservation state. Make its fields private and do not implement `Clone`.

Add a coordinator entrypoint for the operator host:

```rust
pub fn claim_reserved_handoff(
    &self,
    proof: HandoffClaimProof,
) -> Result<PreparedSessionLaunch, SessionCoordinationError>;
```

It acquires the physical lease, calls `SessionStore::claim_handoff`, verifies the session remains `Prepared`, and returns ownership only after the `Owned` document is synced.

### HANDOFF-03 â private capability transport

In `lexicon-framework/src/supervision/mod.rs`:

- remove `#[serde(default)]` from all handoff fields;
- reject empty tokens/references;
- remove the plaintext token from the JSON invocation and command-line arguments;
- add `handoff_epoch` and `operator_instance_nonce` to the private invocation reference;
- pass the plaintext capability through `LEXICON_OPERATOR_HANDOFF_CAPABILITY_V1` or an inherited private pipe;
- remove that environment variable before the operator host spawns the source runtime;
- ensure diagnostics, `Debug`, and process-spawn error text never echo it.

If an environment variable is selected, the operator host must read and remove it immediately:

```rust
let token = std::env::var_os(HANDOFF_CAPABILITY_ENV)
    .ok_or(OperatorHostError::MissingHandoffCapability)?;
unsafe {
    std::env::remove_var(HANDOFF_CAPABILITY_ENV);
}
```

Because environment mutation is process-global, perform this before starting any threads and document that precondition. An inherited one-shot pipe is preferred if the implementation already introduces platform-specific handle management.

### HANDOFF-03 â rewrite `data/background.rs`

The initiator flow must be this state machine:

```rust
let token = HandoffToken::generate()?;
let instance_nonce = generate_operator_instance_nonce()?;
let reserved = prepared.reserve_handoff(HandoffReservationRequest {
    token_digest: token.digest(prepared.session(), epoch),
    expected_instance_nonce: instance_nonce.clone(),
    expires_at: deadline,
})?;

let mut child = spawn_real_operator_host(
    reserved.session(),
    reserved.epoch(),
    &instance_nonce,
    token,
)?;

let ready = wait_for_authenticated_ready(&mut child, &reserved, READY_TIMEOUT)?;
let transferable = reserved.release_after_authenticated_ready(&ready.operator)?;
let owned = wait_for_durable_owned(&mut child, &transferable, OWNERSHIP_TIMEOUT)?;
validate_exact_owned_successor(&owned, &ready.operator, transferable.epoch())?;

Ok(BackgroundDataOutcome::started(
    transferable.session().clone(),
    owned.operator,
))
```

The operator-host flow must be:

```rust
let proof = decode_private_handoff_proof()?;
let reservation = coordinator.validate_reserved_handoff(&proof)?;
coordinator.mark_handoff_ready(&proof)?;
let prepared = retry_claim_until_deadline(&coordinator, proof)?;
coordinator.verify_durable_owned(prepared.session(), prepared.handoff_epoch())?;
spawn_and_supervise(PreparedForegroundExecution::new(
    prepared,
    operation,
    project_name,
    source_name,
))
```

The initiator must validate both authenticated `Ready` and durable `Owned`, and may return only after `Owned`. A file merely appearing, a lease merely being `Owned`, or the child merely remaining alive is insufficient.

Replace plain intent/ack writes with the shared durable publisher. A JSON parse failure while a staged publication may still be in progress is retryable until deadline; a fully published malformed document is a typed authentication/protocol failure.

### HANDOFF-03 â failure resolution

Implement one function that resolves every parent-side failure:

```rust
fn resolve_failed_handoff(
    child: &mut Child,
    handoff: ParentHandoffOwner,
    stage: HandoffStage,
    cause: HandoffFailureCause,
) -> Result<SessionRecordV1, BackgroundDataExecutionError>;
```

It must distinguish:

1. parent still owns the lease: revoke, kill/reap child, fail session;
2. parent released and can reacquire: reacquire, fence epoch, kill/reap child, fail session;
3. expected operator has durably claimed: do not kill or reconcile authority no longer owned; return the actual owned outcome or a typed âtransfer completed while acknowledgement failedâ result;
4. ownership cannot be determined: return a typed ownership-uncertain error and retain every available diagnostic without asserting terminal success.

Cover spawn failure, exit before Ready, exit after Ready, timeout before release, timeout after release, malformed/mismatched authentication, persistence error, cleanup error, initiator death at every transition, and operator death after ownership.

Calling `Child::kill` is not enough: wait until reaped. Dropping `Child` does not terminate or reap it and must never be described as doing so.

### HANDOFF-04 â real multiprocess suite

Create `lexicon-cli/tests/background_handoff.rs` plus a tiny controlled source fixture. Every success case must execute:

```text
<copied CARGO_BIN_EXE_lexicon> __operator-host <private-reference>
```

Do not use `sh -c`, `cmd /C`, a sleeping child, or a fake executor that writes an acknowledgement itself.

Add a test-only synchronization facility compiled only for integration-test binaries. It may pause at named state transitions but must not bypass admission or change production policy. Required tests:

```text
real_operator_host_claims_reserved_handoff
initiator_returns_only_after_expected_host_is_durably_owned
contender_cannot_steal_held_by_initiator
contender_cannot_steal_reserved
contender_cannot_steal_ready
contender_cannot_steal_after_physical_release
contender_cannot_steal_during_claim
unrelated_process_cannot_authenticate_reservation
ready_is_not_owned
operator_exit_before_ready_reconciles
operator_exit_after_ready_reconciles
operator_exit_after_owned_uses_terminal_supervision
initiator_exit_before_ready_allows_expiry_recovery
initiator_exit_after_ready_preserves_valid_successor
stale_epoch_cannot_claim
replayed_token_cannot_claim
expired_reservation_cannot_claim
late_fenced_successor_cannot_claim
partial_publication_is_never_accepted
handoff_timeout_terminates_and_reaps_child
acquisition_background_run_reaches_terminal_session
processing_background_run_reaches_terminal_session
```

Run this suite natively on Linux x86_64 and Windows x86_64. If ARM64 is a supported release target, run architecture smoke coverage there and keep the protocol architecture-independent. Platform unavailability is reported by the outer workflow as a skip; a Rust test must not print âskippedâ and return success.

## 9. Gate 4 â foreground cancellation and process-tree ownership

### Current code that must change

`lexicon-framework/src/data/foreground.rs:146` blocks in `Child::wait()` and treats `Interrupted` only as a reason to retry. `ProcessCommandLauncher` creates neither a Unix process group nor a Windows Job Object. The supervisor therefore has no contract-level way to forward cancellation to the whole runtime tree, apply a grace period, force termination, and prove reaping.

### FOREGROUND-01 â process supervision boundary

Create:

```text
lexicon-framework/src/process/mod.rs
lexicon-framework/src/process/unix.rs
lexicon-framework/src/process/windows.rs
lexicon-framework/src/process/cancellation.rs
```

Declare `pub(crate) mod process;` from `lexicon-framework/src/lib.rs`; `process/mod.rs` selects exactly one platform implementation with `#[cfg(unix)]` and `#[cfg(windows)]`. Other targets fail at compile time with a clear unsupported-platform error until deliberately supported.

The shared API is:

```rust
use std::{ffi::{OsStr, OsString}, path::Path, process::ExitStatus, time::Duration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationKind {
    Interrupt,
    Terminate,
    ConsoleClose,
}

pub trait CancellationSource: Send + Sync {
    fn requested(&self) -> Option<CancellationKind>;
}

pub trait SupervisedChild {
    fn id(&self) -> u32;
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>>;
    fn request_graceful_shutdown(&mut self, kind: CancellationKind) -> std::io::Result<()>;
    fn force_terminate_tree(&mut self) -> std::io::Result<()>;
    fn wait_reaped(&mut self) -> std::io::Result<ExitStatus>;
}

pub trait ProcessTreeLauncher {
    fn spawn(
        &self,
        executable: &Path,
        arguments: &[OsString],
        context_environment: &OsStr,
        working_directory: &Path,
    ) -> std::io::Result<Box<dyn SupervisedChild>>;
}

pub struct CancellationPolicy {
    pub graceful_timeout: Duration,
    pub poll_interval: Duration,
}
```

`RunningForegroundExecution` stores `Box<dyn SupervisedChild>` and a `CancellationSource`, not a bare `std::process::Child`.

### FOREGROUND-01 â Unix implementation

In `process/unix.rs`, spawn the runtime in a new process group:

```rust
use std::os::unix::process::CommandExt;

command.process_group(0);
let child = command.spawn()?;
let process_group = child.id() as libc::pid_t;
```

Required control operations:

```rust
fn signal_group(process_group: libc::pid_t, signal: libc::c_int) -> std::io::Result<()> {
    let result = unsafe { libc::kill(-process_group, signal) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}
```

- graceful interrupt sends `SIGINT` for an interrupt and `SIGTERM` for termination;
- force sends `SIGKILL` to the process group;
- `ESRCH` is accepted only after `try_wait` or `wait` proves the owned child exited;
- the direct child is always reaped;
- a test descendant must also disappear before the supervisor returns.

Install CLI signal handlers before dispatch using `sigaction`. The handler performs only async-signal-safe work: store a small code into a static atomic. Preserve and restore prior handlers in tests. Do not allocate, lock, print, or touch session files inside a signal handler.

### FOREGROUND-01 â Windows implementation

Extend the Windows dependency features in `lexicon-framework/Cargo.toml`:

```toml
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.59", features = [
  "Win32_Foundation",
  "Win32_System_Console",
  "Win32_System_JobObjects",
  "Win32_System_Threading",
] }
```

In `process/windows.rs`:

1. spawn with `CREATE_NEW_PROCESS_GROUP`;
2. create a Job Object;
3. set `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`;
4. assign the child process to the job before normal execution can fan out, using suspended creation if necessary to make assignment race-free;
5. resume the primary process only after assignment;
6. on graceful cancellation call `GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, process_group_id)`;
7. after the grace deadline call `TerminateJobObject`;
8. wait for and reap the direct child;
9. close all thread/process/job handles exactly once.

Use RAII wrappers with nonzero owned handles. `Drop` must close handles; explicit termination remains a method with observable errors. Job assignment failure is a spawn failure and must fail the prepared session before returning.

Install `SetConsoleCtrlHandler` once in the CLI. The callback stores `CTRL_C_EVENT`, `CTRL_BREAK_EVENT`, close, logoff, or shutdown in an atomic and returns the documented handled value. It performs no I/O or allocation.

### FOREGROUND-01 â cancellation-aware wait loop

Replace the blocking loop in `foreground.rs` with behavior equivalent to:

```rust
loop {
    if let Some(kind) = cancellation.requested() {
        child.request_graceful_shutdown(kind)?;
        let deadline = std::time::Instant::now() + policy.graceful_timeout;

        loop {
            if let Some(status) = child.try_wait()? {
                return reconcile_cancelled(self, kind, status, CancellationEscalation::Graceful);
            }
            if std::time::Instant::now() >= deadline {
                child.force_terminate_tree()?;
                let status = child.wait_reaped()?;
                return reconcile_cancelled(self, kind, status, CancellationEscalation::Forced);
            }
            std::thread::sleep(policy.poll_interval);
        }
    }

    if let Some(status) = child.try_wait()? {
        return reconcile_terminal_execution(self, observe_termination(status));
    }

    std::thread::sleep(policy.poll_interval);
}
```

The production implementation may use an event/condition variable instead of polling. It must preserve the lease until terminal session reconciliation completes.

If graceful signaling fails, inspect process state. If still live, force the tree. If termination or reaping remains uncertain, return the existing ownership-uncertain class with signal, force, wait, and session-reconciliation causes retained separately.

### FOREGROUND-02 â durable cancellation result

Add explicit session failure codes in `lexicon-core/src/session/model.rs`:

```rust
OperatorCancelledGracefully,
OperatorCancelledForcefully,
OperatorCancellationUncertain,
```

The persisted safe failure records cancellation kind, escalation, and sanitized process outcome. It never includes source arguments or secrets.

Add a typed CLI error:

```rust
CliError::Interrupted {
    kind: CancellationKind,
    unix_signal: Option<u8>,
    forced: bool,
}
```

The operator host uses the same `spawn_and_supervise` implementation after HANDOFF-03 ownership. Background initiation does not install a second runtime supervisor.

### FOREGROUND-02 â native tests

Create `lexicon-cli/tests/foreground_cancellation.rs` with helper child modes in a dedicated test fixture executable. Required native tests:

```text
unix_sigint_is_forwarded_to_runtime_group
unix_sigterm_is_forwarded_to_runtime_group
unix_uncooperative_tree_is_sigkilled_after_deadline
unix_descendant_cannot_survive_parent_reconciliation
windows_ctrl_break_is_forwarded_to_runtime_group
windows_uncooperative_tree_is_terminated_by_job_object
windows_descendant_cannot_escape_job_object
cancellation_keeps_lease_through_terminal_reconciliation
cancellation_records_graceful_failure_code
cancellation_records_forced_failure_code
cancellation_returns_shell_appropriate_status
wait_or_kill_error_never_reports_false_success
```

Tests send real signals/console events to the copied `lexicon` process and use bounded waits. The Windows suite runs on native Windows; Wine is not sufficient evidence for Job Object and console-control behavior.

## 10. Gate 5 â processing, checkpoints, and paired publication

Most code in this gate has substantial typed error handling, but `processing/transactions.rs`, `processing/context.rs`, and `publication/runtime_pair.rs` contain no focused test modules at the baseline. `status.md` lists production functions and behavioral phrases as if they were tests. This gate converts the claims into deterministic evidence and adds failure seams where otherwise unforceable branches exist.

### PROCESS-01 â transaction catalog admission

Add `#[cfg(test)] mod tests;` to `lexicon-core/src/processing/transactions.rs` and create `lexicon-core/src/processing/transactions/tests.rs`.

Build fixtures through production encoders/stores/recorders; do not hand-author âvalidâ JSON except in corruption tests. Required cases:

```text
processing_catalog_accepts_only_finalized_admitted_transactions
processing_catalog_orders_by_completion_then_identity
processing_catalog_rejects_duplicate_transaction_identity
processing_catalog_ignores_well_formed_live_staging_directory
processing_catalog_rejects_malformed_staging_name
processing_catalog_rejects_symlink_and_unexpected_file
processing_catalog_requires_succeeded_acquisition_session
processing_catalog_rejects_project_runtime_session_mismatch
processing_catalog_rejects_transaction_outside_session_time_bounds
processing_catalog_rejects_corrupt_transaction_metadata_or_body_hash
processing_catalog_rejects_missing_acquisition_session
processing_catalog_does_not_mutate_raw_data
```

Each test asserts the exact typed error variant, not only `is_err()` or display text. Snapshot raw-tree names and hashes before/after discovery to prove read-only behavior.

### PROCESS-01 â Core-owned SQLite transaction

Add test-only handler descriptors to `lexicon-core/src/processing/runner.rs` and direct context tests to `processing/context.rs`. Required cases:

```text
core_begins_transaction_before_source_handler
successful_handler_commits_database_once
handler_error_rolls_back_and_preserves_previous_database
handler_panic_is_caught_reconciled_and_rolls_back
source_commit_or_rollback_attempt_is_detected
commit_failure_never_reports_session_success
uncertain_commit_retains_uncertain_typed_outcome
durability_failure_after_commit_reports_partial_commit
sqlite_wal_journal_and_shm_sidecars_fail_closed
processing_context_exposes_read_only_admitted_catalog
processing_cannot_open_unadmitted_raw_transaction
processing_failure_preserves_previous_database_bytes
```

If the runner currently does not catch unwinding at the source-handler boundary, add:

```rust
let handler_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    (contract.process())(&mut context, source_arguments)
}));
```

Translate panic payloads to a fixed sanitized failure code; do not persist arbitrary panic text. Roll back while the Core-owned connection is still under supervision, finalize the session as failed, and then return a typed panic outcome.

Tests begin with an existing database containing a sentinel table and rows. A failing handler attempts schema and data changes. After failure, compare logical contents and, where stable, exact bytes; the sentinel database must remain usable and unchanged.

Add a database-operation failpoint seam around begin, handler boundary validation, commit, rollback, file sync, directory sync, and sidecar inspection. It is private and unavailable in release builds.

### CHECKPOINT-01 â checkpoint-backed durable progress

Add `lexicon-core/src/protocols/http/checkpoint/tests.rs` and declare it from the module under `#[cfg(test)]`.

Required cases:

```text
checkpoint_commit_requires_progress_published_transaction
checkpoint_commit_requires_completed_response
checkpoint_commit_rejects_transaction_from_other_session
checkpoint_commit_rejects_logical_key_mismatch
checkpoint_commit_rejects_attempt_identity_mismatch
checkpoint_commit_rejects_missing_or_corrupt_backing_transaction
checkpoint_commit_is_atomically_published_and_directory_synced
checkpoint_lookup_admits_current_session_checkpoint
checkpoint_lookup_finds_compatible_historical_session
checkpoint_lookup_rejects_incompatible_runtime_or_project
checkpoint_lookup_rejects_duplicate_or_corrupt_candidates
crash_after_response_before_checkpoint_replays_work
checkpoint_after_response_allows_resume_without_duplicate_commit
late_checkpoint_timestamp_is_rejected
```

Use the real response recorder and checkpoint encoder. The crash/resume cases run separate processes or a child fixture killed at a deterministic barrier. A source-owned WorkLedger fixture may demonstrate a source policy, but it is labeled as such and does not become a Core queue.

The checkpoint publisher must expose the same test-only durability events as HTTP transaction publication. If a checkpoint destination already exists, admit and compare its full typed identity before treating the operation as idempotent; a mismatched or corrupt existing file is an error.

### PUBLISH-01 â filesystem transaction seam

Introduce an internal filesystem operation trait in `lexicon-framework/src/publication/mod.rs`:

```rust
pub(crate) trait PublicationFileSystem {
    fn metadata(&self, path: &Path) -> std::io::Result<std::fs::Metadata>;
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;
    fn remove_file(&self, path: &Path) -> std::io::Result<()>;
    fn sync_file(&self, path: &Path) -> std::io::Result<()>;
    fn sync_directory(&self, path: &Path) -> std::io::Result<()>;
    fn sleep_before_retry(&self, delay: Duration);
}
```

Production delegates to the durable filesystem helpers. Tests use a scripted implementation that fails the exact Nth operation. Refactor `runtime_bundle_replacement.rs` and `runtime_pair.rs` to accept the trait internally while retaining a simple production entrypoint.

### PUBLISH-01 â paired runtime invariant

The pair state is:

```rust
pub struct StagedRuntimePair {
    acquisition: StagedRuntimeBundle,
    processing: StagedRuntimeBundle,
    compatibility: RuntimePairCompatibilityV1,
}
```

Before replacing either current runtime:

1. admit both staged bundles;
2. verify source, protocol, Core revision, contract versions, and pair generation agree;
3. sync both staged executables and manifests;
4. create recoverable backups of both current bundles;
5. replace acquisition;
6. replace processing;
7. sync both runtime directories;
8. re-admit the published pair;
9. only then delete backups and sync cleanup.

On any failure before step 8, restore both prior bundles and re-admit the restored pair. If rollback is incomplete, return a `RuntimePairRollbackIncomplete` error containing per-side typed outcomes and leave recovery artifacts intact. Never label a mixed-generation pair successful.

Required tests in `lexicon-framework/src/publication/runtime_pair.rs` or a dedicated integration target:

```text
paired_publication_rejects_incompatible_staged_pair
paired_publication_rejects_one_missing_staged_bundle
paired_publication_success_replaces_both_and_removes_backups
failure_before_first_replace_preserves_old_pair
failure_after_acquisition_replace_restores_old_pair
failure_after_processing_replace_restores_old_pair
failure_during_first_directory_sync_restores_old_pair
failure_during_second_directory_sync_restores_old_pair
readmission_failure_restores_old_pair
rollback_first_side_failure_is_reported_without_false_success
rollback_second_side_failure_is_reported_without_false_success
backup_cleanup_failure_reports_partial_commit_without_deleting_evidence
published_pair_uses_same_core_revision_and_generation
```

### PUBLISH-01 â Windows replacement behavior

Keep retry policy bounded by attempt count and elapsed deadline. Retry only the documented sharing/access errors. Do not retry malformed paths, permission policy failures, or incompatible bundles.

Add a native Windows integration test that:

1. holds the current executable open in a mode that denies replacement;
2. starts pair publication;
3. proves bounded retries occur;
4. releases the handle and proves both bundles publish; and
5. repeats while holding through the deadline, proving the old pair is restored and no false success is returned.

Record retry count and typed final outcome. A platform-neutral fake is useful unit evidence but does not replace this native test.

## 11. Gate 6 â real, pinned MZA release construction and installation

### Audited upstream facts

At MZA commit `d2c2406ed9f83d2de4c7a38fbf1ac3a568d1e410`:

- `Cargo.toml` defines only package `lexicon-release-build`; there is no `[lib]` target;
- `src/main.rs` hardcodes `<MZA manifest dir>/artifacts.toml`;
- `cargo-bundler-v0.1.0` sets `MZA_BUNDLE_INPUTS` to a TOML `bundle-spec.toml` path;
- the consumer `build.rs` must copy archives and generate `$OUT_DIR/mza_bundle_inputs.rs`;
- consumer source must use `include!(concat!(env!("OUT_DIR"), "/mza_bundle_inputs.rs"));`;
- the protocol embeds archive bytes but defines no install, upgrade, uninstall, PATH registration, or command-registration API;
- `make-artifact.sh` may interactively install `cargo-zigbuild` and invokes `cargo run` without `--locked`.

Those are immutable-source findings, not assumptions.

### MZA-01 â pin the selected source

Add MZA as a Git submodule at:

```text
automation/build_bundle_mza/mza
```

The superproject gitlink must point to an accepted MZA commit. Add `.gitmodules`:

```ini
[submodule "automation/build_bundle_mza/mza"]
	path = automation/build_bundle_mza/mza
	url = https://github.com/ssr2zvy/mza
```

Release CI must run:

```bash
git submodule update --init --recursive
test "$(git -C automation/build_bundle_mza/mza rev-parse HEAD)" = "<accepted-mza-sha>"
git -C automation/build_bundle_mza/mza diff --exit-code
```

Do not clone `main`, download a moving archive, or accept a tag without resolving and recording its commit.

### MZA-01 â required upstream changes before final integration

Open and merge an MZA change that provides all of the following at one immutable commit:

1. a `--config <absolute-path>` option instead of only hardcoded `artifacts.toml`;
2. a noninteractive mode that never installs tools or edits the host;
3. locked self-build execution;
4. workspace-aware lockfile discovery that verifies the lockfile Cargo will actually use;
5. an explicit target-triple mechanism that can represent and build the accepted Windows ARM64 target instead of synthesizing nonexistent `aarch64-pc-windows-gnu`;
6. no automatic `rustup target add` during the sealed release step;
7. a versioned installer-runtime library owned by MZA;
8. typed install/upgrade/uninstall entrypoints for Linux and Windows;
9. explicit command registration/removal and rollback behavior;
10. tests for install, upgrade, uninstall, PATH/command registration, interruption, and rollback;
11. a documented MSRV/toolchain and Protocol 1 compatibility version.

The minimum acceptable upstream public shape is conceptually:

```rust
pub struct EmbeddedArtifact {
    pub label: &'static str,
    pub archive: &'static [u8],
}

pub struct InstallerDefinition {
    pub product_name: &'static str,
    pub command_name: &'static str,
    pub artifacts: &'static [EmbeddedArtifact],
}

pub fn run_installer(
    definition: InstallerDefinition,
    arguments: impl IntoIterator<Item = std::ffi::OsString>,
) -> std::process::ExitCode;
```

These names are not authorized Lexicon code until the accepted MZA commit publishes its real names. Once it does, replace this conceptual block in `current.md` with an immutable link and the exact upstream imports/signatures before implementing MZA-02. No Lexicon-local substitute may be counted as conformance.

### MZA-01 â exact Lexicon artifact configuration

Replace `automation/build_bundle_mza/mza_artifacts.toml` with the configuration accepted by the pinned MZA revision. The current MZA grammar can represent the first three settled targets:

```toml
[[artifact]]
label = "lexicon_cli"
crate = "../../lexicon-cli"
output_path = "../../artifacts/"
type = "main"
name = "lexicon"

[[target]]
label = "linux-x86_64-musl"
os = "linux"
arch = "x86_64"
environment = "musl"

[[target]]
label = "linux-aarch64-musl"
os = "linux"
arch = "aarch64"
environment = "musl"

[[target]]
label = "windows-x86_64-gnu"
os = "windows"
arch = "x86_64"
environment = "gnu"

[[bundle]]
label = "lexicon_bundle"
crate = "../../lexicon-bundle"
output_path = "../../artifacts/"
type = "main"
name = "lexicon-installer"
protocol = "cargo-bundler-v0.1.0"
inputs = ["lexicon_cli"]
```

Do not add `arch = "aarch64", environment = "gnu"` for Windows at the audited MZA revision: its source synthesizes `aarch64-pc-windows-gnu`, which is not the accepted Rust Windows ARM64 target. The required MZA change must introduce an exact target-triple field or an accepted `gnullvm` mapping and prove the complete Linux-host cross-build. After that change lands, append the exact upstream-documented stanza for the accepted target, expected to resolve to `aarch64-pc-windows-gnullvm` unless the toolchain decision changes. Replace this paragraph with literal accepted syntax before Gate 6 can pass.

Resolve relative paths against the actual configuration location accepted by the new MZA `--config` implementation. Assert the resolved canonical paths and all four exact triples in an MZA dry-run test.

Do not add `lexicon-framework` as a standalone artifact. It is a library linked into the one installed `lexicon` executable.

### MZA-01 â literal Protocol 1 input adapter

Replace `lexicon-bundle/Cargo.toml` with the existing workspace package fields plus:

```toml
[build-dependencies]
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```

The final MZA installer library dependency is added only after its exact package/revision exists:

```toml
# Replace placeholders from the accepted upstream commit; do not commit this literally.
mza_installer = { package = "<actual-package>", git = "https://github.com/ssr2zvy/mza", rev = "<accepted-mza-sha>" }
```

Replace `lexicon-bundle/build.rs` with a parser/generator equivalent to:

```rust
use serde::Deserialize;
use std::{env, fs, path::{Path, PathBuf}};

const PROTOCOL: &str = "cargo-bundler-v0.1.0";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleSpec {
    protocol: String,
    bundle: String,
    target: String,
    inputs: Vec<BundleInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleInput {
    label: String,
    archive: PathBuf,
}

fn main() {
    println!("cargo:rerun-if-env-changed=MZA_BUNDLE_INPUTS");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let generated = out.join("mza_bundle_inputs.rs");

    let Some(spec_path) = env::var_os("MZA_BUNDLE_INPUTS").map(PathBuf::from) else {
        fs::write(
            &generated,
            "pub static MZA_BUNDLE_INPUTS: &[MzaBundleInput] = &[];\n",
        ).expect("write standalone MZA input adapter");
        return;
    };

    println!("cargo:rerun-if-changed={}", spec_path.display());
    let text = fs::read_to_string(&spec_path).expect("read MZA bundle spec");
    let spec: BundleSpec = toml::from_str(&text).expect("parse MZA bundle spec");
    assert_eq!(spec.protocol, PROTOCOL, "unexpected MZA protocol");
    assert_eq!(spec.bundle, "lexicon_bundle", "unexpected bundle identity");
    assert!(!spec.target.trim().is_empty(), "empty MZA target");
    assert_eq!(spec.inputs.len(), 1, "Lexicon bundle requires one CLI input");
    assert_eq!(spec.inputs[0].label, "lexicon_cli", "unexpected MZA input");

    let archive = &spec.inputs[0].archive;
    assert!(archive.is_absolute(), "MZA input archive must be absolute");
    let file_name = archive.file_name().expect("archive file name");
    let copied = out.join(file_name);
    fs::copy(archive, &copied).expect("copy MZA input archive into OUT_DIR");

    let literal = format!(
        "pub static MZA_BUNDLE_INPUTS: &[MzaBundleInput] = &[\n    MzaBundleInput {{ label: \"lexicon_cli\", archive: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{}\")) }},\n];\n",
        rust_string_literal_component(file_name),
    );
    fs::write(generated, literal).expect("write generated MZA inputs");
}

fn rust_string_literal_component(name: &std::ffi::OsStr) -> String {
    let value = name.to_str().expect("MZA archive name must be UTF-8");
    assert!(value.bytes().all(|byte| byte.is_ascii_alphanumeric()
        || matches!(byte, b'-' | b'_' | b'.')), "unsafe archive name");
    value.to_owned()
}
```

Define `MzaBundleInput` in `lexicon-bundle/src/main.rs` before the generated include, exactly as the selected MZA protocol specifies:

```rust
pub struct MzaBundleInput {
    pub label: &'static str,
    pub archive: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/mza_bundle_inputs.rs"));
```

The empty standalone case is allowed only so ordinary workspace checks compile. A release/bundle integration test must fail unless exactly one nonempty `lexicon_cli` archive is embedded. The present `include!(env!("MZA_BUNDLE_INPUTS"))` and empty Rust comment stub are deleted.

### MZA-02 â invoke the real installer

After MZA publishes its accepted API, `lexicon-bundle/src/main.rs` must contain only:

1. the protocol-defined embedded-input type/include;
2. construction of the exact upstream installer definition; and
3. a call to the exact upstream installer entrypoint.

Delete the current `println!`-only main. Delete `automation/build_bundle_install/install.sh` and every Lexicon implementation of install, uninstall, upgrade, PATH editing, or command registration. Test helpers may inspect results but must not perform the installation on MZAâs behalf.

### RELEASE-01 â locked, noninteractive release build

Delete `automation/build_bundle_install/update_lock_file.sh` and every call to it. Lockfiles are reviewed inputs, never regenerated during release construction.

Move the remaining build entrypoint:

```text
automation/build_bundle_install/build_bundle_install.sh
â automation/build_bundle_mza/build_release.sh
```

Replace it with this noninteractive wrapper:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MZA_DIR="$ROOT_DIR/automation/build_bundle_mza/mza"
CONFIG="$ROOT_DIR/automation/build_bundle_mza/mza_artifacts.toml"

test -f "$ROOT_DIR/Cargo.lock"
test "$(git -C "$MZA_DIR" rev-parse HEAD)" = "<accepted-mza-sha>"
git -C "$ROOT_DIR" diff --exit-code -- Cargo.lock

cargo run --release --locked \
  --manifest-path "$MZA_DIR/Cargo.toml" \
  -- --config "$CONFIG"

git -C "$ROOT_DIR" diff --exit-code -- Cargo.lock
```

Replace `<accepted-mza-sha>` only after MZA-01 exists. Pre-provision exact Rust, Zig, and cargo-zigbuild versions in the release image. The release job may not prompt, run `cargo install`, or mutate the toolchain.

The repository is one Cargo workspace, so the root `Cargo.lock` is the release lockfile Cargo actually uses for workspace members. Remove misleading member lockfiles unless a tested standalone build intentionally excludes that crate from the workspace. MZA must discover the Cargo workspace root through `cargo metadata --locked` and check that lockfile, not demand an ignored `Cargo.lock` beside every member manifest.

### RELEASE-02 â delete obsolete local installer orchestration

Delete:

```text
automation/build_bundle_install/get_build_variables.sh
automation/build_bundle_install/install.sh
automation/build_bundle_install/update_lock_file.sh
automation/build_bundle_install/local_build_variables.sh (if generated)
```

The release pipeline no longer parses MZA TOML with `awk`, extracts archives with `find | head`, uninstalls the developerâs current command, or runs a Lexicon-owned install wrapper. MZA builds the target installer; disposable target-native tests execute that installer.

Update `README.md` to name the pinned submodule, exact MZA SHA, `mza_artifacts.toml`, and `automation/build_bundle_mza/build_release.sh`. Remove instructions to clone latest MZA, copy to singular `artifact.toml`, or drive `make-artifact.sh` interactively.

Update `instructions.md` and `containerization/test-container/{README.md,entrypoint.sh}` to remove the deleted scripts from required paths and examples. The test containerâs release mode invokes `automation/build_bundle_mza/build_release.sh` and never pipes menu answers.

Update `containerization/lexicon-container/{Containerfile,README.md}` only after the real MZA installer syntax is accepted. It may execute the produced target installer noninteractively, but it must use the exact upstream install argument and archive/binary name. It must not reconstruct installation using shell copies.

Keep `lexicon-install.toml` only if the accepted MZA installer API consumes it as typed product metadata. Validate it in the bundle build and pass its embedded content to MZA. Otherwise migrate its fields to the exact upstream type/configuration and delete the unconsumed file; an unconsumed manifest is not evidence.

### RELEASE-01 â release verification

For every supported target, CI must:

1. inspect the archive structure and hashes;
2. run the installer in a disposable native VM of the target OS/architecture;
3. prove `lexicon --version` resolves without a repository checkout;
4. run installed/no-Git scaffold and build tests;
5. upgrade from the prior supported release;
6. uninstall and prove command/path registration is removed;
7. force failures at each installer publication step and prove rollback;
8. record MZA SHA, Lexicon SHA, toolchain identities, artifact hashes, and results.

Cross-compilation proves compilation, not target execution. Windows and ARM64 target behavior require native or faithful hardware/VM execution evidence appropriate to the guarantee.

## 12. Gate 7 â official-release supply-chain policy

### SUPPLY-01 â policy placement

Create `workspace/specs/release-policy.md`. This is an official-release policy, not a retroactive claim that Contract V1 sandboxes trusted source runtimes.

The file must contain these normative sections:

```markdown
# Lexicon official-release supply-chain policy

## Scope

This policy controls official Lexicon build inputs, build-time execution,
artifact handling, verification, and provenance. It does not claim that a
container makes dependencies trustworthy, and it does not change Contract V1's
trusted-native source execution model.

## Immutable inputs

An official release identifies exact Lexicon and MZA commits; an exact Rust
toolchain; exact Zig and cargo-zigbuild versions and hashes; the reviewed
Cargo.lock; a vendored Cargo source tree; container/VM image digests; release
configuration; and target triples. Release-time resolution or lockfile mutation
is forbidden.

## Build-time code inventory

Before approval, inventory every package containing build.rs, every procedural
macro, every native compiler/linker dependency, and every tool invoked by those
components. Record package name, version, registry/source checksum, purpose,
review status, and reviewer. A changed inventory blocks release approval.

## Dependency review

Review newly introduced and changed dependency source, build scripts,
procedural macros, unsafe code concentration, licenses, advisories, and
maintainer/source changes. A clean advisory scan is evidence, not proof of
benign behavior.

## Isolated source build

Fetch and vendor in a separate networked preparation step. Verify checksums,
then build in a fresh rootless container with network disabled, project and
vendor sources mounted read-only, and only target/output/temp directories
writable. Run Cargo with --locked --offline. Do not mount developer credentials,
SSH agents, Docker/Podman sockets, host home directories, or signing keys.

## Artifact quarantine

Treat produced binaries and build logs as untrusted until structural inspection,
malware scanning, native functional tests, reproducibility comparison where
supported, and provenance verification finish. Signing occurs only after those
gates and in a separate minimal environment.

## Evidence

Publish source and artifact hashes, SBOM, dependency/build-time-code inventory,
toolchain identities, exact commands, test results, MZA run record, builder image
digests, and signed provenance tied to the exact commit.

## Exceptions

An exception names the invariant, reason, owner, expiry, compensating controls,
and approving reviewers. It cannot be described as conformance with the waived
property.
```

### SUPPLY-01 â repository automation

Add scripts or a small reviewed tool that produces:

```text
verification/dependencies/cargo-metadata.json
verification/dependencies/cargo-tree.txt
verification/dependencies/build-scripts.json
verification/dependencies/proc-macros.json
verification/dependencies/licenses.json
verification/dependencies/advisories.json
verification/sbom.cdx.json
```

The inventory must derive from `cargo metadata --locked --offline` and the vendored graph, not only grep. It identifies custom build targets, the package manifest that declares them, and the resolved source/checksum.

Add `vendor/` only if the project deliberately commits it; otherwise publish a content-addressed vendor archive and its hash as release input. In either case generate `.cargo/config.toml` for offline source replacement in the release workspace and test that `cargo build --locked --offline` succeeds with network disabled.

The hardened build invocation must have behavior equivalent to:

```bash
podman run --rm --read-only --network none \
  --userns keep-id --cap-drop all --security-opt no-new-privileges \
  --mount type=bind,src="$SOURCE_SNAPSHOT",dst=/src,ro=true \
  --mount type=bind,src="$VENDOR_SNAPSHOT",dst=/vendor,ro=true \
  --mount type=volume,dst=/target \
  --mount type=tmpfs,dst=/tmp \
  --workdir /src \
  "$PINNED_BUILDER_IMAGE" \
  cargo build --workspace --release --locked --offline --target-dir /target
```

Use explicit validated absolute paths, not unresolved broad environment variables. This isolation contains many build-time effects; it does not establish that the produced output is benign.

## 13. Gate 7 â durable CI and platform evidence

### CI-01 â `.github/workflows/conformance.yml`

Add a workflow triggered for pull requests and pushes to `main`. Pin actions by full commit SHA in the committed file. The logical job graph is:

```yaml
name: conformance

on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read

jobs:
  linux-container:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          submodules: recursive
      - name: Build test container
        run: podman build --pull=never -f containerization/test-container/Containerfile -t lexicon-test:${{ github.sha }} .
      - name: Check exact workspace
        run: podman run --rm -v "$PWD:/lexicon:ro" --mount type=volume,destination=/target --workdir /lexicon lexicon-test:${{ github.sha }} bash -lc 'CARGO_TARGET_DIR=/target cargo check --workspace --locked'
      - name: Test exact workspace
        run: podman run --rm -v "$PWD:/lexicon:ro" --mount type=volume,destination=/target --workdir /lexicon lexicon-test:${{ github.sha }} bash -lc 'CARGO_TARGET_DIR=/target cargo test --workspace --locked --quiet'
      - name: Real Linux process suites
        run: podman run --rm -v "$PWD:/lexicon:ro" --mount type=volume,destination=/target --workdir /lexicon lexicon-test:${{ github.sha }} bash -lc 'CARGO_TARGET_DIR=/target cargo test --locked --test background_handoff --test foreground_cancellation -- --nocapture'

  windows-native:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          submodules: recursive
      - uses: dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772
        with:
          toolchain: 1.98.0
      - run: cargo check --workspace --locked
      - run: cargo test --workspace --locked --quiet
      - run: cargo test --locked --test background_handoff --test foreground_cancellation -- --nocapture
      - run: cargo test --locked --test windows_runtime_replacement -- --nocapture

  matrix-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
      - run: podman build -f containerization/test-container/Containerfile -t lexicon-test:${{ github.sha }} .
      - run: podman run --rm -v "$PWD:/lexicon:ro" --mount type=volume,destination=/target --workdir /lexicon lexicon-test:${{ github.sha }} bash -lc 'CARGO_TARGET_DIR=/target cargo run --locked -p lexicon-conformance-matrix -- check workspace/specs/conformance.toml'
```

The pinned checkout commit above corresponds to `actions/checkout` v7.0.1 at preparation time; the pinned Rust action commit is the reviewed `dtolnay/rust-toolchain` state used to install Rust 1.98.0. Review their source and compatibility before merge rather than treating recency as trust. Do not replace either SHA with a moving tag.

`instructions.md` ordinarily requires Cargo only in the Linux test container. This master milestone explicitly requires native Windows Cargo execution for Windows process, locking, and replacement guarantees because a Linux container cannot exercise them. That narrow native-Windows exception is part of this `current.md`; host-local Cargo remains forbidden for ordinary Linux development validation.

The current test container installs moving `cargo-zigbuild` state and downloads during image construction. For official verification, pin its version and checksum or start from a reviewed image digest. `--pull=never` requires that exact base image to have been pre-resolved by the workflow; adjust the preparation job without weakening the digest pin.

### CI-02 â verification manifest

Add `verification/README.md` documenting evidence format and retention. Each CI job emits a machine-readable `verification-manifest.json` with:

```json
{
  "schema_version": 1,
  "repository": "https://github.com/ssr2zvy/lexicon",
  "commit": "<40-character sha>",
  "dirty": false,
  "os": "<exact OS/version>",
  "architecture": "<architecture>",
  "rustc": "<verbose version>",
  "cargo": "<verbose version>",
  "mza_commit": "<40-character sha>",
  "commands": [
    {
      "argv": ["cargo", "test", "--workspace", "--locked", "--quiet"],
      "exit_code": 0,
      "started_at": "<RFC3339>",
      "finished_at": "<RFC3339>",
      "stdout_sha256": "<sha256>",
      "stderr_sha256": "<sha256>"
    }
  ],
  "tests": {
    "passed": 0,
    "failed": 0,
    "ignored": 0,
    "outer_workflow_skipped": 0
  },
  "artifacts": []
}
```

Upload the manifest and raw UTF-8 logs as GitHub workflow artifacts. Include the workflow run ID and artifact URL in the completion report/status matrix. Preserve native Windows logs as UTF-8. A number in `current.md` without the attached exact-SHA manifest is not evidence.

Add a final required `conformance` job that depends on Linux, Windows, matrix, and release jobs and fails if any required job is absent, skipped, neutral, or cancelled. Branch protection must require this job for `main`.

## 14. Immutable baseline evidence anchors

These anchors identify the audited defect locations at commit `0494cd751114312028aced50cc62ef80a0fd3157`. Line numbers are baseline anchors; implementation work may move them.

| Finding | Immutable baseline evidence |
|---|---|
| Completion report changes no implementation | [`current.md`](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/current.md) and the commit diff |
| No durable green commit status was returned | [combined status for `0494cd7`](https://api.github.com/repos/ssr2zvy/lexicon/commits/0494cd751114312028aced50cc62ef80a0fd3157/status) |
| CLI package has no explicit `lexicon` binary | [`lexicon-cli/Cargo.toml`](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-cli/Cargo.toml) |
| CLI maps every dispatch failure to exit 1 | [`lexicon-cli/src/main.rs` lines 6â11](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-cli/src/main.rs#L6-L11) |
| Scaffold omits acquisition state directory | [`lexicon-framework/src/lib.rs` lines 923â934](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/src/lib.rs#L923-L934) |
| Scaffold omits both root session-status files | [`lexicon-framework/src/lib.rs` lines 949â1021](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/src/lib.rs#L949-L1021) |
| Scaffold writes are not file-synced | [`lexicon-framework/src/lib.rs` lines 1023â1035](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/src/lib.rs#L1023-L1035) |
| Source-name validation is only path-oriented | [`lexicon-framework/src/lib.rs` lines 1864â1890](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/src/lib.rs#L1864-L1890) |
| Build-time Core identity has Git/env fallback but no source-tree proof | [`lexicon-framework/build.rs` lines 4â68](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/build.rs#L4-L68) |
| Runtime embeds only Core revision | [`lexicon-framework/src/lib.rs` lines 2223â2259](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/src/lib.rs#L2223-L2259) |
| Managed dependency admission checks package name, not exact source/revision | [`lexicon-framework/src/lib.rs` lines 2993â3031](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/src/lib.rs#L2993-L3031) |
| Artifact selector omits release-path/profile containment proof | [`lexicon-framework/src/lib.rs` lines 1743â1805](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/src/lib.rs#L1743-L1805) |
| Source state accessor is optional and legacy env remains | [`lexicon-core/src/protocols/http/context.rs` lines 95â128](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-core/src/protocols/http/context.rs#L95-L128) |
| Request/response redaction use inconsistent mandatory sets | [`transaction/recorder.rs` lines 468â506](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-core/src/protocols/http/transaction/recorder.rs#L468-L506) |
| Admission rejects custom structural redaction | [`transaction/metadata.rs` lines 905â930](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-core/src/protocols/http/transaction/metadata.rs#L905-L930) |
| Redirect origin check uses `domain()` | [`protocols/http/context.rs` lines 1152â1208](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-core/src/protocols/http/context.rs#L1152-L1208) |
| Operator writes ACK before lease acquisition | [`data/background.rs` lines 379â428](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/src/data/background.rs#L379-L428) |
| Initiator releases after ACK and returns | [`data/background.rs` lines 229â343](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/src/data/background.rs#L229-L343) |
| Handoff primitive drops the physical lease | [`session/coordinator.rs` lines 77â89](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/src/session/coordinator.rs#L77-L89) |
| Repository test explicitly documents the race | [`session/coordinator.rs` lines 543â571](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/src/session/coordinator.rs#L543-L571) |
| Foreground waits without cancellation | [`data/foreground.rs` lines 120â156](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-framework/src/data/foreground.rs#L120-L156) |
| Status calls continuous handoff implemented/tested | [`workspace/specs/status.md` lines 483â488](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/workspace/specs/status.md#L483-L488) |
| Bundle silently creates empty generated-code stub | [`lexicon-bundle/build.rs` lines 10â21](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-bundle/build.rs#L10-L21) |
| Bundle invokes no MZA installer | [`lexicon-bundle/src/main.rs` lines 1â8](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/lexicon-bundle/src/main.rs#L1-L8) |
| Release automation regenerates lockfiles | [`update_lock_file.sh` lines 1â6](https://github.com/ssr2zvy/lexicon/blob/0494cd751114312028aced50cc62ef80a0fd3157/automation/build_bundle_install/update_lock_file.sh#L1-L6) |
| Audited MZA is binary-only | [`MZA Cargo.toml`](https://github.com/ssr2zvy/mza/blob/d2c2406ed9f83d2de4c7a38fbf1ac3a568d1e410/Cargo.toml) |
| Audited MZA protocol only defines input embedding | [`cargo-bundler-v0.1.0.md`](https://github.com/ssr2zvy/mza/blob/d2c2406ed9f83d2de4c7a38fbf1ac3a568d1e410/docs/protocols/cargo-bundler-v0.1.0.md) |

Absence-of-tests findings are based on repository search at the baseline: `processing/transactions.rs`, `processing/context.rs`, and `publication/runtime_pair.rs` have no focused behavioral test modules, while `status.md` cites phrases or production symbols as proof.

## 15. Required test targets and names

The implementation may group unit tests differently, but these stable integration targets and exact invariant-bearing names must exist so the matrix is mechanically checkable.

| Target | Required focus |
|---|---|
| `lexicon-core` unit tests | sensitivity, transaction admission, processing, checkpoints, handoff model/store |
| `lexicon-core/tests/contract_ui.rs` | actual compile-fail contract cases only |
| `lexicon-core/tests/managed_runner_contract.rs` | real generated-runner compile/link boundary |
| `lexicon-cli/tests/installed_core_identity.rs` | copied executable outside checkout with Git unavailable |
| `lexicon-cli/tests/background_handoff.rs` | actual `__operator-host`, contention, crash, fencing |
| `lexicon-cli/tests/foreground_cancellation.rs` | native process-tree signals/events and reconciliation |
| `lexicon-framework` publication tests | paired replacement, failpoints, rollback |
| `lexicon-framework/tests/windows_runtime_replacement.rs` | native Windows sharing/retry behavior |
| `lexicon-bundle` tests | exact bundle-spec parsing and nonempty embedded CLI archive |
| release VM tests | MZA-owned install, upgrade, uninstall, rollback |

Test names in Â§Â§6â11 are normative identifiers. If a name changes, update `conformance.toml` and this document in the same commit with the same assertion. Names that promise more than the assertions prove are defects.

## 16. Clean verification sequence

Run from a fresh checkout of the exact candidate commit with initialized pinned submodules and no uncommitted changes.

### Source identity

```bash
git rev-parse HEAD
git status --porcelain=v1
git submodule status --recursive
```

Expected: the reported 40-character candidate SHA, empty status, and the accepted MZA SHA with no `-`, `+`, or dirty suffix.

### Linux Cargo validation

Build and run only through the repositoryâs pinned test container:

```bash
podman build -f containerization/test-container/Containerfile -t lexicon-test:<sha> .
podman run --rm \
  --mount type=bind,src="$PWD",dst=/lexicon,ro=true \
  --mount type=volume,dst=/target \
  --workdir /lexicon \
  lexicon-test:<sha> \
  bash -lc 'CARGO_TARGET_DIR=/target cargo check --workspace --locked'
podman run --rm \
  --mount type=bind,src="$PWD",dst=/lexicon,ro=true \
  --mount type=volume,dst=/target \
  --workdir /lexicon \
  lexicon-test:<sha> \
  bash -lc 'CARGO_TARGET_DIR=/target cargo test --workspace --locked --quiet'
```

Run focused HTTP, installed/no-Git, handoff, cancellation, processing, checkpoint, publication, matrix, and release-adapter suites with `--nocapture` logs retained.

### Native Windows validation

On a clean native Windows runner at the same commit:

```powershell
cargo check --workspace --locked
cargo test --workspace --locked --quiet
cargo test --locked --test background_handoff -- --nocapture
cargo test --locked --test foreground_cancellation -- --nocapture
cargo test --locked --test windows_runtime_replacement -- --nocapture
```

Capture UTF-8 logs and the exact OS/architecture/toolchain manifest.

### Release validation

After the MZA external prerequisite is satisfied:

```bash
bash automation/build_bundle_mza/build_release.sh
git diff --exit-code -- Cargo.lock
```

Then execute every produced installer in the target-native disposable environment and run the install/upgrade/uninstall suite.

### Supply-chain validation

Build once from verified vendored inputs in the network-disabled rootless builder, produce the SBOM/inventory/provenance, scan the quarantined outputs, and compare independent builds where reproducibility is claimed.

## 17. Completion report format

When every gate passes, replace this active ledger with a completion report containing exactly:

```markdown
# Lexicon Contract V1 / Specification V1 conformance report

## Declared tree
* Repository: ...
* Commit: <40-character SHA>
* Git tree: <tree SHA>
* Dirty state: false
* MZA repository and commit: ...

## Implemented diff ledger
| Diff ID | Commit(s) | Production symbols | Exact tests | Result |

## Verification environments
| Environment | OS/arch | Toolchains | Commands | Pass/fail/ignored/outer-skip | Durable run/artifact |

## Release artifacts
| Target | Installer hash | Native install | Upgrade | Uninstall | MZA run record |

## Supply-chain evidence
* Builder image digest: ...
* Vendor hash: ...
* SBOM hash: ...
* Build-time code inventory hash: ...
* Provenance/signature: ...

## Contract exceptions
None.

## Final verdict
Conformant only if every required row above is complete and linked.
```

Do not reuse test totals from an earlier worktree. Derive counts from attached exact-SHA logs. Report ignored tests and outer-workflow skips separately; neither is a pass.

## 18. Master completion criteria

This milestone is complete only when all of the following are true:

1. `current.md` and `status.md` contain no unsupported completion claim.
2. The private-handler, invocation-transport, and MZA generated-input contradictions are corrected without weakening the contract.
3. The installed binary is named `lexicon`.
4. Project and source names use the one typed safe grammar before interpolation or path use.
5. Source creation emits the exact tree, durable state directory, and two valid initial status files.
6. Project/source publication is staged, file-synced, atomically renamed, and parent-synced with failure tests.
7. Generated workspaces use the exact embedded Core URL and commit.
8. The framework build fails closed if its compiled Core tree does not match that identity.
9. Manifest and resolved Cargo dependency admission reject every alternate Core source/revision.
10. The copied installed CLI works outside the checkout with Git unavailable and builds both generated runners locked.
11. Legacy schema/path entrypoints are absent from the default conformant build.
12. Build selection admits exactly one current-host release executable under the isolated target.
13. Every mandatory/explicit HTTP secret is structurally redacted before persistence and remains admissible.
14. Cross-origin redirects, including IP literals, cannot forward mandatory or explicit secrets.
15. HTTP tests prove exact bytes, hashes, attempts, redirect/retry lineage, failure/truncation records, and return-after-sync ordering.
16. Background handoff has durable reservation, authentication, epoch, expiry, revocation, and fencing.
17. Every ordinary acquisition/reconciliation path refuses an active successor reservation.
18. `Ready` and `Owned` are distinct; the initiator returns only after exact durable ownership.
19. Real multiprocess acquisition and processing tests execute `lexicon __operator-host` under forced contention and crash interleavings.
20. Foreground and operator-host supervision forward cancellation, apply a bounded grace period, kill the full tree if required, reap, retain the lease, and reconcile terminal state.
21. Unix and Windows native process semantics have their own passing evidence.
22. Processing admits only compatible finalized raw transactions and its Core-owned SQLite transaction rolls back failures/panics without damaging the previous database.
23. Checkpoints are backed by matching completed durable transactions and prove crash/resume behavior.
24. Runtime-pair publication proves compatibility, all failure steps, rollback, and native Windows replacement behavior.
25. MZA is pinned by immutable Git identity and accepts an explicit config noninteractively and locked.
26. The bundle consumes the real Protocol 1 spec, embeds the nonempty CLI archive, and calls a real upstream-owned installer API.
27. All supported target installers pass native install, upgrade, uninstall, and rollback tests.
28. Official release inputs are inventoried, vendored, offline-built in a hardened rootless environment, quarantined, inspected, and accompanied by SBOM and provenance.
29. `conformance.toml` maps every requirement to real test identifiers and the checker passes.
30. GitHub required checks provide green Linux, native Windows, matrix, supply-chain, and release evidence for the exact declared commit with no required skip.
31. `cargo check --workspace --locked` and `cargo test --workspace --locked --quiet` pass under the required environments.
32. The exact committed tree is clean and is the tree named in the completion report.

If any criterion is unavailable because the external MZA prerequisite has not landed, the correct status is âblocked, not conformant.â

## 19. Explicit non-solutions

The following do not close this milestone:

- adding another pre-ownership acknowledgement;
- polling until any process owns the lease;
- a fake executor that writes handoff files;
- a zero or version-derived Core revision;
- checking only that a dependency package is named `lexicon-core`;
- an in-process CLI dispatch test described as installed/no-Git;
- an HTTP test that checks only `is_err()` or a filename;
- a production function listed as a test;
- regenerating a release lockfile immediately before `--locked`;
- an empty generated MZA input stub described as integration;
- Lexicon reimplementing installation while claiming MZA owns it;
- a container described as proof that dependency output is trustworthy;
- Linux-container evidence labeled native Windows;
- a test that returns early and is counted as a platform pass;
- temporary external logs without an exact-SHA durable record;
- a passing test total copied from a different worktree.

## 20. Committee disposition

The baseline is architecturally recognizable as the intended Lexicon design, but it is not contract-complete. Several recent changes are useful partial work: managed runners, typed session models, source-owned work-state examples, bundle admission, handoff tokens, and extensive HTTP scaffolding. The defect is not that those components are worthless; it is that readiness, names, comments, and test totals were allowed to stand in for the stronger invariants the contract actually names.

This master milestone is intentionally demanding. The disciplined way to execute it is through the ordered gates above, with each diff independently reviewable and every status claim demoted until its evidence exists. The repository may carry all gates on one coordinated branch, but it receives one conformance verdict only at the end.

### Preparation note

This document is a source-audited implementation prescription. Its simple literal blocks are direct edits; its new state-machine and platform APIs are normative target shapes whose implementations must be compiled, formatted, tested, and adjusted only where the real compiler or accepted upstream MZA API requires. No code in this document is represented as already implemented or compile-verified at the baseline.
