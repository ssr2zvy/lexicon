Implementation report: session integration closure (source-only)

Files changed
- lexicon-core/Cargo.toml
- lexicon-core/src/lib.rs
- lexicon-core/src/processing/context.rs
- lexicon-core/src/processing/runner.rs
- lexicon-core/src/protocols/http/runner.rs
- lexicon-core/src/session/binding.rs
- lexicon-core/src/session/context.rs
- lexicon-core/src/session/error.rs
- lexicon-core/src/session/lease.rs
- lexicon-core/src/session/mod.rs
- lexicon-core/src/session/model.rs
- lexicon-core/src/session/store.rs
- lexicon-framework/src/lib.rs
- lexicon-framework/src/session/coordinator.rs
- lexicon-framework/src/session/error.rs

Repository defects corrected
- Parent/child lease contention removed from child normal path by removing child lease acquisition and moving both runtime runners to the single bind_runtime_session → enter_running lifecycle path.
- Duplicate child lifecycle route reduced to one authoritative route (binding module type-state path is now used by both runners).
- Prepared-session creation now writes both detailed record and root status; summary write failure after record write returns SessionStoreError::PartialCommit.
- Source scaffolding no longer creates placeholder session_status.json files in operation workspaces.
- Ordinary-path panic extraction removed from runtime runner flow by keeping RunningRuntimeSession in runner-owned scope.
- Arbitrary source error text is no longer persisted by runner failure paths; source failures use Core-authored safe failure values.
- Launch failure ordering fixed in PreparedSessionLaunch::fail_launch by transitioning before releasing owner value.
- Placeholder session identity removed from coordinator preparation input; SessionStore now generates session identity exactly once.

Final supervisor lease ownership model
- PreparedSessionLaunch retains the supervisor lease.
- Child lifecycle no longer acquires or owns that lease value.
- Child admission requires confirmation that an external supervisor owner is active.

Child lease acquisition confirmation
- Child runners no longer call SessionStore::acquire_lease.
- bind_runtime_session checks lease state via SessionStore::inspect_lease_state / inspect_session_lease.

Supervisor lease inspection behavior
- Added SessionLeaseState { Available, Owned }.
- Added inspect_session_lease(path) and SessionStore::inspect_lease_state(session).
- Binding returns RuntimeSessionBindingError::SupervisorLeaseUnavailable when no active owner exists.

Final lease lifetime
- Lease remains held by the supervisor-side owner object.
- Launch failure transition executes while owner value is still alive; release occurs only when owning value is dropped.

Final authoritative child type-state sequence
- bind_runtime_session(...) → BoundRuntimeSession::enter_running() → RunningRuntimeSession::{complete, fail_source, fail_runtime}.

Duplicate lifecycle types/APIs removed
- Production runner usage of RunningSession and RunningSession::from_parts removed.
- Child lease-taking transition helpers are no longer used by managed runner paths.

Prepared-session publication behavior
- Detailed record publication and root summary publication are both part of successful create_prepared.
- Partial publication returns PartialCommit with reconciliation path preserved.

Initial session_status scaffold behavior
- source create now scaffolds sessions/ directories only; no invalid placeholder session_status.json is created.

Root-summary inconsistency behavior
- Existing rebuild_status_from_record recovery path remains available and compatible with PartialCommit handling.

Safe durable failure representation
- Added SafeSessionFailure and SessionFailureCode.
- Failed transitions now persist structured failure data (kind, stable code, optional bounded diagnostic).

Confirmation about arbitrary source error persistence
- HTTP/processing runner failure persistence no longer stores source Display output.

Final HTTP runner session flow
- parse/admit invocation → decode runtime context → open store → bind_runtime_session → enter_running → construct context from SessionDataPaths → invoke handler → complete/fail via RunningRuntimeSession.

Final processing runner session flow
- parse/admit invocation → decode runtime context → open store → bind_runtime_session → enter_running → construct context from SessionDataPaths → invoke handler → complete/fail via RunningRuntimeSession.

Ordinary-path panic removal
- Removed expect("running session must be present") extraction path.

Final acquisition context representation
- HttpAcquisitionContext now binds to SessionDataPaths plus session identity in managed path.
- Compatibility source_directory accessor now returns protocol-root semantics for managed construction.

Final processing context representation
- ProcessingContext now stores SessionDataPaths and SessionIdentity; no running-session ownership hidden in context.

Project/source/protocol/operation path binding behavior
- Runtime context serialization now uses explicit native-path encoding documents and decoding validates platform encoding compatibility.
- RuntimeContextPaths validation still enforces operation/session/data path relationships and traversal checks.

Configured sources-directory behavior
- Source scaffolding changes preserve existing directory structure and configured source roots; only placeholder status-file creation was removed.

Native path round-trip representation
- Unix: encoded as {"encoding":"unix-bytes-base64","value":"..."}.
- Windows: encoded as {"encoding":"windows-utf16","value":[...]}.

Runtime-context encoding/decoding error separation
- Added RuntimeContextEncodingError and RuntimeContextDecodingError and integrated them into RuntimeContextError.

Coordinator typed-error behavior
- SessionCoordinationError now carries typed RuntimeContextError for context encoding and invalid operation-root derivation paths.

Launch-failure transition ordering
- fail_launch now transitions to Failed before dropping the owning lease-bearing value.

Placeholder session identity removal
- Removed placeholder_session_identity and removed session identity from NewSessionRecord input.

Legacy session APIs removed/intentionally retained
- RunningSession-based production route removed from managed runners.
- Legacy LEXICON_SOURCE_DIRECTORY path remains quarantined for run_http_source compatibility.

Probe independence confirmation
- Probe mode logic was not changed and remains session-independent.

Excluded implementations confirmation
- No process launching, HTTP transport execution changes, raw recording, checkpoints, or SQLite features were implemented.

Command-execution constraints confirmation
- No cargo test/check/build/fmt/clippy/metadata/rustc commands were run.
- No generated runners were executed.
- No workspace validation or bundle/install pipeline commands were run.
