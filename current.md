Current milestone: restore truthful contract and specification conformance

Status

The repository is not complete against:

workspace/specs/contract.md
workspace/specs/specs.md

The implementation baseline is:

9893631c4a995923f78d3d4b37359481d0eaa4ea

The documentation-only head is:

a8b80f1ce396abdada33b9b87a9d5b603b31fc16

The completion claims in current.md and workspace/specs/status.md are not authoritative evidence of conformance. This milestone must correct the implementation and then regenerate those documents from verified evidence.

Objective

Close the currently demonstrated conformance gaps:

1. Installed source creation depends on the original Lexicon Git checkout.
2. Lexicon reimplements MZA installer behavior instead of consuming MZA Protocol 1.
3. The runtime-information argument does not match the normative specification.
4. The required-test matrix claims coverage that has not been mapped to concrete behavioral tests.
5. The conformance documentation describes unverified requirements as implemented and tested.

Do not declare the repository complete merely because the existing workspace test suite passes.

Required implementation

1. Embed the Core dependency identity at Lexicon build time

Remove the production runtime behavior implemented through:

fn current_lexicon_git_rev() -> Result<String, String>

The installed Lexicon executable and its linked framework must not:

inspect env!("CARGO_MANIFEST_DIR") to locate the original repository
run git rev-parse during lexicon source create
require Git to be installed for source creation
require the build-machine checkout to remain present
generate an unpinned Core dependency

Resolve the immutable Core dependency identity while building Lexicon and embed it in the resulting program.

Acceptable identities include:

an exact Git commit
an exact package version with integrity information
another reproducible immutable dependency reference

Local-development builds may obtain the identity from the repository during compilation. That resolution must occur in build-time machinery, not when the installed executable handles lexicon source create.

Production scaffold generation must consume only the embedded identity.

Add an end-to-end test that:

1. builds the lexicon executable;
2. copies it outside the repository;
3. makes the original checkout unavailable;
4. makes git unavailable at execution time;
5. invokes lexicon init;
6. invokes lexicon source create;
7. verifies successful source creation;
8. verifies both generated operation workspaces use the exact embedded Core identity;
9. verifies both generated lockfiles resolve that identity.

A test that invokes scaffold formatting from inside the repository is insufficient.

2. Implement the actual MZA Protocol 1 boundary

Remove Lexicon-owned substitutes for MZA installer behavior.

The current bundle must not independently own:

archive extraction policy
application installation
uninstallation
PATH modification
Windows registry modification
installation records
installer destination behavior

Inspect and pin the selected MZA Protocol 1 revision. Implement the adapter using the actual types and entrypoints exported by that dependency.

The final dependency graph must contain the selected MZA Protocol 1 package. Lexicon must not copy or approximate its protocol internally.

Where MZA exposes generated Rust through MZA_BUNDLE_INPUTS, consume it using the protocol-defined form:

include!(env!("MZA_BUNDLE_INPUTS"));

Do not reinterpret MZA_BUNDLE_INPUTS as a Lexicon-owned input specification and generate a substitute mza_bundle_inputs.rs.

Remove dependencies and modules used solely by the replaced Lexicon installer implementation, including applicable portions of:

tar
xz2
winreg
windows-sys
lexicon-bundle/src/install.rs
lexicon-bundle/src/envpath.rs
Lexicon-owned installation-record handling

Retain only the thin Lexicon-to-MZA adapter and Lexicon-specific bundle declarations permitted by Protocol 1.

Verification must demonstrate:

source builds do not invoke MZA
complete-product release construction does invoke MZA
the resulting bundle contains the lexicon executable
installation behavior is executed by MZA
no standalone lexicon-framework executable is installed
Lexicon contains no second installer implementation

Do not weaken specs.md §41 to legitimize the existing bundle.

3. Correct the runtime-information mode

The normative runtime-information argument is:

--lexicon-runtime-info

Replace the current canonical value:

--lexicon-runtime-information-v1

Update consistently across:

lexicon-core
lexicon-framework
managed runner generation
runtime probing
runtime admission
HTTP runtime tests
processing runtime tests
fixture executables
status documentation

The exact normative argument must:

1. be handled before source context construction;
2. avoid invoking source code;
3. emit only the machine-readable runtime-information document and its terminating newline;
4. obey the probe timeout and output-size limits;
5. work for both acquisition and processing runtimes.

A temporary compatibility alias may be retained only if it is clearly private, tested, and does not replace the normative argument.

4. Establish a literal §44 test-conformance matrix

Audit every required test listed in specs.md §44 individually.

For every requirement, record:

requirement
exact test name
exact test location
environment
what behavior is exercised
status

Do not map an entire test category to a source file without naming the tests.

Do not count:

type-construction tests as runtime behavior tests
serialization round trips as process-supervision tests
test-only work-ledger logic as proof of unrelated HTTP behavior
a passing workspace suite as proof that an absent test exists
documentation as a test location

At minimum, verify that concrete behavioral tests exist for every required HTTP case:

one GET
POST request-body byte preservation
compressed response byte preservation before decoding
redirect-chain recording
independently recorded retry attempts
connection failure
truncated response with preserved partial evidence
request metadata
response metadata
mandatory case-insensitive header redaction
sensitive-query redaction
record-before-return

Each HTTP test must exercise the real Core request, transport, recording, and admission path. Testing helper functions in isolation is not sufficient where §44 requires end-to-end behavior.

Also map and, where missing, implement the complete required sets for:

source-contract compile checks
scaffold and validation
build and paired publication
checkpoints
durable source state
sessions and supervision
processing
environment handling

5. Verify platform-specific supervision and publication honestly

The Linux container result cannot prove Windows behavior.

Run platform-appropriate verification for requirements that are explicitly platform-specific, including:

Windows Unicode argument preservation
Windows child termination and reconciliation
Windows executable-lock publication rollback
Unix foreground interruption and signal handling
background operator-host acknowledgement
continuous lease ownership during background handoff
operator-host terminal reconciliation

If a required platform test cannot run in the current environment, record it as unverified. Do not mark it implemented and tested based only on compilation, mocking, or a Linux result.

Unsupported-environment handling must follow specs.md §44:

a skipped test is reported as skipped
the reason is specific
the invariant is tested in a supported environment
the skip cannot be reported as a successful assertion

6. Correct conformance documentation

Rewrite:

workspace/specs/status.md
current.md

Only after implementation and verification are complete.

Until then, remove or replace claims including:

100% full conformance
all requirements implemented and tested
ready for production operation
complete against contract.md and specs.md

The new status.md must contain, for each requirement:

contract or specification requirement
implementation location
exact test location and test name
verification environment
conformance status
known gap
planned milestone where incomplete

Use only these statuses:

implemented and tested
implemented but insufficiently tested
partially implemented
not implemented
intentionally deferred

“Intentionally deferred” is valid only where the contract or specification explicitly permits deferral. It must not be used for a mandatory requirement.

Documentation must describe the repository as it exists, not the intended architecture.

Required verification

Run the complete Linux workspace baseline:

podman build \
    -f containerization/test-container/Containerfile \
    -t lexicon-local-test-image \
    .
podman run \
    -d \
    --name lexicon-local-test \
    -v "$PWD":/lexicon \
    --workdir /lexicon \
    lexicon-local-test-image
podman exec lexicon-local-test \
    bash -lc 'cd /lexicon && cargo check --workspace'
podman exec lexicon-local-test \
    bash -lc 'cd /lexicon && cargo test --workspace --quiet'

Additionally run:

installed-executable source-create test outside the original checkout
actual MZA Protocol 1 bundle construction
actual bundle installation verification
actual Unix interruption/supervision tests
actual Windows argument, supervision, and publication tests

Record exact commands, platforms, target triples, commit SHA, test counts, failures, and skipped tests.

A green Linux cargo test --workspace result alone does not complete this milestone.

Scope constraints

Do not introduce:

a Core-owned job queue
durable-work-v1
multiple acquisition workers
new source phases
a workflow language
new protocols
untrusted-source sandboxing
project-wide all-or-nothing build publication
new public CLI commands
unrelated refactoring

Preserve the selected architecture:

one installed lexicon operator executable
reusable lexicon-framework library
narrow lexicon-core library
managed native runners
source-owned durable SQLite work state
Core-owned HTTP effects, recording, checkpoints, sessions, and supervision
MZA-owned release bundling and installation

Do not change contract.md or specs.md merely to make the current implementation conform.

Completion criteria

This milestone is complete only when:

1. lexicon source create works from an installed executable without Git or the original checkout.
2. Generated workspaces use an embedded immutable Core dependency identity.
3. Lexicon consumes the actual pinned MZA Protocol 1 implementation.
4. Lexicon no longer owns duplicate installer or PATH-management behavior.
5. --lexicon-runtime-info is the canonical working runtime probe.
6. Every mandatory §44 test has an exact test mapping.
7. Missing mandatory behavioral tests have been implemented.
8. Platform-specific requirements have been tested on their supported platforms.
9. No required test succeeds through an early return or disguised skip.
10. cargo check --workspace passes.
11. cargo test --workspace --quiet passes.
12. The installed-executable and MZA release workflows pass.
13. status.md accurately distinguishes verified, insufficiently tested, partial, missing, and deferred work.
14. current.md no longer makes claims broader than the collected evidence.

Completion report

When the milestone is complete, replace this file with a concise evidence report containing:

exact commit tested
exact Linux and Windows environments
cargo check result
cargo test result and test counts
installed source-create test result
embedded Core identity and how it was produced
selected MZA Protocol 1 revision
actual MZA bundle and installation result
runtime-information argument verification
complete §44 mapping
all skipped or environment-limited tests
remaining contract or specification gaps

If any mandatory requirement remains incomplete or unverified, state that plainly and produce the next bounded milestone.

Do not declare full conformance until the evidence matrix contains no mandatory requirement classified as:

implemented but insufficiently tested
partially implemented
not implemented

Then stop.