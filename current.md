Current implementation request: verified HTTP runtime candidate

Objective

Combine the completed executable hashing and bounded runtime-information probe primitives into one framework-level candidate verification operation.

The operation must prove that:

1. the candidate is a hashable regular executable artifact;
2. the candidate successfully reports compatible runtime information;
3. the candidate’s bytes remain unchanged across the probe.

Do not connect this operation to source build, staging, publication, or runtime.json yet.

Required module

Create or extend:

lexicon-framework/src/build/runtime_verification.rs

Export the public API through:

lexicon-framework/src/build/mod.rs

Reuse:

hash_runtime_executable(...)
probe_http_runtime_information(...)

Do not duplicate their hashing, subprocess, decoding, or compatibility logic.

Verified result type

Define an opaque result:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedHttpRuntime {
    artifact: HashedRuntimeArtifact,
    information: AdmittedRuntimeInformation,
}

Provide:

impl VerifiedHttpRuntime {
    pub fn artifact(&self) -> &HashedRuntimeArtifact;
    pub fn information(&self) -> &RuntimeInformationV1;
}

An accessor returning the admitted wrapper instead is also acceptable:

pub fn admitted_information(
    &self,
) -> &AdmittedRuntimeInformation;

Do not provide a public unchecked constructor.

A VerifiedHttpRuntime value must only exist after the complete verification sequence succeeds.

Public verification API

Provide:

pub fn verify_http_runtime_candidate(
    executable: &Path,
    expected_identity: RuntimeIdentity,
) -> Result<
    VerifiedHttpRuntime,
    HttpRuntimeVerificationError,
>;

Required verification sequence

Perform these operations in this exact order:

1. Hash the candidate using:

hash_runtime_executable(executable)

2. Run and admit its bounded information probe using:

probe_http_runtime_information(
    executable,
    expected_identity,
)

3. Hash the same candidate path again.
4. Compare the pre-probe and post-probe artifact results.
5. Return VerifiedHttpRuntime only if they agree.

The comparison must include at least:

* artifact path;
* byte size;
* SHA-256 digest.

Why the second hash is required

The first hash identifies the bytes selected for verification.

The probe executes the file after that hash. The second hash confirms that the candidate at the selected path did not change during the probe interval.

The verified result must represent the agreed pre-probe and post-probe artifact.

Do not merely trust unchanged metadata.

Changed-during-probe behavior

If both hashes succeed but their path, size, or SHA-256 values differ, return:

HttpRuntimeVerificationError::ArtifactChangedDuringProbe {
    before: HashedRuntimeArtifact,
    after: HashedRuntimeArtifact,
}

Do not return admitted runtime information when the artifact changed.

Do not silently retry verification in this step. A caller may decide whether an explicit future operation should retry.

Typed error

Define:

#[derive(Debug)]
pub enum HttpRuntimeVerificationError {
    InitialHash(RuntimeArtifactHashError),
    Probe(RuntimeProbeExecutionError),
    FinalHash(RuntimeArtifactHashError),
    ArtifactChangedDuringProbe {
        before: HashedRuntimeArtifact,
        after: HashedRuntimeArtifact,
    },
}

Equivalent naming is acceptable, but callers must distinguish:

* failure before execution;
* probe execution or admission failure;
* failure hashing after execution;
* successful hashes that disagree.

Implement:

std::fmt::Display
std::error::Error

Expose nested errors through source() where applicable.

Do not return String, print diagnostics, or exit.

Failure semantics

Initial hashing failure

If the first hash fails:

* do not execute the candidate;
* return InitialHash.

Probe failure

If the probe fails:

* return Probe;
* do not construct a verified result.

A final hash is not required after a failed probe in this micro-step because no artifact will be admitted.

Final hashing failure

If the probe succeeds but the second hash fails:

* return FinalHash;
* discard the admitted information;
* do not construct a verified result.

Changed artifact

If both hashes succeed but differ:

* return ArtifactChangedDuringProbe;
* preserve both typed artifact values in the error.

No executable copying

This operation verifies the executable at its existing candidate path.

It must not:

* copy it;
* rename it;
* stage it;
* publish it;
* modify permissions;
* write metadata beside it.

Staging and publication belong to later build steps.

Test seam

If needed for deterministic mutation tests, implement a private generic orchestration helper accepting injected hash and probe functions.

The public function must always call the real hashing and probe implementations.

Do not expose dependency injection as public API.

Required tests

Add tests proving:

1. A stable valid candidate produces VerifiedHttpRuntime.
2. The verified artifact path matches the supplied path.
3. The verified size matches the candidate.
4. The verified SHA-256 matches the candidate bytes.
5. The verified runtime information matches the expected identity.
6. Required and available capabilities remain accessible.
7. Initial hash failure prevents probe execution.
8. A missing candidate returns InitialHash.
9. Probe spawn failure returns Probe.
10. Probe timeout returns Probe.
11. Probe nonzero exit returns Probe.
12. Probe malformed output returns Probe.
13. Probe incompatibility returns Probe.
14. Final hash failure returns FinalHash.
15. Different pre-probe and post-probe bytes return ArtifactChangedDuringProbe.
16. A same-size content change is detected through SHA-256.
17. Both before and after artifacts remain inspectable in the changed error.
18. No verified result is created on any failure.
19. The candidate is not copied, renamed, or modified by successful verification.
20. Existing hashing tests remain unchanged.
21. Existing probe execution and admission tests remain unchanged.
22. All workspace tests pass.

Use a deterministic injected orchestration seam for final-hash and mutation cases if modifying an executing fixture is unreliable across supported operating systems.

Preserve existing behavior

Do not change:

* Core runtime-information behavior;
* probe argument or JSON schema;
* probe timeout and output limits;
* hashing algorithm or symlink policy;
* source scaffolding;
* source implementation crates;
* source create;
* source build;
* Cargo invocation;
* Cargo JSON artifact selection;
* runtime publication;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-framework remains library-only.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-framework --quiet

Run:

cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If the known external MZA dependency remains unavailable, report it separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* integration with source build;
* Cargo build-plan changes;
* Cargo artifact-selection changes;
* runtime.json;
* runtime bundle directories;
* staging;
* publication changes;
* rollback changes;
* managed-runner generation;
* runner main.rs;
* runner::run;
* source workspace migration;
* invocation envelopes;
* acquisition or resume execution;
* HTTP transport;
* raw recording;
* sessions;
* supervision;
* __operator-host;
* processing-runtime verification.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the public verification API;
* the opaque verified-runtime representation;
* exact verification order;
* initial and final hash behavior;
* probe delegation behavior;
* changed-during-probe detection;
* typed error representation;
* deterministic mutation-test arrangement;
* successful verification results;
* each failure result;
* confirmation that no staging or publication occurred;
* framework and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not integrate candidate verification into source build.