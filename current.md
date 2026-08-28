Implementation report: checkpoint identity, admission, and historical lookup closure

Files changed
- /home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/checkpoint/error.rs
- /home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/checkpoint/model.rs
- /home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/checkpoint/mod.rs
- /home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/context.rs
- /home/runner/work/lexicon/lexicon/lexicon-core/src/protocols/http/mod.rs
- /home/runner/work/lexicon/lexicon/current.md

Implemented state
- complete checkpoint runtime identity representation: checkpoints persist source, protocol, operation, and source_contract_version, and admission reconstructs and compares OwnedRuntimeIdentity
- typed checkpoint admission inputs: checkpoint admission uses typed ProjectIdentity, OwnedRuntimeIdentity, and optional SessionIdentity inputs
- committed checkpoint identity accessors: committed checkpoints retain typed project, runtime, and session identities and expose read-only accessors
- exact checkpoint path-containment behavior: admission derives the canonical sessions/<session>/checkpoints/<sha256>.json path beneath the trusted operation root and requires exact equality
- managed symlink/path error behavior: checkpoint and lookup flows continue to use typed managed-path validation backed by symlink_metadata semantics
- typed session-record failure behavior: checkpoint lookup, commit-context validation, and historical lookup now retain typed operation-root, session-store-open, and current-session-load failures through source()
- active-context validation for checkpoint lookup: lookup validates managed protocol root, operation root, raw root, current session directory, current session record, running acquisition state, runtime identity, session identity, and owned supervisor lease before cross-session search
- session-directory enumeration behavior: session enumeration rejects symlinks, non-UTF-8 names, invalid session identifiers, and unexpected files deterministically
- native session-name behavior: session names are admitted through SessionIdentity constructors and are never lossily decoded in checkpoint lookup paths
- exact referenced-transaction resolution: checkpoint admission resolves referenced transactions only from exact parsed <timestamp>-<transaction-id> finalized directory names
- ambiguous referenced-transaction behavior: missing or multiple exact transaction matches remain typed admission failures
- recorded transaction session/timestamp provenance: admitted recorded transactions retain typed session provenance and creation timestamps and checkpoint logic relies on admitted transaction state
- complete attempt-identity validation: checkpoints and admitted transactions both use the checked HttpAttemptIdentity constructor for full attempt invariants
- checkpoint timestamp ordering: admission and commit enforce checkpoint timestamps after transaction completion and session start, and before terminal session finish when present
- Linux checkpoint publication behavior: Linux publication continues to use atomic no-replace renameat2 semantics from a fully synced temporary file plus checkpoint-directory sync
- macOS checkpoint publication behavior: macOS publication continues to use atomic no-replace renamex_np semantics from a fully synced temporary file plus checkpoint-directory sync
- Windows checkpoint publication behavior: Windows publication continues to use native wide-path MoveFileExW no-replace semantics without a preflight exists check, plus directory flush
- post-publication partial-commit behavior: post-publication directory-sync failures still surface as typed partial commits after publication succeeds
- committed checkpoint ownership in partial errors: partial-commit errors retain the committed checkpoint and expose checkpoint, path, key, session, transaction, and attempt accessors
- pre-publication encoded-size enforcement: checkpoint encoding still rejects serialized documents that exceed MAX_HTTP_CHECKPOINT_DOCUMENT_BYTES before publication
- checkpoint path revalidation behavior: commit revalidates the checkpoint directory and final target after directory creation and again after managed-context revalidation immediately before publication
- canonical registry-key behavior: the transaction registry remains keyed by canonical HttpLogicalRequestKey values
- managed historical-transaction validation: historical lookup requires the same managed running HTTP acquisition context validation path before scanning raw transactions
- historical transaction source/runtime/session filtering: historical lookup strictly admits transactions, validates their session records, and requires project, runtime, acquisition operation, and session identity agreement before selection
- typed latest-header errors: latest_response_header keeps typed invalid-header-name, non-UTF-8, and redacted-header failures
- explicit redaction representation: stored HTTP headers continue to use explicit Utf8, Base64, and Redacted variants rather than magic marker strings
- managed-redacted header lookup behavior: historical header lookup returns a typed HeaderRedacted error when a matching persisted header exists but is redacted
- repeated-header selection behavior: latest_response_header explicitly returns the first matching persisted response header in recorded transport order
- deterministic all-candidate checkpoint lookup: has_checkpoint validates every matching checkpoint candidate across sorted session identities and returns typed candidate-admission failures instead of silently ignoring corrupt matches
- checkpoint public/internal API boundary: internal checkpoint filename, hash, and disk-admission helpers are no longer re-exported from the public HTTP API surface
- capability-set result: HttpCapabilitySet::empty() remains unchanged and ClientCertificateV1 is still not advertised here
- confirmation that processing, SQLite, background supervision, and build behavior were not added: no processing-path, SQLite, background-supervision, or build-pipeline behavior was introduced in this milestone
- confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, runtime execution, HTTP execution, workspace validation, or bundle/install pipeline were run: no cargo test, cargo check, cargo build, cargo fmt, cargo clippy, cargo metadata, rustc, Lexicon CLI commands, generated runners, HTTP servers, real or test HTTP requests, workspace validation, or bundle/install automation were executed
