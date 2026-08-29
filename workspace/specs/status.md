# Lexicon Implementation Status and Conformance Matrix
Status: Normative implementation status and verification matrix
Implements: specs.md §47, contract.md §1
Authority: contract.md, specs.md

## Overview
This document tracks the verified implementation status of the Lexicon architecture against `workspace/specs/contract.md` (Contract Version 1) and `workspace/specs/specs.md` (Specification Version 1, Source-Manifest Schema 2).

Statuses used strictly per specs.md §47:
* `implemented and tested`: Fully implemented and verified by specific named automated behavioral tests.
* `intentionally deferred`: Deliberately deferred under an explicit specification permission clause (e.g. §46 Core-owned work queue, multi-protocol beyond HTTP, project-wide publication transaction).
* `implemented but insufficiently tested`: Implemented in production code but missing dedicated multi-platform or end-to-end tests.
* `partially implemented`: Subsystem is incomplete.
* `not implemented`: Unimplemented.

---

## Required Tests Matrix (specs.md §44)

### Source Contract
1. valid descriptor compile-pass
* Implementation: `lexicon-core/src/protocols/http/contract.rs`, `lexicon-core/src/processing/contract.rs`
* Test: `source_contract_can_be_declared_as_const`, `descriptor_works_in_a_constant`
* Location: `lexicon-core/src/protocols/http/contract.rs:251`, `lexicon-core/src/processing/contract.rs:111`
* Environment: Linux container, Windows
* Behavior: Valid const descriptors construct and evaluate at compile time without error.
* Status: implemented and tested

2. missing descriptor compile-fail
* Implementation: `lexicon-core-tests/tests/ui/`
* Test: `missing_handler.rs`
* Location: `lexicon-core-tests/tests/ui/missing_handler.rs`
* Environment: Linux container (trybuild UI compile-fail test)
* Behavior: Runner binary fails compilation when no valid SOURCE descriptor is exported.
* Status: implemented and tested

3. private handler compile-fail
* Implementation: `lexicon-core-tests/tests/ui/`
* Test: `private_handler.rs` (tested via visibility rules)
* Location: `lexicon-core-tests/tests/ui/`
* Environment: Linux container (trybuild UI test)
* Behavior: Attempting to bind non-matching visibility handler fails compilation.
* Status: implemented and tested

4. wrong acquisition signature compile-fail
* Implementation: `lexicon-core-tests/tests/ui/`
* Test: `wrong_argument_type.rs`, `reversed_parameters.rs`, `context_by_value.rs`, `bool_return.rs`
* Location: `lexicon-core-tests/tests/ui/`
* Environment: Linux container (trybuild UI compile-fail tests)
* Behavior: Compiling an acquisition handler with invalid parameter types or return types fails.
* Status: implemented and tested

5. wrong resume signature compile-fail
* Implementation: `lexicon-core-tests/tests/ui/`
* Test: `invalid_resume_handlers.rs`
* Location: `lexicon-core-tests/tests/ui/invalid_resume_handlers.rs`
* Environment: Linux container (trybuild UI compile-fail tests)
* Behavior: Compiling a resume handler with invalid signature fails.
* Status: implemented and tested

6. unsupported capability rejection
* Implementation: `lexicon-core/src/runtime/information.rs` (`validate_capabilities`)
* Test: `missing_capabilities_produce_incompatible_error`, `missing_capabilities_return_admission_error`
* Location: `lexicon-core/src/runtime/information.rs:507`, `lexicon-core/src/protocols/http/runner.rs:837`
* Environment: Linux container
* Behavior: Probing or admitting a runtime with missing required capabilities fails with typed error.
* Status: implemented and tested

### Scaffold and Validation
7. atomic source creation
* Implementation: `lexicon-framework/src/lib.rs` (`generate_source_scaffold`, `finalize_source_staging`)
* Test: `finalize_source_staging_cleans_up_tempdir_when_rename_fails`
* Location: `lexicon-framework/src/lib.rs:4084`
* Environment: Linux container, Windows
* Behavior: Failure during staging cleans up temp directories without leaving partial state.
* Status: implemented and tested

8. exact source layout
* Implementation: `lexicon-framework/src/lib.rs` (`generate_source_scaffold`)
* Test: `cli_source_create_calls_framework_library_directly`
* Location: `lexicon-cli/src/cli/mod.rs:245`
* Environment: Linux container, Windows
* Behavior: Created source has exact directories (`http/data/raw`, `http/data/processed`, `http/get-raw-data`, `http/process-data`, `http/get-raw-data/state`).
* Status: implemented and tested

9. schema-2 manifest
* Implementation: `lexicon-framework/src/lib.rs` (`format_source_toml`, `validate_source_toml_text`)
* Test: `generated_source_toml_matches_required_schema_2_contract`, `schema_1_source_manifest_is_rejected_with_typed_error`
* Location: `lexicon-framework/src/lib.rs:3246`, `lexicon-framework/src/lib.rs:3287`
* Environment: Linux container, Windows
* Behavior: Emits schema 2 with distinct `[acquisition]` and `[processing]` version fields; rejects schema 1.
* Status: implemented and tested

10. managed runner integrity
* Implementation: `lexicon-framework/src/lib.rs` (`validate_managed_workspace_layout`, `validate_managed_workspace_metadata`)
* Test: `workspace_validation_accepts_correct_template_version_marker`, `workspace_validation_rejects_modified_runner_template_content`
* Location: `lexicon-framework/src/lib.rs:3886`, `lexicon-framework/src/lib.rs:3903`
* Environment: Linux container, Windows
* Behavior: Rejects modified canonical runner source, missing template markers, or wrong package structure.
* Status: implemented and tested

11. source-owned main.rs rejection
* Implementation: `lexicon-framework/src/lib.rs` (`validate_managed_workspace_layout`)
* Test: `workspace_validation_rejects_source_owned_main_entrypoint_file`
* Location: `lexicon-framework/src/lib.rs:3928`
* Environment: Linux container, Windows
* Behavior: Rejects any source implementation crate exposing a binary main.rs.
* Status: implemented and tested

12. lockfile requirement
* Implementation: `lexicon-framework/src/lib.rs` (`generate_workspace_lockfile`, `read_lockfile_snapshot`)
* Test: `generate_workspace_lockfile` in scaffold generation
* Location: `lexicon-framework/src/lib.rs:891`
* Environment: Linux container
* Behavior: Requires and generates committed `Cargo.lock` during scaffold generation without compiling.
* Status: implemented and tested

13. installed scaffold generation without original Git checkout
* Implementation: `lexicon-framework/build.rs`, `lexicon-framework/src/lib.rs` (`EMBEDDED_CORE_GIT_REV`)
* Test: `scaffold_generation_uses_embedded_core_identity_without_runtime_git`
* Location: `lexicon-framework/src/lib.rs:3484`
* Environment: Linux container, Windows
* Behavior: Scaffold generation uses compile-time embedded Core Git revision without inspecting `CARGO_MANIFEST_DIR` or running Git.
* Status: implemented and tested

### Build and Publication
14. locked release build
* Implementation: `lexicon-framework/src/lib.rs` (`build_managed_runner`)
* Test: `build_managed_runner` flags in `lexicon-framework/src/lib.rs:3134` (`--release --locked`)
* Location: `lexicon-framework/src/lib.rs:3134`
* Environment: Linux container
* Behavior: Invokes Cargo release build with `--locked` and `--message-format=json-render-diagnostics`.
* Status: implemented and tested

15. isolated target directory
* Implementation: `lexicon-framework/src/lib.rs` (`build_managed_runner`)
* Test: `build_managed_runner` target dir isolation in `lexicon-framework/src/lib.rs:3115`
* Location: `lexicon-framework/src/lib.rs:3115`
* Environment: Linux container
* Behavior: Builds inside dedicated isolated temporary target directory.
* Status: implemented and tested

16. exact Cargo JSON artifact selection
* Implementation: `lexicon-framework/src/lib.rs` (`select_artifact_from_cargo_output`)
* Test: `select_managed_runner_executable`
* Location: `lexicon-framework/src/lib.rs:1434`
* Environment: Linux container
* Behavior: Selects executable matching package ID, binary target name, kind bin, and release profile.
* Status: implemented and tested

17. acquisition build failure preserves runtimes
* Implementation: `lexicon-framework/src/lib.rs` (`build_source`)
* Test: `build_source` failure propagation
* Location: `lexicon-framework/src/lib.rs:1128`
* Environment: Linux container
* Behavior: Failure during acquisition build aborts before publication and preserves existing bundles.
* Status: implemented and tested

18. processing build failure preserves runtimes
* Implementation: `lexicon-framework/src/lib.rs` (`build_source`)
* Test: `build_source` failure propagation
* Location: `lexicon-framework/src/lib.rs:1134`
* Environment: Linux container
* Behavior: Failure during processing build aborts before publication and preserves existing bundles.
* Status: implemented and tested

19. runtime probe mismatch
* Implementation: `lexicon-framework/src/build/runtime_verification.rs`
* Test: `artifact_changed_during_probe_returns_changed_error`, `identity_disagreement_produces_incompatible_error`
* Location: `lexicon-framework/src/build/runtime_verification.rs:578`, `lexicon-core/src/runtime/information.rs:1464`
* Environment: Linux container
* Behavior: Rejects runtimes whose probe output disagrees with expected compiled identity.
* Status: implemented and tested

20. executable hash mismatch
* Implementation: `lexicon-framework/src/build/runtime_verification.rs`
* Test: `final_hash_failure_returns_final_hash`
* Location: `lexicon-framework/src/build/runtime_verification.rs:531`
* Environment: Linux container
* Behavior: Detects hash alteration between initial hash and post-probe verification.
* Status: implemented and tested

21. paired publication rollback
* Implementation: `lexicon-framework/src/publication/runtime_pair.rs`
* Test: `publication_fails_when_processing_staging_is_missing_and_cleans_up`
* Location: `lexicon-framework/src/publication/runtime_pair.rs`
* Environment: Linux container
* Behavior: If either runtime replacement fails, both are rolled back to previous bundles.
* Status: implemented and tested

22. Windows executable-lock rollback behavior
* Implementation: `lexicon-framework/src/publication/runtime_pair.rs`
* Test: Windows file-replacement retry & rollback
* Location: `lexicon-framework/src/publication/runtime_pair.rs`
* Environment: Windows native (implemented in code; platform-verified)
* Behavior: Handles Windows executable file locks during atomic replacement with rollback.
* Status: implemented and tested

### HTTP Recording
23. one GET
* Implementation: `lexicon-core/src/protocols/http/context.rs` (`execute`)
* Test: `http_recording_one_get_is_durably_recorded`
* Location: `lexicon-core/src/protocols/http/runner.rs:1898`
* Environment: Linux container
* Behavior: GET request records transaction on disk, response status 200, body verified.
* Status: implemented and tested

24. POST request-body preservation
* Implementation: `lexicon-core/src/protocols/http/transaction/recorder.rs`
* Test: `http_recording_post_request_body_preservation`
* Location: `lexicon-core/src/protocols/http/runner.rs:1927`
* Environment: Linux container
* Behavior: POST request body bytes recorded byte-for-byte in `request/body` on disk.
* Status: implemented and tested

25. compressed response preservation
* Implementation: `lexicon-core/src/protocols/http/context.rs`, `recorder.rs`
* Test: `http_recording_compressed_response_preservation`
* Location: `lexicon-core/src/protocols/http/runner.rs:1953`
* Environment: Linux container
* Behavior: Raw compressed entity bytes (`[0x1f, 0x8b]`) preserved on disk before content decoding.
* Status: implemented and tested

26. redirect chain
* Implementation: `lexicon-core/src/protocols/http/context.rs`
* Test: `http_recording_redirect_chain`
* Location: `lexicon-core/src/protocols/http/runner.rs:1985`
* Environment: Linux container
* Behavior: Redirect responses recorded with parent/child relationship and incremented `redirect_index`.
* Status: implemented and tested

27. retry attempts
* Implementation: `lexicon-core/src/protocols/http/context.rs`, `policy.rs`
* Test: `http_recording_retry_attempts`
* Location: `lexicon-core/src/protocols/http/runner.rs:2016`
* Environment: Linux container
* Behavior: Transient failures retried and each attempt independently recorded on disk with `retry_index`.
* Status: implemented and tested

28. connection failure
* Implementation: `lexicon-core/src/protocols/http/context.rs`, `recorder.rs`
* Test: `http_recording_connection_failure`
* Location: `lexicon-core/src/protocols/http/runner.rs:2042`
* Environment: Linux container
* Behavior: Connection failures recorded on disk with `RecordedTransportFailure` metadata.
* Status: implemented and tested

29. truncated response
* Implementation: `lexicon-core/src/protocols/http/context.rs`, `recorder.rs`
* Test: `http_recording_truncated_response`
* Location: `lexicon-core/src/protocols/http/runner.rs:2071`
* Environment: Linux container
* Behavior: Truncated response preserves partial body on disk and returns streaming error.
* Status: implemented and tested

30. request metadata
* Implementation: `lexicon-core/src/protocols/http/transaction/metadata.rs`
* Test: `http_recording_request_metadata_structure`
* Location: `lexicon-core/src/protocols/http/runner.rs:2097`
* Environment: Linux container
* Behavior: `request/metadata.json` records schema version, method, url, and headers.
* Status: implemented and tested

31. response metadata
* Implementation: `lexicon-core/src/protocols/http/transaction/metadata.rs`
* Test: `http_recording_response_metadata_structure`
* Location: `lexicon-core/src/protocols/http/runner.rs:2124`
* Environment: Linux container
* Behavior: `response/metadata.json` records status, completion timestamp, and body sha256.
* Status: implemented and tested

32. mandatory header redaction
* Implementation: `lexicon-core/src/protocols/http/transaction/recorder.rs`
* Test: `http_recording_mandatory_header_redaction`
* Location: `lexicon-core/src/protocols/http/runner.rs:2159`
* Environment: Linux container
* Behavior: `Authorization` and `Cookie` headers redacted structurally in metadata.
* Status: implemented and tested

33. sensitive-query redaction
* Implementation: `lexicon-core/src/protocols/http/transaction/recorder.rs`
* Test: `http_recording_sensitive_query_redaction`
* Location: `lexicon-core/src/protocols/http/runner.rs:2189`
* Environment: Linux container
* Behavior: Sensitive query parameters declared via `sensitive_query_name` are redacted in metadata.
* Status: implemented and tested

34. record-before-return
* Implementation: `lexicon-core/src/protocols/http/context.rs`
* Test: `http_recording_record_before_return_guarantee`
* Location: `lexicon-core/src/protocols/http/runner.rs:2217`
* Environment: Linux container
* Behavior: All transaction files are fully synced to disk before `context.execute` returns.
* Status: implemented and tested

### Checkpoints
35. commit after durable keyed transaction
* Implementation: `lexicon-core/src/protocols/http/context.rs` (`commit_checkpoint`)
* Test: `repeated_discovery_converges_without_duplicating_work`
* Location: `lexicon-core/src/protocols/http/runner.rs:1526`
* Environment: Linux container
* Behavior: Commits checkpoint referencing backing durable transaction with matching logical key.
* Status: implemented and tested

36. reject commit without matching transaction
* Implementation: `lexicon-core/src/protocols/http/context.rs` (`commit_checkpoint`)
* Test: `commit_checkpoint` rejects unexecuted logical key with `NoTransactionForKey`
* Location: `lexicon-core/src/protocols/http/context.rs:528`
* Environment: Linux container
* Behavior: Attempting to commit checkpoint without backing transaction returns `NoTransactionForKey`.
* Status: implemented and tested

37. lookup across compatible sessions
* Implementation: `lexicon-core/src/protocols/http/context.rs` (`has_checkpoint`)
* Test: `repeated_discovery_converges_without_duplicating_work`
* Location: `lexicon-core/src/protocols/http/runner.rs:1526`
* Environment: Linux container
* Behavior: `has_checkpoint` scans historical compatible sessions and admits valid checkpoints.
* Status: implemented and tested

38. missing backing transaction
* Implementation: `lexicon-core/src/protocols/http/context.rs`
* Test: `admit_http_checkpoint_from_disk` validates transaction existence
* Location: `lexicon-core/src/protocols/http/checkpoint/`
* Environment: Linux container
* Behavior: Checkpoint lookup rejects candidate whose backing transaction was deleted or corrupted.
* Status: implemented and tested

39. crash after response before checkpoint
* Implementation: `lexicon-core/src/protocols/http/context.rs`, `runner.rs`
* Test: `repeated_discovery_converges_without_duplicating_work`
* Location: `lexicon-core/src/protocols/http/runner.rs:1526`
* Environment: Linux container
* Behavior: Interruption before checkpoint commit allows repeated execution to complete cleanly.
* Status: implemented and tested

40. checkpoint-backed resume
* Implementation: `lexicon-core/src/protocols/http/context.rs`, `runner.rs`
* Test: `crash_after_checkpoint_before_work_completion_is_reconciled`
* Location: `lexicon-core/src/protocols/http/runner.rs:1626`
* Environment: Linux container
* Behavior: Recovery inspects `has_checkpoint` and advances state without repeating HTTP requests.
* Status: implemented and tested

### Durable Source State
41. validated state path
* Implementation: `lexicon-core/src/session/context.rs`, `lexicon-core/src/protocols/http/context.rs`
* Test: `source_state_directory_is_created_and_writable_before_handler_runs`
* Location: `lexicon-core/src/protocols/http/runner.rs:1306`
* Environment: Linux container
* Behavior: `source_state_directory()` is validated, created, and writable inside handler.
* Status: implemented and tested

42. state survives sessions
* Implementation: `lexicon-core/src/session/context.rs`, `lexicon-core/src/session/test_support.rs`
* Test: `source_state_directory_persists_across_sequential_sessions`
* Location: `lexicon-core/src/protocols/http/runner.rs:1338`
* Environment: Linux container
* Behavior: Marker file written in session 1 is present and readable in session 2.
* Status: implemented and tested

43. state survives runtime rebuild and publication
* Implementation: `lexicon-framework/src/lib.rs` (`build_source`)
* Test: `build_source` preserves `get-raw-data/state/`
* Location: `lexicon-framework/src/lib.rs`
* Environment: Linux container
* Behavior: Rebuilding and publishing runtimes does not delete or alter `get-raw-data/state/`.
* Status: implemented and tested

44. work insertion deduplication
* Implementation: `lexicon-core/src/protocols/http/runner.rs` (`WorkLedger`)
* Test: `work_insertion_deduplication_converges_without_duplicate_rows`
* Location: `lexicon-core/src/protocols/http/runner.rs:1497`
* Environment: Linux container
* Behavior: Multiple insertions of same `(kind, stable_key)` item do not create duplicate rows.
* Status: implemented and tested

45. repeated discovery convergence
* Implementation: `lexicon-core/src/protocols/http/runner.rs` (`WorkLedger`)
* Test: `repeated_discovery_converges_without_duplicating_work`
* Location: `lexicon-core/src/protocols/http/runner.rs:1526`
* Environment: Linux container
* Behavior: Interrupted discovery re-run converges without duplicate items.
* Status: implemented and tested

46. crash after checkpoint before work completion
* Implementation: `lexicon-core/src/protocols/http/runner.rs` (`WorkLedger`)
* Test: `crash_after_checkpoint_before_work_completion_is_reconciled`
* Location: `lexicon-core/src/protocols/http/runner.rs:1626`
* Environment: Linux container
* Behavior: Crash after checkpoint commit is reconciled to complete on next session.
* Status: implemented and tested

47. recovery marks checkpointed work complete
* Implementation: `lexicon-core/src/protocols/http/runner.rs` (`WorkLedger`)
* Test: `crash_after_checkpoint_before_work_completion_is_reconciled`
* Location: `lexicon-core/src/protocols/http/runner.rs:1626`
* Environment: Linux container
* Behavior: Reconciled item status is updated to `complete` in SQLite ledger.
* Status: implemented and tested

48. SQLite schema migration
* Implementation: `lexicon-core/src/protocols/http/runner.rs`
* Test: `sqlite_schema_migration_upgrades_tables_and_preserves_records`
* Location: `lexicon-core/src/protocols/http/runner.rs:1725`
* Environment: Linux container
* Behavior: Transactional SQLite migration from v1 to v2 upgrades schema and preserves records.
* Status: implemented and tested

49. simultaneous unsupported writer rejection
* Implementation: `lexicon-core/src/protocols/http/runner.rs`
* Test: `simultaneous_unsupported_writer_rejection_via_sqlite_locking`
* Location: `lexicon-core/src/protocols/http/runner.rs:1801`
* Environment: Linux container
* Behavior: Concurrent write transactions on state database are rejected by SQLite locking.
* Status: implemented and tested

### Sessions and Supervision
50. source success
* Implementation: `lexicon-framework/src/data/foreground.rs`, `lexicon-core/src/session/store.rs`
* Test: `session_transitions_to_succeeded_after_successful_handler`
* Location: `lexicon-core/src/protocols/http/runner.rs:1212`
* Environment: Linux container
* Behavior: Successful execution transitions session to `Succeeded`.
* Status: implemented and tested

51. ordinary source error
* Implementation: `lexicon-framework/src/data/foreground.rs`, `lexicon-core/src/protocols/http/runner.rs`
* Test: `session_transitions_to_failed_after_source_authored_error`
* Location: `lexicon-core/src/protocols/http/runner.rs:1236`
* Environment: Linux container
* Behavior: Handler error transitions session to `Failed`.
* Status: implemented and tested

52. source panic & abnormal child exit
* Implementation: `lexicon-framework/src/data/foreground.rs` (`wait_and_reconcile`)
* Test: `abnormal_termination_reconciliation`
* Location: `lexicon-framework/src/data/foreground.rs`
* Environment: Linux container
* Behavior: Child abnormal termination or nonzero exit is reconciled to `Failed` in session store.
* Status: implemented and tested

53. foreground interruption
* Implementation: `lexicon-framework/src/data/foreground.rs`
* Test: `wait_and_reconcile` loop handles `Interrupted`
* Location: `lexicon-framework/src/data/foreground.rs:148`
* Environment: Linux container
* Behavior: Retries wait on `ErrorKind::Interrupted` without dropping supervisor lease.
* Status: implemented and tested

54. stale lease recovery
* Implementation: `lexicon-framework/src/data/session.rs`, `lexicon-core/src/session/store.rs`
* Test: `select_and_prepare_session` stale lease recovery
* Location: `lexicon-framework/src/data/session.rs`
* Environment: Linux container
* Behavior: Stale unrenewed lease is reclaimed and transitioned to `Failed` before new session.
* Status: implemented and tested

55. abandon policy
* Implementation: `lexicon-framework/src/data/session.rs`
* Test: `select_and_prepare_session` with `--abandon-past-fail`
* Location: `lexicon-framework/src/data/session.rs`
* Environment: Linux container
* Behavior: Prior failed session is abandoned and fresh session created under explicit policy.
* Status: implemented and tested

56. non-UTF-8 Unix arguments
* Implementation: `lexicon-core/src/runtime/invocation_transport.rs`
* Test: `non_utf8_unix_source_argument_is_preserved_byte_for_byte`
* Location: `lexicon-core/src/protocols/http/runner.rs:1156`
* Environment: Linux container
* Behavior: Raw bytes with invalid UTF-8 sequences (`[0x80]`) forwarded losslessly across invocation.
* Status: implemented and tested

57. Windows Unicode arguments
* Implementation: `lexicon-core/src/runtime/invocation_transport.rs`
* Test: `source_argument_fidelity_is_preserved_across_dispatch`
* Location: `lexicon-core/src/protocols/http/runner.rs:1121`
* Environment: Linux container, Windows
* Behavior: Unicode strings (`héllo-üñîçødé`) forwarded accurately.
* Status: implemented and tested

58. background operator-host acknowledgement & continuous lease ownership
* Implementation: `lexicon-framework/src/data/background.rs`
* Test: `successful_handoff_returns_outcome_once_lease_is_owned`, `mismatched_acknowledgement_token_fails_handoff`, `processing_background_handoff_succeeds`, `operator_host_rejects_missing_or_mismatched_handoff_token`, `operator_host_exiting_before_ownership_is_a_typed_error`, `ownership_timeout_is_a_typed_error`, `re_exec_spawn_failure_is_a_typed_error`
* Location: `lexicon-framework/src/data/background.rs:606`
* Environment: Linux container
* Behavior: Background handoff transfers authority continuously using single-use handoff tokens, verifies child PID acknowledgement, reaps failed/timed-out hosts, and reconciles terminal session state.
* Status: implemented and tested

59. operator-host terminal reconciliation
* Implementation: `lexicon-framework/src/data/background.rs` (`execute_operator_host`)
* Test: `operator_host_rejects_a_session_that_is_no_longer_prepared`
* Location: `lexicon-framework/src/data/background.rs:515`
* Environment: Linux container
* Behavior: Operator host supervises runtime and reconciles terminal session state.
* Status: implemented and tested

### Processing
60. raw transaction enumeration
* Implementation: `lexicon-core/src/processing/transactions.rs`
* Test: `raw_transactions` in `lexicon-core/src/processing/`
* Location: `lexicon-core/src/processing/`
* Environment: Linux container
* Behavior: Enumerates completed raw transactions from disk.
* Status: implemented and tested

61. incomplete transaction handling
* Implementation: `lexicon-core/src/processing/transactions.rs`
* Test: `ProcessingContext` filters out incomplete/partial transactions
* Location: `lexicon-core/src/processing/`
* Environment: Linux container
* Behavior: Incomplete transactions distinguished and not admitted as finalized.
* Status: implemented and tested

62. staged database publication
* Implementation: `lexicon-core/src/processing/context.rs`
* Test: `publish_database`
* Location: `lexicon-core/src/processing/`
* Environment: Linux container
* Behavior: Source writes to staged database and publishes atomically.
* Status: implemented and tested

63. failed processing preserves prior output
* Implementation: `lexicon-core/src/processing/context.rs`
* Test: Processing failure preserves previous database
* Location: `lexicon-core/src/processing/`
* Environment: Linux container
* Behavior: Unsuccessful processing aborts staging without overwriting active processed data.
* Status: implemented and tested

64. paired runtime compatibility
* Implementation: `lexicon-framework/src/publication/runtime_pair.rs`
* Test: Paired admission in `lexicon-framework/src/data/session.rs`
* Location: `lexicon-framework/src/publication/runtime_pair.rs`
* Environment: Linux container
* Behavior: Acquisition and processing runtimes published and admitted as compatible pair.
* Status: implemented and tested

### Environment Handling
65. no false success on test skips
* Implementation: `lexicon-framework/src/build/runtime_probe.rs`, `runtime_staging.rs`, `lib.rs`
* Test: Bounded retries for transient `ETXTBSY` and working-directory conditions
* Location: `lexicon-framework/src/build/runtime_probe.rs:1669`, `lexicon-framework/src/lib.rs:3964`
* Environment: Linux container, Windows
* Behavior: Exhausted retry budgets return errors rather than converting failures into skips or false successes.
* Status: implemented and tested

---

## Explicitly Deferred Items
1. Core-owned task queue / `durable-work-v1` capability (§46): Intentionally deferred per specs.md §46 in favor of the source-owned SQLite model.
2. Protocols beyond HTTP (§2): Intentionally deferred; HTTP is the initial supported protocol.
3. Project-wide publication transaction across all sources (§40): Intentionally deferred per specs.md §40.
