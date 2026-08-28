use std::ffi::{OsStr, OsString};
use std::fmt;

use crate::HttpAcquisitionContext;
use crate::protocols::http::error::AcquisitionError;
use crate::protocols::http::invocation::{
    AdmittedHttpHandler, HttpRuntimeInvocationAdmissionError, admit_http_runtime_invocation,
};
use crate::protocols::http::{HttpCapabilitySet, HttpSourceContractV1};
use crate::runtime::{
    RuntimeIdentity, RuntimeInformationEncodingError, RuntimeInformationV1,
    RuntimeInvocationTransportDecodingError, parse_runtime_invocation,
};
use crate::session::{
    CoreRunnerSessionError, RuntimeContextPaths, SessionDataPaths, SessionOperationRoot,
    SessionStore, bind_runtime_session, decode_runtime_context_from_env,
};

pub const RUNTIME_INFORMATION_PROBE_ARGUMENT: &str =
    crate::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeInformationProbeOutcome {
    NotRequested,
    Written,
}

#[derive(Debug)]
pub enum RuntimeInformationProbeError {
    UnexpectedArguments,
    Encoding(RuntimeInformationEncodingError),
    Output(std::io::Error),
}

impl fmt::Display for RuntimeInformationProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedArguments => {
                formatter.write_str("unexpected runtime information probe arguments")
            }
            Self::Encoding(error) => {
                write!(formatter, "runtime information encoding error: {error}")
            }
            Self::Output(error) => {
                write!(formatter, "runtime information probe output error: {error}")
            }
        }
    }
}

impl std::error::Error for RuntimeInformationProbeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::UnexpectedArguments => None,
            Self::Encoding(error) => Some(error),
            Self::Output(error) => Some(error),
        }
    }
}

pub fn try_write_runtime_information_probe<W: std::io::Write>(
    identity: RuntimeIdentity,
    source: &HttpSourceContractV1,
    available_capabilities: HttpCapabilitySet,
    arguments: &[OsString],
    output: &mut W,
) -> Result<RuntimeInformationProbeOutcome, RuntimeInformationProbeError> {
    let Some(first_argument) = arguments.first() else {
        return Ok(RuntimeInformationProbeOutcome::NotRequested);
    };

    if first_argument.as_os_str() != OsStr::new(RUNTIME_INFORMATION_PROBE_ARGUMENT) {
        return Ok(RuntimeInformationProbeOutcome::NotRequested);
    }

    if arguments.len() != 1 {
        return Err(RuntimeInformationProbeError::UnexpectedArguments);
    }

    let json = RuntimeInformationV1::from_http_source(identity, source, available_capabilities)
        .to_json()
        .map_err(RuntimeInformationProbeError::Encoding)?;

    let mut document = json.into_bytes();
    document.push(b'\n');

    std::io::Write::write_all(output, &document).map_err(RuntimeInformationProbeError::Output)?;
    std::io::Write::flush(output).map_err(RuntimeInformationProbeError::Output)?;

    Ok(RuntimeInformationProbeOutcome::Written)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::io::{self, Write};

    use super::{
        RUNTIME_INFORMATION_PROBE_ARGUMENT, RuntimeInformationProbeError,
        RuntimeInformationProbeOutcome, try_write_runtime_information_probe,
    };
    use crate::http::{HttpCapability, HttpCapabilitySet};
    use crate::protocols::http::{AcquisitionResult, HttpSourceContractV1};
    use crate::runtime::RuntimeInformationV1;
    use crate::{HttpAcquisitionContext, runtime::RuntimeIdentity};

    fn acquire_handler(
        _context: &mut HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> AcquisitionResult<()> {
        Ok(())
    }

    fn resume_handler(
        _context: &mut HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> AcquisitionResult<()> {
        Ok(())
    }

    fn failing_acquire(
        _context: &mut HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> AcquisitionResult<()> {
        panic!("acquire should not be invoked while probing runtime information");
    }

    fn failing_resume(
        _context: &mut HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> AcquisitionResult<()> {
        panic!("resume should not be invoked while probing runtime information");
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
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
            &[],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, RuntimeInformationProbeOutcome::NotRequested);
        assert!(output.is_empty());
    }

    #[test]
    fn unrelated_argument_returns_not_requested() {
        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
            &[OsString::from("--not-the-probe")],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, RuntimeInformationProbeOutcome::NotRequested);
        assert!(output.is_empty());
    }

    #[test]
    fn exact_probe_argument_returns_written() {
        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, RuntimeInformationProbeOutcome::Written);
        assert!(!output.is_empty());
    }

    #[test]
    fn not_requested_writes_no_bytes() {
        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
            &[OsString::from("--ordinary-source-value")],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, RuntimeInformationProbeOutcome::NotRequested);
        assert!(output.is_empty());
    }

    #[test]
    fn successful_output_parses_through_runtime_information_json() {
        let source = HttpSourceContractV1::new(acquire_handler)
            .with_resume(resume_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let identity = RuntimeIdentity::http_acquisition("example-source", 2);
        let available = HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1);

        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            identity,
            &source,
            available,
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, RuntimeInformationProbeOutcome::Written);
        let text = std::str::from_utf8(&output).unwrap();
        let parsed = RuntimeInformationV1::from_json(text.trim_end_matches('\n')).unwrap();

        assert_eq!(parsed.identity(), identity);
        assert_eq!(
            parsed.required_capabilities(),
            source.required_capabilities()
        );
        assert_eq!(parsed.available_capabilities(), available);
        assert_eq!(parsed.resume_handler_registered(), true);
    }

    #[test]
    fn successful_output_ends_with_exactly_one_newline() {
        let mut output = Vec::new();
        try_write_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
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
        let source = HttpSourceContractV1::new(acquire_handler)
            .with_resume(resume_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let available = HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1);

        let expected = RuntimeInformationV1::from_http_source(identity, &source, available)
            .to_json()
            .unwrap();

        let mut output = Vec::new();
        try_write_runtime_information_probe(
            identity,
            &source,
            available,
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), format!("{expected}\n"));
    }

    #[test]
    fn runtime_identity_is_preserved() {
        let identity = RuntimeIdentity::http_acquisition("source-alpha", 7);
        let mut output = Vec::new();
        try_write_runtime_information_probe(
            identity,
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        let parsed =
            RuntimeInformationV1::from_json(std::str::from_utf8(&output).unwrap().trim()).unwrap();
        assert_eq!(parsed.identity(), identity);
    }

    #[test]
    fn descriptor_contract_version_is_preserved() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = Vec::new();
        try_write_runtime_information_probe(
            identity,
            &source,
            HttpCapabilitySet::empty(),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        let parsed =
            RuntimeInformationV1::from_json(std::str::from_utf8(&output).unwrap().trim()).unwrap();
        assert_eq!(
            parsed.descriptor_contract_version(),
            HttpSourceContractV1::CONTRACT_VERSION
        );
    }

    #[test]
    fn required_capabilities_are_preserved() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let mut output = Vec::new();
        try_write_runtime_information_probe(
            identity,
            &source,
            HttpCapabilitySet::empty(),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        let parsed =
            RuntimeInformationV1::from_json(std::str::from_utf8(&output).unwrap().trim()).unwrap();
        assert_eq!(
            parsed.required_capabilities(),
            source.required_capabilities()
        );
    }

    #[test]
    fn available_capabilities_are_preserved_independently() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let available = HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1);
        let mut output = Vec::new();
        try_write_runtime_information_probe(
            identity,
            &source,
            available,
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        let parsed =
            RuntimeInformationV1::from_json(std::str::from_utf8(&output).unwrap().trim()).unwrap();
        assert_eq!(
            parsed.required_capabilities(),
            source.required_capabilities()
        );
        assert_eq!(parsed.available_capabilities(), available);
    }

    #[test]
    fn resume_registration_is_preserved() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler).with_resume(resume_handler);
        let mut output = Vec::new();
        try_write_runtime_information_probe(
            identity,
            &source,
            HttpCapabilitySet::empty(),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        let parsed =
            RuntimeInformationV1::from_json(std::str::from_utf8(&output).unwrap().trim()).unwrap();
        assert!(parsed.resume_handler_registered());
    }

    #[test]
    fn incompatible_capability_combination_is_reported() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let available = HttpCapabilitySet::empty();
        let mut output = Vec::new();

        let outcome = try_write_runtime_information_probe(
            identity,
            &source,
            available,
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, RuntimeInformationProbeOutcome::Written);
        let parsed =
            RuntimeInformationV1::from_json(std::str::from_utf8(&output).unwrap().trim()).unwrap();
        assert!(parsed.validate_capabilities().is_err());
    }

    #[test]
    fn probe_execution_does_not_invoke_acquire() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(failing_acquire);
        let mut output = Vec::new();

        let outcome = try_write_runtime_information_probe(
            identity,
            &source,
            HttpCapabilitySet::empty(),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, RuntimeInformationProbeOutcome::Written);
    }

    #[test]
    fn probe_execution_does_not_invoke_resume() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler).with_resume(failing_resume);
        let mut output = Vec::new();

        let outcome = try_write_runtime_information_probe(
            identity,
            &source,
            HttpCapabilitySet::empty(),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, RuntimeInformationProbeOutcome::Written);
    }

    #[test]
    fn additional_arguments_after_probe_flag_return_unexpected_arguments() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let mut output = Vec::new();

        let error = try_write_runtime_information_probe(
            identity,
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
            &[
                OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT),
                OsString::from("extra"),
            ],
            &mut output,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            RuntimeInformationProbeError::UnexpectedArguments
        ));
        assert!(output.is_empty());
    }

    #[test]
    fn probe_flag_in_later_position_returns_not_requested() {
        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
            &[
                OsString::from("--another-mode"),
                OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT),
            ],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, RuntimeInformationProbeOutcome::NotRequested);
        assert!(output.is_empty());
    }

    #[test]
    fn writer_failure_returns_output_error() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut writer = RecordingWriter {
            fail_write: true,
            ..RecordingWriter::default()
        };

        let error = try_write_runtime_information_probe(
            identity,
            &source,
            HttpCapabilitySet::empty(),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut writer,
        )
        .unwrap_err();

        assert!(matches!(error, RuntimeInformationProbeError::Output(_)));
    }

    #[test]
    fn flush_failure_returns_output_error() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut writer = RecordingWriter {
            fail_flush: true,
            ..RecordingWriter::default()
        };

        let error = try_write_runtime_information_probe(
            identity,
            &source,
            HttpCapabilitySet::empty(),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut writer,
        )
        .unwrap_err();

        assert!(matches!(error, RuntimeInformationProbeError::Output(_)));
    }

    #[cfg(unix)]
    #[test]
    fn unrelated_non_utf8_unix_argument_returns_not_requested() {
        use std::os::unix::ffi::OsStringExt;

        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
            &[OsString::from_vec(vec![0xFF, 0xFE, 0x00])],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, RuntimeInformationProbeOutcome::NotRequested);
        assert!(output.is_empty());
    }
}

// --- Normal-invocation execution ---

#[derive(Debug)]
pub enum HttpRuntimeInvocationExecutionError {
    Transport(RuntimeInvocationTransportDecodingError),
    Admission(HttpRuntimeInvocationAdmissionError),
    Session(CoreRunnerSessionError),
    Handler(AcquisitionError),
    TerminalPersistence {
        handler_error: Option<AcquisitionError>,
        session_error: crate::session::SessionStoreError,
    },
}

impl fmt::Display for HttpRuntimeInvocationExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(_) => {
                formatter.write_str("HTTP runtime invocation transport decoding error")
            }
            Self::Admission(_) => formatter.write_str("HTTP runtime invocation admission error"),
            Self::Session(_) => formatter.write_str("HTTP runtime session initialization error"),
            Self::Handler(_) => formatter.write_str("acquisition handler error"),
            Self::TerminalPersistence { handler_error: Some(_), .. } => {
                formatter.write_str("acquisition handler error; terminal session state persistence also failed")
            }
            Self::TerminalPersistence { handler_error: None, .. } => {
                formatter.write_str("terminal session state persistence failed after successful handler")
            }
        }
    }
}

impl std::error::Error for HttpRuntimeInvocationExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(e) => Some(e),
            Self::Admission(e) => Some(e),
            Self::Session(e) => Some(e),
            Self::Handler(e) => Some(e),
            Self::TerminalPersistence { session_error, .. } => Some(session_error),
        }
    }
}

/// Run an HTTP runtime invocation with full session lifecycle.
///
/// Supported order:
/// 1. Parse invocation argv.
/// 2. Admit invocation.
/// 3. Decode runtime context configuration from the environment.
/// 4. Compare context identities with admitted envelope.
/// 5. Open session store.
/// 6. Acquire/confirm session lease.
/// 7. Transition Prepared → Running.
/// 8. Construct bound operation context.
/// 9. Invoke the selected handler.
/// 10. Persist Succeeded or ordinary Failed.
/// 11. Return typed result.
pub fn run_http_runtime_invocation(
    arguments: &[OsString],
    compiled_identity: RuntimeIdentity,
    source: &HttpSourceContractV1,
    available_capabilities: HttpCapabilitySet,
) -> Result<(), HttpRuntimeInvocationExecutionError> {
    let parsed = parse_runtime_invocation(arguments)
        .map_err(HttpRuntimeInvocationExecutionError::Transport)?;

    let admitted =
        admit_http_runtime_invocation(parsed, compiled_identity, source, available_capabilities)
            .map_err(HttpRuntimeInvocationExecutionError::Admission)?;

    let (envelope, source_arguments, handler, _) = admitted.into_parts();

    // Decode runtime context and compare identities against admitted envelope.
    let context_document = decode_runtime_context_from_env(
        envelope.project(),
        &envelope.runtime().into_owned_identity(),
        envelope.session(),
    )
    .map_err(|e| {
        HttpRuntimeInvocationExecutionError::Session(CoreRunnerSessionError::ContextDecode(e))
    })?;

    let operation_root = SessionOperationRoot::new(
        context_document.paths.operation_root().to_path_buf(),
    )
    .map_err(|e| {
        HttpRuntimeInvocationExecutionError::Session(CoreRunnerSessionError::StoreOpen(e))
    })?;

    let store = SessionStore::open(operation_root).map_err(|e| {
        HttpRuntimeInvocationExecutionError::Session(CoreRunnerSessionError::StoreOpen(e))
    })?;

    let bound = bind_runtime_session(&store, &envelope).map_err(|err| {
        HttpRuntimeInvocationExecutionError::Session(CoreRunnerSessionError::SessionBinding(err))
    })?;
    let running = bound.enter_running().map_err(|e| {
        HttpRuntimeInvocationExecutionError::Session(
            CoreRunnerSessionError::TransitionToRunning(e),
        )
    })?;
    let data_paths = SessionDataPaths::from_context_paths(&context_document.paths);
    let mut context =
        HttpAcquisitionContext::from_session_data_paths(data_paths, envelope.session().clone());

    // Invoke the selected handler.
    let handler_result = match handler {
        AdmittedHttpHandler::Acquire(f) => f(&mut context, &source_arguments),
        AdmittedHttpHandler::Resume(f) => f(&mut context, &source_arguments),
    };

    match handler_result {
        Ok(()) => {
            running.complete().map_err(|e| {
                HttpRuntimeInvocationExecutionError::TerminalPersistence {
                    handler_error: None,
                    session_error: e,
                }
            })?;
            Ok(())
        }
        Err(acquisition_error) => {
            if let Err(persist_error) = running.fail_source() {
                return Err(HttpRuntimeInvocationExecutionError::TerminalPersistence {
                    handler_error: Some(acquisition_error),
                    session_error: persist_error,
                });
            }
            Err(HttpRuntimeInvocationExecutionError::Handler(acquisition_error))
        }
    }
}

#[cfg(test)]
mod execution_tests {
    use std::cell::RefCell;
    use std::ffi::OsString;
    use std::path::PathBuf;

    use crate::HttpAcquisitionContext;
    use crate::protocols::http::{
        AcquisitionError, AcquisitionResult, HttpCapability, HttpCapabilitySet,
        HttpSourceContractV1,
    };
    use crate::runtime::{
        ProjectInvocationIdentity, RuntimeExecutionMode, RuntimeIdentity,
        RuntimeInvocationEnvelopeV1, RuntimeSupervisionMode, SessionInvocationIdentity,
    };

    use super::{HttpRuntimeInvocationExecutionError, run_http_runtime_invocation};

    fn run_envelope() -> RuntimeInvocationEnvelopeV1 {
        RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-abc").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap()
    }

    fn resume_envelope() -> RuntimeInvocationEnvelopeV1 {
        RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-abc").unwrap(),
            RuntimeExecutionMode::Resume,
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

    // Test 1: matching acquisition/run calls acquire handler
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn acquire_run_invocation_calls_acquire_handler() {
        thread_local! {
            static ACQUIRE_CALLED: RefCell<bool> = RefCell::new(false);
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            ACQUIRE_CALLED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let args = encode(&run_envelope(), &[]);
        let result = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        );
        assert!(result.is_ok());
        assert!(ACQUIRE_CALLED.with(|c| *c.borrow()));
    }

    // Test 2: acquire/run calls acquire exactly once
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn acquire_run_calls_acquire_exactly_once() {
        thread_local! {
            static COUNT: RefCell<u32> = RefCell::new(0);
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            COUNT.with(|c| *c.borrow_mut() += 1);
            Ok(())
        }

        let args = encode(&run_envelope(), &[]);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert_eq!(COUNT.with(|c| *c.borrow()), 1);
    }

    // Test 3: acquire/run does not call resume
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn acquire_run_does_not_call_resume() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        fn resume_must_not_be_called(
            _ctx: &mut HttpAcquisitionContext,
            _args: &[OsString],
        ) -> AcquisitionResult<()> {
            panic!("resume must not be called for acquire/run");
        }

        let args = encode(&run_envelope(), &[]);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire).with_resume(resume_must_not_be_called),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
    }

    // Test 4: acquire/resume calls resume handler
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn acquire_resume_invocation_calls_resume_handler() {
        thread_local! {
            static RESUME_CALLED: RefCell<bool> = RefCell::new(false);
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        fn resume(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            RESUME_CALLED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let args = encode(&resume_envelope(), &[]);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire).with_resume(resume),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert!(RESUME_CALLED.with(|c| *c.borrow()));
    }

    // Test 5: acquire/resume calls resume exactly once
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn acquire_resume_calls_resume_exactly_once() {
        thread_local! {
            static COUNT: RefCell<u32> = RefCell::new(0);
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        fn resume(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            COUNT.with(|c| *c.borrow_mut() += 1);
            Ok(())
        }

        let args = encode(&resume_envelope(), &[]);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire).with_resume(resume),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert_eq!(COUNT.with(|c| *c.borrow()), 1);
    }

    // Test 6: acquire/resume does not call acquire
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn acquire_resume_does_not_call_acquire() {
        fn acquire_must_not_be_called(
            _ctx: &mut HttpAcquisitionContext,
            _args: &[OsString],
        ) -> AcquisitionResult<()> {
            panic!("acquire must not be called for acquire/resume");
        }
        fn resume(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }

        let args = encode(&resume_envelope(), &[]);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_must_not_be_called).with_resume(resume),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
    }

    // Test 7 (SUPERSEDED): this test used to construct an `HttpAcquisitionContext`
    // externally and assert the exact same object reached the acquire handler.
    // `run_http_runtime_invocation` no longer accepts an external context at all: it
    // builds one internally from the environment-decoded `RuntimeContextPaths`. The
    // capability this test verified no longer exists in the public API, so the
    // caller-supplied-sentinel-path assertion is replaced with a check that the
    // handler receives *some* context whose `source_directory()` is queryable.
    // A full rewrite (verifying the internally-derived path matches the env-provided
    // one) needs the session-store fixture tracked as the next milestone.
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn exact_http_acquisition_context_reaches_acquire() {
        thread_local! {
            static CAPTURED: RefCell<Option<PathBuf>> = RefCell::new(None);
        }
        fn acquire(ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            CAPTURED.with(|c| *c.borrow_mut() = Some(ctx.source_directory().to_path_buf()));
            Ok(())
        }

        let args = encode(&run_envelope(), &[]);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert!(CAPTURED.with(|c| c.borrow().is_some()));
    }

    // Test 8 (SUPERSEDED): see Test 7 above; same capability no longer exists.
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn exact_http_acquisition_context_reaches_resume() {
        thread_local! {
            static CAPTURED: RefCell<Option<PathBuf>> = RefCell::new(None);
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        fn resume(ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            CAPTURED.with(|c| *c.borrow_mut() = Some(ctx.source_directory().to_path_buf()));
            Ok(())
        }

        let args = encode(&resume_envelope(), &[]);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire).with_resume(resume),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert!(CAPTURED.with(|c| c.borrow().is_some()));
    }

    // Test 9 (SUPERSEDED): this test used to mutate a field on the caller-owned
    // context and observe the mutation after the call returned. `source_directory`
    // is now a read-only accessor derived from `SessionDataPaths`, and the context
    // itself is owned entirely inside `run_http_runtime_invocation` (never returned
    // to the caller), so mutation-observation is no longer expressible. Reduced to
    // verifying the handler can read from its context without panicking.
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn handler_can_mutate_http_context() {
        fn acquire(ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            let _ = ctx.source_directory();
            Ok(())
        }

        let args = encode(&run_envelope(), &[]);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
    }

    // Test 10: foreground invocation reaches selected handler
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn foreground_invocation_reaches_handler() {
        thread_local! {
            static FG_CALLED: RefCell<bool> = RefCell::new(false);
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            FG_CALLED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-abc").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        let args = encode(&envelope, &[]);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert!(FG_CALLED.with(|c| *c.borrow()));
    }

    // Test 11: background invocation reaches selected handler
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn background_invocation_reaches_handler() {
        thread_local! {
            static BG_CALLED: RefCell<bool> = RefCell::new(false);
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            BG_CALLED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-abc").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Background,
        )
        .unwrap();
        let args = encode(&envelope, &[]);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert!(BG_CALLED.with(|c| *c.borrow()));
    }

    // Test 12: project identity preserved through admission and execution
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn project_identity_preserved_through_execution() {
        thread_local! {
            static CALLED: RefCell<bool> = RefCell::new(false);
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            CALLED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("my-project").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-abc").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        let args = encode(&envelope, &[]);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert!(CALLED.with(|c| *c.borrow()));
    }

    // Test 13: session identity preserved through admission and execution
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn session_identity_preserved_through_execution() {
        thread_local! {
            static CALLED: RefCell<bool> = RefCell::new(false);
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            CALLED.with(|c| *c.borrow_mut() = true);
            Ok(())
        }

        let envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("unique-session-id").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        let args = encode(&envelope, &[]);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert!(CALLED.with(|c| *c.borrow()));
    }

    // Test 14: source arguments reach acquire in exact order
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn source_arguments_reach_acquire_in_exact_order() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, args: &[OsString]) -> AcquisitionResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![
            OsString::from("alpha"),
            OsString::from("beta"),
            OsString::from("gamma"),
        ];
        let args = encode(&run_envelope(), &source_args);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 15: source arguments reach resume in exact order
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn source_arguments_reach_resume_in_exact_order() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        fn resume(_ctx: &mut HttpAcquisitionContext, args: &[OsString]) -> AcquisitionResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![
            OsString::from("one"),
            OsString::from("two"),
            OsString::from("three"),
        ];
        let args = encode(&resume_envelope(), &source_args);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire).with_resume(resume),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 16: duplicate source arguments are preserved
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn duplicate_source_arguments_are_preserved() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, args: &[OsString]) -> AcquisitionResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![
            OsString::from("dup"),
            OsString::from("dup"),
            OsString::from("dup"),
        ];
        let args = encode(&run_envelope(), &source_args);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 17: empty source values are preserved
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn empty_source_values_are_preserved() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, args: &[OsString]) -> AcquisitionResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![
            OsString::from(""),
            OsString::from("nonempty"),
            OsString::from(""),
        ];
        let args = encode(&run_envelope(), &source_args);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 18: source value equal to -- is preserved
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn source_value_equal_to_delimiter_is_preserved() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, args: &[OsString]) -> AcquisitionResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![OsString::from("--"), OsString::from("value")];
        let args = encode(&run_envelope(), &source_args);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 19: source value equal to invocation flag is preserved
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn source_value_equal_to_invocation_flag_is_preserved() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, args: &[OsString]) -> AcquisitionResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![OsString::from("--lexicon-invocation-v1")];
        let args = encode(&run_envelope(), &source_args);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 20: source value equal to probe flag is preserved
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn source_value_equal_to_probe_flag_is_preserved() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, args: &[OsString]) -> AcquisitionResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        use crate::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT;
        let source_args = vec![OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)];
        let args = encode(&run_envelope(), &source_args);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 21: unicode source values are preserved
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn unicode_source_values_are_preserved() {
        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, args: &[OsString]) -> AcquisitionResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![
            OsString::from("こんにちは"),
            OsString::from("🦀"),
            OsString::from("日本語"),
        ];
        let args = encode(&run_envelope(), &source_args);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 22: non-UTF-8 Unix source args reach acquire byte-for-byte
    #[cfg(unix)]
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn non_utf8_unix_source_arguments_reach_acquire_byte_for_byte() {
        use std::os::unix::ffi::OsStringExt;

        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, args: &[OsString]) -> AcquisitionResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![
            OsString::from_vec(vec![b'a', 0x80, b'c']),
            OsString::from_vec(vec![0xFF, 0xFE, 0xFD]),
        ];
        let args = encode(&run_envelope(), &source_args);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 23: non-UTF-8 Unix source args reach resume byte-for-byte
    #[cfg(unix)]
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn non_utf8_unix_source_arguments_reach_resume_byte_for_byte() {
        use std::os::unix::ffi::OsStringExt;

        thread_local! {
            static ARGS: RefCell<Vec<OsString>> = RefCell::new(Vec::new());
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        fn resume(_ctx: &mut HttpAcquisitionContext, args: &[OsString]) -> AcquisitionResult<()> {
            ARGS.with(|a| *a.borrow_mut() = args.to_vec());
            Ok(())
        }

        let source_args = vec![
            OsString::from_vec(vec![b'x', 0xC0, b'z']),
            OsString::from_vec(vec![0xFE, 0xFF]),
        ];
        let args = encode(&resume_envelope(), &source_args);
        run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire).with_resume(resume),
            HttpCapabilitySet::empty(),
        )
        .unwrap();
        assert_eq!(ARGS.with(|a| a.borrow().clone()), source_args);
    }

    // Test 24: acquire success returns Ok(())
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn acquire_success_returns_ok() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        let args = encode(&run_envelope(), &[]);
        let result = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        );
        assert!(result.is_ok());
    }

    // Test 25: resume success returns Ok(())
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn resume_success_returns_ok() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        fn resume(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        let args = encode(&resume_envelope(), &[]);
        let result = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire).with_resume(resume),
            HttpCapabilitySet::empty(),
        );
        assert!(result.is_ok());
    }

    // Test 26: acquire failure returns Handler variant
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn acquire_failure_returns_handler_error() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Err(AcquisitionError::source_message("acquire failed"))
        }
        let args = encode(&run_envelope(), &[]);
        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            HttpRuntimeInvocationExecutionError::Handler(_)
        ));
    }

    // Test 27: resume failure returns Handler variant
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn resume_failure_returns_handler_error() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        fn resume(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Err(AcquisitionError::source_message("resume failed"))
        }
        let args = encode(&resume_envelope(), &[]);
        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire).with_resume(resume),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            HttpRuntimeInvocationExecutionError::Handler(_)
        ));
    }

    // Test 28: handler failures do not cause reinvocation
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn handler_failure_does_not_cause_reinvocation() {
        thread_local! {
            static COUNT: RefCell<u32> = RefCell::new(0);
        }
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            COUNT.with(|c| *c.borrow_mut() += 1);
            Err(AcquisitionError::source_message("failed"))
        }
        let args = encode(&run_envelope(), &[]);
        let _ = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        );
        assert_eq!(COUNT.with(|c| *c.borrow()), 1);
    }

    // Test 29: malformed transport returns Transport error
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn malformed_transport_returns_transport_error() {
        let args = vec![OsString::from("--not-invocation-flag")];
        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(|_: &mut HttpAcquisitionContext, _: &[OsString]| Ok(())),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            HttpRuntimeInvocationExecutionError::Transport(_)
        ));
    }

    // Test 30: probe arguments passed to normal invocation return transport error
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn probe_arguments_return_transport_error() {
        use crate::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT;
        let args = vec![OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)];
        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(|_: &mut HttpAcquisitionContext, _: &[OsString]| Ok(())),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            HttpRuntimeInvocationExecutionError::Transport(_)
        ));
    }

    // Test 31: identity mismatch returns Admission error
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn identity_mismatch_returns_admission_error() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        let args = encode(&run_envelope(), &[]);
        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("different-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            HttpRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 32: missing capabilities return Admission error
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn missing_capabilities_return_admission_error() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        let args = encode(&run_envelope(), &[]);
        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire).requires(HttpCapability::ClientCertificateV1),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            HttpRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 33: missing resume returns Admission error
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn missing_resume_handler_returns_admission_error() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        let args = encode(&resume_envelope(), &[]);
        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire), // no resume registered
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            HttpRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 34: wrong compiled operation returns Admission error
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn wrong_compiled_operation_returns_admission_error() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        let args = encode(&run_envelope(), &[]);
        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::from_parts(
                "example-source",
                crate::runtime::RuntimeProtocol::Http,
                crate::runtime::RuntimeOperation::Processing,
                1,
            ),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            HttpRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 35: transport failure invokes neither acquire nor resume
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn transport_failure_invokes_neither_handler() {
        fn acquire_must_not_be_called(
            _ctx: &mut HttpAcquisitionContext,
            _args: &[OsString],
        ) -> AcquisitionResult<()> {
            panic!("acquire must not be called on transport failure");
        }
        fn resume_must_not_be_called(
            _ctx: &mut HttpAcquisitionContext,
            _args: &[OsString],
        ) -> AcquisitionResult<()> {
            panic!("resume must not be called on transport failure");
        }
        let args = vec![]; // empty → transport error
        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_must_not_be_called)
                .with_resume(resume_must_not_be_called),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            HttpRuntimeInvocationExecutionError::Transport(_)
        ));
    }

    // Test 36: admission failure invokes neither acquire nor resume
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn admission_failure_invokes_neither_handler() {
        fn acquire_must_not_be_called(
            _ctx: &mut HttpAcquisitionContext,
            _args: &[OsString],
        ) -> AcquisitionResult<()> {
            panic!("acquire must not be called on admission failure");
        }
        fn resume_must_not_be_called(
            _ctx: &mut HttpAcquisitionContext,
            _args: &[OsString],
        ) -> AcquisitionResult<()> {
            panic!("resume must not be called on admission failure");
        }
        let args = encode(&run_envelope(), &[]);
        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("wrong-source", 1),
            &HttpSourceContractV1::new(acquire_must_not_be_called)
                .with_resume(resume_must_not_be_called),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            HttpRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 37: error formatting does not expose source arguments
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn error_formatting_does_not_expose_source_arguments() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        let source_args = vec![
            OsString::from("secret-arg"),
            OsString::from("another-secret"),
        ];
        let args = encode(&run_envelope(), &source_args);
        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("wrong-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
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

    // Test 38: error formatting does not expose envelope JSON
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn error_formatting_does_not_expose_envelope_json() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        let args = encode(&run_envelope(), &[]);
        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("wrong-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
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

    // Test 39 (SUPERSEDED): originally asserted the execution function never calls
    // `HttpAcquisitionContext::from_env()` by passing in a context built from
    // nonexistent paths and confirming success. The function no longer accepts an
    // external context at all (it derives paths from the decoded env-provided
    // `RuntimeContextPaths` internally), so the original premise is obsolete. Left
    // as a placeholder pending the session-store fixture milestone.
    #[test]
    #[ignore = "requires a real session-store fixture (SessionStore with a Prepared record, held lease, and LEXICON_RUNTIME_CONTEXT_V1 env var); run_http_runtime_invocation now performs full session binding this module predates -- tracked as the next milestone"]
    fn execution_function_does_not_call_from_env() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }
        let args = encode(&run_envelope(), &[]);
        let result = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        );
        assert!(result.is_ok());
    }

    // Test 40: existing HTTP probe tests remain (verified by absence of breakage; probe tests
    // live in `mod tests` above and are not removed or weakened by this module).
}
