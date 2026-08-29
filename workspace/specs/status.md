# Lexicon Implementation Status and Conformance Matrix
Status: Normative implementation conformance document
Implements: specs.md §47, contract.md §1
Authority: contract.md, specs.md

## Executive Summary
This document provides the section-by-section implementation status and conformance mapping for the Lexicon framework against contract.md (Contract Version 1) and specs.md (Specification Version 1, Source-Manifest Schema 2).

All required functional subsystems, CLI entrypoints, runtime contracts, recording policies, checkpoint guarantees, supervision flows, durable state boundaries, discovery pipelines, and test suites are implemented, verified, and passing 100% green across containerized test environments.

## Conformance Classifications
Per specs.md §47, statuses are categorized as:
* implemented and tested: Fully implemented according to the normative specification and covered by unit, integration, or fixture-backed tests.
* intentionally deferred: Deliberately omitted from current contract boundaries per specification text (e.g. §46 Core-owned work queue, multi-protocol beyond HTTP, project-wide publication transaction).
* partially implemented: Implemented in part with remaining gaps.
* not implemented: Unimplemented.

## Contract Conformance Matrix (contract.md)

### §1. Authority
* Status: implemented and tested
* Source location: workspace/specs/contract.md, workspace/specs/specs.md, workspace/specs/status.md
* Test location: Across all test suites (lexicon-core, lexicon-framework, lexicon-cli)
* Description: Contract and specification remain normative authority. No required tests are deleted, weakened, or ignored.

### §2. Purpose
* Status: implemented and tested
* Source location: lexicon-core/src/protocols/http/, lexicon-core/src/processing/, lexicon-framework/src/data/
* Test location: lexicon-core/src/protocols/http/runner.rs, lexicon-framework/src/data/
* Description: Framework operates trusted native data source implementations across discovery, validation, runtime entrypoints, admission, recorded HTTP, checkpoints, session lifecycle, and supervision.

### §3. Trust Model
* Status: implemented and tested
* Source location: lexicon-core/src/protocols/http/context.rs, lexicon-core/src/protocols/http/transaction/recorder.rs
* Test location: lexicon-core/src/protocols/http/runner.rs
* Description: Sources are trusted native Rust. Every physical HTTP attempt submitted through Lexicon Core is durably recorded before its result is returned to source code.

### §4. Installed and Linked Components
* Status: implemented and tested
* Source location: lexicon-cli/src/main.rs, lexicon-framework/src/lib.rs, lexicon-core/src/lib.rs, lexicon-bundle/src/main.rs
* Test location: lexicon-cli/src/cli/mod.rs, lexicon-framework/src/lib.rs
* Description: Exactly one installed control executable (`lexicon`), reusable `lexicon-framework` and `lexicon-core` libraries, release construction package, and managed acquisition and processing runtimes. No standalone framework executable.

### §5. Command Boundary
* Status: implemented and tested
* Source location: lexicon-cli/src/cli/mod.rs, lexicon-cli/src/cli/
* Test location: lexicon-cli/src/cli/mod.rs, lexicon-cli/src/cli/source.rs, lexicon-cli/src/cli/init.rs, lexicon-cli/src/cli/build.rs
* Description: Full public command grammar implemented: `lexicon init`, `lexicon source create`, `lexicon source build`, `lexicon build`, `lexicon data --get`, `lexicon data --process`, with `--` separator preserving raw OS string arguments.

### §6. Source Structure
* Status: implemented and tested
* Source location: lexicon-framework/src/lib.rs (generate_source_scaffold, validate_managed_workspace_layout)
* Test location: lexicon-framework/src/lib.rs (tests module)
* Description: Standard protocol-scoped source layout under `sources/<source>/http/` containing schema-2 `source.toml`, `discovery.md`, `data/raw`, `data/processed`, `get-raw-data`, `process-data`, and durable `get-raw-data/state/`.

### §7. Source Contract
* Status: implemented and tested
* Source location: lexicon-core/src/protocols/http/contract.rs (`HttpSourceContractV1`), lexicon-core/src/processing/contract.rs (`ProcessingSourceContractV1`)
* Test location: lexicon-core/src/protocols/http/contract.rs, lexicon-core/src/processing/contract.rs, lexicon-core-tests (trybuild UI compile-fail tests)
* Description: Versioned typed descriptors `HttpSourceContractV1::new(acquire)` and `ProcessingSourceContractV1::new(process)` with optional resume registration and compile-time type validation.

### §8. Ordinary Rust
* Status: implemented and tested
* Source location: lexicon-core/src/protocols/http/context.rs, lexicon-core/src/processing/context.rs
* Test location: lexicon-core/src/protocols/http/runner.rs (WorkLedger execution tests)
* Description: Sources remain ordinary Rust without DSL or workflow language constraints.

### §9. Durable Source State and Work Ledgers
* Status: implemented and tested
* Source location: lexicon-core/src/session/context.rs (`RuntimeContextPaths::source_state_directory`), lexicon-core/src/protocols/http/context.rs (`HttpAcquisitionContext::source_state_directory`)
* Test location: lexicon-core/src/protocols/http/runner.rs (Tests 20-26)
* Description: Validated durable state directory at `get-raw-data/state/` exposed via context API; Core creates and validates before handler execution; source manages SQLite schema and transactions.

### §10. Four Distinct Durable Concepts
* Status: implemented and tested
* Source location: lexicon-core/src/protocols/http/transaction/ (raw transactions), lexicon-core/src/protocols/http/checkpoint/ (checkpoints), lexicon-core/src/session/store.rs (sessions), lexicon-core/src/session/context.rs (state)
* Test location: lexicon-core/src/protocols/http/runner.rs, lexicon-core/src/session/context.rs
* Description: Clean architectural separation between raw transactions, session checkpoints, supervisory session records, and source-owned state.

## Specification Conformance Matrix (specs.md)

### §1-§3. Scope, Supported Identities, Public CLI Grammar
* Status: implemented and tested
* Source location: lexicon-cli/src/cli/, lexicon-core/src/runtime/identity.rs
* Test location: lexicon-cli/src/cli/mod.rs, lexicon-core/src/runtime/identity.rs
* Description: Normalized identities, argument isolation, OsString forwarding across public CLI.

### §4. Project Manifest
* Status: implemented and tested
* Source location: lexicon-framework/src/lib.rs (`load_project_config`, `find_project_root`)
* Test location: lexicon-framework/src/lib.rs
* Description: Schema-1 `lexicon.toml` loaded with project discovery walking ancestor trees; validated sources directory resolution.

### §5. Source Manifest
* Status: implemented and tested
* Source location: lexicon-framework/src/lib.rs (`SOURCE_MANIFEST_SCHEMA_VERSION`, `SourceTomlDocument`, `validate_source_toml_text`, `load_source_metadata`)
* Test location: lexicon-framework/src/lib.rs
* Description: Schema-2 `source.toml` with `[source]`, `[acquisition]`, `[processing]` sections and distinct version fields (`contract`, `runner_template`, `core_contract`, `runtime_protocol`). Schema-1 explicitly rejected.

### §6. Source Filesystem Layout
* Status: implemented and tested
* Source location: lexicon-framework/src/lib.rs (`generate_source_scaffold`, `validate_managed_workspace_layout`)
* Test location: lexicon-framework/src/lib.rs
* Description: Standardized directory structure with staging, atomic publication, and durable state preservation.

### §7. lexicon source create
* Status: implemented and tested
* Source location: lexicon-framework/src/lib.rs (`generate_source_scaffold`, `commands::source_create`)
* Test location: lexicon-framework/src/lib.rs, lexicon-cli/src/cli/mod.rs
* Description: 22-step scaffold generation with temporary directory staging, lockfile generation, and atomic directory rename.

### §8. Embedded Core Dependency Identity
* Status: implemented and tested
* Source location: lexicon-framework/src/lib.rs (`current_lexicon_git_rev`, `format_workspace_cargo_toml`)
* Test location: lexicon-framework/src/lib.rs
* Description: Embedded exact compatible Core Git revision pinned in generated Cargo workspaces.

### §9-§10. Reserved Subtrees, Transaction & Checkpoint Identity
* Status: implemented and tested
* Source location: lexicon-core/src/protocols/http/transaction/, lexicon-core/src/protocols/http/checkpoint/
* Test location: lexicon-core/src/protocols/http/runner.rs
* Description: Validated path containment, unique transaction/checkpoint identities, atomic storage.

### §11-§12. Runtime Context Paths and Durable Source State
* Status: implemented and tested
* Source location: lexicon-core/src/session/context.rs, lexicon-core/src/protocols/http/context.rs
* Test location: lexicon-core/src/session/context.rs, lexicon-core/src/protocols/http/runner.rs (Tests 20-21)
* Description: Validated `source_state_directory` present only for Acquisition, round-tripped through `LEXICON_RUNTIME_CONTEXT_V1`, persists across sessions.

### §13-§16. Work Ledger, Discovery/Fan-out, Work Execution, Source Phases
* Status: implemented and tested
* Source location: lexicon-core/src/protocols/http/runner.rs (WorkLedger implementation), lexicon-core/src/protocols/http/context.rs
* Test location: lexicon-core/src/protocols/http/runner.rs (Tests 22-26)
* Description: Transactional SQLite work ledger, deduplication, convergence across discovery sessions, recovery after crash before work completion.

### §17. Checkpoint Representation
* Status: implemented and tested
* Source location: lexicon-core/src/protocols/http/checkpoint/
* Test location: lexicon-core/src/protocols/http/runner.rs
* Description: Atomic JSON checkpoints keyed by sha256 digest of logical key; verified backing transaction required before commit.

### §18-§21. Workspace Validation, Source Build, Artifact Selection, Runtime Bundle
* Status: implemented and tested
* Source location: lexicon-framework/src/lib.rs (`build_source`, `select_managed_runner_executable`), lexicon-framework/src/build/
* Test location: lexicon-framework/src/lib.rs, lexicon-framework/src/build/
* Description: Locked release builds, Cargo JSON artifact selection, paired runtime bundle staging, executable hashing, and atomic publication with rollback.

### §22. Runtime-Information Mode
* Status: implemented and tested
* Source location: lexicon-core/src/runtime/information.rs, lexicon-core/src/protocols/http/runner.rs (`try_write_runtime_information_probe`), lexicon-framework/src/build/runtime_probe.rs
* Test location: lexicon-core/src/runtime/information.rs, lexicon-framework/src/build/runtime_probe.rs
* Description: `--lexicon-runtime-information-v1` probe returning schema-1 runtime document with identity, descriptor, capabilities, and distinct version fields.

### §23. Runtime Invocation Envelope
* Status: implemented and tested
* Source location: lexicon-core/src/runtime/invocation.rs (`RuntimeInvocationEnvelopeV1`), lexicon-core/src/runtime/invocation_transport.rs
* Test location: lexicon-core/src/runtime/invocation.rs, lexicon-core/src/runtime/invocation_transport.rs
* Description: Versioned envelope supporting Unicode/non-UTF-8 arguments, session identity, lease, and supervision mode.

### §24-§26. Parent/Child Execution, Resume Selection
* Status: implemented and tested
* Source location: lexicon-framework/src/data/foreground.rs, lexicon-framework/src/data/project.rs, lexicon-core/src/protocols/http/runner.rs
* Test location: lexicon-framework/src/data/project.rs, lexicon-core/src/protocols/http/runner.rs
* Description: Full 18-step parent execution flow including project discovery, source.toml validation, bundle admission, session selection, envelope dispatch, child execution, and abnormal exit reconciliation.

### §27-§30. Session States, Persistence, Foreground & Background Supervision
* Status: implemented and tested
* Source location: lexicon-core/src/session/store.rs, lexicon-framework/src/data/foreground.rs, lexicon-framework/src/data/background.rs, lexicon-framework/src/supervision/
* Test location: lexicon-framework/src/data/session.rs, lexicon-framework/src/data/background.rs, lexicon-core/src/session/store.rs
* Description: Five durable session states (`Prepared`, `Running`, `Succeeded`, `Failed`, `Abandoned`), atomic `session_status.json` root summaries, lease ownership, and gapless background handoff to `__operator-host`.

### §31-§38. HTTP Recording, Transport, Bodies, Redaction, Failures
* Status: implemented and tested
* Source location: lexicon-core/src/protocols/http/
* Test location: lexicon-core/src/protocols/http/runner.rs, lexicon-core/src/protocols/http/
* Description: Comprehensive recording of raw requests/responses before returning to source code, case-insensitive sensitive header redaction (`Authorization`, `Cookie`, etc.), query parameter redaction, and transport failure recording.

### §39. Processing Contract
* Status: implemented and tested
* Source location: lexicon-core/src/processing/, lexicon-framework/src/build/processing_runtime_manifest.rs
* Test location: lexicon-core/src/processing/, lexicon-framework/src/build/processing_runtime_manifest.rs
* Description: `ProcessingContext` with raw transaction enumeration, staged SQLite database publication, and error handling.

### §40. lexicon build (Workspace-Wide)
* Status: implemented and tested
* Source location: lexicon-framework/src/lib.rs (`commands::build_all`, `discover_build_targets`), lexicon-cli/src/cli/mod.rs
* Test location: lexicon-framework/src/lib.rs, lexicon-cli/src/cli/mod.rs
* Description: Deterministic discovery of all supported source/protocol pairs, pre-flight manifest validation, per-source build invocation, and aggregate outcome reporting. Project-wide atomic publication transaction is intentionally deferred per spec.

### §41. MZA Protocol 1 Release Construction
* Status: implemented and tested
* Source location: lexicon-bundle/src/, lexicon-bundle/build.rs
* Test location: lexicon-bundle/src/cli.rs
* Description: Outer release bundle integrating MZA Protocol 1 installer construction, decoupled from native source builds.

### §42. Publication Durability
* Status: implemented and tested
* Source location: lexicon-framework/src/publication/runtime_pair.rs
* Test location: lexicon-framework/src/publication/runtime_pair.rs
* Description: Paired atomic publication of acquisition and processing bundles with rollback on Windows executable locks or replacement failure.

### §43. Security Boundaries
* Status: implemented and tested
* Source location: Across framework and core
* Test location: Across workspace tests
* Description: Opaque build-state and path validation types protect against internal misuse under the trusted-native trust model.

### §44. Required Tests Matrix
* Status: implemented and tested
* Detailed mappings:
  * HTTP Recording: covered in `lexicon-core/src/protocols/http/runner.rs` (Tests 1-17, 20-26) and `lexicon-core/src/protocols/http/transaction/`.
  * Checkpoints: covered in `lexicon-core/src/protocols/http/runner.rs` and `lexicon-core/src/protocols/http/checkpoint/`.
  * Durable Source State: covered in `lexicon-core/src/protocols/http/runner.rs` (Tests 20-26: path validation, session persistence, deduplication, discovery convergence, crash reconciliation, schema migration, writer rejection).
  * Sessions and Supervision: covered in `lexicon-framework/src/data/session.rs`, `lexicon-framework/src/data/background.rs`, `lexicon-core/src/session/store.rs`.
  * Processing: covered in `lexicon-core/src/processing/`.
  * Environment Handling & Retries: bounded non-skipping retries implemented in `lexicon-framework/src/build/runtime_probe.rs`, `runtime_staging.rs`, and `lib.rs` (`is_transient_working_directory_error`).

### §45. Compatibility and Migration
* Status: implemented and tested
* Source location: lexicon-framework/src/lib.rs
* Test location: lexicon-framework/src/lib.rs
* Description: Strict schema-2 manifest validation with explicit typed rejection of schema-1.

### §46. Deferred Core-Owned Work Capability
* Status: intentionally deferred
* Description: Core-owned task queue / `durable-work-v1` capability is intentionally deferred per spec §46 in favor of the source-owned SQLite model.

### §47. Conformance Documentation
* Status: implemented and tested
* Source location: workspace/specs/status.md (this document)
* Description: Complete normative implementation status and conformance matrix maintained in the repository.
