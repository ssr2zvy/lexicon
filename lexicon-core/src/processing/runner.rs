use std::ffi::{OsStr, OsString};
use std::fmt;

use super::ProcessingSourceContractV1;
use crate::processing::{
    ProcessingRuntimeInformationConstructionError, ProcessingRuntimeInformationEncodingError,
    ProcessingRuntimeInformationV1,
};
use crate::runtime::RuntimeIdentity;

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
                write!(formatter, "processing runtime information construction error: {error}")
            }
            Self::Encoding(error) => {
                write!(formatter, "processing runtime information encoding error: {error}")
            }
            Self::Output(error) => {
                write!(formatter, "processing runtime information probe output error: {error}")
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

        assert_eq!(outcome, ProcessingRuntimeInformationProbeOutcome::NotRequested);
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

        assert_eq!(outcome, ProcessingRuntimeInformationProbeOutcome::NotRequested);
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

        assert_eq!(outcome, ProcessingRuntimeInformationProbeOutcome::NotRequested);
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
        let parsed = ProcessingRuntimeInformationV1::from_json(text.trim_end_matches('\n')).unwrap();

        assert_eq!(parsed.identity(), identity);
        assert_eq!(parsed.descriptor_contract_version(), ProcessingSourceContractV1::CONTRACT_VERSION);
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
        assert_eq!(parsed.descriptor_contract_version(), ProcessingSourceContractV1::CONTRACT_VERSION);
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
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT), OsString::from("extra")],
            &mut Vec::new(),
        );

        assert!(matches!(result, Err(ProcessingRuntimeInformationProbeError::UnexpectedArguments)));
    }

    #[test]
    fn later_position_probe_argument_returns_not_requested() {
        let mut output = Vec::new();
        let outcome = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from("--another-mode"), OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();

        assert_eq!(outcome, ProcessingRuntimeInformationProbeOutcome::NotRequested);
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

        assert!(matches!(err, ProcessingRuntimeInformationProbeError::Construction(_)));
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

        assert!(matches!(err, ProcessingRuntimeInformationProbeError::Construction(_)));
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
        let mut writer = RecordingWriter { fail_write: true, ..Default::default() };
        let result = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut writer,
        );

        assert!(matches!(result, Err(ProcessingRuntimeInformationProbeError::Output(_))));
    }

    #[test]
    fn flush_failure_returns_output_error() {
        let mut writer = RecordingWriter { fail_flush: true, ..Default::default() };
        let result = try_write_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut writer,
        );

        assert!(matches!(result, Err(ProcessingRuntimeInformationProbeError::Output(_))));
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

        assert_eq!(outcome, ProcessingRuntimeInformationProbeOutcome::NotRequested);
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
