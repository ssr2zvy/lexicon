Current implementation request: bounded processing runtime-information subprocess probe

Objective

Add the framework function that directly executes a candidate processing runtime in Core’s reserved information-probe mode, captures bounded stdout and stderr, and passes successful stdout through:

admit_processing_runtime_information_probe(...)

Acquisition and processing must share the same private subprocess transport implementation.

Do not add processing executable verification or build integration yet.

Required module

Extend:

lexicon-framework/src/build/runtime_probe.rs

Export the processing API through:

lexicon-framework/src/build/mod.rs

lexicon-framework remains library-only.

Public processing API

Provide:

pub fn probe_processing_runtime_information(
    executable: &Path,
    expected_identity: RuntimeIdentity,
) -> Result<
    AdmittedProcessingRuntimeInformation,
    ProcessingRuntimeProbeExecutionError,
>;

The public operation uses the existing:

RUNTIME_INFORMATION_PROBE_TIMEOUT
MAX_RUNTIME_INFORMATION_PROBE_BYTES
MAX_RUNTIME_INFORMATION_PROBE_STDERR_BYTES

Do not introduce different processing limits.

Shared private subprocess transport

Refactor the existing acquisition executor so both operations use one private transport function.

Conceptually:

fn execute_runtime_information_probe(
    executable: &Path,
    timeout: Duration,
) -> Result<
    CapturedRuntimeProbe,
    RuntimeProbeTransportError,
>;

A successful private result should contain at least:

struct CapturedRuntimeProbe {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

The transport function owns:

* direct process spawning;
* the shared reserved argument;
* null stdin;
* concurrent stdout/stderr draining;
* bounded retained buffers;
* overflow detection;
* timeout enforcement;
* termination;
* reaping;
* exit-status validation.

Operation-specific admission remains outside this helper.

Preserve acquisition API

The existing:

probe_http_runtime_information(...)

must continue returning its established acquisition-specific result and error type.

Internally it should now:

1. call the shared transport;
2. map transport errors into its existing execution error;
3. call acquisition admission.

Do not break existing acquisition callers.

Exact child invocation

Both operations launch:

<candidate-executable> --lexicon-runtime-information-v1

Use:

lexicon_core::runtime::
    RUNTIME_INFORMATION_PROBE_ARGUMENT

Do not duplicate the literal.

Configure:

stdin  = Stdio::null()
stdout = Stdio::piped()
stderr = Stdio::piped()

Do not use a shell or Cargo.

Processing execution sequence

The processing public function must:

1. execute the shared bounded transport;
2. require successful child exit;
3. obtain captured stdout;
4. call:

admit_processing_runtime_information_probe(
    expected_identity,
    &stdout,
)

5. return the admitted processing information.

A successful process with invalid or incompatible processing information is still an error.

Typed processing execution error

Define:

#[derive(Debug)]
pub enum ProcessingRuntimeProbeExecutionError {
    Spawn {
        source: std::io::Error,
    },
    Wait {
        source: std::io::Error,
    },
    Timeout {
        timeout: Duration,
        cleanup_error: Option<String>,
    },
    StdoutRead {
        source: std::io::Error,
    },
    StderrRead {
        source: std::io::Error,
    },
    StdoutTooLarge {
        maximum: usize,
    },
    StderrTooLarge {
        maximum: usize,
    },
    UnsuccessfulExit {
        status: ExitStatus,
        stderr: Vec<u8>,
        stderr_truncated: bool,
    },
    Admission(
        ProcessingRuntimeProbeAdmissionError,
    ),
}

Equivalent organization is acceptable.

The processing error must distinguish:

* transport failures;
* timeout;
* output overflow;
* unsuccessful exit;
* processing admission failure.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String, print child output, or exit.

Error precedence

Preserve the existing deterministic transport precedence:

1. timeout;
2. wait or cleanup failure;
3. stdout read failure;
4. stderr read failure;
5. stdout overflow;
6. stderr overflow;
7. unsuccessful exit;
8. operation-specific admission failure.

Acquisition and processing must use the same transport precedence.

Timeout and cleanup

Every successfully spawned child must be reaped before return.

On timeout:

1. attempt to kill the child;
2. wait for it;
3. finish draining both streams;
4. join the drainers;
5. return the timeout error.

Do not leave a known processing probe child running.

Bounded output

Stdout and stderr must be drained concurrently.

After reaching a retention limit:

* stop retaining additional bytes;
* continue draining and discarding;
* record overflow;
* avoid pipe deadlocks;
* keep retained buffers within their limits.

Do not copy the acquisition capture implementation into a second processing implementation.

Test fixture behavior

Extend the existing test-only runtime probe fixture to emit processing probe documents in deterministic modes, including:

* valid processing information;
* malformed processing JSON;
* incompatible processing identity;
* descriptor-version disagreement;
* acquisition information from a processing candidate;
* nonzero exit;
* delayed exit;
* oversized stdout;
* oversized stderr;
* simultaneous noisy streams.

The fixture must not become an installed executable, MZA artifact, or bundle input.

Avoid shared mutable fixture files that can cause file-busy or parallel-test races. Give mutating fixtures unique per-test paths.

Required tests

Add tests proving:

1. A valid processing fixture is executed and admitted.
2. The child receives exactly the shared reserved probe argument.
3. No extra source arguments reach the child.
4. Processing identity survives the process boundary.
5. A missing executable returns Spawn.
6. Malformed processing JSON returns Admission.
7. Incompatible processing identity returns Admission.
8. Descriptor-version disagreement returns Admission.
9. Acquisition runtime information returns processing Admission.
10. A nonzero exit returns UnsuccessfulExit.
11. Valid stdout with a nonzero exit is rejected.
12. Bounded stderr is retained for unsuccessful exit.
13. Successful exit with bounded stderr may still be admitted.
14. A delayed child returns Timeout.
15. The timed-out child is terminated and reaped.
16. Oversized stdout returns StdoutTooLarge.
17. Oversized stderr returns StderrTooLarge.
18. Simultaneous noisy stdout and stderr do not deadlock.
19. Retained buffers never exceed their limits.
20. Processing admission does not invoke the processing handler.
21. Acquisition probe execution continues to pass unchanged.
22. Acquisition and processing share the same private transport.
23. Existing pure admission tests remain unchanged.
24. Parallel probe tests do not share mutable fixture paths.
25. All workspace tests pass repeatedly.

Use an internal timeout-accepting helper for short timeout tests.

Preserve existing behavior

Do not change:

* Core processing probe behavior;
* processing probe-output admission;
* acquisition probe public APIs;
* acquisition execution errors;
* shared probe argument;
* timeout and output limits;
* hashing;
* acquisition verification;
* manifests;
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

Run the workspace tests at least twice to catch fixture races:

cargo test --workspace --quiet
cargo test --workspace --quiet

If the external MZA checkout is available, run:

bash automation/build_bundle_install/build_bundle_install.sh

If unavailable, report the known external blocker separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* processing executable hash/probe/hash verification;
* processing manifests;
* processing staging;
* processing bundle admission;
* paired publication;
* processing runner main.rs;
* processing execution;
* SQLite behavior;
* raw-data discovery;
* processing sessions;
* source workspace migration;
* managed acquisition runners;
* runner::run;
* invocation envelopes;
* HTTP execution;
* raw recording;
* supervision;
* __operator-host;
* source build integration.

Completion report

After completion, replace current.md with a report containing:

* files changed;
* processing subprocess API;
* shared private transport structure;
* preserved acquisition API;
* exact child invocation;
* timeout and output-limit behavior;
* typed processing execution errors;
* error precedence;
* processing fixture modes;
* successful processing probe results;
* malformed, incompatible, nonzero, timeout, and overflow results;
* parallel fixture-race prevention;
* acquisition regression results;
* repeated workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not add processing verification or build integration.