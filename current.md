Implementation report: durable session model and supervisor lease foundation

Files created

lexicon-core/src/session/binding.rs
  New child-side session binding module.

Files changed

lexicon-core/src/session/mod.rs
  Added `pub mod binding` and exported all new public items.
lexicon-core/src/session/context.rs
  Added `SessionDataPaths`.
lexicon-core/src/session/store.rs
  Added `pub fn operation_root(&self) -> &SessionOperationRoot` accessor.

All other session module files (error.rs, lease.rs, model.rs, store.rs, transition.rs,
coordinator.rs, selection.rs) were already present from a prior session and remain
unchanged except for the single accessor addition in store.rs.

Core session module structure

lexicon-core/src/session/
├── mod.rs          — public re-exports
├── binding.rs      — child-side bind/enter/complete/fail type states (new)
├── context.rs      — RuntimeContextPaths, SessionDataPaths, encode/decode context
├── error.rs        — typed error hierarchy
├── lease.rs        — SessionLease (OS advisory lock)
├── model.rs        — all value types and serde documents
├── store.rs        — SessionStore, SessionOperationRoot, PreparedSession, RunningSession
└── transition.rs   — legal-transition validator (crate-internal)

Framework session module structure

lexicon-framework/src/session/
├── mod.rs          — public re-exports
├── coordinator.rs  — SessionCoordinator, PreparedSessionLaunch
├── selection.rs    — assess_current_session, validate_run_selection, validate_resume_selection
└── error.rs        — SessionCoordinationError

(reconciliation logic lives inside coordinator.rs and selection.rs;
equivalent structure was acceptable per current.md)

Session schema

SESSION_SCHEMA_VERSION: u32 = 1 — distinct from all other version constants.

Serialized document fields: schema_version, project, runtime, session, operation,
execution_mode, supervision_mode, state, revision, created_at, updated_at,
started_at, finished_at, failure.

Detailed session record

SessionRecordV1 — opaque struct with private fields and read-only accessors.

Fields: schema_version, project (ProjectInvocationIdentity), runtime (OwnedRuntimeIdentity),
session (SessionInvocationIdentity), operation (SessionOperation), execution_mode,
supervision_mode, state (SessionState), revision (u64), created_at, updated_at,
started_at (Option), finished_at (Option), failure (Option<SessionFailureV1>).

Root summary schema

SessionStatusV1 — opaque struct.

Fields: schema_version, project, runtime, operation, current_session (Option),
current_state (Option<SessionState>), revision, updated_at.

File: session_status.json at the operation root.

Stable state identifiers

prepared / running / succeeded / failed / abandoned (serde rename_all = "snake_case")

Legal transition table

new → Prepared           (create_prepared)
Prepared → Running       (enter_running / promote_to_running)
Prepared → Failed        (direct transition)
Prepared → Abandoned     (abandon_current_failure)
Running → Succeeded      (complete / complete_succeeded)
Running → Failed         (fail_source / fail_runtime / complete_failed)
Running → Abandoned      (direct transition)
Failed → Abandoned       (abandon_current_failure)
Running(stale) → Failed(StaleOwnership)  (reconcile_stale_current_session)

All other transitions are rejected with SessionTransitionError::InvalidTransition.

Revision behavior

Starts at 0 on creation (Prepared). Incremented exactly once per durable transition.
Every update API requires the caller's expected revision; mismatches return
SessionStoreError::RevisionConflict.

Operation-root validation

SessionOperationRoot::new requires an absolute path.
SessionOperationRoot exposes sessions_directory, session_directory, session_record_path,
lease_path, and status_path derived from the validated root.

Session path derivation

<operation-root>/session_status.json
<operation-root>/sessions/<session-id>/session.json
<operation-root>/sessions/<session-id>/session.lock

SessionDataPaths is derived from a validated RuntimeContextPaths; fields:
protocol_root, raw_data_directory, processed_data_directory, operation_root,
session_directory.

Atomic persistence implementation

write_atomic in store.rs:
1. serialize complete document
2. write unique NamedTempFile (tempfile crate) in destination directory
3. flush + sync_all on the temp file
4. atomically replace destination (NamedTempFile::persist)
5. best-effort directory sync_all

No in-place truncation; no fixed shared temp name; temp file cleaned on failure by
tempfile crate; final newline boundary from serde_json pretty-print.

Session size limits

MAX_SESSION_RECORD_BYTES and MAX_SESSION_STATUS_BYTES are defined in model.rs:
MAX_SESSION_RECORD_BYTES = 128 * 1024
MAX_SESSION_STATUS_BYTES = 64 * 1024

Strict decoding behavior

serde deny_unknown_fields on all document structs. Unknown schema versions return
UnknownSchemaVersion. Unknown state/operation/protocol identifiers return UnknownField.
Zero revisions, failure/state inconsistency, operation/runtime disagreement, and
timestamp problems return InvalidInvariant or InvalidRevision. JSON syntax errors
return JsonSyntax; structural errors return StructuralDocument.

Session lease implementation and platform behavior

SessionLease (lease.rs) wraps an open File with an OS advisory lock.
Linux/macOS: flock(LOCK_EX | LOCK_NB) — non-blocking exclusive.
Windows: LockFileEx(LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY).
Other: returns Io(Unsupported).
Released on Drop via explicit flock(LOCK_UN) / UnlockFileEx.
Lock file is not deleted on unlock. PID written for diagnostics only.

Prepared-session type state

PreparedSession — returned by SessionStore::create_prepared.
Proves: session directory exists, initial record is durable, root summary updated,
state is Prepared. No public constructor.

Bound/running session type states

BoundRuntimeSession<'store> — returned by bind_runtime_session.
Proves: envelope validated against durable Prepared record.
enter_running(self, lease) → RunningRuntimeSession.

RunningRuntimeSession<'store> — returned by BoundRuntimeSession::enter_running.
Proves: transition to Running was durably persisted.

Child transition APIs

bind_runtime_session(&store, &envelope) → Result<BoundRuntimeSession, RuntimeSessionBindingError>
BoundRuntimeSession::enter_running(self, &lease) → Result<RunningRuntimeSession, SessionStoreError>
RunningRuntimeSession::complete(self, &lease) → Result<SessionRecordV1, SessionStoreError>
RunningRuntimeSession::fail_source(self, &lease, &dyn Error) → Result<SessionRecordV1, SessionStoreError>
RunningRuntimeSession::fail_runtime(self, &lease, &dyn Error) → Result<SessionRecordV1, SessionStoreError>

All methods are consuming (take self). Lease validated by comparing path against
expected path derived from store's operation root and session identity. Error
messages sanitized via Display and truncated to MAX_FAILURE_SUMMARY_BYTES.

Framework coordinator APIs

SessionCoordinator — owns project, runtime, operation, SessionStore, RuntimeContextPaths.
PreparedSessionLaunch — returned by coordinator operations; holds record, lease,
context_document (JSON for child env).

Run preparation

prepare_run(supervision) → PreparedSessionLaunch:
1. load status; 2. assess current session; 3. reject live/unresolved-failed;
4. create_prepared with new session ID; 5. acquire lease; 6. build RuntimeContextPaths;
7. encode context document; 8. return PreparedSessionLaunch with held lease.

Resume preparation

prepare_resume(supervision) → PreparedSessionLaunch:
Requires Acquisition operation; requires prior Failed or StaleReconciled session;
rejects live sessions; creates new Prepared record with ExecutionMode::Resume.

Abandonment behavior

abandon_current_failure() — acquires no new lease; calls store.transition(ToAbandoned)
on Failed or StaleReconciled current session. Preserves session directory and history.
Does not delete raw or processed data.

Stale reconciliation behavior

reconcile_stale_current_session (SessionStore and SessionCoordinator):
1. Load record; 2. return None if terminal or no session;
3. attempt non-blocking lease acquisition; 4. AlreadyOwned → live owner → return None;
5. success → transition to Failed(StaleOwnership); 6. update root summary; 7. release lease.
Does not use wall-clock time to decide staleness.

Session data-path representation

SessionDataPaths — constructed from RuntimeContextPaths; exposes:
protocol_root, raw_data_directory, processed_data_directory, operation_root,
session_directory. All paths are derived from validated roots; no raw arithmetic
exposed to callers.

Typed error hierarchy

SessionEncodingError — Serialization
SessionDecodingError — JsonSyntax, UnknownSchemaVersion, UnknownField, InvalidInvariant,
  IdentityMismatch, InvalidTimestamp, InvalidRevision, StructuralDocument
SessionTransitionError — InvalidTransition, ImmutableFieldMutation, RevisionRollback,
  TerminalStateReached
SessionLeaseError — AlreadyOwned, Io
SessionStoreError — Encoding, Decoding, InvalidTransition, Io, DirectoryCreation,
  AtomicPersistence, RootSummaryUpdate, PartialCommit, MissingSession, CorruptSession,
  RevisionConflict, LeaseRequired, StaleOwnershipReconciliationFailed
RuntimeContextError — MissingEnvironmentVariable, InvalidUtf8, Decoding, IdentityMismatch,
  PathMismatch, RelativeProjectRoot, PathTraversal, OperationRootDisagreement,
  ProtocolRootDisagreement, SessionDirectoryDisagreement
CoreRunnerSessionError — ContextDecode, StoreOpen, LeaseAcquisition, TransitionToRunning,
  TerminalPersistence
RuntimeSessionBindingError — StoreLoad, IdentityMismatch, SessionNotPrepared,
  ExecutionModeMismatch, SupervisionModeMismatch
SessionCoordinationError — Store, Lease, LiveSessionAlreadyActive, UnresolvedFailure,
  ResumeUnavailable, ResumeNotSupportedForOperation, AbandonmentUnavailable,
  ContextEncoding, InvalidOperationRoot

All errors implement Display and std::error::Error with source() for nested failures.
No plain String returned inside Core or framework session engines.

Generated runners and build integration unchanged

Managed runner templates, the immutable Core Git revision pin, source scaffolding,
source build, runtime probe, runtime verification, staging, paired publication,
invocation JSON, argv transport, handler signatures, acquisition/processing admission,
CLI commands, MZA, Protocol 1, and installer behavior are all unchanged.

No tests or validation commands were run

Validation was deferred by instruction. No cargo test, cargo check, cargo build,
cargo fmt, or cargo clippy was executed. Validation will occur during the final
project-wide validation phase.