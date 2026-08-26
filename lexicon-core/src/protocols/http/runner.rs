use std::ffi::{OsStr, OsString};
use std::fmt;

use crate::protocols::http::{HttpCapabilitySet, HttpSourceContractV1};
use crate::runtime::{RuntimeIdentity, RuntimeInformationEncodingError, RuntimeInformationV1};

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
