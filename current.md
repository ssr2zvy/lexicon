Current implementation milestone: HTTP cross-platform publication and partial-failure ownership closure

Objective

Correct the remaining HTTP transaction-engine defects at commit:

de08286e80276db8570f3d207b497bd3f463755f

The previous closure implemented most of the required transaction typing and durability work. However, the current source still has several blocking gaps:

* finalized HTTP transaction publication is unsupported on Windows;
* progress partial-commit handling discards the finalized transaction owner;
* incomplete-response recovery discards body-sync failures and stringifies typed failures;
* some redirect paths clone finalized transactions to satisfy closure ownership;
* Reqwest configuration and TLS classification retain string-based internal errors;
* transaction admission and managed test source still require closure.

This milestone completes the HTTP engine across Lexicon’s supported operating systems.

Do not begin checkpoints, processing, background supervision, or lexicon build.

Contract authority

Follow:

workspace/specs/contract.md

The relevant guarantees remain:

* each physical HTTP exchange receives one raw transaction;
* finalized transactions are immutable;
* complete transactions are durably published before source code receives them;
* incomplete transactions remain recognizable;
* managed metadata is redacted;
* retries and redirects retain every physical exchange;
* Windows and Linux remain supported target platforms;
* source code receives recorded outcomes rather than live transport responses.

Repository-grounded corrections

Complete every correction below.

1. Windows transaction publication is currently unsupported

The current publication implementation provides specialized paths for:

Linux
macOS

and routes every other platform—including Windows—to:

HttpRecorderError::UnsupportedPlatformPublication

Windows is a supported Lexicon target. A normal HTTP acquisition cannot remain structurally unsupported there.

Required correction

Implement atomic no-replace directory publication on Windows.

Required semantics:

move staging directory to final transaction path
only when the final path does not already exist

Use a Windows API with no-replace behavior.

A suitable implementation may use the Windows filesystem API directly through a narrowly scoped dependency or FFI boundary.

Requirements:

* source path and destination path use native Windows path encoding;
* no lossy UTF-8 path conversion;
* destination collision fails without replacing the existing directory;
* successful move is atomic at the supported filesystem boundary;
* staging remains present when publication fails before the move;
* existing final transaction remains untouched on collision;
* successful publication is followed by the applicable parent-directory durability operation;
* ordinary Windows errors remain available through typed nested causes.

Add:

#[cfg(target_os = "windows")]

for the Windows implementation.

The generic unsupported-platform branch must exclude Windows after this correction.

Do not use copy-then-delete as a substitute for atomic publication.

2. Platform publication behavior must have one typed abstraction

The platform-specific functions currently return broad recorder variants directly.

Required correction

Create one private publication boundary equivalent to:

fn publish_transaction_directory_no_replace(
    staging: &Path,
    final_path: &Path,
) -> Result<(), HttpTransactionPublicationError>;

Platform implementations must map into:

pub enum HttpTransactionPublicationError {
    Collision,
    Io(std::io::Error),
    UnsupportedPlatform,
}

Equivalent organization is acceptable.

HttpRecorderError should wrap this typed publication error rather than duplicate:

* atomic-finalize I/O;
* collision;
* unsupported-platform variants.

Use Error::source().

Do not use an exists() check as the publication guarantee. It may remain only as a non-authoritative early diagnostic.

3. Linux no-replace behavior must use the named constant

The Linux implementation currently passes:

1u32

as the renameat2 flag.

Required correction

Use the platform’s named RENAME_NOREPLACE constant when available, or define one narrowly with an explanatory compatibility boundary if the dependency does not expose it.

Do not leave an unexplained numeric flag in correctness-critical filesystem code.

Distinguish:

* destination collision;
* unsupported syscall/kernel behavior;
* other filesystem I/O failure.

If renameat2 is unavailable on the running Linux kernel or filesystem, do not silently fall back to overwrite-capable rename.

Return a typed unsupported-no-replace error.

4. macOS no-replace behavior must remain native-path safe

Review the current renamex_np(..., RENAME_EXCL) path and preserve exact native Unix path bytes.

Required correction

Ensure:

* interior-NUL rejection is typed distinctly from unsupported platform behavior;
* destination collision maps to Collision;
* other errors retain std::io::Error;
* existing destination is never replaced;
* staging is preserved on failure.

Do not treat malformed path encoding as an unsupported operating system.

5. Post-publication parent synchronization must be platform-aware

The current implementation synchronizes directories with:

File::open(path)?.sync_all()

That model is not necessarily portable to Windows directory handles.

Required correction

Define one platform-aware managed-directory synchronization boundary.

Required behavior:

* Linux and macOS synchronize the directory containing the renamed entry;
* Windows uses the strongest supported durable-directory or volume/file-handle operation available through the selected API;
* if Windows cannot provide an exactly equivalent directory sync, document the precise guarantee actually supplied;
* do not claim an operation occurred when it did not;
* a failure after successful publication is represented as a post-publication partial commit;
* the finalized transaction is preserved.

Do not downgrade every Windows execution to failure merely because Unix directory opening semantics differ.

6. Progress partial-commit mapping discards the finalized owner

persist_progress(...) correctly returns an error containing:

(FinalizedRecordedAttempt, ProgressPersistenceError)

but the caller immediately maps it with behavior equivalent to:

.map_err(|(_, error)| ...)

The finalized attempt is discarded.

The nested error retains an identity and path, but not the complete typed recorded transaction.

Required correction

Define a typed partial-commit error equivalent to:

pub struct HttpProgressPartialCommit {
    finalized: FinalizedRecordedAttempt,
    source: AcquisitionProgressError,
}

Equivalent public/private organization is acceptable.

Expose read-only access to:

* finalized RecordedTransaction;
* transaction identity;
* final transaction path;
* attempt identity;
* recorded outcome;
* progress failure.

Use it through:

HttpExecutionError::ProgressPartialCommit(...)

Do not discard the finalized owner in map_err.

A caller receiving this error must be able to establish that:

* the physical exchange was recorded;
* the transaction was finalized;
* progress publication did not complete.

Do not incorrectly expose the transaction as ordinary successful execution.

7. Progress error hierarchy duplicates partial-commit state

The current progress error embeds transaction identity and path, while the caller separately carries FinalizedRecordedAttempt.

Required correction

Separate:

progress operation failure

from:

finalized transaction + progress operation failure

Use:

AcquisitionProgressError

for the underlying progress failure.

Use:

HttpProgressPartialCommit

for the combined state.

Do not duplicate transaction identity and path as unrelated strings when the typed finalized transaction already owns them.

The combined error’s source() must return the progress error.

8. Incomplete-response body synchronization errors are discarded

During response streaming failures, the current code performs behavior equivalent to:

let _ = file.sync_all();

This discards a durability-relevant error before writing the incomplete-response marker.

Required correction

On a response-stream read, write, length-overflow, or final-sync failure, retain separately:

* primary streaming failure;
* partial-body sync result;
* incomplete-marker persistence result.

Define a typed failure equivalent to:

pub struct IncompleteHttpResponseFailure {
    stream_error: HttpBodyStreamingError,
    partial_body_sync_error: Option<std::io::Error>,
    marker_error: Option<HttpIncompleteMarkerError>,
    bytes_recorded: u64,
    partial_body_sha256: Option<String>,
}

Equivalent organization is acceptable.

No relevant error may be discarded.

If the body sync fails but the marker succeeds, return both facts.

If the marker fails, preserve the body-sync and stream errors as well.

The .partial-* directory must remain recognizable in every case.

9. Incomplete-response marker stringifies typed clock and path failures

The marker helper currently converts typed failures using behavior equivalent to:

std::io::Error::other(error.to_string())

This destroys typed provenance.

Required correction

Create a typed marker error covering:

* clock acquisition;
* metadata encoding;
* managed-path validation;
* temporary-file creation;
* metadata write;
* metadata file sync;
* atomic marker publication;
* response-directory sync.

Implement Display, Error, and source().

Do not convert:

HttpClockError
HttpRecorderError
serde_json::Error

into arbitrary strings.

Do not route managed-path validation through io::Error::other.

10. Metadata persistence stringifies managed-path failures

write_json_bytes_atomic(...) currently converts managed-path errors to:

std::io::Error::other(error.to_string())

Required correction

Make the metadata persistence helper return a typed error.

Equivalent structure:

pub enum HttpMetadataPersistenceError {
    ManagedPath(HttpManagedPathError),
    TemporaryFile(std::io::Error),
    Write(std::io::Error),
    FileSync(std::io::Error),
    Persist(std::io::Error),
    DirectorySync(std::io::Error),
}

Do not stringify the managed-path cause.

Use this typed helper for:

* request metadata;
* complete response metadata;
* transport-failure metadata;
* incomplete-response metadata.

11. Managed-path validation belongs in its own error type

The shared validator currently returns HttpRecorderError, coupling general managed-path validation to the recorder.

The context, progress writer, and transaction admission layer then remap it broadly.

Required correction

Extract:

HttpManagedPathError

or an equivalent shared type.

It must distinguish:

* relative path;
* component inspection failure;
* symlink;
* non-directory ancestor;
* wrong target type;
* path outside expected root.

The validator must receive both:

trusted_root
target_path

and prove descendant containment.

Do not validate only that the target is absolute.

Do not let context.rs collapse every non-symlink path failure into InvalidPaths.

Preserve the typed managed-path cause through:

* session validation;
* recorder;
* progress persistence;
* transaction admission.

12. Missing ancestor handling is too permissive

The current component walker returns success as soon as an intermediate component does not exist.

That is acceptable only when the caller explicitly intends to create the remaining descendant path beneath a verified existing parent.

Required correction

Provide distinct validation modes:

ExistingDirectory
ExistingRegularFile
CreatableDirectory
CreatableRegularFile

Equivalent naming is acceptable.

For a creatable target:

* find the deepest existing ancestor;
* validate every existing component;
* require the deepest existing ancestor to be a directory;
* ensure the uncreated suffix contains only normal path components;
* reject .., root replacement, or platform prefixes in the suffix;
* verify containment under the trusted root.

For an existing target:

* missing components are an error.

Do not use one ambiguous RegularFileIfPresent mode for every boundary.

13. Redirect error construction still clones finalized transactions

The missing-Location and invalid-target closures currently clone the transaction to satisfy closure ownership.

Required correction

Restructure redirect handling so the transaction is moved exactly once into either:

* the next redirect state;
* a typed redirect failure;
* successful return.

Do not clone RecordedTransaction merely to satisfy ok_or_else or map_err.

Prefer explicit match statements where ownership is clearer.

If RecordedTransaction: Clone has no deliberate public use, remove Clone.

A finalized transaction should represent an owned durable artifact, not an implicitly duplicated execution-state token.

14. Recorded outcome is cloned before orchestration

The context currently clones:

transaction.response().outcome()

Required correction

Branch by reference or through an outcome-inspection method that returns a small copyable discriminator.

For example:

pub enum HttpRecordedOutcomeKind {
    Response,
    TransportFailure,
}

Then borrow the typed response or failure as required.

Do not clone the complete outcome merely to decide control flow.

The transaction must remain available for final movement into success or error.

15. Reqwest configuration failure still stores a string

HttpTransportConfigurationError currently stores:

String

derived from:

reqwest::Error::to_string()

Its Error::source() exposes nothing.

Required correction

Retain the actual typed client-construction cause.

Because the context needs to retain the initialization result, use a bounded owned representation such as:

Arc<reqwest::Error>

or redesign managed context construction to return the typed failure immediately.

Do not store raw Reqwest diagnostic text.

The public Display remains sanitized.

Use source() where the underlying type can safely be exposed as a nested cause.

If exposing Reqwest’s Display through generic error-chain rendering could reveal sensitive information, wrap it in a private typed cause and keep the public diagnostic sanitized.

16. TLS classification parses arbitrary error text

The transport currently detects TLS failure through behavior equivalent to:

error.to_string().to_ascii_lowercase().contains("tls")

This is unstable and may inspect strings containing request information.

Required correction

Do not classify errors by searching their display text.

Use typed Reqwest error properties and typed source-chain inspection where available.

If the selected client cannot distinguish TLS from connection failure through stable typed APIs:

* classify it as Connect or ExchangeIo;
* do not claim a distinct TLS classification.

Remove HttpTransportFailure::Tls if Core cannot assign it reliably.

Stored failure classes must describe what Core can actually prove.

17. StoredHttpVersion::Unknown weakens strict admission

The current HTTP version representation includes:

Unknown

The prior closure required a stable identifier and preferred rejection of unsupported values.

Required correction

Do not serialize an unstable or unproven version as ordinary Unknown.

Use one of:

1. Persist None when the transport cannot prove a supported version; or
2. Return a typed unsupported-version recording error.

Known values remain:

http_0_9
http_1_0
http_1_1
http_2
http_3

Strict admission must reject unknown serialized identifiers.

Do not use a catch-all enum variant that accepts future values as if they were understood.

18. Transaction admission must validate directory-name identity

admit_transaction_from_disk(...) validates metadata and body integrity, but the final directory name is part of the raw-data contract:

<timestamp>-<transaction-id>

Required correction

Parse and validate the final directory name.

Require:

* no .partial- prefix;
* one valid timestamp component;
* one canonical transaction identity suffix;
* metadata transaction identity matches the directory identity;
* request and response metadata agree;
* directory is an immediate child of the trusted raw-data root;
* no extra nesting or traversal;
* directory is not a symlink.

Do not admit a transaction merely because valid metadata was copied into an arbitrary directory.

19. Transaction admission must reject unexpected managed entries

A finalized transaction has an exact managed shape.

Required correction

Admission must require:

request/
request/metadata.json
request/body when has_body is true
response/
response/metadata.json
response/body

Reject:

* missing required entries;
* unexpected top-level managed files;
* unexpected files inside request or response;
* nested directories inside request or response;
* symlinks;
* non-regular body or metadata files;
* incomplete-response outcome in a finalized directory.

If source-authored auxiliary files are ever desired, that must be a future explicit schema change.

Do not silently ignore unexpected entries in a finalized Core-owned transaction directory.

20. Transaction admission must validate timestamps and attempt relationships

Required correction

Validate:

* creation timestamp is nonzero;
* response/failure timestamp is nonzero;
* response/failure timestamp is not earlier than request creation;
* physical attempt index starts at one;
* initial redirect and retry indices start at zero;
* retry index is compatible with physical attempt index;
* parent identity is absent for the first physical attempt;
* parent identity differs from the current identity;
* redirect/retry indices do not exceed the physical attempt index.

Do not infer complete chain validity without scanning other transactions. Validate only invariants provable from one transaction.

21. Progress validation incorrectly validates revision against itself

The current load path calls behavior equivalent to:

validate_existing(
    &parsed,
    session_id,
    parsed.revision,
)

This makes exact revision validation tautological.

Required correction

Remove the self-comparison parameter.

A loaded progress document must validate its own internal invariants.

The update operation must then:

1. retain the exact loaded revision as the expected prior revision;
2. produce exactly revision + 1;
3. persist one complete replacement;
4. confirm the produced revision matches the checked increment.

Do not claim optimistic revision validation by comparing a field to itself.

If no competing writer can exist after lease validation, document that the exact single-writer rule—not a tautological parameter—provides concurrency ownership.

22. Progress persistence does not prevent replacement races

Atomic replacement prevents torn files, but another writer could replace the document between load and persist.

The supervisor lease is the intended single-writer authority.

Required correction

Immediately before progress replacement, revalidate:

* the same session remains Running;
* the supervisor lease remains owned;
* the progress document on disk still has the revision loaded earlier.

If the revision changed:

* preserve the finalized transaction;
* return a typed progress revision conflict;
* do not overwrite the newer progress document.

Do not introduce a global lock.

Do not acquire the supervisor lease in the child.

23. In-memory progress ownership must remain intact on failure

The corrected progress partial-commit type must retain the finalized transaction regardless of whether failure occurs during:

* session revalidation;
* path validation;
* progress load;
* progress decoding;
* progress invariant validation;
* clock acquisition;
* progress advance;
* revision recheck;
* temporary persistence;
* replacement;
* directory sync.

No map_err may discard the finalized attempt.

24. Existing test source is not aligned with the current context API

Some existing test source in the HTTP contract module still constructs HttpAcquisitionContext through obsolete public fields such as:

source_directory
raw_data_directory

Those fields no longer exist.

Required correction

Adjust existing test source only for production API alignment.

Use supported private test constructors or session-path fixtures.

Do not restore obsolete public fields.

Do not weaken context privacy merely so old test code remains syntactically convenient.

Do not run the tests.

25. Remove stale or duplicate error variants

The newer source defines standalone errors such as:

HttpTransactionPublicationError
HttpTransactionIdentityAllocationError

while recorder errors still duplicate some of their variants.

Required correction

Make each error concept have one authoritative typed representation.

At minimum consolidate:

* publication collision;
* publication I/O;
* unsupported platform;
* identity allocation exhaustion;
* clock failure;
* managed-path validation.

Do not retain unused parallel error types solely because they were introduced in the previous milestone.

Update re-exports to expose only supported public errors.

Final HTTP ownership sequence

After this milestone, the authoritative path must be:

validated managed session
→ finalized request
→ exclusively allocated staging transaction
→ physical exchange
→ complete or recognizable partial recording
→ atomic no-replace publication on Linux, macOS, and Windows
→ platform-appropriate parent durability step
→ FinalizedRecordedAttempt
→ session/lease/progress revision revalidation
→ atomic progress replacement
→ progress-directory durability step
→ ProgressPublishedRecordedAttempt
→ source-visible RecordedTransaction

Every failure after transaction publication must retain the finalized transaction owner.

Typed error hierarchy

Use a coherent hierarchy equivalent to:

HttpManagedPathError
HttpClockError
HttpBodyStreamingError
HttpIncompleteResponseError
HttpMetadataPersistenceError
HttpTransactionIdentityAllocationError
HttpTransactionPublicationError
HttpRecorderError
AcquisitionProgressError
HttpProgressPartialCommit
HttpTransportConfigurationError
HttpTransportFailure
HttpRetryExhaustionError
HttpRedirectFailure
HttpExecutionError
AcquisitionError

Equivalent nesting is acceptable.

All errors must implement:

std::fmt::Display
std::error::Error

Use source().

Do not stringify typed errors inside the Core-owned HTTP engine.

Sensitive diagnostics

Do not reveal:

* request URLs;
* redirect targets;
* headers;
* cookies;
* authorization values;
* request or response bodies;
* source arguments;
* environment-variable names or values;
* runtime-context JSON;
* invocation-envelope JSON;
* arbitrary source errors;
* raw Reqwest diagnostic strings.

Safe diagnostics may identify:

* transaction identity;
* attempt indices;
* stable transport class;
* HTTP status;
* retry or redirect counts;
* managed path category;
* supported platform operation.

Preserve existing behavior

Do not change:

* source handler signatures;
* acquisition/resume registration;
* request builder semantics;
* redaction rules;
* raw body fidelity;
* retry policy semantics;
* redirect method/body rules;
* invocation-envelope JSON;
* argv transport;
* HTTP admission;
* processing admission;
* source arguments;
* session lifecycle states;
* supervisor lease ownership;
* foreground execution;
* foreground reconciliation;
* runtime-information probes;
* capability identifiers;
* managed runner layout;
* source creation;
* source build;
* verification;
* staging;
* bundle admission;
* paired publication;
* CLI syntax;
* MZA;
* Protocol 1;
* installer behavior.

Keep:

HttpCapabilitySet::empty()

Do not advertise ClientCertificateV1.

Source-level acceptance requirements

Correct the source so that:

1. Linux publication is atomic and no-replace.
2. macOS publication is atomic and no-replace.
3. Windows publication is atomic and no-replace.
4. Windows paths use native encoding.
5. Publication collision never overwrites an existing transaction.
6. Publication errors use one authoritative type.
7. Parent durability behavior is accurate per platform.
8. Post-publication durability failure preserves the transaction.
9. Progress partial commit owns the finalized transaction.
10. Progress failures never discard finalized ownership.
11. Partial-body sync failures are retained.
12. Incomplete-marker failures remain typed.
13. Managed-path errors are never stringified.
14. Metadata persistence errors remain typed.
15. Managed paths prove containment beneath a trusted root.
16. Creatable and existing path validation are distinct.
17. Redirect failures do not require transaction cloning.
18. Outcome inspection does not clone the recorded outcome.
19. Reqwest configuration errors are not stored as strings.
20. TLS classification does not inspect error display text.
21. Unsupported HTTP versions are not persisted as understood values.
22. Transaction directory identity is strictly admitted.
23. Unexpected transaction entries are rejected.
24. Transaction timestamps and attempt invariants are validated.
25. Progress revision validation is not self-referential.
26. Progress replacement detects revision conflicts.
27. Session and lease are revalidated before replacement.
28. Existing HTTP test source matches the private context API.
29. Duplicate and stale error variants are removed.
30. No failure after transaction publication loses the finalized transaction.

Command-execution constraint

This is a source-only milestone.

Do not run:

cargo test
cargo check
cargo build
cargo fmt
cargo clippy
cargo metadata
rustc

Do not execute:

* Lexicon CLI commands;
* generated runners;
* HTTP servers;
* real or test HTTP requests;
* workspace validation;
* bundle/install automation.

Existing test source may be adjusted only for API alignment.

Do not add or execute the broad validation matrix.

Explicit exclusions

Do not implement:

* checkpoints;
* checkpoint persistence;
* checkpoint recovery;
* latest-response lookup;
* processing transaction discovery;
* processing SQLite behavior;
* client certificates;
* proxy configuration;
* decoded response readers;
* background operator host;
* signal forwarding;
* background supervision;
* lexicon build;
* automatic build-before-run;
* source migration;
* cross-compilation;
* MZA changes;
* installer changes.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* final publication abstraction;
* Linux no-replace implementation;
* macOS no-replace implementation;
* Windows no-replace implementation;
* native Windows path handling;
* per-platform parent durability behavior;
* publication partial-commit behavior;
* progress partial-commit ownership representation;
* confirmation that every progress failure retains the transaction;
* partial-response body-sync behavior;
* incomplete-marker error hierarchy;
* metadata persistence error hierarchy;
* managed-path containment behavior;
* existing versus creatable path validation;
* redirect transaction ownership behavior;
* recorded-outcome borrowing behavior;
* typed Reqwest configuration failure behavior;
* transport classification behavior without string inspection;
* stable HTTP-version behavior;
* transaction directory-name admission;
* exact finalized transaction shape admission;
* timestamp and attempt-invariant admission;
* progress revision semantics;
* progress replacement conflict behavior;
* session/lease revalidation before replacement;
* existing test-source API alignment;
* duplicate error types removed;
* final HTTP success guarantee;
* final HTTP partial-commit guarantee;
* capability-set result;
* confirmation that checkpoints, processing, background supervision, and build behavior were not added;
* confirmation that no tests, checks, builds, formatting, linting, metadata commands, CLI execution, runtime execution, HTTP execution, workspace validation, or bundle/install pipeline were run.

Then stop.

Do not begin checkpoints until the HTTP engine supports durable publication and failure ownership on every supported platform.