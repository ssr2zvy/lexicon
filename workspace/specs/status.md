# Lexicon Implementation Status

Normative implementation status for Lexicon achieving Contract V1 and
Specification V1 conformance (`workspace/specs/contract.md`,
`workspace/specs/specs.md`). The mechanical conformance matrix lives in
[`workspace/specs/conformance.toml`](conformance.toml); this document is
its prose counterpart. Every row uses the **exact** MATRIX-01 shape from
`current.md` §5.

The master milestone remains open until:

1. MZA publishes and pins a Protocol 1 installer API that owns install,
   upgrade, uninstall, command registration, and platform integration
   (`current.md` §3); or
2. the committee explicitly amends `contract.md` to remove that
   requirement.

Until one of those events occurs, every `Gate 6`/`Gate 8` row carries
`Status: not implemented` with `Open gap` describing the upstream
prerequisite. All other rows are `Status: implemented, unverified` —
implementation is in place and unit / integration targets exist, but no
durable CI evidence (a green workflow run URL) is available yet. CI-01
publishes the manifest schema but the first green run is still pending.

No row claims `Status: conformant` — that flag requires durable
evidence linked to an exact commit, and per the audit's own §17 wording
"a number in current.md without the attached exact-SHA manifest is not
evidence."

---

## 1. Source contract compile-time guarantees (specs.md §44)

### specs-44-valid-descriptor-compile-pass

* Contract/spec authority: `workspace/specs/specs.md#44`
* Implementation:
  * `lexicon-core/src/protocols/http/contract.rs:HttpSourceContractV1`
  * `lexicon-core/src/processing/contract.rs:ProcessingSourceContractV1`
* Automated evidence:
  * `core::protocols::http::contract::tests::source_contract_can_be_declared_as_const`
  * `core::processing::contract::tests::descriptor_works_in_a_constant`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-44-private-acquisition-handler

* Contract/spec authority: `workspace/specs/specs.md#44`
* Implementation:
  * `lexicon-core/src/protocols/http/contract.rs`
  * `lexicon-core/tests/ui-pass/private_acquisition_handler.rs`
* Automated evidence:
  * `core::tests::contract_ui::compile_pass_contracts`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-44-private-processing-handlers

* Contract/spec authority: `workspace/specs/specs.md#44`
* Implementation:
  * `lexicon-core/src/processing/contract.rs`
  * `lexicon-core/tests/ui-pass/private_processing_handlers.rs`
* Automated evidence:
  * `core::tests::contract_ui::compile_pass_contracts`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-44-missing-public-source-descriptor

* Contract/spec authority: `workspace/specs/specs.md#44`
* Implementation:
  * `lexicon-core/tests/ui/missing_exported_source_descriptor.rs`
  * `lexicon-core/tests/managed_runner_contract.rs`
* Automated evidence:
  * `core::tests::managed_runner_contract::missing_exported_source_descriptor_in_runner_boundary_fails_compile`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### contract-7-wrong-acquisition-signature-compile-fail

* Contract/spec authority: `workspace/specs/contract.md#7`
* Implementation:
  * `lexicon-core/tests/ui/wrong_argument_type.rs`
  * `lexicon-core/tests/ui/reversed_parameters.rs`
  * `lexicon-core/tests/ui/context_by_value.rs`
  * `lexicon-core/tests/ui/bool_return.rs`
  * `lexicon-core/tests/contract_ui.rs`
* Automated evidence:
  * `core::tests::contract_ui::compile_fail_contracts`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### contract-7-unsupported-capability-rejection

* Contract/spec authority: `workspace/specs/contract.md#7`
* Implementation: `lexicon-core/src/runtime/information.rs:validate_capabilities`
* Automated evidence:
  * `core::runtime::information::tests::missing_capabilities_produce_incompatible_error`
  * `core::runtime::information::tests::missing_capabilities_return_admission_error`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

## 2. Source creation, durability, and validation (specs.md §6, §7, §10)

### specs-7-atomic-source-creation

* Contract/spec authority: `workspace/specs/specs.md#7`
* Implementation:
  * `lexicon-framework/src/lib.rs:generate_source_scaffold`
  * `lexicon-framework/src/fs/durable.rs`
* Automated evidence:
  * `lexicon-framework::lib::tests::finalize_source_staging_cleans_up_tempdir_when_rename_fails`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-6-exact-source-layout

* Contract/spec authority: `workspace/specs/specs.md#6`
* Implementation: `lexicon-framework/src/lib.rs:generate_source_scaffold`
* Automated evidence:
  * `cli::cli::mod::tests::cli_source_create_calls_framework_library_directly`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-5-schema-2-manifest

* Contract/spec authority: `workspace/specs/specs.md#5`
* Implementation:
  * `lexicon-framework/src/lib.rs:format_source_toml`
  * `lexicon-framework/src/lib.rs:validate_source_toml_text`
* Automated evidence:
  * `lexicon-framework::lib::tests::generated_source_toml_matches_required_schema_2_contract`
  * `lexicon-framework::lib::tests::schema_1_source_manifest_is_rejected_with_typed_error`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-10-managed-runner-integrity

* Contract/spec authority: `workspace/specs/specs.md#10`
* Implementation:
  * `lexicon-framework/src/lib.rs:validate_managed_workspace_layout`
  * `lexicon-framework/src/lib.rs:validate_managed_workspace_metadata`
* Automated evidence:
  * `lexicon-framework::lib::tests::workspace_validation_accepts_correct_template_version_marker`
  * `lexicon-framework::lib::tests::workspace_validation_rejects_modified_runner_template_content`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-10-source-owned-main-rejection

* Contract/spec authority: `workspace/specs/specs.md#10`
* Implementation: `lexicon-framework/src/lib.rs:validate_managed_workspace_layout`
* Automated evidence:
  * `lexicon-framework::lib::tests::workspace_validation_rejects_source_owned_main_entrypoint_file`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-7-lockfile-requirement

* Contract/spec authority: `workspace/specs/specs.md#7`
* Implementation:
  * `lexicon-framework/src/lib.rs:generate_workspace_lockfile`
  * `lexicon-framework/src/lib.rs:read_lockfile_snapshot`
* Automated evidence:
  * `lexicon-framework::lib::tests::generate_workspace_lockfile`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

## 3. Installed-CLI core identity (specs.md §8)

### specs-8-installed-scaffold-without-checkout

* Contract/spec authority: `workspace/specs/specs.md#8`
* Implementation:
  * `lexicon-framework/build.rs`
  * `lexicon-framework/src/lib.rs:EMBEDDED_CORE_GIT_REV`
  * `lexicon-framework/src/build/core_dependency.rs:REQUIRED_CORE_GIT_URL`
* Automated evidence:
  * `installed_core_identity::installed_lexicon_source_create_embeds_compiled_core_identity_outside_repo`
  * `installed_core_identity::installed_lexicon_source_create_fails_clearly_when_protocol_is_unknown`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

## 4. Build and publication (specs.md §19, §21)

### specs-19-locked-release-build

* Contract/spec authority: `workspace/specs/specs.md#19`
* Implementation: `lexicon-framework/src/lib.rs:build_managed_runner`
* Automated evidence:
  * `lexicon-framework::lib::tests::build_managed_runner`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-21-paired-runtime-compatibility

* Contract/spec authority: `workspace/specs/specs.md#21`
* Implementation:
  * `lexicon-framework/src/publication/runtime_pair.rs:publish_runtime_pair`
  * `lexicon-framework/src/publication/file_system.rs`
* Automated evidence:
  * `lexicon-framework::publication::runtime_pair::tests::publication_fails_when_processing_staging_is_missing_and_cleans_up`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

## 5. HTTP recording and audit (specs.md §24)

### specs-24-compressed-response-exact-bytes

* Contract/spec authority: `workspace/specs/specs.md#24`
* Implementation: `lexicon-core/src/protocols/http/transaction/recorder.rs`
* Automated evidence:
  * `core::protocols::http::runner::tests::compressed_response_preserves_exact_wire_bytes_and_hash`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-24-redirect-chain-lineage

* Contract/spec authority: `workspace/specs/specs.md#24`
* Implementation: `lexicon-core/src/protocols/http/context.rs`
* Automated evidence:
  * `core::protocols::http::runner::tests::redirect_chain_persists_each_attempt_with_parent_identity`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-24-retry-policy-three-attempts

* Contract/spec authority: `workspace/specs/specs.md#24`
* Implementation: `lexicon-core/src/protocols/http/context.rs`
* Automated evidence:
  * `core::protocols::http::runner::tests::retry_policy_persists_exactly_three_distinct_attempts`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-24-connection-failure-final-metadata

* Contract/spec authority: `workspace/specs/specs.md#24`
* Implementation: `lexicon-core/src/protocols/http/context.rs`
* Automated evidence:
  * `core::protocols::http::runner::tests::connection_failure_persists_finalized_failure_metadata`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-24-truncated-body-partial-bytes

* Contract/spec authority: `workspace/specs/specs.md#24`
* Implementation: `lexicon-core/src/protocols/http/transaction/recorder.rs`
* Automated evidence:
  * `core::protocols::http::runner::tests::truncated_body_preserves_partial_bytes_and_incomplete_marker`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-24-mandatory-explicit-redaction

* Contract/spec authority: `workspace/specs/specs.md#24`
* Implementation:
  * `lexicon-core/src/protocols/http/sensitivity.rs`
  * `lexicon-core/src/protocols/http/transaction/recorder.rs`
  * `lexicon-core/src/protocols/http/transaction/metadata.rs`
* Automated evidence:
  * `core::protocols::http::runner::tests::all_mandatory_and_explicit_headers_are_structurally_redacted`
  * `core::protocols::http::runner::tests::sensitive_query_never_appears_in_any_durable_or_diagnostic_text`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-24-cross-origin-strip-secrets

* Contract/spec authority: `workspace/specs/specs.md#24`
* Implementation: `lexicon-core/src/protocols/http/context.rs:same_origin`
* Automated evidence:
  * `core::protocols::http::runner::tests::cross_origin_redirect_strips_secrets_for_ip_literal_hosts`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-24-execute-returns-after-sync

* Contract/spec authority: `workspace/specs/specs.md#24`
* Implementation: `lexicon-core/src/protocols/http/context.rs`
* Automated evidence:
  * `core::protocols::http::runner::tests::execute_returns_only_after_transaction_directory_sync`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

## 6. Checkpoints (specs.md §36)

### specs-36-checkpoint-backs-completed-transaction

* Contract/spec authority: `workspace/specs/specs.md#36`
* Implementation:
  * `lexicon-core/src/protocols/http/checkpoint/mod.rs`
  * `lexicon-core/src/protocols/http/checkpoint/tests.rs`
* Automated evidence:
  * `core::protocols::http::checkpoint::tests::checkpoint_commit_requires_progress_published_transaction`
  * `core::protocols::http::checkpoint::tests::checkpoint_commit_requires_completed_response`
  * `core::protocols::http::checkpoint::tests::checkpoint_commit_rejects_transaction_from_other_session`
  * `core::protocols::http::checkpoint::tests::checkpoint_commit_rejects_logical_key_mismatch`
  * `core::protocols::http::checkpoint::tests::checkpoint_commit_rejects_attempt_identity_mismatch`
  * `core::protocols::http::checkpoint::tests::checkpoint_commit_rejects_missing_or_corrupt_backing_transaction`
  * `core::protocols::http::checkpoint::tests::checkpoint_commit_is_atomically_published_and_directory_synced`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

## 7. Processing (specs.md §39)

### specs-39-raw-transaction-enumeration

* Contract/spec authority: `workspace/specs/specs.md#39`
* Implementation: `lexicon-core/src/processing/transactions.rs`
* Automated evidence:
  * `processing_catalog_accepts_only_finalized_admitted_transactions`
  * `processing_catalog_orders_by_completion_then_identity`
  * `processing_catalog_rejects_duplicate_transaction_identity`
  * `processing_catalog_ignores_well_formed_live_staging_directory`
  * `processing_catalog_rejects_malformed_staging_name`
  * `processing_catalog_rejects_symlink_and_unexpected_file`
  * `processing_catalog_requires_succeeded_acquisition_session`
  * `processing_catalog_rejects_project_runtime_session_mismatch`
  * `processing_catalog_rejects_transaction_outside_session_time_bounds`
  * `processing_catalog_rejects_corrupt_transaction_metadata_or_body_hash`
  * `processing_catalog_rejects_missing_acquisition_session`
  * `processing_catalog_does_not_mutate_raw_data`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-39-incomplete-transaction-handling

* Contract/spec authority: `workspace/specs/specs.md#39`
* Implementation: `lexicon-core/src/processing/transactions.rs`
* Automated evidence:
  * `processing_context_filters_out_incomplete_transactions`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-39-staged-database-publication

* Contract/spec authority: `workspace/specs/specs.md#39`
* Implementation:
  * `lexicon-core/src/processing/context.rs`
  * `lexicon-core/src/protocols/http/runner.rs:catch_unwind`
* Automated evidence:
  * `core_begins_transaction_before_source_handler`
  * `successful_handler_commits_database_once`
  * `source_commit_or_rollback_attempt_is_detected`
  * `uncertain_commit_retains_uncertain_typed_outcome`
  * `commit_failure_never_reports_session_success`
  * `processing_context_exposes_read_only_admitted_catalog`
  * `require_transaction_active_distinguishes_open_from_after_handler`
  * `handler_panic_is_caught_reconciled_and_rolls_back`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL. Audit-named
  panic-recovery test (`handler_panic_is_caught_reconciled_and_rolls_back`)
  is now landed: `lexicon-core/src/protocols/http/runner.rs` wires
  `std::panic::catch_unwind` around the source-handler invocation and
  translates the unwinding termination into a typed
  `HttpRuntimeInvocationExecutionError::HandlerPanicked` plus the durable
  `SessionFailureCode::HandlerPanicked` failure code; the test asserts
  neither the durable `diagnostic` nor the typed error carries the
  source panic payload text.

* Contract/spec authority: `workspace/specs/specs.md#39`
* Implementation: `lexicon-core/src/processing/context.rs`
* Automated evidence:
  * `handler_error_rolls_back_and_preserves_previous_database`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

## 8. Foreground supervision and cancellation (specs.md §29, contract.md §20)

### specs-29-foreground-interruption

* Contract/spec authority: `workspace/specs/specs.md#29`
* Implementation:
  * `lexicon-framework/src/process/mod.rs:wait_with_cancellation`
  * `lexicon-framework/src/process/cancellation.rs:CancellationState`
  * `lexicon-cli/tests/foreground_cancellation.rs:UncoopFakeChild`
  * `lexicon-cli/tests/foreground_cancellation.rs:FailingTermChild`
* Automated evidence:
  * `foreground_cancellation::completed_outcome_when_child_exits_before_any_cancellation`
  * `foreground_cancellation::graceful_cancellation_path_uses_recorded_kind`
  * `foreground_cancellation::termination_kind_maps_to_documented_cancel_outcome`
  * `foreground_cancellation::shell_exit_codes_collapses_graceful_and_forceful_to_same_shell_code`
  * `foreground_cancellation::cancellation_records_graceful_failure_code`
  * `foreground_cancellation::cancellation_records_forced_failure_code`
  * `foreground_cancellation::wait_or_kill_error_never_reports_false_success`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### contract-20-stale-lease-recovery

* Contract/spec authority: `workspace/specs/contract.md#20`
* Implementation:
  * `lexicon_framework::data::session`
  * `lexicon-core/src/session/store.rs`
* Automated evidence:
  * `select_and_prepare_session`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-26-abandon-policy

* Contract/spec authority: `workspace/specs/specs.md#26`
* Implementation: `lexicon-framework/src/data/session.rs`
* Automated evidence:
  * `select_and_prepare_session`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-23-non-utf8-unix-arg-preservation

* Contract/spec authority: `workspace/specs/specs.md#23`
* Implementation:
  * `lexicon-core/src/runtime/invocation_transport.rs`
* Automated evidence:
  * `non_utf8_unix_source_argument_is_preserved_byte_for_byte`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-23-windows-unicode-arg-preservation

* Contract/spec authority: `workspace/specs/specs.md#23`
* Implementation:
  * `lexicon-core/src/runtime/invocation_transport.rs`
* Automated evidence:
  * `source_argument_fidelity_is_preserved_across_dispatch`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

## 9. Background handoff and operator host (specs.md §30, contract.md §30)

### specs-30-background-operator-host-handoff

* Contract/spec authority: `workspace/specs/specs.md#30`
* Implementation: `lexicon-framework/src/data/background.rs`
* Automated evidence:
  * `successful_handoff_returns_outcome_once_lease_is_owned`
  * `mismatched_acknowledgement_token_fails_handoff`
  * `processing_background_handoff_succeeds`
  * `operator_host_rejects_missing_or_mismatched_handoff_token`
  * `operator_host_exiting_before_ownership_is_a_typed_error`
  * `ownership_timeout_is_a_typed_error`
  * `re_exec_spawn_failure_is_a_typed_error`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### specs-30-operator-host-terminal-reconciliation

* Contract/spec authority: `workspace/specs/specs.md#30`
* Implementation: `lexicon-framework/src/data/background.rs`
* Automated evidence:
  * `operator_host_rejects_a_session_that_is_no_longer_prepared`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### current-15-cli-bg-fg-cancellation-integration

* Contract/spec authority: `current.md#15`
* Implementation:
  * `lexicon-cli/tests/background_handoff.rs`
  * `lexicon-cli/tests/foreground_cancellation.rs`
  * `lexicon-framework/src/data/background.rs`
  * `lexicon-framework/src/process/mod.rs`
* Automated evidence:
  * `operator_host_invocation_round_trips_typed_reference_through_json`
  * `operator_host_invocation_decoder_rejects_unknown_operation`
  * `operator_host_invocation_decoder_rejects_unknown_schema_version`
  * `operator_host_invocation_decoder_rejects_empty_protocol`
  * `operator_host_invocation_decoder_rejects_unknown_fields`
  * `execute_data_rejects_foreground_path_for_background_request`
  * `background_outcome_carries_project_source_session_and_operation`
  * `operator_host_binary_surfaces_typed_error_for_malformed_reference`
  * `operator_host_binary_against_nonexistent_project_does_not_succeed`
  * `completed_outcome_when_child_exits_before_any_cancellation`
  * `graceful_cancellation_path_uses_recorded_kind`
  * `termination_kind_maps_to_documented_cancel_outcome`
  * `shell_exit_codes_collapses_graceful_and_forceful_to_same_shell_code`
  * `cancellation_records_graceful_failure_code`
  * `cancellation_records_forced_failure_code`
  * `wait_or_kill_error_never_reports_false_success`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

## 10. Sessions and supervision end-to-end (specs.md §30, contract.md §20)

### contract-20-ordinary-source-error

* Contract/spec authority: `workspace/specs/contract.md#20`
* Implementation:
  * `lexicon-framework/src/data/foreground.rs`
  * `lexicon-core/src/protocols/http/runner.rs`
* Automated evidence: `session_transitions_to_failed_after_source_authored_error`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

### contract-20-source-panic-and-abnormal-exit

* Contract/spec authority: `workspace/specs/contract.md#20`
* Implementation:
  * `lexicon-framework/src/data/foreground.rs:wait_and_reconcile`
* Automated evidence: `abnormal_termination_reconciliation`
* Required environment: `linux-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

## 11. Environment handling for retries and probes (specs.md §44)

### specs-44-no-false-success-on-test-skips

* Contract/spec authority: `workspace/specs/specs.md#44`
* Implementation:
  * `lexicon-framework/src/build/runtime_probe.rs`
  * `lexicon-framework/src/build/runtime_staging.rs`
  * `lexicon-framework/src/lib.rs`
* Automated evidence:
  * `bounded_retries_for_transient_ETXTBSY`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that captures the manifest URL.

## 12. CI durable evidence (current.md §13/§14)

### ci-01-conformance-workflow

* Contract/spec authority: `current.md#13`
* Implementation: `.github/workflows/conformance.yml`
* Automated evidence:
  * `.github/workflows/conformance.yml` consumed by `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1`
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First green workflow run; manifest URL published; cycle re-tried until clean.

### ci-02-verification-manifest

* Contract/spec authority: `current.md#14`
* Implementation:
  * `verification/README.md`
  * `verification/verification-manifest.json`
* Automated evidence: `verification/verification-manifest.json` produced by `linux-container` and `windows-native` jobs.
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run produces a real manifest with the recorded SHA, toolchain, and exit codes.

## 13. Supply chain policy (current.md §12)

### supply-01-release-policy

* Contract/spec authority: `current.md#12`
* Implementation:
  * `workspace/specs/release-policy.md`
  * `automation/build_bundle_mza/produce_supply_inventory.sh`
  * `automation/build_bundle_mza/hardened_build.sh`
  * `verification/dependencies/cargo-metadata.json`
  * `verification/dependencies/cargo-tree.txt`
  * `verification/dependencies/build-scripts.json`
  * `verification/dependencies/proc-macros.json`
  * `verification/dependencies/licenses.json`
  * `verification/dependencies/advisories.json`
  * `verification/sbom.cdx.json`
* Automated evidence: producer scripts produce the inventory.
* Required environment: `linux-x86_64` (offline)
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First offline producer run; SBOM and inventory hashed and stored.

## 14. Windows runtime replacement (current.md §15)

### current-15-windows-runtime-replacement

* Contract/spec authority: `current.md#15`
* Implementation:
  * `lexicon-framework/tests/windows_runtime_replacement.rs`
  * `lexicon-framework/src/publication/runtime_pair.rs:publish_runtime_pair`
  * `lexicon-framework/src/publication/file_system.rs`
* Automated evidence:
  * `published_runtime_pair_exposes_documented_accessors`
  * `staged_temp_layout_round_trip_blob_under_windows_runner`
  * `retry_pause_window_for_real_publication_is_at_least_a_few_milliseconds`
  * `publication_primitive_reachable_from_cross_platform_integration_target`
* Required environment: `linux-x86_64` (compile-pass only), `windows-x86_64` (real evidence)
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First native Windows runner publish round-trip that proves bounded retry under real ERROR_SHARING_VIOLATION.

## 15. MZA release construction (current.md §11) — blocked

### mza-01-pin-upstream-source

* Contract/spec authority: `current.md#11`
* Implementation:
  * `.gitmodules`
  * `automation/build_bundle_mza/mza/` (MZA submodule)
* Automated evidence: `.gitmodules` declares the submodule; `git submodule status --recursive` runs without dirty suffixes (per §16).
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `not implemented`
* Open gap: MZA does not publish a Protocol 1 installer API that owns install/upgrade/uninstall/command registration/platform integration. The audit itself records this as an exogenous blocker in `current.md` §3.

### mza-02-real-installer-entrypoint

* Contract/spec authority: `current.md#11`
* Implementation:
  * `lexicon-bundle/build.rs` (empty adapter path)
  * `lexicon-bundle/src/main.rs` (`MzaBundleInput` include adapter)
  * `lexicon-bundle/Cargo.toml` (build-deps `serde`, `toml`)
* Automated evidence: `cargo check` succeeds against the empty adapter; adapter slices in once a non-empty TOML is provided through `MZA_BUNDLE_INPUTS`.
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `not implemented`
* Open gap: Same upstream MZA installer API blocker recorded against `mza-01`.

### release-01-locked-noninteractive-build

* Contract/spec authority: `current.md#11`
* Implementation: `automation/build_bundle_mza/build_release.sh`
* Automated evidence: shell script refuses to run while `<accepted-mza-sha>` is a placeholder.
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `not implemented`
* Open gap: `<accepted-mza-sha>` placeholder; depends on accepted MZA release commit.

### release-02-obsolete-orchestration-deleted

* Contract/spec authority: `current.md#11`
* Implementation:
  * `automation/build_bundle_install/` removed
  * `lexicon-install.toml` removed
  * `README.md` retargeted to `build_release.sh`
  * `containerization/test-container/{entrypoint.sh,README.md}` point at `build_release.sh`
  * `containerization/lexicon-container/{Containerfile,README.md}` marked inert pending accepted MZA
  * Member `Cargo.lock` files removed; root `Cargo.lock` is the workspace lockfile
  * `.cargo/config.toml` provides `vendored-sources` shim
* Automated evidence: directory listings and shell guards in `build_release.sh` confirm removal.
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `implemented, unverified`
* Open gap: First CI run that observes the cleanup commit through the conformance workflow.

## 16. Master completion criteria (current.md §18) — blocked

### final-18-conformance

* Contract/spec authority: `current.md#18`
* Implementation: this status document and `workspace/specs/conformance.toml`.
* Automated evidence: the conformance workflow `.github/workflows/conformance.yml` runs the `conformance-final` gate.
* Required environment: `linux-x86_64`, `windows-x86_64`
* Durable evidence: `none`
* Status: `not implemented`
* Open gap: Items 1..29 of §18 remain until every prerequisite passes — including Gate 6 (MZA upstream API) and durable green CI evidence.

---

## Explicitly Deferred Items (per `current.md` §16)

These are deferred by contract, not by stagnation:

1. `specs-46-core-owned-task-queue`: deferred per `workspace/specs/specs.md#46`
   in favor of the source-owned SQLite model.
2. `specs-2-protocols-beyond-http`: deferred per `workspace/specs/specs.md#2`;
   HTTP is the initial supported protocol.
3. `specs-40-project-wide-publication-transaction`: deferred per
   `workspace/specs/specs.md#40`; per-pair runtime publication is
   completed and tested, but a project-wide transaction across every
   source/protocol pairing is out of scope for Contract V1.

No row above claims `Status: conformant`. Conformant status requires
durable evidence tied to an exact commit, and per `current.md` §17
"a number in current.md without the attached exact-SHA manifest is not
evidence." A row reaches `conformant` only after `.github/workflows/conformance.yml`
publishes a green manifest URL and that URL replaces `durable_evidence = none`
in this same commit.
