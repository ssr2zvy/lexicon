use std::fmt;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use lexicon_core::processing::{
    ProcessingRuntimeCompatibilityError, ProcessingRuntimeInformationDecodingError,
    ProcessingRuntimeInformationV1,
};
use lexicon_core::protocols::http::runner::RUNTIME_INFORMATION_PROBE_ARGUMENT;
use lexicon_core::runtime::{
    RuntimeCompatibilityError, RuntimeIdentity, RuntimeInformationDecodingError, RuntimeInformationV1,
};

pub const MAX_RUNTIME_INFORMATION_PROBE_BYTES: usize = 64 * 1024;
pub const RUNTIME_INFORMATION_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_RUNTIME_INFORMATION_PROBE_STDERR_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedCapturedStream {
    retained: Vec<u8>,
    truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedRuntimeInformation {
    information: RuntimeInformationV1,
}

impl AdmittedRuntimeInformation {
    pub fn information(&self) -> &RuntimeInformationV1 {
        &self.information
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedProcessingRuntimeInformation {
    information: ProcessingRuntimeInformationV1,
}

impl AdmittedProcessingRuntimeInformation {
    pub fn information(&self) -> &ProcessingRuntimeInformationV1 {
        &self.information
    }
}

#[derive(Debug)]
enum RuntimeProbeOutputBoundaryError {
    OutputTooLarge {
        maximum: usize,
        actual: usize,
    },
    EmptyOutput,
    ContainsNul,
    InvalidUtf8(std::str::Utf8Error),
    InvalidOutputBoundary,
}

#[derive(Debug)]
pub enum RuntimeProbeAdmissionError {
    OutputTooLarge {
        maximum: usize,
        actual: usize,
    },
    EmptyOutput,
    ContainsNul,
    InvalidUtf8(std::str::Utf8Error),
    InvalidOutputBoundary,
    Decode(RuntimeInformationDecodingError),
    Incompatible(RuntimeCompatibilityError),
}

#[derive(Debug)]
pub enum ProcessingRuntimeProbeAdmissionError {
    OutputTooLarge {
        maximum: usize,
        actual: usize,
    },
    EmptyOutput,
    ContainsNul,
    InvalidUtf8(std::str::Utf8Error),
    InvalidOutputBoundary,
    Decode(ProcessingRuntimeInformationDecodingError),
    Incompatible(ProcessingRuntimeCompatibilityError),
}

impl fmt::Display for RuntimeProbeAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputTooLarge { maximum, actual } => write!(
                formatter,
                "runtime information probe output exceeds {} bytes (actual: {actual})",
                maximum
            ),
            Self::EmptyOutput => formatter.write_str("runtime information probe output is empty"),
            Self::ContainsNul => formatter.write_str("runtime information probe output contains a NUL byte"),
            Self::InvalidUtf8(error) => write!(formatter, "runtime information probe output is not valid UTF-8: {error}"),
            Self::InvalidOutputBoundary => formatter.write_str("runtime information probe output does not match the required exact boundary"),
            Self::Decode(error) => write!(formatter, "runtime information probe decode failed: {error}"),
            Self::Incompatible(error) => write!(formatter, "runtime information probe compatibility validation failed: {error}"),
        }
    }
}

impl std::error::Error for RuntimeProbeAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidUtf8(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Incompatible(error) => Some(error),
            Self::OutputTooLarge { .. }
            | Self::EmptyOutput
            | Self::ContainsNul
            | Self::InvalidOutputBoundary => None,
        }
    }
}

impl fmt::Display for ProcessingRuntimeProbeAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputTooLarge { maximum, actual } => write!(
                formatter,
                "processing runtime information probe output exceeds {} bytes (actual: {actual})",
                maximum
            ),
            Self::EmptyOutput => formatter.write_str("processing runtime information probe output is empty"),
            Self::ContainsNul => formatter.write_str("processing runtime information probe output contains a NUL byte"),
            Self::InvalidUtf8(error) => write!(formatter, "processing runtime information probe output is not valid UTF-8: {error}"),
            Self::InvalidOutputBoundary => formatter.write_str("processing runtime information probe output does not match the required exact boundary"),
            Self::Decode(error) => write!(formatter, "processing runtime information probe decode failed: {error}"),
            Self::Incompatible(error) => write!(formatter, "processing runtime information probe compatibility validation failed: {error}"),
        }
    }
}

impl std::error::Error for ProcessingRuntimeProbeAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidUtf8(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Incompatible(error) => Some(error),
            Self::OutputTooLarge { .. }
            | Self::EmptyOutput
            | Self::ContainsNul
            | Self::InvalidOutputBoundary => None,
        }
    }
}

fn validate_runtime_probe_output(stdout: &[u8]) -> Result<&str, RuntimeProbeOutputBoundaryError> {
    if stdout.len() > MAX_RUNTIME_INFORMATION_PROBE_BYTES {
        return Err(RuntimeProbeOutputBoundaryError::OutputTooLarge {
            maximum: MAX_RUNTIME_INFORMATION_PROBE_BYTES,
            actual: stdout.len(),
        });
    }

    if stdout.is_empty() {
        return Err(RuntimeProbeOutputBoundaryError::EmptyOutput);
    }

    if stdout.iter().any(|byte| *byte == 0) {
        return Err(RuntimeProbeOutputBoundaryError::ContainsNul);
    }

    let text = std::str::from_utf8(stdout).map_err(RuntimeProbeOutputBoundaryError::InvalidUtf8)?;

    if !text.ends_with('\n') {
        return Err(RuntimeProbeOutputBoundaryError::InvalidOutputBoundary);
    }

    if text.bytes().filter(|byte| *byte == b'\n').count() != 1 {
        return Err(RuntimeProbeOutputBoundaryError::InvalidOutputBoundary);
    }

    if text.starts_with('\n') || text.starts_with('\r') || text.contains('\r') {
        return Err(RuntimeProbeOutputBoundaryError::InvalidOutputBoundary);
    }

    let json_text = &text[..text.len() - 1];
    if json_text
        .chars()
        .next()
        .is_some_and(|character| character.is_whitespace())
        || json_text
            .chars()
            .next_back()
            .is_some_and(|character| character.is_whitespace())
    {
        return Err(RuntimeProbeOutputBoundaryError::InvalidOutputBoundary);
    }

    Ok(json_text)
}

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

impl fmt::Display for RuntimeProbeExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { source } => write!(formatter, "failed to spawn runtime information probe: {source}"),
            Self::Wait { source } => write!(formatter, "failed waiting for runtime information probe to exit: {source}"),
            Self::Timeout { timeout, cleanup_error } => {
                let mut message = format!("runtime information probe timed out after {timeout:?}");
                if let Some(cleanup_error) = cleanup_error {
                    message.push_str(&format!(" (cleanup: {cleanup_error})"));
                }
                formatter.write_str(&message)
            }
            Self::StdoutRead { source } => write!(formatter, "failed reading stdout from runtime information probe: {source}"),
            Self::StderrRead { source } => write!(formatter, "failed reading stderr from runtime information probe: {source}"),
            Self::StdoutTooLarge { maximum } => write!(formatter, "runtime information probe stdout exceeded {maximum} bytes"),
            Self::StderrTooLarge { maximum } => write!(formatter, "runtime information probe stderr exceeded {maximum} bytes"),
            Self::UnsuccessfulExit { status, .. } => write!(formatter, "runtime information probe exited unsuccessfully: {status}"),
            Self::Admission(error) => write!(formatter, "runtime information probe output was rejected: {error}"),
        }
    }
}

impl std::error::Error for RuntimeProbeExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source } => Some(source),
            Self::Wait { source } => Some(source),
            Self::StdoutRead { source } => Some(source),
            Self::StderrRead { source } => Some(source),
            Self::Timeout { .. }
            | Self::StdoutTooLarge { .. }
            | Self::StderrTooLarge { .. }
            | Self::UnsuccessfulExit { .. }
            | Self::Admission(_) => None,
        }
    }
}

fn drain_bounded_stream<R: Read>(reader: R, maximum: usize) -> io::Result<BoundedCapturedStream> {
    let mut reader = reader;
    let mut retained = Vec::new();
    let mut truncated = false;
    let mut buffer = [0_u8; 4096];

    loop {
        let bytes_read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => bytes_read,
            Err(error) => return Err(error),
        };

        let mut consumed = 0;
        while consumed < bytes_read {
            if retained.len() >= maximum {
                truncated = true;
                break;
            }
            let remaining_capacity = maximum - retained.len();
            let available = bytes_read - consumed;
            let chunk_len = remaining_capacity.min(available);
            retained.extend_from_slice(&buffer[consumed..consumed + chunk_len]);
            consumed += chunk_len;
            if retained.len() == maximum && consumed < bytes_read {
                truncated = true;
            }
        }
    }

    Ok(BoundedCapturedStream { retained, truncated })
}

pub(crate) fn probe_http_runtime_information_with_timeout(
    executable: &Path,
    expected_identity: RuntimeIdentity,
    timeout: Duration,
) -> Result<AdmittedRuntimeInformation, RuntimeProbeExecutionError> {
    let mut child = {
        let mut command = Command::new(executable);
        command
            .arg(RUNTIME_INFORMATION_PROBE_ARGUMENT)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        {
            command.process_group(0);
        }

        command.spawn().map_err(|source| RuntimeProbeExecutionError::Spawn { source })?
    };

    let stdout = child.stdout.take().expect("stdout piped for runtime probe");
    let stderr = child.stderr.take().expect("stderr piped for runtime probe");

    let stdout_handle = thread::spawn(move || drain_bounded_stream(stdout, MAX_RUNTIME_INFORMATION_PROBE_BYTES));
    let stderr_handle = thread::spawn(move || drain_bounded_stream(stderr, MAX_RUNTIME_INFORMATION_PROBE_STDERR_BYTES));

    let deadline = Instant::now() + timeout;
    let mut exit_status: Option<ExitStatus> = None;
    let mut wait_error: Option<std::io::Error> = None;
    let mut timeout_error: Option<String> = None;
    let mut timed_out = false;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_status = Some(status);
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;

                   #[cfg(unix)]
                   {
                       let group_id = child.id() as i32;
                       let kill_result = Command::new("kill")
                           .arg("-KILL")
                           .arg(format!("-{group_id}"))
                           .status();
                       if let Err(error) = kill_result {
                           timeout_error = Some(format!("process-group kill failed: {error}"));
                       }
                   }

                   #[cfg(not(unix))]
                   {
                       if let Err(error) = child.kill() {
                           timeout_error = Some(format!("kill failed: {error}"));
                       }
                   }

                   match child.wait() {
                       Ok(status) => {
                           exit_status = Some(status);
                       }
                       Err(error) => {
                           if let Some(existing) = timeout_error.as_mut() {
                               existing.push_str(&format!("; wait failed: {error}"));
                           } else {
                               timeout_error = Some(format!("wait failed: {error}"));
                           }
                       }
                   }
                   break;
               }

                let remaining = deadline.saturating_duration_since(Instant::now());
                thread::sleep(remaining.min(Duration::from_millis(10)));
            }
            Err(error) => {
                wait_error = Some(error);
                break;
            }
        }
    }

    if !timed_out && wait_error.is_none() {
        match child.wait() {
            Ok(status) => exit_status = Some(status),
            Err(error) => wait_error = Some(error),
        }
    }

    let stdout_result = stdout_handle.join();
    let stderr_result = stderr_handle.join();

    let mut stdout_read_error = None;
    let mut stderr_read_error = None;
    let stdout_capture = match stdout_result {
        Ok(Ok(stream)) => Some(stream),
        Ok(Err(error)) => {
            stdout_read_error = Some(error);
            None
        }
        Err(_) => {
            stdout_read_error = Some(std::io::Error::new(
                std::io::ErrorKind::Other,
                "stdout drainer thread panicked",
            ));
            None
        }
    };

    let stderr_capture = match stderr_result {
        Ok(Ok(stream)) => Some(stream),
        Ok(Err(error)) => {
            stderr_read_error = Some(error);
            None
        }
        Err(_) => {
            stderr_read_error = Some(std::io::Error::new(
                std::io::ErrorKind::Other,
                "stderr drainer thread panicked",
            ));
            None
        }
    };

    if timed_out {
        return Err(RuntimeProbeExecutionError::Timeout {
            timeout,
            cleanup_error: timeout_error,
        });
    }

    if let Some(source) = wait_error {
        return Err(RuntimeProbeExecutionError::Wait { source });
    }

    if let Some(source) = stdout_read_error {
        return Err(RuntimeProbeExecutionError::StdoutRead { source });
    }

    if let Some(source) = stderr_read_error {
        return Err(RuntimeProbeExecutionError::StderrRead { source });
    }

    let stdout_bytes = stdout_capture
        .as_ref()
        .map(|stream| stream.retained.clone())
        .unwrap_or_default();
    let stderr_bytes = stderr_capture
        .as_ref()
        .map(|stream| stream.retained.clone())
        .unwrap_or_default();

    if stdout_capture
        .as_ref()
        .is_some_and(|stream| stream.truncated)
    {
        return Err(RuntimeProbeExecutionError::StdoutTooLarge {
            maximum: MAX_RUNTIME_INFORMATION_PROBE_BYTES,
        });
    }

    if stderr_capture
        .as_ref()
        .is_some_and(|stream| stream.truncated)
    {
        return Err(RuntimeProbeExecutionError::StderrTooLarge {
            maximum: MAX_RUNTIME_INFORMATION_PROBE_STDERR_BYTES,
        });
    }

    let exit_status = match exit_status {
        Some(status) => status,
        None => {
            return Err(RuntimeProbeExecutionError::Wait {
                source: std::io::Error::new(std::io::ErrorKind::Other, "runtime information probe exited without a status"),
            });
        }
    };

    if !exit_status.success() {
        return Err(RuntimeProbeExecutionError::UnsuccessfulExit {
            status: exit_status,
            stderr: stderr_bytes,
            stderr_truncated: stderr_capture
                .as_ref()
                .is_some_and(|stream| stream.truncated),
        });
    }

    match admit_http_runtime_information_probe(expected_identity, &stdout_bytes) {
        Ok(admitted) => Ok(admitted),
        Err(error) => Err(RuntimeProbeExecutionError::Admission(error)),
    }
}

pub fn probe_http_runtime_information(
    executable: &Path,
    expected_identity: RuntimeIdentity,
) -> Result<AdmittedRuntimeInformation, RuntimeProbeExecutionError> {
    probe_http_runtime_information_with_timeout(
        executable,
        expected_identity,
        RUNTIME_INFORMATION_PROBE_TIMEOUT,
    )
}

pub fn admit_http_runtime_information_probe(
    expected_identity: RuntimeIdentity,
    stdout: &[u8],
) -> Result<AdmittedRuntimeInformation, RuntimeProbeAdmissionError> {
    let json_text = validate_runtime_probe_output(stdout).map_err(|error| match error {
        RuntimeProbeOutputBoundaryError::OutputTooLarge { maximum, actual } => {
            RuntimeProbeAdmissionError::OutputTooLarge { maximum, actual }
        }
        RuntimeProbeOutputBoundaryError::EmptyOutput => RuntimeProbeAdmissionError::EmptyOutput,
        RuntimeProbeOutputBoundaryError::ContainsNul => RuntimeProbeAdmissionError::ContainsNul,
        RuntimeProbeOutputBoundaryError::InvalidUtf8(error) => {
            RuntimeProbeAdmissionError::InvalidUtf8(error)
        }
        RuntimeProbeOutputBoundaryError::InvalidOutputBoundary => {
            RuntimeProbeAdmissionError::InvalidOutputBoundary
        }
    })?;

    let information = RuntimeInformationV1::from_json(json_text)
        .map_err(RuntimeProbeAdmissionError::Decode)?;
    information
        .validate_compatibility(expected_identity)
        .map_err(RuntimeProbeAdmissionError::Incompatible)?;

    Ok(AdmittedRuntimeInformation { information })
}

pub fn admit_processing_runtime_information_probe(
    expected_identity: RuntimeIdentity,
    stdout: &[u8],
) -> Result<AdmittedProcessingRuntimeInformation, ProcessingRuntimeProbeAdmissionError> {
    let json_text = validate_runtime_probe_output(stdout).map_err(|error| match error {
        RuntimeProbeOutputBoundaryError::OutputTooLarge { maximum, actual } => {
            ProcessingRuntimeProbeAdmissionError::OutputTooLarge { maximum, actual }
        }
        RuntimeProbeOutputBoundaryError::EmptyOutput => ProcessingRuntimeProbeAdmissionError::EmptyOutput,
        RuntimeProbeOutputBoundaryError::ContainsNul => ProcessingRuntimeProbeAdmissionError::ContainsNul,
        RuntimeProbeOutputBoundaryError::InvalidUtf8(error) => {
            ProcessingRuntimeProbeAdmissionError::InvalidUtf8(error)
        }
        RuntimeProbeOutputBoundaryError::InvalidOutputBoundary => {
            ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary
        }
    })?;

    let information = ProcessingRuntimeInformationV1::from_json(json_text)
        .map_err(ProcessingRuntimeProbeAdmissionError::Decode)?;
    information
        .validate_compatibility(expected_identity)
        .map_err(ProcessingRuntimeProbeAdmissionError::Incompatible)?;

    Ok(AdmittedProcessingRuntimeInformation { information })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;

    use lexicon_core::processing::{
        ProcessingContext, ProcessingResult, ProcessingRuntimeCompatibilityError,
        ProcessingRuntimeInformationV1, ProcessingSourceContractV1,
    };
    use lexicon_core::protocols::http::{HttpCapability, HttpCapabilitySet, HttpSourceContractV1};
    use lexicon_core::runtime::{RuntimeCompatibilityError, RuntimeIdentity, RuntimeInformationV1};
    use lexicon_core::{
        HttpAcquisitionContext,
        protocols::http::runner::{
            RUNTIME_INFORMATION_PROBE_ARGUMENT, try_write_runtime_information_probe,
        },
    };

    use super::{
        MAX_RUNTIME_INFORMATION_PROBE_BYTES, MAX_RUNTIME_INFORMATION_PROBE_STDERR_BYTES,
        AdmittedProcessingRuntimeInformation, AdmittedRuntimeInformation,
        ProcessingRuntimeProbeAdmissionError, RuntimeProbeAdmissionError, RuntimeProbeExecutionError,
        admit_http_runtime_information_probe, admit_processing_runtime_information_probe,
    };

    fn acquire_handler(
        _context: &mut HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> lexicon_core::protocols::http::AcquisitionResult<()> {
        Ok(())
    }

    fn resume_handler(
        _context: &mut HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> lexicon_core::protocols::http::AcquisitionResult<()> {
        Ok(())
    }

    fn failing_acquire(
        _context: &mut HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> lexicon_core::protocols::http::AcquisitionResult<()> {
        panic!("acquire should not be invoked while admitting runtime information")
    }

    fn failing_resume(
        _context: &mut HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> lexicon_core::protocols::http::AcquisitionResult<()> {
        panic!("resume should not be invoked while admitting runtime information")
    }

    fn process_handler(
        _context: &mut ProcessingContext,
        _args: &[std::ffi::OsString],
    ) -> ProcessingResult<()> {
        Ok(())
    }

    fn failing_process_handler(
        _context: &mut ProcessingContext,
        _args: &[std::ffi::OsString],
    ) -> ProcessingResult<()> {
        panic!("processing handler should not be invoked while admitting runtime information")
    }

    fn valid_probe_output(
        identity: RuntimeIdentity,
        source: &HttpSourceContractV1,
        available: HttpCapabilitySet,
    ) -> Vec<u8> {
        let mut output = Vec::new();
        try_write_runtime_information_probe(
            identity,
            source,
            available,
            &[OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();
        output
    }

    #[test]
    fn exact_output_from_core_probe_is_admitted() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let available = HttpCapabilitySet::empty();
        let output = valid_probe_output(identity, &source, available);

        let admitted = admit_http_runtime_information_probe(identity, &output).unwrap();
        let json = std::str::from_utf8(&output).unwrap();
        let expected = RuntimeInformationV1::from_json(json.trim_end_matches('\n')).unwrap();

        assert_eq!(admitted.information(), &expected);
    }

    #[test]
    fn admitted_wrapper_exposes_decoded_information() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .with_resume(resume_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let available = HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1);
        let output = valid_probe_output(identity, &source, available);

        let admitted = admit_http_runtime_information_probe(identity, &output).unwrap();
        assert_eq!(admitted.information().identity(), identity);
        assert_eq!(admitted.information().required_capabilities(), source.required_capabilities());
        assert_eq!(admitted.information().available_capabilities(), available);
        assert!(admitted.information().resume_handler_registered());
    }

    #[test]
    fn matching_identity_and_capabilities_succeed() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .with_resume(resume_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let available = HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1);
        let output = valid_probe_output(identity, &source, available);

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn empty_output_is_rejected() {
        let result = admit_http_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &[],
        );
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::EmptyOutput)));
    }

    #[test]
    fn oversized_output_is_rejected_before_decoding() {
        let mut oversized = vec![b'{'];
        while oversized.len() <= MAX_RUNTIME_INFORMATION_PROBE_BYTES {
            oversized.push(b'x');
        }
        oversized.push(b'\n');

        let result = admit_http_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            &oversized,
        );
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::OutputTooLarge { maximum, actual })
                if maximum == MAX_RUNTIME_INFORMATION_PROBE_BYTES && actual > maximum
        ));
    }

    #[test]
    fn nul_containing_output_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(output.len() / 2, 0);

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::ContainsNul)));
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output[0] = 0xff;

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidUtf8(_))));
    }

    #[test]
    fn missing_final_newline_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.pop();

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn two_final_newlines_are_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.push(b'\n');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn carriage_return_line_ending_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(output.len() - 1, b'\r');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn leading_spaces_are_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(0, b' ');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn leading_newline_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(0, b'\n');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn trailing_spaces_before_final_newline_are_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(output.len() - 1, b' ');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn diagnostic_text_before_json_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.splice(..0, b"noise ".iter().copied());

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::Decode(_))));
    }

    #[test]
    fn diagnostic_text_after_json_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(output.len() - 1, b'x');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)
                | Err(RuntimeProbeAdmissionError::Decode(_))
        ));
    }

    #[test]
    fn multiple_json_documents_are_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.extend_from_slice(&valid_probe_output(identity, &source, HttpCapabilitySet::empty()));

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)
                | Err(RuntimeProbeAdmissionError::Decode(_))
        ));
    }

    #[test]
    fn structurally_invalid_json_produces_decode_error() {
        let result = admit_http_runtime_information_probe(
            RuntimeIdentity::http_acquisition("example-source", 1),
            b"{not json}\n",
        );
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::Decode(_))));
    }

    #[test]
    fn unknown_schema_version_produces_decode_error() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());

        let json = String::from_utf8(output.clone()).unwrap();
        let mut document: serde_json::Value = serde_json::from_str(json.trim_end()).unwrap();
        document["schema_version"] = serde_json::Value::from(2);
        let mut candidate = serde_json::to_vec(&document).unwrap();
        candidate.push(b'\n');

        let result = admit_http_runtime_information_probe(identity, &candidate);
        assert!(matches!(result, Err(RuntimeProbeAdmissionError::Decode(_))));
    }

    #[test]
    fn identity_disagreement_produces_incompatible_error() {
        let actual_identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let output = valid_probe_output(actual_identity, &source, HttpCapabilitySet::empty());

        let result = admit_http_runtime_information_probe(
            RuntimeIdentity::http_acquisition("other-source", 1),
            &output,
        );
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::Incompatible(
                RuntimeCompatibilityError::IdentityMismatch { expected, actual }
            )) if expected == RuntimeIdentity::http_acquisition("other-source", 1)
                && actual == actual_identity
        ));
    }

    #[test]
    fn descriptor_version_disagreement_produces_incompatible_error() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());

        let json = String::from_utf8(output.clone()).unwrap();
        let mut document: serde_json::Value = serde_json::from_str(json.trim_end()).unwrap();
        document["descriptor"]["contract_version"] = serde_json::Value::from(2);
        let mut candidate = serde_json::to_vec(&document).unwrap();
        candidate.push(b'\n');

        let result = admit_http_runtime_information_probe(identity, &candidate);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::Incompatible(
                RuntimeCompatibilityError::DescriptorContractVersionMismatch {
                    identity_version,
                    descriptor_version,
                }
            )) if identity_version == 1 && descriptor_version == 2
        ));
    }

    #[test]
    fn missing_required_capabilities_produce_incompatible_error() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let available = HttpCapabilitySet::empty();
        let output = valid_probe_output(identity, &source, available);

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::Incompatible(
                RuntimeCompatibilityError::MissingCapabilities(missing)
            )) if missing.missing() == HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1)
        ));
    }

    #[test]
    fn missing_capability_set_remains_inspectable() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .requires(HttpCapability::ClientCertificateV1)
            .requires(HttpCapability::ClientCertificateV1);
        let available = HttpCapabilitySet::empty();
        let output = valid_probe_output(identity, &source, available);

        let error = admit_http_runtime_information_probe(identity, &output).unwrap_err();
        match error {
            RuntimeProbeAdmissionError::Incompatible(RuntimeCompatibilityError::MissingCapabilities(missing)) => {
                assert_eq!(missing.missing().ordered_capabilities().len(), 1);
                assert_eq!(missing.missing().ordered_capabilities()[0], HttpCapability::ClientCertificateV1);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn admission_does_not_invoke_acquire_or_resume_handlers() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(failing_acquire).with_resume(failing_resume);
        let available = HttpCapabilitySet::empty();
        let output = valid_probe_output(identity, &source, available);

        let admitted = admit_http_runtime_information_probe(identity, &output).unwrap();
        assert_eq!(admitted.information().identity(), identity);
    }

    #[test]
    fn type_is_not_publicly_constructible() {
        let _ = AdmittedRuntimeInformation {
            information: RuntimeInformationV1::from_http_source(
                RuntimeIdentity::http_acquisition("example-source", 1),
                &HttpSourceContractV1::new(acquire_handler),
                HttpCapabilitySet::empty(),
            ),
        };
    }

    fn valid_processing_probe_output(identity: RuntimeIdentity, source: &ProcessingSourceContractV1) -> Vec<u8> {
        let mut output = Vec::new();
        lexicon_core::processing::try_write_runtime_information_probe(
            identity,
            source,
            &[std::ffi::OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)],
            &mut output,
        )
        .unwrap();
        output
    }

    #[test]
    fn processing_probe_output_from_core_is_admitted() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let output = valid_processing_probe_output(identity, &source);

        let admitted = admit_processing_runtime_information_probe(identity, &output).unwrap();
        let expected = ProcessingRuntimeInformationV1::from_json(std::str::from_utf8(&output).unwrap().trim_end_matches('\n')).unwrap();

        assert_eq!(admitted.information(), &expected);
    }

    #[test]
    fn processing_admitted_wrapper_exposes_decoded_information() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let output = valid_processing_probe_output(identity, &source);

        let admitted = admit_processing_runtime_information_probe(identity, &output).unwrap();
        assert_eq!(admitted.information().identity(), identity);
        assert_eq!(admitted.information().descriptor_contract_version(), ProcessingSourceContractV1::CONTRACT_VERSION);
    }

    #[test]
    fn processing_matching_identity_succeeds() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let output = valid_processing_probe_output(identity, &source);

        let result = admit_processing_runtime_information_probe(identity, &output);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn processing_empty_output_is_rejected() {
        let result = admit_processing_runtime_information_probe(RuntimeIdentity::http_processing("example-source", 1), &[]);
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::EmptyOutput)));
    }

    #[test]
    fn processing_oversized_output_is_rejected_before_decoding() {
        let mut oversized = vec![b'{'];
        while oversized.len() <= MAX_RUNTIME_INFORMATION_PROBE_BYTES {
            oversized.push(b'x');
        }
        oversized.push(b'\n');

        let result = admit_processing_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            &oversized,
        );
        assert!(matches!(
            result,
            Err(ProcessingRuntimeProbeAdmissionError::OutputTooLarge { maximum, actual })
                if maximum == MAX_RUNTIME_INFORMATION_PROBE_BYTES && actual > maximum
        ));
    }

    #[test]
    fn processing_nul_containing_output_is_rejected() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let mut output = valid_processing_probe_output(identity, &source);
        output.insert(output.len() / 2, 0);

        let result = admit_processing_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::ContainsNul)));
    }

    #[test]
    fn processing_invalid_utf8_is_rejected() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let mut output = valid_processing_probe_output(identity, &source);
        output[0] = 0xff;

        let result = admit_processing_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::InvalidUtf8(_))));
    }

    #[test]
    fn processing_missing_final_newline_is_rejected() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let mut output = valid_processing_probe_output(identity, &source);
        output.pop();

        let result = admit_processing_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn processing_two_final_newlines_are_rejected() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let mut output = valid_processing_probe_output(identity, &source);
        output.push(b'\n');

        let result = admit_processing_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn processing_carriage_return_line_ending_is_rejected() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let mut output = valid_processing_probe_output(identity, &source);
        output.insert(output.len() - 1, b'\r');

        let result = admit_processing_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn processing_leading_spaces_are_rejected() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let mut output = valid_processing_probe_output(identity, &source);
        output.insert(0, b' ');

        let result = admit_processing_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn processing_leading_newline_is_rejected() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let mut output = valid_processing_probe_output(identity, &source);
        output.insert(0, b'\n');

        let result = admit_processing_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn processing_trailing_spaces_before_newline_are_rejected() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let mut output = valid_processing_probe_output(identity, &source);
        output.insert(output.len() - 1, b' ');

        let result = admit_processing_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn processing_diagnostic_text_is_rejected() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let mut output = valid_processing_probe_output(identity, &source);
        output.splice(..0, b"noise ".iter().copied());

        let result = admit_processing_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::Decode(_) | ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary)));
    }

    #[test]
    fn processing_invalid_json_returns_decode() {
        let result = admit_processing_runtime_information_probe(
            RuntimeIdentity::http_processing("example-source", 1),
            b"{not valid}\n",
        );
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::Decode(_))));
    }

    #[test]
    fn processing_unknown_schema_version_returns_decode() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let output = valid_processing_probe_output(identity, &source);
        let json = String::from_utf8(output.clone()).unwrap();
        let mut document: serde_json::Value = serde_json::from_str(json.trim_end()).unwrap();
        document["schema_version"] = serde_json::Value::from(2);
        let mut candidate = serde_json::to_vec(&document).unwrap();
        candidate.push(b'\n');

        let result = admit_processing_runtime_information_probe(identity, &candidate);
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::Decode(_))));
    }

    #[test]
    fn processing_acquisition_document_returns_decode() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let output = valid_probe_output(RuntimeIdentity::http_acquisition("example-source", 1), &HttpSourceContractV1::new(acquire_handler), HttpCapabilitySet::empty());

        let result = admit_processing_runtime_information_probe(identity, &output);
        assert!(matches!(result, Err(ProcessingRuntimeProbeAdmissionError::Decode(_))));
    }

    #[test]
    fn processing_source_identity_mismatch_returns_incompatible() {
        let actual_identity = RuntimeIdentity::http_processing("example-source", 1);
        let output = valid_processing_probe_output(actual_identity, &ProcessingSourceContractV1::new(process_handler));

        let result = admit_processing_runtime_information_probe(
            RuntimeIdentity::http_processing("other-source", 1),
            &output,
        );
        assert!(matches!(
            result,
            Err(ProcessingRuntimeProbeAdmissionError::Incompatible(
                ProcessingRuntimeCompatibilityError::IdentityMismatch { expected, actual }
            )) if expected == RuntimeIdentity::http_processing("other-source", 1)
                && actual == actual_identity
        ));
    }

    #[test]
    fn processing_descriptor_version_mismatch_returns_incompatible() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(process_handler);
        let output = valid_processing_probe_output(identity, &source);

        let json = String::from_utf8(output.clone()).unwrap();
        let mut document: serde_json::Value = serde_json::from_str(json.trim_end()).unwrap();
        document["descriptor"]["contract_version"] = serde_json::Value::from(2);
        let mut candidate = serde_json::to_vec(&document).unwrap();
        candidate.push(b'\n');

        let result = admit_processing_runtime_information_probe(identity, &candidate);
        assert!(matches!(
            result,
            Err(ProcessingRuntimeProbeAdmissionError::Incompatible(
                ProcessingRuntimeCompatibilityError::DescriptorContractVersionMismatch {
                    identity_version,
                    descriptor_version,
                }
            )) if identity_version == 1 && descriptor_version == 2
        ));
    }

    #[test]
    fn processing_admission_does_not_invoke_processing_handler() {
        let identity = RuntimeIdentity::http_processing("example-source", 1);
        let source = ProcessingSourceContractV1::new(failing_process_handler);
        let output = valid_processing_probe_output(identity, &source);

        let admitted = admit_processing_runtime_information_probe(identity, &output).unwrap();
        assert_eq!(admitted.information().identity(), identity);
    }

    #[test]
    fn processing_admission_wrapper_is_not_publicly_constructible() {
        let _ = AdmittedProcessingRuntimeInformation {
            information: ProcessingRuntimeInformationV1::from_processing_source(
                RuntimeIdentity::http_processing("example-source", 1),
                &ProcessingSourceContractV1::new(process_handler),
            )
            .unwrap(),
        };
    }

    fn probe_fixture(mode: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("probe-fixture.sh");

        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let valid_output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        let valid_text = String::from_utf8(valid_output).unwrap();
        let malformed_text = "{not-valid-json}\n";
        let incompatible_text = String::from_utf8(valid_probe_output(
            RuntimeIdentity::http_acquisition("other-source", 1),
            &source,
            HttpCapabilitySet::empty(),
        ))
        .unwrap();
        let oversized_stdout = "x".repeat(MAX_RUNTIME_INFORMATION_PROBE_BYTES + 1024);
        let oversized_stderr = "y".repeat(MAX_RUNTIME_INFORMATION_PROBE_STDERR_BYTES + 1024);

        let shell_valid = valid_text.replace('\\', "\\\\");
        let shell_malformed = malformed_text.replace('\\', "\\\\");
        let shell_incompatible = incompatible_text.replace('\\', "\\\\");
        let shell_oversized_stdout = oversized_stdout.replace('\\', "\\\\");
        let shell_oversized_stderr = oversized_stderr.replace('\\', "\\\\");

        let script_text = format!(
            r#"#!/usr/bin/env bash
set -eu
VALID_TEXT='{}'
MALFORMED_TEXT='{}'
INCOMPATIBLE_TEXT='{}'
OVERSIZED_STDOUT='{}'
OVERSIZED_STDERR='{}'
case "${{LEXICON_PROBE_FIXTURE_MODE:-{mode}}}" in
  valid)
    printf '%s' "$VALID_TEXT"
    ;;
  malformed-stdout)
    printf '%s' "$MALFORMED_TEXT"
    ;;
  incompatible-runtime)
    printf '%s' "$INCOMPATIBLE_TEXT"
    ;;
  nonzero-exit)
    printf '%s' "$VALID_TEXT"
    echo 'probe failed' >&2
    exit 7
    ;;
  delayed-exit)
    sleep 10
    printf '%s' "$VALID_TEXT"
    ;;
  oversized-stdout)
    printf '%s' "$OVERSIZED_STDOUT"
    ;;
  oversized-stderr)
    printf '%s' "$OVERSIZED_STDERR" >&2
    ;;
  noisy)
    yes 'noise-out' | head -c 200000
    yes 'noise-err' | head -c 200000 >&2
    ;;
  *)
    echo "unexpected mode: ${{LEXICON_PROBE_FIXTURE_MODE:-{mode}}}" >&2
    exit 2
    ;;
esac
"#,
            shell_valid,
            shell_malformed,
            shell_incompatible,
            shell_oversized_stdout,
            shell_oversized_stderr,
            mode = mode,
        );

        std::fs::write(&script, script_text).unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();
        (temp, script)
    }

    #[test]
    fn probe_http_runtime_information_accepts_valid_probe_output() {
        let (_temp, script) = probe_fixture("valid");
        let result = super::probe_http_runtime_information(
            &script,
            RuntimeIdentity::http_acquisition("example-source", 1),
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn probe_http_runtime_information_times_out_for_delayed_exit() {
        let (_temp, script) = probe_fixture("delayed-exit");
        let result = super::probe_http_runtime_information_with_timeout(
            &script,
            RuntimeIdentity::http_acquisition("example-source", 1),
            std::time::Duration::from_millis(50),
        );
        assert!(matches!(result, Err(RuntimeProbeExecutionError::Timeout { .. })));
    }

    #[test]
    fn probe_http_runtime_information_rejects_oversized_stdout() {
        let (_temp, script) = probe_fixture("oversized-stdout");
        let result = super::probe_http_runtime_information(
            &script,
            RuntimeIdentity::http_acquisition("example-source", 1),
        );
        assert!(matches!(result, Err(RuntimeProbeExecutionError::StdoutTooLarge { .. })));
    }

    #[test]
    fn probe_http_runtime_information_rejects_nonzero_exit_status() {
        let (_temp, script) = probe_fixture("nonzero-exit");
        let result = super::probe_http_runtime_information(
            &script,
            RuntimeIdentity::http_acquisition("example-source", 1),
        );
        assert!(matches!(result, Err(RuntimeProbeExecutionError::UnsuccessfulExit { .. })));
    }
}
