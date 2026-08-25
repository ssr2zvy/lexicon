Current implementation request: bounded runtime-information subprocess probe

Objective

Add the framework function that executes a candidate runtime in Core’s reserved runtime-information probe mode.

The framework must launch the executable directly, enforce a timeout, capture stdout and stderr concurrently with bounded memory, validate its exit status, and pass stdout to the existing:

admit_http_runtime_information_probe(...)

Do not connect this executor to source build yet.

Required module

Extend:

lexicon-framework/src/build/runtime_probe.rs

Export the new public API through:

lexicon-framework/src/build/mod.rs

lexicon-framework must remain library-only.

Execution limits

Define:

pub const RUNTIME_INFORMATION_PROBE_TIMEOUT: Duration =
    Duration::from_secs(5);
pub const MAX_RUNTIME_INFORMATION_PROBE_STDERR_BYTES: usize =
    64 * 1024;

Continue using:

MAX_RUNTIME_INFORMATION_PROBE_BYTES

as the stdout limit.

The public operation always uses the fixed timeout.

A private or pub(crate) helper may accept a shorter timeout for tests.

Public API

Provide:

pub fn probe_http_runtime_information(
    executable: &Path,
    expected_identity: RuntimeIdentity,
) -> Result<
    AdmittedRuntimeInformation,
    RuntimeProbeExecutionError,
>;

The function returns only admitted information. A successful process with invalid or incompatible information is still an error.

Exact child invocation

Launch the supplied executable directly without a shell:

<executable> --lexicon-runtime-information-v1

Use Core’s exported:

RUNTIME_INFORMATION_PROBE_ARGUMENT

Do not duplicate the argument literal in the framework.

Configure the child with:

stdin  = Stdio::null()
stdout = Stdio::piped()
stderr = Stdio::piped()

Pass no additional arguments.

Do not invoke Cargo.

Required execution sequence

The framework must:

1. Spawn the exact supplied executable.
2. Pass only the reserved probe argument.
3. Start concurrent stdout and stderr drainers.
4. Retain stdout up to its configured limit.
5. Retain stderr up to its configured limit.
6. Continue draining and discarding excess bytes so full pipes cannot deadlock the child.
7. Record independently whether either stream exceeded its limit.
8. wait for the process using the fixed deadline.
9. Terminate the child if the deadline expires.
10. Reap the child after normal exit or termination.
11. Join both drainer operations.
12. Reject stream read failures.
13. Reject stdout or stderr overflow.
14. Reject unsuccessful exit status.
15. Pass stdout to:

admit_http_runtime_information_probe(
    expected_identity,
    &stdout,
)

16. Return the resulting AdmittedRuntimeInformation.

Every successfully spawned child must be reaped before the function returns.

Bounded stream capture

Implement a bounded capture result equivalent to:

struct BoundedCapturedStream {
    retained: Vec<u8>,
    truncated: bool,
}

The reader must:

* retain at most the configured maximum;
* continue reading after the maximum is reached;
* discard excess bytes;
* set truncated to true;
* never grow the retained buffer beyond the limit.

Stdout and stderr must be drained concurrently.

Do not read one complete stream before beginning the other.

Timeout behavior

When the timeout expires:

1. Attempt Child::kill().
2. Call Child::wait() to reap the child.
3. finish and join both stream drainers;
4. return a typed timeout error.

The timeout must remain the primary error classification even if cleanup also reports a failure.

Preserve cleanup failure information where practical.

Do not leave a known child running after returning.

Exit-status behavior

Admission occurs only after a successful exit status.

A nonzero exit returns an execution error even if stdout contains valid and compatible JSON.

Preserve:

* the ExitStatus;
* bounded stderr bytes;
* whether stderr was truncated.

Do not print stderr.

Successful exit with nonempty but bounded stderr may continue to stdout admission. Stderr is not part of the Core probe document.

Typed execution error

Define an error equivalent to:

#[derive(Debug)]
pub enum RuntimeProbeExecutionError {
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
    Admission(RuntimeProbeAdmissionError),
}

Equivalent representations are acceptable, but callers must distinguish:

* spawn failure;
* wait failure;
* timeout;
* stdout read failure;
* stderr read failure;
* stdout overflow;
* stderr overflow;
* unsuccessful exit;
* admission failure.

Implement:

std::fmt::Display
std::error::Error

Do not return plain String, print diagnostics, or terminate the parent process.

Deterministic error precedence

After cleanup and stream collection, classify errors in this order:

1. Timeout.
2. Wait or cleanup failure.
3. Stdout read failure.
4. Stderr read failure.
5. Stdout overflow.
6. Stderr overflow.
7. Unsuccessful exit.
8. Admission failure.

If operating-system behavior requires a minor difference, document and test the exact precedence.

Test fixture

Add a test-only executable fixture supporting deterministic modes for:

* valid runtime information;
* malformed stdout;
* incompatible runtime information;
* nonzero exit with stderr;
* delayed exit;
* oversized stdout;
* oversized stderr;
* simultaneous noisy stdout and stderr.

The fixture must not become:

* an installed Lexicon executable;
* an MZA artifact;
* a Protocol 1 bundle input;
* a production lexicon-framework binary target.

Do not use a shell in the production implementation.

Required tests

Add tests proving:

1. A valid fixture runtime is executed and admitted.
2. The child receives the exact reserved probe argument.
3. The child receives no additional arguments.
4. The admitted identity matches the expected identity.
5. Required and available capabilities survive the process boundary.
6. A missing executable returns Spawn.
7. Malformed stdout returns Admission.
8. Incompatible identity returns Admission.
9. Missing required capabilities return Admission.
10. A nonzero exit returns UnsuccessfulExit.
11. Nonzero exit is rejected even when stdout is valid.
12. Bounded stderr is retained for a nonzero exit.
13. Successful exit with bounded stderr may still be admitted.
14. A delayed child exceeding the deadline returns Timeout.
15. The timed-out child is terminated.
16. The timed-out child is reaped.
17. Oversized stdout returns StdoutTooLarge.
18. Oversized stderr returns StderrTooLarge.
19. Simultaneous noisy stdout and stderr do not deadlock.
20. Retained stream buffers never exceed their limits.
21. Existing pure admission tests remain unchanged.
22. Existing Core probe tests remain unchanged.
23. All workspace tests pass.

Use the internal timeout helper for short timeout tests. Do not make the normal test suite wait five seconds for each timeout case.

Preserve existing behavior

Do not change:

* source scaffolding;
* source implementation crates;
* source create;
* source build;
* Cargo build invocation;
* artifact selection;
* runtime publication;
* CLI behavior;
* Core probe behavior;
* probe JSON schema;
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

If the known MZA dependency remains unavailable, report it separately. Do not modify MZA or installer code.

Explicit exclusions

Do not implement:

* source build integration;
* Cargo metadata changes;
* Cargo artifact-selection changes;
* executable hashing;
* runtime.json;
* staged runtime bundles;
* publication changes;
* managed-runner generation;
* runner main.rs;
* runner::run;
* source workspace migration;
* invocation envelopes;
* acquisition execution;
* resume execution;
* child runtime admission;
* HTTP transport;
* raw recording;
* sessions;
* foreground supervision;
* background supervision;
* __operator-host;
* processing runtime probing.

Completion report

After completion, replace current.md with a report containing:

* files created and changed;
* the public probe-execution API;
* timeout and stream limits;
* exact child invocation;
* concurrent bounded-capture behavior;
* termination and reaping behavior;
* typed execution errors;
* error precedence;
* test-fixture arrangement;
* successful probe result;
* nonzero-exit behavior;
* timeout behavior;
* stdout and stderr overflow behavior;
* confirmation that no Cargo or source build integration was added;
* framework and workspace test results;
* bundle/install result or the known external-MZA blocker.

Then stop. Do not connect the executor to source build or generate managed runners.