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

// A prior `execution_tests` module here (matching-acquire/resume dispatch, source-
// argument fidelity, and success/handler-error assertions) predated
// `run_http_runtime_invocation` growing full session-store/lease/env-context binding
// (see the function's doc comment above: steps 3-8). Those tests never set up a
// `SessionStore`, a `Prepared` session record, a held lease, or the
// `LEXICON_RUNTIME_CONTEXT_V1` env var, so every test that expected the handler to be
// reached failed with `Session(ContextDecode(MissingEnvironmentVariable))`, and were
// removed rather than left disabled with `#[ignore]`. The tests immediately below are
// the subset that never reached session-context decoding in the first place (pure
// transport/admission rejection paths) and remain valid, unmodified coverage.
//
// Full handler-dispatch coverage is restored further down using
// `crate::session::test_support::RuntimeInvocationFixture`, which builds the real
// minimum session-store/lease/runtime-context environment the production path
// requires (see `session::test_support` for the fixture's design).
#[cfg(test)]
mod execution_tests {
    use std::ffi::OsString;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::HttpAcquisitionContext;
    use crate::protocols::http::{
        AcquisitionError, AcquisitionResult, HttpCapability, HttpCapabilitySet,
        HttpSourceContractV1,
    };
    use crate::runtime::{
        ProjectInvocationIdentity, RuntimeExecutionMode, RuntimeIdentity,
        RuntimeInvocationEnvelopeV1, RuntimeSupervisionMode, SessionInvocationIdentity,
    };
    use crate::session::test_support::RuntimeInvocationFixture;

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

    // Test 1: malformed transport returns Transport error
    #[test]
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

    // Test 2: probe arguments passed to normal invocation return transport error
    #[test]
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

    // Test 3: identity mismatch returns Admission error
    #[test]
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

    // Test 4: missing capabilities return Admission error
    #[test]
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

    // Test 5: missing resume returns Admission error
    #[test]
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

    // Test 6: wrong compiled operation returns Admission error
    #[test]
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

    // Test 7: transport failure invokes neither acquire nor resume
    #[test]
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

    // Test 8: admission failure invokes neither acquire nor resume
    #[test]
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

    // Test 9: error formatting does not expose source arguments
    #[test]
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

    // Test 10: error formatting does not expose envelope JSON
    #[test]
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

    // --- Fixture-backed handler-dispatch coverage ---
    //
    // Each test below builds a real `SessionStore`-backed `Prepared` session, an owned
    // lease, and a valid `LEXICON_RUNTIME_CONTEXT_V1` environment via
    // `RuntimeInvocationFixture`, then drives `run_http_runtime_invocation` through the
    // unmodified production path so the acquire/resume handler is genuinely reached.

    // Test 11: a matching invocation reaches the acquire handler exactly once with a
    // real, mutable `HttpAcquisitionContext`.
    #[test]
    fn matching_invocation_reaches_acquire_handler_exactly_once_with_real_context() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn acquire(
            context: &mut HttpAcquisitionContext,
            _args: &[OsString],
        ) -> AcquisitionResult<()> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            assert!(context.session_identity().is_some());
            assert!(
                context
                    .protocol_root()
                    .to_string_lossy()
                    .contains("example-source")
            );
            Ok(())
        }

        let fixture = RuntimeInvocationFixture::foreground_run(RuntimeIdentity::http_acquisition(
            "example-source",
            1,
        ));
        let args = fixture.build_argv(&[]);

        let result = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        );

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    // Test 12: background supervision also reaches the handler.
    #[test]
    fn background_invocation_reaches_handler() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        let fixture = RuntimeInvocationFixture::background_run(RuntimeIdentity::http_acquisition(
            "example-source",
            1,
        ));
        let args = fixture.build_argv(&[]);

        let result = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        );

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    // Test 13: a resume-mode invocation reaches the resume handler, never acquire.
    #[test]
    fn resume_invocation_reaches_resume_handler_not_acquire() {
        static ACQUIRE_CALLS: AtomicUsize = AtomicUsize::new(0);
        static RESUME_CALLS: AtomicUsize = AtomicUsize::new(0);
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            ACQUIRE_CALLS.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn resume(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            RESUME_CALLS.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        let fixture = RuntimeInvocationFixture::new(
            RuntimeIdentity::http_acquisition("example-source", 1),
            RuntimeExecutionMode::Resume,
            RuntimeSupervisionMode::Foreground,
        );
        let args = fixture.build_argv(&[]);

        let result = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire).with_resume(resume),
            HttpCapabilitySet::empty(),
        );

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(ACQUIRE_CALLS.load(Ordering::SeqCst), 0);
        assert_eq!(RESUME_CALLS.load(Ordering::SeqCst), 1);
    }

    // Test 14: source-argument fidelity — order, duplicates, empty values, a literal
    // `--`, reserved-looking flags, and Unicode all survive dispatch unchanged.
    #[test]
    fn source_argument_fidelity_is_preserved_across_dispatch() {
        static SEEN: OnceLock<Vec<OsString>> = OnceLock::new();
        fn acquire(_ctx: &mut HttpAcquisitionContext, args: &[OsString]) -> AcquisitionResult<()> {
            let _ = SEEN.set(args.to_vec());
            Ok(())
        }

        let fixture = RuntimeInvocationFixture::foreground_run(RuntimeIdentity::http_acquisition(
            "example-source",
            1,
        ));
        let source_args = vec![
            OsString::from("alpha"),
            OsString::from("alpha"),
            OsString::from(""),
            OsString::from("--"),
            OsString::from("--looks-like-a-flag"),
            OsString::from("héllo-üñîçødé"),
        ];
        let args = fixture.build_argv(&source_args);

        let result = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        );

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(SEEN.get().unwrap(), &source_args);
    }

    // Test 15: non-UTF-8 Unix source arguments are preserved byte-for-byte.
    #[cfg(unix)]
    #[test]
    fn non_utf8_unix_source_argument_is_preserved_byte_for_byte() {
        use std::os::unix::ffi::OsStringExt;

        static SEEN: OnceLock<Vec<OsString>> = OnceLock::new();
        fn acquire(_ctx: &mut HttpAcquisitionContext, args: &[OsString]) -> AcquisitionResult<()> {
            let _ = SEEN.set(args.to_vec());
            Ok(())
        }

        let fixture = RuntimeInvocationFixture::foreground_run(RuntimeIdentity::http_acquisition(
            "example-source",
            1,
        ));
        let source_args = vec![OsString::from_vec(vec![b'a', 0x80, b'z'])];
        let args = fixture.build_argv(&source_args);

        let result = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        );

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(SEEN.get().unwrap(), &source_args);
    }

    // Test 16: a source-authored error maps to `Handler(_)` with exactly one dispatch.
    #[test]
    fn source_authored_error_returns_handler_error_without_reinvocation() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Err(AcquisitionError::source_message("network unreachable"))
        }

        let fixture = RuntimeInvocationFixture::foreground_run(RuntimeIdentity::http_acquisition(
            "example-source",
            1,
        ));
        let args = fixture.build_argv(&[]);

        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();

        assert!(matches!(err, HttpRuntimeInvocationExecutionError::Handler(_)));
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    // Test 17: a successful handler moves the session Prepared -> Running -> Succeeded.
    #[test]
    fn session_transitions_to_succeeded_after_successful_handler() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Ok(())
        }

        let fixture = RuntimeInvocationFixture::foreground_run(RuntimeIdentity::http_acquisition(
            "example-source",
            1,
        ));
        let args = fixture.build_argv(&[]);

        let result = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        );
        assert!(result.is_ok(), "{result:?}");

        let record = fixture.store().load(fixture.session()).unwrap();
        assert_eq!(record.state(), crate::session::SessionState::Succeeded);
    }

    // Test 18: a source-authored failure moves the session Prepared -> Running -> Failed.
    #[test]
    fn session_transitions_to_failed_after_source_authored_error() {
        fn acquire(_ctx: &mut HttpAcquisitionContext, _args: &[OsString]) -> AcquisitionResult<()> {
            Err(AcquisitionError::source_message("network unreachable"))
        }

        let fixture = RuntimeInvocationFixture::foreground_run(RuntimeIdentity::http_acquisition(
            "example-source",
            1,
        ));
        let args = fixture.build_argv(&[]);

        let result = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire),
            HttpCapabilitySet::empty(),
        );
        assert!(result.is_err());

        let record = fixture.store().load(fixture.session()).unwrap();
        assert_eq!(record.state(), crate::session::SessionState::Failed);
    }

    // Test 19: an envelope session that was never prepared/leased in this fixture's
    // store is rejected before the handler is ever dispatched (lease/invocation
    // session identities must agree).
    #[test]
    fn session_identity_mismatch_is_rejected_before_handler_dispatch() {
        fn acquire_must_not_be_called(
            _ctx: &mut HttpAcquisitionContext,
            _args: &[OsString],
        ) -> AcquisitionResult<()> {
            panic!("acquire must not be called when the envelope session was never prepared");
        }

        // Establishes a real session-store/lease/env-context environment for a
        // different session id than the one encoded below.
        let _fixture = RuntimeInvocationFixture::foreground_run(RuntimeIdentity::http_acquisition(
            "example-source",
            1,
        ));

        let foreign_envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("never-prepared-session").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        let args = vec![
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(foreign_envelope.to_json().unwrap()),
            OsString::from("--"),
        ];

        let err = run_http_runtime_invocation(
            &args,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_must_not_be_called),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();

        assert!(matches!(err, HttpRuntimeInvocationExecutionError::Session(_)));
    }
}
