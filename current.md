Completed milestone: make background supervision transfer continuous and failure-atomic
Exact commit tested
Local uncommitted worktree against branch `continuous-background-supervision-transfer` based on commit `8512a2f` on `main`, containerized verification via podman machine ssh -> podman exec lexicon-local-test (image `lexicon-local-test-image`). Logs written to `$env:TEMP\lexicon-verify-logs\cargo-{check,test}.txt`.
Operating system and architecture
* Linux: `Linux x86_64` (podman container `lexicon-local-test` on `ammachine` WSL VM)
* Windows: `Microsoft Windows 11 x86_64-pc-windows-msvc`
Verification result
* `cargo check --workspace`: passed (exit 0).
* `cargo test --workspace --quiet`: passed (exit 0). Batches in order:
  * lexicon-cli:                                     30 passed, 0 failed, 0 ignored
  * lexicon-core:                                   263 passed, 0 failed, 0 ignored
  * lexicon-core-tests (trybuild UI suite):           1 passed (meta-test), 0 failed; 11 ui compile-fail tests pass
  * lexicon-framework:                             147 passed, 0 failed, 0 ignored (up from 144; +3 new background supervision transfer tests)
  * doctests:                                         0 / 0 / 1 ignored (pre-existing placeholder)
  * Total automated tests:                           441 passed, 0 failed.
Selected continuous-ownership mechanism
* Single-use unguessable handoff token (`HandoffIntentDocumentV1`) written to session directory while the initiator holds the `session.lock` lease.
* `OperatorHostInvocationV1` carries `handoff_token` to the spawned `__operator-host`.
* Operator host verifies `handoff_intent.json` against its invocation token, writes `handoff_ack.json` (`HandoffAcknowledgementDocumentV1` with `session_id`, `handoff_token`, and its PID), and waits on `session.lock`.
* Initiator verifies `handoff_ack.json` matching the session, token, and spawned child PID. Upon verification, initiator yields `session.lock` directly to the waiting operator host, which acquires it without an unowned gap.
How failed operator-host processes are terminated and reaped
* On acknowledgement timeout, acknowledgement mismatch, or early child exit: initiator retains lease authority, kills the spawned child (`child.kill()`), reaps it (`child.wait()`), and reconciles the prepared session to `Failed` via `prepared.fail_launch`.
Test names establishing each handoff invariant
* `successful_handoff_returns_outcome_once_lease_is_owned` (`lexicon-framework/src/data/background.rs:606`): verifies complete successful handoff with acknowledgement and continuous lease transfer.
* `operator_host_exiting_before_ownership_is_a_typed_error` (`lexicon-framework/src/data/background.rs:641`): verifies that host exit before acknowledgement produces `OperatorHostExitedBeforeOwnership` and cleans up.
* `ownership_timeout_is_a_typed_error` (`lexicon-framework/src/data/background.rs:661`): verifies timeout kills/reaps host, transitions session to `Failed`, and returns `OperatorHostOwnershipTimeout`.
* `re_exec_spawn_failure_is_a_typed_error` (`lexicon-framework/src/data/background.rs:681`): verifies spawn failure produces `OperatorHostReExec` and reconciles session.
* `operator_host_rejects_missing_or_mismatched_handoff_token` (`lexicon-framework/src/data/background.rs:761`): verifies unauthorized operator host invocation without matching handoff intent is rejected with `OperatorHostUnauthorizedHandoff`.
* `mismatched_acknowledgement_token_fails_handoff` (`lexicon-framework/src/data/background.rs:801`): verifies corrupted/mismatched acknowledgement token causes handoff rejection with `OperatorHostAcknowledgementMismatch`.
* `processing_background_handoff_succeeds` (`lexicon-framework/src/data/background.rs:835`): verifies processing operations share the exact same background handoff invariant.
* `operator_host_rejects_a_session_that_is_no_longer_prepared` (`lexicon-framework/src/data/background.rs:708`): verifies operator host rejects sessions that are no longer `Prepared`.
Confirmations
* No session-ownership gap exists during background handoff.
* Acknowledgement identifies the expected operator host by token and PID.
* Unrelated ownership cannot produce false success.
* Every handoff failure durably reconciles the session to `Failed`.
* Failed or timed-out operator hosts are killed and reaped.
Following milestone
The following milestone should be derived from the updated contract and specification once this one lands. Candidate: fail-closed embedded Core dependency identity with installed-CLI black-box test proving source workspaces resolve and build without access to original checkout or Git.
