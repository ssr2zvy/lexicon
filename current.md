# Implementation report: Core normal-invocation execution

## Files changed

- `lexicon-core/src/protocols/http/runner.rs` — added `HttpRuntimeInvocationExecutionError`, `run_http_runtime_invocation`, and 40 execution tests in `mod execution_tests`
- `lexicon-core/src/processing/runner.rs` — added `ProcessingRuntimeInvocationExecutionError`, `run_processing_runtime_invocation`, and 30 execution tests in `mod execution_tests`
- `lexicon-core/src/protocols/http/mod.rs` — exported `HttpRuntimeInvocationExecutionError` and `run_http_runtime_invocation`
- `lexicon-core/src/processing/mod.rs` — exported `ProcessingRuntimeInvocationExecutionError` and `run_processing_runtime_invocation`

## HTTP normal-invocation execution API

```rust
pub fn run_http_runtime_invocation(
    arguments: &[OsString],
    compiled_identity: RuntimeIdentity,
    source: &HttpSourceContractV1,
    available_capabilities: HttpCapabilitySet,
    context: &mut HttpAcquisitionContext,
) -> Result<(), HttpRuntimeInvocationExecutionError>
```

Exported via `lexicon_core::http`.

## Processing normal-invocation execution API

```rust
pub fn run_processing_runtime_invocation(
    arguments: &[OsString],
    compiled_identity: RuntimeIdentity,
    source: &ProcessingSourceContractV1,
    context: &mut ProcessingContext,
) -> Result<(), ProcessingRuntimeInvocationExecutionError>
```

Exported via `lexicon_core::processing`.

## Exact execution order

### HTTP

1. `parse_runtime_invocation(arguments)` → `ParsedRuntimeInvocation` or `Transport` error
2. `admit_http_runtime_invocation(parsed, compiled_identity, source, available_capabilities)` → `AdmittedHttpRuntimeInvocation` or `Admission` error
3. `admitted.into_parts()` → extract `(envelope, source_arguments, handler, available_capabilities)`
4. Match `handler`:
   - `AdmittedHttpHandler::Acquire(f)` → `f(context, &source_arguments)`
   - `AdmittedHttpHandler::Resume(f)` → `f(context, &source_arguments)`
5. Map handler error → `Handler` variant; return `Ok(())` on success

### Processing

1. `parse_runtime_invocation(arguments)` → `ParsedRuntimeInvocation` or `Transport` error
2. `admit_processing_runtime_invocation(parsed, compiled_identity, source)` → `AdmittedProcessingRuntimeInvocation` or `Admission` error
3. `admitted.into_parts()` → extract `(envelope, source_arguments, handler)`
4. Match `handler`:
   - `AdmittedProcessingHandler::Process(f)` → `f(context, &source_arguments)`
5. Map handler error → `Handler` variant; return `Ok(())` on success

## Existing handler types reused

- `HttpAcquireFn = fn(&mut HttpAcquisitionContext, &[OsString]) -> AcquisitionResult<()>`
- `HttpResumeFn = fn(&mut HttpAcquisitionContext, &[OsString]) -> AcquisitionResult<()>`
- `ProcessDataFn = fn(&mut ProcessingContext, &[OsString]) -> ProcessingResult<()>`

No handler signatures were changed.

## Existing context types reused

- `HttpAcquisitionContext` — supplied by caller, not constructed inside the function
- `ProcessingContext` — supplied by caller; constructed via `ProcessingContext::default()` in tests

## Typed execution error representations

### HTTP

```rust
#[derive(Debug)]
pub enum HttpRuntimeInvocationExecutionError {
    Transport(RuntimeInvocationTransportDecodingError),
    Admission(HttpRuntimeInvocationAdmissionError),
    Handler(AcquisitionError),
}
```

`Display` returns static strings (no dynamic content, no args, no envelope JSON). `Error::source()` chains the nested error.

### Processing

```rust
#[derive(Debug)]
pub enum ProcessingRuntimeInvocationExecutionError {
    Transport(RuntimeInvocationTransportDecodingError),
    Admission(ProcessingRuntimeInvocationAdmissionError),
    Handler(ProcessingError),
}
```

Same Display and source() approach.

## Results

### Acquisition/run

- Calls `HttpAcquireFn` exactly once with the caller-supplied `HttpAcquisitionContext` and admitted source arguments.
- Returns `Ok(())` on success.
- Returns `HttpRuntimeInvocationExecutionError::Handler(AcquisitionError)` on failure.

### Acquisition/resume

- Calls `HttpResumeFn` exactly once with the caller-supplied `HttpAcquisitionContext` and admitted source arguments.
- Returns `Ok(())` on success.
- Returns `HttpRuntimeInvocationExecutionError::Handler(AcquisitionError)` on failure.

### Processing/run

- Calls `ProcessDataFn` exactly once with the caller-supplied `ProcessingContext` and admitted source arguments.
- Returns `Ok(())` on success.
- Returns `ProcessingRuntimeInvocationExecutionError::Handler(ProcessingError)` on failure.

### Exact-once invocation

All tests confirm each handler is called exactly once per successful admission. Atomic call counters in tests 2, 5 (HTTP) and 2, 19 (processing) verify this.

### Source-argument delivery

Arguments are extracted from `AdmittedHttpRuntimeInvocation` / `AdmittedProcessingRuntimeInvocation` via `into_parts()` and passed directly to the handler without modification. Tests 14–21 (HTTP) and 9–16 (processing) cover ordering, duplicates, empty values, `--`, invocation flag, probe flag, Unicode, and non-UTF-8 Unix bytes.

### Non-UTF-8 Unix preservation

Tests 22–23 (HTTP) and 16 (processing) verify byte-for-byte delivery of invalid-UTF-8 `OsString` values on Unix.

### Ordinary handler failure

`AcquisitionError` → `HttpRuntimeInvocationExecutionError::Handler`. `ProcessingError` → `ProcessingRuntimeInvocationExecutionError::Handler`. No retry, no cross-handler invocation, no conversion.

### Panic behavior

Handler panics unwind normally. The execution functions do not call `catch_unwind`. Pre-handler failures (transport or admission) are proved not to invoke handlers via panicking handler functions in tests 35–36 (HTTP) and 26–27 (processing).

### Probe preservation

The probe function `try_write_runtime_information_probe` (HTTP) and its processing counterpart are unchanged. Passing probe arguments to the normal execution functions returns `HttpRuntimeInvocationExecutionError::Transport(ProbeMode)` / `ProcessingRuntimeInvocationExecutionError::Transport(ProbeMode)` via the existing parser's `ProbeMode` rejection path. Tests 30 (HTTP) and 21 (processing) verify this.

### Normal execution rejects probe arguments via transport parsing

`parse_runtime_invocation` returns `RuntimeInvocationTransportDecodingError::ProbeMode` when the first argument is `--lexicon-runtime-information-v1`. The execution functions map this to `Transport(...)`.

### No generic dispatcher added

No `lexicon-core/src/runtime/runner.rs` was created. No cross-operation runner abstraction exists.

### No handler signature changed

All existing handler signatures are identical to before this milestone.

### No environment, filesystem, HTTP, SQLite, printing, exit, or subprocess behavior added

`run_http_runtime_invocation` and `run_processing_runtime_invocation` take all inputs as explicit parameters. They do not call `HttpAcquisitionContext::from_env()`, read environment variables, access the filesystem, perform network I/O, print output, call `std::process::exit`, or launch subprocesses. Test 39 (HTTP) confirms `from_env()` is not called.

## First complete lexicon-core test result

```
test result: ok. 261 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

## Second complete lexicon-core test result

```
test result: ok. 261 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

## Workspace and bundle/install tests

Not run, as specified. Only `cargo test -p lexicon-core --quiet` was executed.
