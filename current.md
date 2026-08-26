Current implementation request: verified processing runtime candidate

Objective

Combine executable hashing and the bounded processing runtime-information probe into one framework-level verification operation.

The operation must prove that:

1. the processing candidate is a hashable regular file;
2. it reports compatible processing runtime information;
3. its bytes remain unchanged across the probe.

Do not add processing manifests, staging, publication, or source build integration yet.

Required module

Extend:

lexicon-framework/src/build/runtime_verification.rs

Export the processing API through:

lexicon-framework/src/build/mod.rs

Reuse:

hash_runtime_executable(...)
probe_processing_runtime_information(...)

Do not duplicate hashing or probe behavior.

Shared private verification orchestration

Refactor acquisition and processing verification to use one private hash/probe/hash orchestration helper where practical.

The shared helper owns:

1. initial hashing;
2. operation-specific probe invocation;
3. final hashing;
4. comparison of path, size, and SHA-256.

Operation-specific public types and errors may remain distinct.

Preserve the existing acquisition public API and behavior.

Verified processing type

Define:

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedProcessingRuntime {
    artifact: HashedRuntimeArtifact,
    information: AdmittedProcessingRuntimeInformation,
}

Provide:

impl VerifiedProcessingRuntime {
    pub fn artifact(
        &self,
    ) -> &HashedRuntimeArtifact;
    pub fn information(
        &self,
    ) -> &ProcessingRuntimeInformationV1;
    pub fn admitted_information(
        &self,
    ) -> &AdmittedProcessingRuntimeInformation;
}

The admitted-wrapper accessor is optional if information() is provided.

Do not provide a public unchecked constructor.

Public API

Provide:

pub fn verify_processing_runtime_candidate(
    executable: &Path,
    expected_identity: RuntimeIdentity,
) -> Result<
    VerifiedProcessingRuntime,
    ProcessingRuntimeVerificationError,
>;

Required verification order

Perform exactly:

1. Initial hash:

hash_runtime_executable(executable)

2. Processing probe:

probe_processing_runtime_information(
    executable,
    expected_identity,
)

3. Final hash of the same path.
4. Compare the initial and final artifact values.
5. Construct VerifiedProcessingRuntime only if they agree.

Compare:

* artifact path;
* byte size;
* SHA-256 digest.

Do not rely only on file metadata.

Changed-during-probe behavior

If both hashes succeed but differ, return:

ProcessingRuntimeVerificationError::
    ArtifactChangedDuringProbe {
        before: HashedRuntimeArtifact,
        after: HashedRuntimeArtifact,
    }

Do not return admitted processing information.

Do not retry silently.

Typed error

Define:

#[derive(Debug)]
pub enum ProcessingRuntimeVerificationError {
    InitialHash(
        RuntimeArtifactHashError,
    ),
    Probe(
        ProcessingRuntimeProbeExecutionError,
    ),
    FinalHash(
        RuntimeArtifactHashError,
    ),
    ArtifactChangedDuringProbe {
        before: HashedRuntimeArtifact,
        after: HashedRuntimeArtifact,
    },
}

Implement:

std::fmt::Display
std::error::Error

Expose nested sources where applicable.

Do not return plain String, print diagnostics, or exit.

Failure semantics

Initial hash failure

* Return InitialHash.
* Do not execute the processing candidate.

Probe failure

* Return Probe.
* Do not construct a verified result.
* A final hash is not required after failed admission.

Final hash failure

* Return FinalHash.
* Discard admitted information.
* Do not construct a verified result.

Artifact disagreement

* Return ArtifactChangedDuringProbe.
* Preserve both artifact values.

No filesystem mutation

Successful verification must not:

* copy the executable;
* rename it;
* change permissions;
* write a manifest;
* create a staging directory;
* publish anything.

Deterministic test seam

Reuse or generalize the private orchestration seam used by acquisition verification so tests can deterministically inject:

* initial hash failure;
* probe failure;
* final hash failure;
* changed bytes;
* same-size changed bytes.

Do not expose dependency injection publicly.

Required tests

Add tests proving:

1. A stable processing candidate produces VerifiedProcessingRuntime.
2. The verified path matches the supplied path.
3. The verified size matches the candidate.
4. The verified SHA-256 matches the candidate.
5. Processing identity matches the expected identity.
6. Initial hash failure prevents probe execution.
7. A missing candidate returns InitialHash.
8. Probe spawn failure returns Probe.
9. Probe timeout returns Probe.
10. Probe nonzero exit returns Probe.
11. Malformed processing output returns Probe.
12. Incompatible processing identity returns Probe.
13. Acquisition information from the candidate returns Probe.
14. Final hash failure returns FinalHash.
15. Changed bytes return ArtifactChangedDuringProbe.
16. Same-size changed bytes are detected through SHA-256.
17. Both before and after artifacts remain inspectable.
18. No verified result is created on failure.
19. Successful verification does not modify the candidate.
20. Acquisition verification behavior remains unchanged.
21. Acquisition and processing use shared private orchestration where practical.
22. Existing hashing and probe tests remain unchanged.
23. Parallel tests use isolated candidate paths.
24. All workspace tests pass repeatedly.

Preserve existing behavior

Do not change:

* Core processing runtime-information behavior;
* acquisition verification public API;
* processing probe behavior;
* shared subprocess limits;
* hashing behavior;
* acquisition manifests;
* staging;
* bundle admission;
* reversible publication;
* source scaffolding;
* source create;
* source build;
* Cargo invocation;
* legacy publication;
* CLI behavior;
* MZA;
* Protocol 1;
* lexicon-bundle;
* installer behavior;
* bundle inputs;
* installed paths.

lexicon-bundle remains a binary installer built through cargo-bundler-v0.1.0.

Validation

Run:

cargo test -p lexicon-framework --quiet

Run the workspace suite twice:

cargo test --workspace --quiet
cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* processing runtime manifests;
* processing staging;
* processing bundle admission;
* paired publication;
* source build integration;
* processing runner main.rs;
* processing execution;
* SQLite behavior;
* raw-data discovery;
* sessions;
* source workspace migration;
* managed acquisition runners;
* runner::run;
* invocation envelopes;
* HTTP execution;
* raw recording;
* supervision;
* __operator-host.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* processing verification API;
* opaque verified processing type;
* shared private orchestration;
* exact hash/probe/hash order;
* typed verification errors;
* successful processing verification;
* initial-hash, probe, final-hash, and mutation failures;
* same-size mutation detection;
* acquisition regression results;
* confirmation that no manifest, staging, or publication occurred;
* repeated workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not add processing manifests or staging.