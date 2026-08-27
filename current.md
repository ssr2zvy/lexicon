Foreground supervision closure and lifecycle correctness — completion report

Files created

None.

Files changed

lexicon-framework/src/data/foreground.rs
lexicon-framework/src/data/outcome.rs

Contract responsibility boundary preserved

The contract assigns foreground ownership to the supervising Lexicon process. The supervisor selects, creates, or resumes a session; acquires the session lease; applies --abandon-past-fail; launches the source runtime; observes process exit and signals; and reconciles abnormal termination. The linked Core inside the source runtime is responsible for validating the invocation, entering Running state, recording ordinary source failure, and recording normal completion. This boundary is preserved.

Foreground owner types

PreparedForegroundExecution: owns PreparedSessionLaunch (and therefore the supervisor lease), DataOperation, project name, and source name. Neither Clone nor Copy. Fields are private.

RunningForegroundExecution: owns both the live std::process::Child handle and the PreparedSessionLaunch (and therefore the supervisor lease), plus operation, project name, and source name. Neither Clone nor Copy. Fields are private.

Prepared-to-running ownership transition

PreparedForegroundExecution is consumed into RunningForegroundExecution only after a successful spawn call. The session record is not altered by the parent process merely because spawn succeeded; the linked Core child is responsible for the Prepared → Running transition.

Exact supervisor lease lifetime

The supervisor lease is held from session preparation through invocation construction, encoding, executable integrity recheck, spawn, the wait loop, terminal reconciliation, final outcome construction, and is released only when the owning value is dropped.

Confirmation that reconciliation occurs before lease release

Terminal reconciliation is performed inside wait_and_reconcile, which holds RunningForegroundExecution (and therefore the PreparedSessionLaunch lease) throughout. The lease is released only after reconciliation completes or produces its final structured error.

Launcher seam

ForegroundRuntimeLauncher is a pub(crate) trait with a single spawn method accepting the exact admitted executable path, argument slice, runtime-context environment value, and working directory. The production implementation is ProcessCommandLauncher using std::process::Command with inherited stdio, no shell, no PATH lookup, the protocol root as working directory, LEXICON_RUNTIME_CONTEXT_V1 set to the context document, and LEXICON_SOURCE_DIRECTORY removed.

Exact production command behavior

Executable: exact admitted path. Arguments: exact encoded invocation argv. No shell. No PATH lookup. Working directory: HTTP protocol root. stdin/stdout/stderr: inherited. LEXICON_RUNTIME_CONTEXT_V1: set to context document. LEXICON_SOURCE_DIRECTORY: removed.

Invocation-construction failure behavior

If RuntimeInvocationEnvelopeV1 construction fails, fail_prepared_execution is called with SessionFailureCode::InvocationConstructionFailed and the typed ForegroundPreparationError::InvocationConstruction cause. The session is transitioned to Failed before returning. The nested ForegroundInvocationConstructionError is preserved.

Invocation-encoding failure behavior

If encode_runtime_invocation fails, fail_prepared_execution is called with SessionFailureCode::InvocationEncodingFailed and the typed ForegroundPreparationError::InvocationEncoding cause. The session is transitioned to Failed before returning. The nested RuntimeInvocationTransportEncodingError is preserved.

Integrity-failure behavior

If recheck_executable_integrity_typed detects a change or I/O error, fail_prepared_execution is called with SessionFailureCode::ExecutableIntegrityFailed and the typed ForegroundPreparationError cause. The session is transitioned to Failed. Executable contents are not included in diagnostics.

Combined preparation/persistence error behavior

If a post-preparation failure transition itself fails to persist, ForegroundDataExecutionError::PreparationFailureAndPersistenceFailure is returned with both the preparation error and the persistence SessionCoordinationError preserved as typed fields. Neither error is discarded or collapsed to a String.

Spawn-failure behavior

If the launcher spawn call fails, fail_prepared_execution is called with SessionFailureCode::LaunchFailed and ForegroundPreparationError::ProcessSpawn. The session is transitioned to Failed before returning.

Interrupted-wait behavior

std::io::ErrorKind::Interrupted errors from child.wait() cause the wait to be retried without releasing ownership.

Non-interrupted wait-failure recovery

Any other wait error triggers handle_wait_error: retains the child handle and lease; attempts Child::kill(); attempts a reap wait(); inspects durable session state and reconciles a nonterminal session to Failed; returns ForegroundDataExecutionError::ProcessWaitRecovery containing the typed WaitRecoveryFailure with the original wait error, any kill error, any reap error, and any session reconciliation error.

Child termination and reap behavior

After the wait loop exits successfully, the child is known to have terminated. RunningForegroundExecution is held through all reconciliation steps and dropped only after the final outcome or error is ready.

Wait-recovery failure behavior

If kill or reap fails, errors are captured in WaitRecoveryFailure and returned through ForegroundDataExecutionError::ProcessWaitRecovery. All errors are available as typed fields.

Detailed session identity validation

After child termination, load_and_validate_terminal_session loads the session record and verifies agreement with the prepared record for: project identity, runtime identity, session identity, operation, execution mode, and supervision mode. A typed SessionIdentityDisagreement error is returned on mismatch. The mismatched record is not overwritten.

Root-summary validation

validate_root_summary_against_record checks the root session_status.json for agreement on: schema version, project, runtime, operation, current session identity, current session state, and revision.

Root-summary rebuild behavior

If validate_root_summary_against_record fails, rebuild_status_from_record is called while the supervisor lease is still held. The summary is reloaded and re-validated. If rebuild or re-validation fails, RootSummaryReconciliationFailed is returned with the detail string and optional rebuild error.

Zero-exit reconciliation

Exit code 0 with Succeeded: validates root summary; attempts rebuild if stale; returns ForegroundDataOutcome only after both agree.
Exit code 0 with Failed: preserves the failed record; returns ChildFailed.
Exit code 0 with Prepared or Running: transitions to Failed with SessionFailureCode::ZeroExitWithoutCompletion while holding the lease; returns ZeroExitSessionIncomplete or AbnormalTerminationPersistence if persistence fails.
Exit code 0 with Abandoned: returns ExitSessionDisagreement without mutation.

Nonzero-exit reconciliation

Nonzero exit with Failed: validates or rebuilds root summary; returns ChildFailed with typed failure kind, failure code, exit code, source, operation, and session identity.
Nonzero exit with Prepared or Running: transitions to Failed with SessionFailureCode::NonzeroExitWithoutFailureRecord; returns AbnormalTermination or AbnormalExitPersistence if persistence fails.
Nonzero exit with Succeeded: returns ExitSessionDisagreement without mutation.
Nonzero exit with Abandoned: returns ExitSessionDisagreement without mutation.

Signaled termination reconciliation

reconcile_signal accepts the full ObservedChildTermination value. Succeeded or Failed: the existing terminal record is preserved; root summary is validated or rebuilt. Abandoned: returns a typed ExitSessionDisagreement without mutating the record. Prepared or Running: transitions to Failed with SessionFailureCode::AbnormalTermination; if persistence fails, returns AbnormalTerminationPersistence with the typed error; on success, validates or rebuilds root summary.

Unknown abnormal termination reconciliation

ObservedChildTermination::UnknownAbnormalTermination is routed to reconcile_signal with that value. The same logic as signaled termination applies. No signal number is invented.

Exit/session disagreement behavior

ForegroundDataExecutionError::ExitSessionDisagreement carries the typed ObservedChildTermination and the typed SessionState. No free-form detail String is used.

Filesystem metadata and type validation

require_directory uses symlink_metadata so symlinks are rejected even when they point to directories. Distinct typed errors are returned for: missing path (MissingPath), symlink (SymlinkNotPermitted), regular file or other non-directory (NotADirectory), and metadata I/O failure (MetadataIo). validate_sources_root_containment uses symlink_metadata for the same reasons.

Typed project-discovery errors

ProjectDiscoveryError::CurrentDirectory wraps std::io::Error. ProjectDiscoveryError::FindRoot carries the String message from the internal find_project_root helper.

Typed project-configuration errors

ProjectConfigurationError::Read, TomlDecode, Schema, Identity, and Other replace the prior ProjectConfiguration(String) variant. The load_project_config internal helper still returns a String; this is mapped to ProjectConfigurationError::Other.

Typed runtime-layout errors

RuntimeProjectLayoutError with variants SourcesRoot, SourceIdentity, NotADirectory, MissingPath, SymlinkNotPermitted, MetadataIo, and PathContainment.

Typed invocation errors

ForegroundInvocationConstructionError (InvalidProjectIdentity, InvalidSessionIdentity, EnvelopeConstruction) wraps the Core-owned error types. ForegroundDataExecutionError::InvocationConstruction(ForegroundInvocationConstructionError) and InvocationEncoding(RuntimeInvocationTransportEncodingError) replace the prior String variants.

Typed child failure kind and code behavior

SessionFailureKind and SessionFailureCode are retained as typed values in ForegroundDataExecutionError::ChildFailed. Display output uses their stable identifier() accessors, not Debug formatting.

Free-form disagreement strings removed

ExitSessionDisagreement carries termination: ObservedChildTermination and durable_state: SessionState instead of a detail: String.

Final ForegroundDataOutcome guarantee

ForegroundDataOutcome may be returned only when: the exact admitted executable was launched; the child exited with code zero; the detailed session record is Succeeded; the detailed record identities match the prepared invocation; the root summary identifies the same session; the root summary state and revision agree; no reconciliation error remains; and the supervisor retained its lease through all checks. This guarantee is documented on the type.

CLI diagnostic behavior

On success, the CLI prints: operation name, source name, and session id. On failure, the CLI renders the typed ForegroundDataExecutionError once at the CLI boundary via its Display impl.

Confirmation of non-printing

Source arguments, invocation-envelope JSON, runtime-context JSON, child environment, and arbitrary source error messages are not printed or persisted.

Confirmation of explicit exclusions

No HTTP transport, raw transaction recording, checkpoints, SQLite processing behavior, background host, or lexicon build was implemented in this milestone.

Existing test source adjusted

No existing test source was structurally broken by these changes. No API-incompatible changes were made to previously compiled test code.

Confirmation of no prohibited commands

No cargo test, cargo check, cargo build, cargo fmt, cargo clippy, cargo metadata, rustc, CLI data command execution, generated-runtime execution, workspace validation, or bundle/install pipeline commands were run.
