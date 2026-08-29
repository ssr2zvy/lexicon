Current milestone: make background supervision transfer continuous and failure-atomic

Baseline

This milestone applies to repository commit:

8512a2f258383b06877c1929f71228954f3112f4

or a direct descendant containing only work required by this milestone.

The Unified Operator Host topology is structurally implemented, but the current background handoff does not satisfy the continuous-ownership guarantee in workspace/specs/specs.md §30.

Objective

Make:

lexicon data --get <source> --protocol http --bg
lexicon data --process <source> --protocol http --bg

transfer supervision from the initiating Lexicon process to the spawned lexicon __operator-host without:

* an unowned session interval;
* ambiguous ownership acknowledgement;
* a concurrent acquisition race;
* an unreconciled session on failure;
* an operator-host process surviving a failed handoff without authority or supervision.

This milestone changes background supervision mechanics only. It does not introduce a Core-owned job queue.

Current defect

The initiating process currently releases its session lease before spawning the operator host:

initiator releases lease
→ session is temporarily unowned
→ operator host is spawned
→ operator host attempts to acquire lease

The initiating process then considers the handoff successful when the session lease appears generally owned.

This is insufficient because:

1. another invocation can acquire the lease during the gap;
2. ownership is not proven to belong to the spawned operator host;
3. there is no handoff-specific nonce or successor identity;
4. spawn failure, early exit, timeout, and inspection failure do not consistently reconcile the prepared session;
5. dropping the child handle does not terminate or reap a running operator host.

Required implementation

1. Introduce a handoff identity

Every background launch must create an unpredictable, single-use handoff identity.

It must be bound to:

* the session identity;
* the expected operator-host invocation;
* the initiating supervisor;
* one handoff attempt.

An unrelated process that acquires or observes the session lease must not be able to satisfy the acknowledgement.

Do not expose the handoff identity as an ordinary source argument.

2. Preserve continuous authority

Implement one explicit continuous-ownership mechanism allowed by the specification, such as:

* inherited lease authority;
* an atomic durable successor transfer;
* an acknowledgement protocol in which the initiator retains authority until the identified operator host is ready;
* another mechanism with equivalent proof.

There must be no externally observable state in which the session is simply unowned and available to an unrelated invocation.

The implementation must work under the project’s supported Unix and Windows process and locking models. Platform-specific internals are acceptable behind one documented invariant.

3. Require identity-bound acknowledgement

The initiating process may report a successful background start only after it has proof that the particular spawned operator host accepted durable supervision for the expected session.

The following is not sufficient:

the lease currently appears owned

The acknowledgement must establish:

expected session
+ expected handoff identity
+ expected operator-host instance
+ durable supervisory authority

Acknowledgement must be bounded by a timeout.

4. Make failures terminal and deterministic

Handle every failure point explicitly:

* operator-host spawn failure;
* operator host exits before acknowledgement;
* acknowledgement timeout;
* malformed or mismatched acknowledgement;
* lease-state inspection failure;
* session persistence failure;
* ownership-transfer failure.

For each failure:

1. the initiating process must retain or recover sufficient authority to reconcile the session;
2. the session must end in the appropriate durable terminal state;
3. a spawned process that lacks acknowledged authority must be terminated and reaped;
4. no child may continue unsupervised after the initiating command reports handoff failure;
5. the returned CLI error must identify the handoff stage that failed.

Do not mark the session successful merely because a process disappeared.

5. Preserve public execution semantics

The public interface remains:

[--bg]

--bg selects supervision mode for one top-level acquisition or processing invocation.

Source-owned work items stored under:

get-raw-data/state/

inherit the mode of that invocation. They do not receive individual background flags, and Lexicon does not become their scheduler.

Do not introduce:

* a Core job-queue schema;
* per-work-item operator hosts;
* automatic background execution for every source work item;
* framework interpretation of source phases or source work payloads.

Required tests

Add real multiprocess tests proving:

1. the actual hidden __operator-host path is executed;
2. the initiator does not release authority before the successor is ready;
3. acknowledgement is tied to the spawned operator host;
4. an unrelated lease holder cannot satisfy acknowledgement;
5. a concurrent Lexicon invocation cannot steal the session during handoff;
6. successful background start is reported only after durable acknowledgement;
7. spawn failure produces a terminal reconciled session;
8. early operator-host exit produces a terminal reconciled session;
9. acknowledgement timeout terminates and reaps the spawned process;
10. malformed or mismatched handoff identities are rejected;
11. successful operator-host execution performs terminal reconciliation when the source child exits;
12. acquisition and processing use the same supervisory invariant.

A fake executor that merely changes the lease state to Owned is not sufficient evidence.

Where Unix and Windows require different mechanisms, each implementation must receive native platform coverage. Unsupported environments must be reported as skipped by the surrounding workflow, never as passing through an early test return.

Documentation corrections

Update workspace/specs/status.md so that:

* background handoff is not marked conformant until the new tests pass;
* every “implemented and tested” row names an existing test;
* prose descriptions and production functions are not listed as tests;
* the tested commit is recorded exactly;
* local or temporary logs are not presented as durable repository evidence.

Do not describe the entire repository as contract-complete when this milestone passes.

Verification

Run from a clean checkout of the exact commit being reported:

cargo check --workspace
cargo test --workspace --quiet

Also run the repository’s containerized verification workflow.

Required native platform testing must cover every platform-specific ownership-transfer implementation.

The completion report must record:

* exact commit SHA;
* exact commands;
* pass, failure, and skip counts;
* operating system and architecture;
* test names establishing each handoff invariant;
* the selected continuous-ownership mechanism;
* how failed operator-host processes are terminated and reaped.

Scope exclusions

Do not include in this milestone:

* foreground signal forwarding;
* Core-revision identity changes;
* MZA integration;
* HTTP recording changes;
* source-state or work-ledger changes;
* a Core-owned job queue;
* dependency-audit infrastructure;
* installer behavior;
* unrelated refactoring.

Completion criteria

This milestone is complete only when:

1. no session-ownership gap exists during background handoff;
2. acknowledgement identifies the expected operator host;
3. unrelated ownership cannot produce false success;
4. every handoff failure durably reconciles the session;
5. failed or timed-out operator hosts cannot remain running unsupervised;
6. real multiprocess tests cover success, contention, early exit, timeout, and malformed acknowledgement;
7. acquisition and processing share the same proven invariant;
8. workspace verification passes on the exact committed state;
9. conformance documentation describes only demonstrated behavior.

Following milestone

After this milestone passes, replace current.md.

The next milestone should make the embedded Core dependency identity fail closed and prove, through an installed-CLI black-box test, that both generated source workspaces resolve and build against the exact declared Core revision without access to the original checkout or Git executable.