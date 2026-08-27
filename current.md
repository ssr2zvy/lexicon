Implementation report: foreground reconciliation closure

Files changed
- /home/runner/work/lexicon/lexicon/lexicon-framework/src/data/error.rs
- /home/runner/work/lexicon/lexicon/lexicon-framework/src/data/foreground.rs
- /home/runner/work/lexicon/lexicon/lexicon-framework/src/data/project.rs
- /home/runner/work/lexicon/lexicon/lexicon-framework/src/data/runtime.rs
- /home/runner/work/lexicon/lexicon/lexicon-framework/src/data/session.rs
- /home/runner/work/lexicon/lexicon/lexicon-framework/src/lib.rs

Contract supervision boundary
- Added a single authoritative foreground terminal reconciliation entrypoint: `reconcile_terminal_execution(...)` in foreground supervision.

Exact child/lease ownership invariant
- Foreground reconciliation is performed while `RunningForegroundExecution` still owns both child handle and session lease.
- Lease release occurs only after authoritative reconciliation returns success or a final typed error.

Nonzero failed-session summary handling
- Nonzero exit + durable `Failed` now runs typed root-summary reconciliation and propagates failures.
- Removed discarded root-summary result behavior.

Signaled-session load-error handling
- Signaled/unknown abnormal termination now always performs terminal record load first and preserves typed load/decode/identity errors.
- Removed `if let Ok(record)` fallthrough behavior.

Post-transition record usage
- For Prepared/Running abnormal paths, reconciliation now uses the `SessionRecordV1` returned by transition-to-failed for identity and summary checks.

Typed root-summary validation error
- Added `RootSummaryValidationError` with typed variants for missing summary, load/decode mismatch cases, schema mismatch, state mismatch, revision mismatch, and identity mismatches.

Typed root-summary reconciliation error
- Added `RootSummaryReconciliationError` with typed variants:
  - `Validation(...)`
  - `Rebuild(...)`
  - `ValidationAfterRebuild(...)`

Root-summary rebuild and mandatory revalidation
- Added authoritative helper `validate_or_rebuild_root_summary(store, record)`.
- Required sequence is enforced: validate -> rebuild on invalid -> revalidate -> success only if revalidation passes.

Detailed-record identity mismatch representation
- Replaced string-field mismatch payloads with typed `TerminalSessionIdentityMismatch` variants retaining typed expected/actual values.

Zero-exit reconciliation
- Zero exit + `Succeeded`: identity validated, summary validated/rebuilt, post-rebuild validation enforced, then success outcome.
- Zero exit + `Failed`: identity validated, summary validated/rebuilt, typed `ChildFailed` returned.
- Zero exit + `Prepared|Running`: transitioned to failed with `ZeroExitWithoutCompletion`, then post-transition identity + summary reconciliation, then typed `ZeroExitSessionIncomplete`.

Nonzero-exit reconciliation
- Nonzero exit + `Failed`: identity validated, summary validated/rebuilt, typed `ChildFailed` returned.
- Nonzero exit + `Prepared|Running`: transitioned to failed with `NonzeroExitWithoutFailureRecord`, then post-transition identity + summary reconciliation, then typed abnormal termination result.

Signaled reconciliation
- Signaled abnormal termination now follows strict load -> identity -> state inspection -> transition (if needed) -> summary reconciliation.

Unknown abnormal reconciliation
- Unknown abnormal termination path uses the same strict reconciliation sequence as signaled termination.

Wait-recovery state machine
- Replaced one-shot wait recovery with explicit staged flow (`WaitRecoveryState`) covering wait failure, try_wait probe, termination request, observed termination, reap, and ownership-uncertain state.

try_wait behavior
- On non-interrupted wait error, recovery first probes `try_wait()` before kill.
- `try_wait` is used again after kill/reap failures to refine child-state certainty.

kill behavior
- Kill failure is preserved and does not imply liveness by itself.
- After kill failure, `try_wait` probing is performed before deciding ownership certainty.

reap behavior
- Reap wait retries interrupted waits and preserves nonrecoverable reap errors.
- Ownership is not released as reconciled-success when child-state certainty cannot be established.

interrupted-wait behavior
- Ordinary wait and recovery reap loops both retry on `Interrupted`.

ownership-uncertain behavior
- Added `ForegroundDataExecutionError::ChildOwnershipUncertain(ChildOwnershipUncertainError)`.
- Preserves original wait error, try_wait error, kill error, reap error, optional session-load error, and optional session-reconciliation error.
- Classified as fatal supervision failure requiring next invocation stale-ownership reconciliation.

wait-recovery session-load behavior
- Added session-load failure retention to `WaitRecoveryFailure` (`session_load_error`).
- Added `session_reconciliation_error` and `final_state` capture when termination is confirmed.

Confirmation that no session load, transition, summary validation, or summary rebuild error is discarded
- Reconciliation now propagates typed failures for all required durable-state operations.

Unit-error helper removal
- Removed `validate_or_rebuild_summary_if_needed(...) -> Result<(), ()>` and replaced call sites with typed reconciliation helper.

`let _ = reconciliation` removal
- Removed best-effort discarded reconciliation patterns on correctness-critical paths.

Integrity `unreachable!` removal
- Removed the integrity adaptation `unreachable!` assumption from foreground pre-launch checks.

Executable-integrity typed error
- Added dedicated `ExecutableIntegrityError` and changed pre-launch integrity recheck to return this typed error directly.

Typed shared project-root discovery
- Refactored shared `find_project_root(...)` and descendant traversal to return `ProjectRootDiscoveryError` (typed).

Typed shared project-configuration loading
- Refactored `load_project_config(...)` to return typed `ProjectConfigLoadError`.
- Updated foreground/source-create/source-build boundaries to convert to user-facing text only at command boundaries.

Free-form project/configuration fallback strings removed
- Removed `ProjectConfigurationError::Other(String)` usage and replaced with typed config-load error mapping.

Final foreground success guarantee
- Foreground success is now produced only after typed identity agreement and typed summary reconciliation.

Confirmation that the supervisor lease remains held through all normal terminal reconciliation
- Reconciliation is executed while `RunningForegroundExecution` still owns prepared lease + child ownership.

Confirmation that no HTTP transport, raw recording, checkpoints, SQLite, background host, signal forwarding, or lexicon build was implemented
- No work was done in HTTP transport execution, raw transaction recording, checkpoints, SQLite, background host/signal forwarding, or lexicon build paths.

Existing test source adjusted only for API alignment, if applicable
- Existing in-file tests were only minimally aligned with changed typed helper signatures where needed.

Confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, generated-runtime execution, workspace validation, or bundle/install pipeline were run
- Confirmed: no cargo tests/check/build/fmt/clippy/metadata, no CLI data execution, no generated-runtime execution, and no workspace/bundle/install automation commands were run.
