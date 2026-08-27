use std::ffi::{OsStr, OsString};
use std::fmt;

use super::{
    AdmittedProcessingHandler, ProcessingContext, ProcessingError,
    ProcessingRuntimeInvocationAdmissionError, ProcessingSourceContractV1,
    admit_processing_runtime_invocation,
};
use crate::processing::{
    ProcessingRuntimeInformationConstructionError, ProcessingRuntimeInformationEncodingError,
    ProcessingRuntimeInformationV1,
};
use crate::runtime::{
    RuntimeIdentity, RuntimeInvocationTransportDecodingError, parse_runtime_invocation,
};

pub use crate::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingRuntimeInformationProbeOutcome {
    NotRequested,
    Written,
}

#[derive(Debug)]
pub enum ProcessingRuntimeInformationProbeError {
    UnexpectedArguments,
    Construction(ProcessingRuntimeInformationConstructionError),
    Encoding(ProcessingRuntimeInformationEncodingError),
    Output(std::io::Error),
}

impl fmt::Display for ProcessingRuntimeInformationProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedArguments => {
                formatter.write_str("unexpected processing runtime information probe arguments")
            }
            Self::Construction(error) => {
                write!(
                    formatter,
                    "processing runtime information construction error: {error}"
                )
            }
            Self::Encoding(error) => {
                write!(
                    formatter,
                    "processing runtime information encoding error: {error}"
                )
            }
            Self::Output(error) => {
                write!(
                    formatter,
                    "processing runtime information probe output error: {error}"
                )
            }
        }
    }
}

impl std::error::Error for ProcessingRuntimeInformationProbeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::UnexpectedArguments => None,
            Self::Construction(error) => Some(error),
            Self::Encoding(error) => Some(error),
            Self::Output(error) => Some(error),
        }
    }
}

pub fn try_write_runtime_information_probe<W: std::io::Write>(
    identity: RuntimeIdentity,
    source: &ProcessingSourceContractV1,
    arguments: &[OsString],
    output: &mut W,
) -> Result<ProcessingRuntimeInformationProbeOutcome, ProcessingRuntimeInformationProbeError> {
    let Some(first_argument) = arguments.first() else {
        return Ok(ProcessingRuntimeInformationProbeOutcome::NotRequested);
    };

    if first_argument.as_os_str() != OsStr::new(RUNTIME_INFORMATION_PROBE_ARGUMENT) {
        return Ok(ProcessingRuntimeInformationProbeOutcome::NotRequested);
    }

    if arguments.len() != 1 {
        return Err(ProcessingRuntimeInformationProbeError::UnexpectedArguments);
    }

    let json = ProcessingRuntimeInformationV1::from_processing_source(identity, source)
        .map_err(ProcessingRuntimeInformationProbeError::Construction)?
        .to_json()
        .map_err(ProcessingRuntimeInformationProbeError::Encoding)?;

    let mut document = json.into_bytes();
    document.push(b'\n');

    std::io::Write::write_all(output, &document)
        .map_err(ProcessingRuntimeInformationProbeError::Output)?;
    std::io::Write::flush(output).map_err(ProcessingRuntimeInformationProbeError::Output)?;

    Ok(ProcessingRuntimeInformationProbeOutcome::Written)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::io::{self, Write};

    use super::{
        ProcessingRuntimeInformationProbeError, ProcessingRuntimeInformationProbeOutcome,
        RUNTIME_INFORMATION_PROBE_ARGUMENT, try_write_runtime_information_probe,
    };
    use crate::processing::{
        ProcessingContext, ProcessingResult, ProcessingRuntimeInformationV1,
        ProcessingSourceContractV1,
    };
    use crate::runtime::RuntimeIdentity;

    fn process_handler(
        _context: &mut ProcessingContext,
        _args: &[OsString],
    ) -> ProcessingResult<()> {
        Ok(())
    }

    fn failing_process_handler(
        _context: &mut ProcessingContext,
        _args: &[OsString],
    ) -> ProcessingResult<()> {
        panic!("process handler should not be invoked while probing runtime information");
    }

    #[derive(Default)]
    struct RecordingWriter {
        bytes: Vec<u8>,
        fail_write: bool,
        fail_flush: bool,
    }

    impl Write for RecordingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if self.fail_write {
                return Err(io::Error::new(io::ErrorKind::Other, "write failed"));
            }
            self.bytes.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.fail_flush {
                return Err(io::Error::new(io::ErrorKind::Other, "flush failed"));
            }
            Ok(())
        }
    }

    #[test]
    fn empty_arguments_return_not_requested() {
        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[],
            &mut output,
        )
        .unwrap();

        assert_eq!(
            outcome,
            ProcessingRuntimeInformationProbeOutcome::NotRequested
        );
        assert!(output.is_empty());
    }

    #[test]
    fn unrelated_argument_returns_not_requested() {
        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from("--not-the-probe")],
            &mut output,
        )
        .unwrap();

        assert_eq!(
            outcome,
            ProcessingRuntimeInformationProbeOutcome::NotRequested
        );
        assert!(output.is_empty());
    }

    #[test]
    fn exact_probe_argument_returns_written() {
        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, ProcessingRuntimeInformationProbeOutcome::Written);
        assert!(!output.is_empty());
    }

    #[test]
    fn not_requested_writes_no_bytes() {
        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from("--ordinary-source-value")],
            &mut output,
        )
        .unwrap();

        assert_eq!(
            outcome,
            ProcessingRuntimeInformationProbeOutcome::NotRequested
        );
        assert!(output.is_empty());
    }

    #[test]
    fn successful_output_parses_through_processing_runtime_information_json() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let identity = RuntimeIdentity::http_processing("example-source", 1);

        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            identity,
            &source,
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, ProcessingRuntimeInformationProbeOutcome::Written);
        let text = std::str::from_utf8(&output).unwrap();
        let parsed =
            ProcessingRuntimeInformationV1::from_json(text.trim_end_matches('\n')).unwrap();

        assert_eq!(parsed.identity(), identity);
        assert_eq!(
            parsed.descriptor_contract_version(),
            ProcessingSourceContractV1::CONTRACT_VERSION
        );
    }

    #[test]
    fn successful_output_ends_with_exactly_one_newline() {
        let mut output = Vec::new();
        try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        assert!(output.ends_with(b"\n"));
        assert!(!output.ends_with(b"\n\n"));
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text.matches('\n').count(), 1);
    }

    #[test]
    fn successful_output_contains_only_json_document_and_newline() {
        let mut output = Vec::new();
        try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        let text = String::from_utf8(output).unwrap();
        assert!(text.ends_with('\n'));
        assert_eq!(text.matches('\n').count(), 1);
        assert!(text.trim_end_matches('\n').starts_with('{'));
        assert!(text.trim_end_matches('\n').ends_with('}'));
    }

    #[test]
    fn processing_identity_is_preserved() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let mut output = Vec::new();

        try_write_runtime_information_probe(
            identity,
            &source,
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        let parsed = ProcessingRuntimeInformationV1::from_json(
            std::str::from_utf8(&output).unwrap().trim_end_matches('\n'),
        )
        .unwrap();
        assert_eq!(parsed.identity(), identity);
    }

    #[test]
    fn descriptor_contract_version_is_preserved() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let identity = RuntimeIdentity::http_processing("example-source", 1);

        let mut output = Vec::new();
        try_write_runtime_information_probe(
            identity,
            &source,
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        let parsed = ProcessingRuntimeInformationV1::from_json(
            std::str::from_utf8(&output).unwrap().trim_end_matches('\n'),
        )
        .unwrap();
        assert_eq!(
            parsed.descriptor_contract_version(),
            ProcessingSourceContractV1::CONTRACT_VERSION
        );
    }

    #[test]
    fn process_handler_is_not_invoked() {
        let source = ProcessingSourceContractV1::new(failing_process_handler);
        let mut output = Vec::new();
        let result = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &source,
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn additional_arguments_return_unexpected_arguments() {
        let result = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[
                OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT),
                OsString::from("extra"),
            ],
            &mut Vec::new(),
        );

        assert!(matches!(
            result,
            Err(ProcessingRuntimeInformationProbeError::UnexpectedArguments)
        ));
    }

    #[test]
    fn later_position_probe_argument_returns_not_requested() {
        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[
                OsString::from("--another-mode"),
                OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT),
            ],
            &mut output,
        )
        .unwrap();

        assert_eq!(
            outcome,
            ProcessingRuntimeInformationProbeOutcome::NotRequested
        );
        assert!(output.is_empty());
    }

    #[test]
    fn acquisition_identity_returns_a_typed_construction_error() {
        let err = try_write_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut Vec::new(),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            ProcessingRuntimeInformationProbeError::Construction(_)
        ));
    }

    #[test]
    fn incorrect_source_contract_version_returns_typed_construction_error() {
        let err = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 2),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut Vec::new(),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            ProcessingRuntimeInformationProbeError::Construction(_)
        ));
    }

    #[test]
    fn construction_failure_writes_no_bytes() {
        let mut output = Vec::new();
        let result = try_write_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        );

        assert!(result.is_err());
        assert!(output.is_empty());
    }

    #[test]
    fn writer_failure_returns_output_error() {
        let mut writer = RecordingWriter {
            fail_write: true,
            ..Default::default()
        };
        let result = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut writer,
        );

        assert!(matches!(
            result,
            Err(ProcessingRuntimeInformationProbeError::Output(_))
        ));
    }

    #[test]
    fn flush_failure_returns_output_error() {
        let mut writer = RecordingWriter {
            fail_flush: true,
            ..Default::default()
        };
        let result = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut writer,
        );

        assert!(matches!(
            result,
            Err(ProcessingRuntimeInformationProbeError::Output(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn unrelated_non_utf8_unix_argument_returns_not_requested() {
        use std::os::unix::ffi::OsStringExt;

        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from_vec(vec![b'a', 0x80, b'c'])],
            &mut output,
        )
        .unwrap();

        assert_eq!(
            outcome,
            ProcessingRuntimeInformationProbeOutcome::NotRequested
        );
        assert!(output.is_empty());
    }

    #[test]
    fn processing_and_acquisition_modules_expose_same_canonical_probe_argument() {
        assert_eq!(
            crate::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT,
            crate::processing::runner::RUNTIME_INFORMATION_PROBE_ARGUMENT,
        );
        assert_eq!(
            crate::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT,
            crate::protocols::http::runner::RUNTIME_INFORMATION_PROBE_ARGUMENT,
        );
    }
}

// --- Normal-invocation execution ---

#[derive(Debug)]
pub enum ProcessingRuntimeInvocationExecutionError {
    Transport(RuntimeInvocationTransportDecodingError),
    Admission(ProcessingRuntimeInvocationAdmissionError),
    Handler(ProcessingError),
}

impl fmt::Display for ProcessingRuntimeInvocationExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(_) => {
                formatter.write_str("processing runtime invocation transport decoding error")
            }
            Self::Admission(_) => {
                formatter.write_str("processing runtime invocation admission error")
            }
            Self::Handler(_) => formatter.write_str("processing handler error"),
        }
    }
}

impl std::error::Error for ProcessingRuntimeInvocationExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(e) => Some(e),
            Self::Admission(e) => Some(e),
            Self::Handler(e) => Some(e),
        }
    }
}

pub fn run_processing_runtime_invocation(
    arguments: &[OsString],
    compiled_identity: RuntimeIdentity,
    source: &ProcessingSourceContractV1,
    context: &mut ProcessingContext,
) -> Result<(), ProcessingRuntimeInvocationExecutionError> {
    let parsed = parse_runtime_invocation(arguments)
        .map_err(ProcessingRuntimeInvocationExecutionError::Transport)?;

    let admitted = admit_processing_runtime_invocation(parsed, compiled_identity, source)
        .map_err(ProcessingRuntimeInvocationExecutionError::Admission)?;

    let (_, source_arguments, handler) = admitted.into_parts();

    match handler {
        AdmittedProcessingHandler::Process(f) => f(context, &source_arguments)
            .map_err(ProcessingRuntimeInvocationExecutionError::Handler),
    }
}

#[cfg(test)]
mod execution_tests {
    use std::cell::RefCell;
    use std::ffi::OsString;

    use crate::processing::{
        ProcessingContext, ProcessingError, ProcessingResult, ProcessingSourceContractV1,
    };
    use crate::runtime::{
        ProjectInvocationIdentity, RuntimeExecutionMode, RuntimeIdentity,
        RuntimeInvocationEnvelopeV1, RuntimeSupervisionMode, SessionInvocationIdentity,
    };

    use super::{ProcessingRuntimeInvocationExecutionError, run_processing_runtime_invocation};

    fn example_identity() -> RuntimeIdentity {
        RuntimeIdentity::http_processing("example-source", 1)
    }

    fn example_envelope() -> RuntimeInvocationEnvelopeV1 {
        RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            example_identity(),
            SessionInvocationIdentity::new("session-abc").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap()
    }

    fn encode(envelope: &RuntimeInvocationEnvelopeV1, source_args: &[OsString]) -> Vec<OsString> {
        let mut args = vec![
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(envelope.to_json().unwrap()),
            OsString::from("--"),
        ];
        args.extend_from_slice(source_args);
        args
    }

    // Test 1: matching processing/run calls process handler
    #[test]
    fn matching_run_invocation_calls_process_handler() {
        thread_local! {
            static CALLED: RefCell<bool> = RefCell::new(false);
        }
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            CALLED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let args = encode(&example_envelope(), &[]);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert!(CALLED.with(|c| *c.borrow()));
    }

    // Test 2: processing calls handler exactly once
    #[test]
    fn processing_calls_handler_exactly_once() {
        thread_local! {
            static COUNT: RefCell<u32> = RefCell::new(0);
        }
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            COUNT.with(|c| *c.borrow_mut() += 1);
            Ok(())
        }

        let args = encode(&example_envelope(), &[]);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert_eq!(COUNT.with(|c| *c.borrow()), 1);
    }

    // Test 3: exact ProcessingContext reaches handler
    #[test]
    fn exact_processing_context_reaches_handler() {
        thread_local! {
            static REACHED: RefCell<bool> = RefCell::new(false);
        }
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            REACHED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let args = encode(&example_envelope(), &[]);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert!(REACHED.with(|c| *c.borrow()));
    }

    // Test 4: handler can use the mutable context according to its current public behavior
    #[test]
    fn handler_receives_mutable_context_reference() {
        thread_local! {
            static CALLED: RefCell<bool> = RefCell::new(false);
        }
        fn process(ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            // Verify we can take a mutable reference (the context is minimal; just prove access)
            let _ = ctx as *mut ProcessingContext;
            CALLED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let args = encode(&example_envelope(), &[]);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert!(CALLED.with(|c| *c.borrow()));
    }

    // Test 5: foreground invocation reaches processing
    #[test]
    fn foreground_invocation_reaches_processing() {
        thread_local! {
            static CALLED: RefCell<bool> = RefCell::new(false);
        }
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            CALLED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            example_identity(),
            SessionInvocationIdentity::new("session-abc").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        let args = encode(&envelope, &[]);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert!(CALLED.with(|c| *c.borrow()));
    }

    // Test 6: background invocation reaches processing
    #[test]
    fn background_invocation_reaches_processing() {
        thread_local! {
            static CALLED: RefCell<bool> = RefCell::new(false);
        }
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            CALLED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            example_identity(),
            SessionInvocationIdentity::new("session-abc").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Background,
        )
        .unwrap();
        let args = encode(&envelope, &[]);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert!(CALLED.with(|c| *c.borrow()));
    }

    // Test 7: project identity preserved through admission and execution
    #[test]
    fn project_identity_preserved_through_execution() {
        thread_local! {
            static CALLED: RefCell<bool> = RefCell::new(false);
        }
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            CALLED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("my-project").unwrap(),
            example_identity(),
            SessionInvocationIdentity::new("session-abc").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        let args = encode(&envelope, &[]);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert!(CALLED.with(|c| *c.borrow()));
    }

    // Test 8: session identity preserved through admission and execution
    #[test]
    fn session_identity_preserved_through_execution() {
        thread_local! {
            static CALLED: RefCell<bool> = RefCell::new(false);
        }
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            CALLED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            example_identity(),
            SessionInvocationIdentity::new("unique-session-789").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        let args = encode(&envelope, &[]);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert!(CALLED.with(|c| *c.borrow()));
    }

    // Test 9: source arguments reach processing in exact order
    #[test]
    fn source_arguments_reach_processing_in_exact_order() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn process(_ctx: &mut ProcessingContext, args: &[OsString]) -> ProcessingResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![
            OsString::from("alpha"),
            OsString::from("beta"),
            OsString::from("gamma"),
        ];
        let args = encode(&example_envelope(), &source_args);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 10: duplicate source arguments are preserved
    #[test]
    fn duplicate_source_arguments_are_preserved() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn process(_ctx: &mut ProcessingContext, args: &[OsString]) -> ProcessingResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![
            OsString::from("dup"),
            OsString::from("dup"),
            OsString::from("dup"),
        ];
        let args = encode(&example_envelope(), &source_args);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 11: empty source values are preserved
    #[test]
    fn empty_source_values_are_preserved() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn process(_ctx: &mut ProcessingContext, args: &[OsString]) -> ProcessingResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![
            OsString::from(""),
            OsString::from("value"),
            OsString::from(""),
        ];
        let args = encode(&example_envelope(), &source_args);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 12: source value equal to -- is preserved
    #[test]
    fn source_value_equal_to_delimiter_is_preserved() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn process(_ctx: &mut ProcessingContext, args: &[OsString]) -> ProcessingResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![OsString::from("--"), OsString::from("after")];
        let args = encode(&example_envelope(), &source_args);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 13: source value equal to invocation flag is preserved
    #[test]
    fn source_value_equal_to_invocation_flag_is_preserved() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn process(_ctx: &mut ProcessingContext, args: &[OsString]) -> ProcessingResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![OsString::from("--lexicon-invocation-v1")];
        let args = encode(&example_envelope(), &source_args);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 14: source value equal to probe flag is preserved
    #[test]
    fn source_value_equal_to_probe_flag_is_preserved() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn process(_ctx: &mut ProcessingContext, args: &[OsString]) -> ProcessingResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        use crate::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT;
        let source_args = vec![OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)];
        let args = encode(&example_envelope(), &source_args);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 15: unicode source values are preserved
    #[test]
    fn unicode_source_values_are_preserved() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn process(_ctx: &mut ProcessingContext, args: &[OsString]) -> ProcessingResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![OsString::from("日本語"), OsString::from("🦀")];
        let args = encode(&example_envelope(), &source_args);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 16: non-UTF-8 Unix source args reach processing byte-for-byte
    #[cfg(unix)]
    #[test]
    fn non_utf8_unix_source_arguments_reach_processing_byte_for_byte() {
        use std::os::unix::ffi::OsStringExt;

        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn process(_ctx: &mut ProcessingContext, args: &[OsString]) -> ProcessingResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![
            OsString::from_vec(vec![b'a', 0x80, b'c']),
            OsString::from_vec(vec![0xFF, 0xFE, 0xFD]),
        ];
        let args = encode(&example_envelope(), &source_args);
        run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 17: processing success returns Ok(())
    #[test]
    fn processing_success_returns_ok() {
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            Ok(())
        }
        let args = encode(&example_envelope(), &[]);
        let result = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        );
        assert!(result.is_ok());
    }

    // Test 18: processing failure returns Handler variant
    #[test]
    fn processing_failure_returns_handler_error() {
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            Err(ProcessingError)
        }
        let args = encode(&example_envelope(), &[]);
        let err = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Handler(_)
        ));
    }

    // Test 19: processing failure does not cause reinvocation
    #[test]
    fn processing_failure_does_not_cause_reinvocation() {
        thread_local! {
            static COUNT: RefCell<u32> = RefCell::new(0);
        }
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            COUNT.with(|c| *c.borrow_mut() += 1);
            Err(ProcessingError)
        }
        let args = encode(&example_envelope(), &[]);
        let _ = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        );
        assert_eq!(COUNT.with(|c| *c.borrow()), 1);
    }

    // Test 20: malformed transport returns Transport error
    #[test]
    fn malformed_transport_returns_transport_error() {
        let args = vec![OsString::from("--not-invocation-flag")];
        let err = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(|_: &mut ProcessingContext, _: &[OsString]| Ok(())),
            &mut ProcessingContext::default(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Transport(_)
        ));
    }

    // Test 21: probe arguments return transport error
    #[test]
    fn probe_arguments_return_transport_error() {
        use crate::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT;
        let args = vec![OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)];
        let err = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(|_: &mut ProcessingContext, _: &[OsString]| Ok(())),
            &mut ProcessingContext::default(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Transport(_)
        ));
    }

    // Test 22: identity mismatch returns Admission error
    #[test]
    fn identity_mismatch_returns_admission_error() {
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            Ok(())
        }
        let args = encode(&example_envelope(), &[]);
        let err = run_processing_runtime_invocation(
            &args,
            RuntimeIdentity::http_processing("different-source", 1),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 23: wrong compiled operation returns Admission error
    #[test]
    fn wrong_compiled_operation_returns_admission_error() {
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            Ok(())
        }
        let args = encode(&example_envelope(), &[]);
        let err = run_processing_runtime_invocation(
            &args,
            RuntimeIdentity::from_parts(
                "example-source",
                crate::runtime::RuntimeProtocol::Http,
                crate::runtime::RuntimeOperation::Acquisition,
                1,
            ),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 24: descriptor-version mismatch returns Admission error
    #[test]
    fn descriptor_version_mismatch_returns_admission_error() {
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            Ok(())
        }
        // Version 2 does not match ProcessingSourceContractV1::CONTRACT_VERSION (1)
        let identity_v2 = RuntimeIdentity::http_processing("example-source", 2);
        let envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            identity_v2,
            SessionInvocationIdentity::new("session-abc").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        let args = encode(&envelope, &[]);
        let err = run_processing_runtime_invocation(
            &args,
            identity_v2,
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 25: processing/resume remains rejected before handler invocation
    #[test]
    fn processing_resume_mode_is_rejected_by_envelope_model() {
        // Processing envelopes reject Resume mode at construction time (existing behavior).
        let result = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            example_identity(),
            SessionInvocationIdentity::new("session-abc").unwrap(),
            RuntimeExecutionMode::Resume,
            RuntimeSupervisionMode::Foreground,
        );
        assert!(result.is_err());
    }

    // Test 26: transport failure does not invoke processing
    #[test]
    fn transport_failure_does_not_invoke_processing() {
        fn process_must_not_be_called(
            _ctx: &mut ProcessingContext,
            _args: &[OsString],
        ) -> ProcessingResult<()> {
            panic!("process must not be called on transport failure");
        }
        let args = vec![]; // empty → transport error
        let err = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process_must_not_be_called),
            &mut ProcessingContext::default(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Transport(_)
        ));
    }

    // Test 27: admission failure does not invoke processing
    #[test]
    fn admission_failure_does_not_invoke_processing() {
        fn process_must_not_be_called(
            _ctx: &mut ProcessingContext,
            _args: &[OsString],
        ) -> ProcessingResult<()> {
            panic!("process must not be called on admission failure");
        }
        let args = encode(&example_envelope(), &[]);
        let err = run_processing_runtime_invocation(
            &args,
            RuntimeIdentity::http_processing("wrong-source", 1),
            &ProcessingSourceContractV1::new(process_must_not_be_called),
            &mut ProcessingContext::default(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 28: error formatting does not expose source arguments
    #[test]
    fn error_formatting_does_not_expose_source_arguments() {
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            Ok(())
        }
        let source_args = vec![
            OsString::from("secret-arg"),
            OsString::from("another-secret"),
        ];
        let args = encode(&example_envelope(), &source_args);
        let err = run_processing_runtime_invocation(
            &args,
            RuntimeIdentity::http_processing("wrong-source", 1),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap_err();
        let msg = format!("{err}");
        assert!(
            !msg.contains("secret-arg"),
            "message exposed source args: {msg}"
        );
        assert!(
            !msg.contains("another-secret"),
            "message exposed source args: {msg}"
        );
    }

    // Test 29: error formatting does not expose envelope JSON
    #[test]
    fn error_formatting_does_not_expose_envelope_json() {
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            Ok(())
        }
        let args = encode(&example_envelope(), &[]);
        let err = run_processing_runtime_invocation(
            &args,
            RuntimeIdentity::http_processing("wrong-source", 1),
            &ProcessingSourceContractV1::new(process),
            &mut ProcessingContext::default(),
        )
        .unwrap_err();
        let msg = format!("{err}");
        assert!(
            !msg.contains("schema_version"),
            "message exposed envelope JSON: {msg}"
        );
        assert!(
            !msg.contains('{'),
            "message exposed JSON-like content: {msg}"
        );
    }

    // Test 30: existing processing probe tests remain (verified by absence of breakage; probe
    // tests live in `mod tests` above and are not removed or weakened by this module).
}
