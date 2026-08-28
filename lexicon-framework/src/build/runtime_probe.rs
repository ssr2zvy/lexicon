use std::fmt;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use lexicon_core::processing::{
    ProcessingRuntimeCompatibilityError, ProcessingRuntimeInformationV1,
};
use lexicon_core::protocols::http::runner::RUNTIME_INFORMATION_PROBE_ARGUMENT;
use lexicon_core::runtime::{
    OwnedRuntimeIdentity, RuntimeCompatibilityError, RuntimeIdentity,
    RuntimeInformationDecodingError, RuntimeInformationV1,
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
pub enum RuntimeProbeAdmissionError {
    OutputTooLarge { maximum: usize, actual: usize },
    EmptyOutput,
    ContainsNul,
    InvalidUtf8(std::str::Utf8Error),
    InvalidOutputBoundary,
    Decode(RuntimeInformationDecodingError),
    Incompatible(RuntimeCompatibilityError),
    IncompatibleOwned(String),
}

#[derive(Debug)]
pub enum ProcessingRuntimeProbeAdmissionError {
    OutputTooLarge { maximum: usize, actual: usize },
    EmptyOutput,
    ContainsNul,
    InvalidUtf8(std::str::Utf8Error),
    InvalidOutputBoundary,
    Decode(lexicon_core::processing::ProcessingRuntimeInformationDecodingError),
    Incompatible(ProcessingRuntimeCompatibilityError),
    IncompatibleOwned(String),
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
            Self::ContainsNul => {
                formatter.write_str("runtime information probe output contains a NUL byte")
            }
            Self::InvalidUtf8(error) => write!(
                formatter,
                "runtime information probe output is not valid UTF-8: {error}"
            ),
            Self::InvalidOutputBoundary => formatter.write_str(
                "runtime information probe output does not match the required exact boundary",
            ),
            Self::Decode(error) => write!(
                formatter,
                "runtime information probe decode failed: {error}"
            ),
            Self::Incompatible(error) => write!(
                formatter,
                "runtime information probe compatibility validation failed: {error}"
            ),
            Self::IncompatibleOwned(description) => {
                write!(formatter, "runtime identity incompatible: {description}")
            }
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
            Self::IncompatibleOwned(description) => write!(formatter, "runtime identity incompatible: {description}"),
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
            | Self::InvalidOutputBoundary
            | Self::IncompatibleOwned(_) => None,
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
            | Self::InvalidOutputBoundary
            | Self::IncompatibleOwned(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum RuntimeProbeTransportError {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedRuntimeProbe {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl CapturedRuntimeProbe {
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
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
    UnexpectedStderr {
        stderr: Vec<u8>,
    },
    Admission(RuntimeProbeAdmissionError),
}

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
    UnexpectedStderr {
        stderr: Vec<u8>,
    },
    Admission(ProcessingRuntimeProbeAdmissionError),
}

impl fmt::Display for RuntimeProbeExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { source } => write!(
                formatter,
                "failed to spawn runtime information probe: {source}"
            ),
            Self::Wait { source } => write!(
                formatter,
                "failed waiting for runtime information probe to exit: {source}"
            ),
            Self::Timeout {
                timeout,
                cleanup_error,
            } => {
                let mut message = format!("runtime information probe timed out after {timeout:?}");
                if let Some(cleanup_error) = cleanup_error {
                    message.push_str(&format!(" (cleanup: {cleanup_error})"));
                }
                formatter.write_str(&message)
            }
            Self::StdoutRead { source } => write!(
                formatter,
                "failed reading stdout from runtime information probe: {source}"
            ),
            Self::StderrRead { source } => write!(
                formatter,
                "failed reading stderr from runtime information probe: {source}"
            ),
            Self::StdoutTooLarge { maximum } => write!(
                formatter,
                "runtime information probe stdout exceeded {maximum} bytes"
            ),
            Self::StderrTooLarge { maximum } => write!(
                formatter,
                "runtime information probe stderr exceeded {maximum} bytes"
            ),
            Self::UnsuccessfulExit { status, .. } => write!(
                formatter,
                "runtime information probe exited unsuccessfully: {status}"
            ),
            Self::UnexpectedStderr { stderr } => write!(
                formatter,
                "runtime information probe wrote unexpected stderr output: {}",
                format_probe_stderr_excerpt(stderr)
            ),
            Self::Admission(error) => write!(
                formatter,
                "runtime information probe output was rejected: {error}"
            ),
        }
    }
}

impl fmt::Display for ProcessingRuntimeProbeExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { source } => write!(
                formatter,
                "failed to spawn processing runtime information probe: {source}"
            ),
            Self::Wait { source } => write!(
                formatter,
                "failed waiting for processing runtime information probe to exit: {source}"
            ),
            Self::Timeout {
                timeout,
                cleanup_error,
            } => {
                let mut message =
                    format!("processing runtime information probe timed out after {timeout:?}");
                if let Some(cleanup_error) = cleanup_error {
                    message.push_str(&format!(" (cleanup: {cleanup_error})"));
                }
                formatter.write_str(&message)
            }
            Self::StdoutRead { source } => write!(
                formatter,
                "failed reading stdout from processing runtime information probe: {source}"
            ),
            Self::StderrRead { source } => write!(
                formatter,
                "failed reading stderr from processing runtime information probe: {source}"
            ),
            Self::StdoutTooLarge { maximum } => write!(
                formatter,
                "processing runtime information probe stdout exceeded {maximum} bytes"
            ),
            Self::StderrTooLarge { maximum } => write!(
                formatter,
                "processing runtime information probe stderr exceeded {maximum} bytes"
            ),
            Self::UnsuccessfulExit { status, .. } => write!(
                formatter,
                "processing runtime information probe exited unsuccessfully: {status}"
            ),
            Self::UnexpectedStderr { stderr } => write!(
                formatter,
                "processing runtime information probe wrote unexpected stderr output: {}",
                format_probe_stderr_excerpt(stderr)
            ),
            Self::Admission(error) => write!(
                formatter,
                "processing runtime information probe output was rejected: {error}"
            ),
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
            | Self::UnexpectedStderr { .. }
            | Self::Admission(_) => None,
        }
    }
}

impl std::error::Error for ProcessingRuntimeProbeExecutionError {
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
            | Self::UnexpectedStderr { .. }
            | Self::Admission(_) => None,
        }
    }
}

fn format_probe_stderr_excerpt(stderr: &[u8]) -> String {
    const MAX_BYTES: usize = 256;

    let retained = &stderr[..stderr.len().min(MAX_BYTES)];
    let mut message = String::from_utf8_lossy(retained).into_owned();
    if stderr.len() > MAX_BYTES {
        message.push_str("…");
    }
    message
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

        if retained.len() >= maximum {
            truncated = true;
            continue;
        }

        let allowed = maximum - retained.len();
        let chunk_len = allowed.min(bytes_read);
        retained.extend_from_slice(&buffer[..chunk_len]);
        if bytes_read > chunk_len {
            truncated = true;
        }
    }

    Ok(BoundedCapturedStream {
        retained,
        truncated,
    })
}

fn map_runtime_probe_transport_error(
    error: RuntimeProbeTransportError,
) -> RuntimeProbeExecutionError {
    match error {
        RuntimeProbeTransportError::Spawn { source } => {
            RuntimeProbeExecutionError::Spawn { source }
        }
        RuntimeProbeTransportError::Wait { source } => RuntimeProbeExecutionError::Wait { source },
        RuntimeProbeTransportError::Timeout {
            timeout,
            cleanup_error,
        } => RuntimeProbeExecutionError::Timeout {
            timeout,
            cleanup_error,
        },
        RuntimeProbeTransportError::StdoutRead { source } => {
            RuntimeProbeExecutionError::StdoutRead { source }
        }
        RuntimeProbeTransportError::StderrRead { source } => {
            RuntimeProbeExecutionError::StderrRead { source }
        }
        RuntimeProbeTransportError::StdoutTooLarge { maximum } => {
            RuntimeProbeExecutionError::StdoutTooLarge { maximum }
        }
        RuntimeProbeTransportError::StderrTooLarge { maximum } => {
            RuntimeProbeExecutionError::StderrTooLarge { maximum }
        }
        RuntimeProbeTransportError::UnsuccessfulExit {
            status,
            stderr,
            stderr_truncated,
        } => RuntimeProbeExecutionError::UnsuccessfulExit {
            status,
            stderr,
            stderr_truncated,
        },
    }
}

fn map_processing_runtime_probe_transport_error(
    error: RuntimeProbeTransportError,
) -> ProcessingRuntimeProbeExecutionError {
    match error {
        RuntimeProbeTransportError::Spawn { source } => {
            ProcessingRuntimeProbeExecutionError::Spawn { source }
        }
        RuntimeProbeTransportError::Wait { source } => {
            ProcessingRuntimeProbeExecutionError::Wait { source }
        }
        RuntimeProbeTransportError::Timeout {
            timeout,
            cleanup_error,
        } => ProcessingRuntimeProbeExecutionError::Timeout {
            timeout,
            cleanup_error,
        },
        RuntimeProbeTransportError::StdoutRead { source } => {
            ProcessingRuntimeProbeExecutionError::StdoutRead { source }
        }
        RuntimeProbeTransportError::StderrRead { source } => {
            ProcessingRuntimeProbeExecutionError::StderrRead { source }
        }
        RuntimeProbeTransportError::StdoutTooLarge { maximum } => {
            ProcessingRuntimeProbeExecutionError::StdoutTooLarge { maximum }
        }
        RuntimeProbeTransportError::StderrTooLarge { maximum } => {
            ProcessingRuntimeProbeExecutionError::StderrTooLarge { maximum }
        }
        RuntimeProbeTransportError::UnsuccessfulExit {
            status,
            stderr,
            stderr_truncated,
        } => ProcessingRuntimeProbeExecutionError::UnsuccessfulExit {
            status,
            stderr,
            stderr_truncated,
        },
    }
}

fn execute_runtime_information_probe(
    executable: &Path,
    timeout: Duration,
) -> Result<CapturedRuntimeProbe, RuntimeProbeTransportError> {
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

        command
            .spawn()
            .map_err(|source| RuntimeProbeTransportError::Spawn { source })?
    };

    let stdout = child.stdout.take().expect("stdout piped for runtime probe");
    let stderr = child.stderr.take().expect("stderr piped for runtime probe");

    let stdout_handle =
        thread::spawn(move || drain_bounded_stream(stdout, MAX_RUNTIME_INFORMATION_PROBE_BYTES));
    let stderr_handle = thread::spawn(move || {
        drain_bounded_stream(stderr, MAX_RUNTIME_INFORMATION_PROBE_STDERR_BYTES)
    });

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
        return Err(RuntimeProbeTransportError::Timeout {
            timeout,
            cleanup_error: timeout_error,
        });
    }

    if let Some(source) = wait_error {
        return Err(RuntimeProbeTransportError::Wait { source });
    }

    if let Some(source) = stdout_read_error {
        return Err(RuntimeProbeTransportError::StdoutRead { source });
    }

    if let Some(source) = stderr_read_error {
        return Err(RuntimeProbeTransportError::StderrRead { source });
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
        return Err(RuntimeProbeTransportError::StdoutTooLarge {
            maximum: MAX_RUNTIME_INFORMATION_PROBE_BYTES,
        });
    }

    if stderr_capture
        .as_ref()
        .is_some_and(|stream| stream.truncated)
    {
        return Err(RuntimeProbeTransportError::StderrTooLarge {
            maximum: MAX_RUNTIME_INFORMATION_PROBE_STDERR_BYTES,
        });
    }

    let exit_status = match exit_status {
        Some(status) => status,
        None => {
            return Err(RuntimeProbeTransportError::Wait {
                source: std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "runtime information probe exited without a status",
                ),
            });
        }
    };

    if !exit_status.success() {
        return Err(RuntimeProbeTransportError::UnsuccessfulExit {
            status: exit_status,
            stderr: stderr_bytes,
            stderr_truncated: stderr_capture
                .as_ref()
                .is_some_and(|stream| stream.truncated),
        });
    }

    Ok(CapturedRuntimeProbe {
        stdout: stdout_bytes,
        stderr: stderr_bytes,
    })
}

impl std::error::Error for RuntimeProbeTransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source } => Some(source),
            Self::Wait { source } => Some(source),
            Self::StdoutRead { source } => Some(source),
            Self::StderrRead { source } => Some(source),
            Self::Timeout { .. }
            | Self::StdoutTooLarge { .. }
            | Self::StderrTooLarge { .. }
            | Self::UnsuccessfulExit { .. } => None,
        }
    }
}

impl fmt::Display for RuntimeProbeTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { source } => write!(
                formatter,
                "failed to spawn runtime information probe: {source}"
            ),
            Self::Wait { source } => write!(
                formatter,
                "failed waiting for runtime information probe to exit: {source}"
            ),
            Self::Timeout {
                timeout,
                cleanup_error,
            } => {
                let mut message = format!("runtime information probe timed out after {timeout:?}");
                if let Some(cleanup_error) = cleanup_error {
                    message.push_str(&format!(" (cleanup: {cleanup_error})"));
                }
                formatter.write_str(&message)
            }
            Self::StdoutRead { source } => write!(
                formatter,
                "failed reading stdout from runtime information probe: {source}"
            ),
            Self::StderrRead { source } => write!(
                formatter,
                "failed reading stderr from runtime information probe: {source}"
            ),
            Self::StdoutTooLarge { maximum } => write!(
                formatter,
                "runtime information probe stdout exceeded {maximum} bytes"
            ),
            Self::StderrTooLarge { maximum } => write!(
                formatter,
                "runtime information probe stderr exceeded {maximum} bytes"
            ),
            Self::UnsuccessfulExit { status, .. } => write!(
                formatter,
                "runtime information probe exited unsuccessfully: {status}"
            ),
        }
    }
}

pub(crate) fn probe_http_runtime_information_with_timeout(
    executable: &Path,
    expected_identity: RuntimeIdentity,
    timeout: Duration,
) -> Result<AdmittedRuntimeInformation, RuntimeProbeExecutionError> {
    let captured = execute_runtime_information_probe(executable, timeout)
        .map_err(map_runtime_probe_transport_error)?;

    match admit_http_runtime_information_probe(expected_identity, captured.stdout()) {
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

pub fn probe_http_runtime_information_owned(
    executable: &Path,
    expected: &OwnedRuntimeIdentity,
) -> Result<AdmittedRuntimeInformation, RuntimeProbeExecutionError> {
    probe_http_runtime_information_with_timeout_owned(
        executable,
        expected,
        RUNTIME_INFORMATION_PROBE_TIMEOUT,
    )
}

fn probe_http_runtime_information_with_timeout_owned(
    executable: &Path,
    expected: &OwnedRuntimeIdentity,
    timeout: Duration,
) -> Result<AdmittedRuntimeInformation, RuntimeProbeExecutionError> {
    let captured = execute_runtime_information_probe(executable, timeout)
        .map_err(map_runtime_probe_transport_error)?;
    if !captured.stderr().is_empty() {
        return Err(RuntimeProbeExecutionError::UnexpectedStderr {
            stderr: captured.stderr().to_vec(),
        });
    }
    match admit_http_runtime_information_probe_owned(expected, captured.stdout()) {
        Ok(admitted) => Ok(admitted),
        Err(error) => Err(RuntimeProbeExecutionError::Admission(error)),
    }
}

pub fn admit_http_runtime_information_probe(
    expected_identity: RuntimeIdentity,
    stdout: &[u8],
) -> Result<AdmittedRuntimeInformation, RuntimeProbeAdmissionError> {
    if stdout.len() > MAX_RUNTIME_INFORMATION_PROBE_BYTES {
        return Err(RuntimeProbeAdmissionError::OutputTooLarge {
            maximum: MAX_RUNTIME_INFORMATION_PROBE_BYTES,
            actual: stdout.len(),
        });
    }

    if stdout.is_empty() {
        return Err(RuntimeProbeAdmissionError::EmptyOutput);
    }

    if stdout.iter().any(|byte| *byte == 0) {
        return Err(RuntimeProbeAdmissionError::ContainsNul);
    }

    let text = std::str::from_utf8(stdout).map_err(RuntimeProbeAdmissionError::InvalidUtf8)?;

    if !text.ends_with('\n') {
        return Err(RuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    if text.bytes().filter(|byte| *byte == b'\n').count() != 1 {
        return Err(RuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    if text.starts_with('\n') || text.starts_with('\r') || text.contains('\r') {
        return Err(RuntimeProbeAdmissionError::InvalidOutputBoundary);
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
        return Err(RuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    let information =
        RuntimeInformationV1::from_json(json_text).map_err(RuntimeProbeAdmissionError::Decode)?;
    information
        .validate_compatibility(expected_identity)
        .map_err(RuntimeProbeAdmissionError::Incompatible)?;

    Ok(AdmittedRuntimeInformation { information })
}

pub fn admit_http_runtime_information_probe_owned(
    expected: &OwnedRuntimeIdentity,
    stdout: &[u8],
) -> Result<AdmittedRuntimeInformation, RuntimeProbeAdmissionError> {
    if stdout.len() > MAX_RUNTIME_INFORMATION_PROBE_BYTES {
        return Err(RuntimeProbeAdmissionError::OutputTooLarge {
            maximum: MAX_RUNTIME_INFORMATION_PROBE_BYTES,
            actual: stdout.len(),
        });
    }

    if stdout.is_empty() {
        return Err(RuntimeProbeAdmissionError::EmptyOutput);
    }

    if stdout.iter().any(|byte| *byte == 0) {
        return Err(RuntimeProbeAdmissionError::ContainsNul);
    }

    let text = std::str::from_utf8(stdout).map_err(RuntimeProbeAdmissionError::InvalidUtf8)?;

    if !text.ends_with('\n') {
        return Err(RuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    if text.bytes().filter(|byte| *byte == b'\n').count() != 1 {
        return Err(RuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    if text.starts_with('\n') || text.starts_with('\r') || text.contains('\r') {
        return Err(RuntimeProbeAdmissionError::InvalidOutputBoundary);
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
        return Err(RuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    let information =
        RuntimeInformationV1::from_json(json_text).map_err(RuntimeProbeAdmissionError::Decode)?;
    information
        .validate_compatibility_owned(expected)
        .map_err(RuntimeProbeAdmissionError::IncompatibleOwned)?;

    Ok(AdmittedRuntimeInformation { information })
}

pub fn admit_processing_runtime_information_probe(
    expected_identity: RuntimeIdentity,
    stdout: &[u8],
) -> Result<AdmittedProcessingRuntimeInformation, ProcessingRuntimeProbeAdmissionError> {
    if stdout.len() > MAX_RUNTIME_INFORMATION_PROBE_BYTES {
        return Err(ProcessingRuntimeProbeAdmissionError::OutputTooLarge {
            maximum: MAX_RUNTIME_INFORMATION_PROBE_BYTES,
            actual: stdout.len(),
        });
    }

    if stdout.is_empty() {
        return Err(ProcessingRuntimeProbeAdmissionError::EmptyOutput);
    }

    if stdout.iter().any(|byte| *byte == 0) {
        return Err(ProcessingRuntimeProbeAdmissionError::ContainsNul);
    }

    let text =
        std::str::from_utf8(stdout).map_err(ProcessingRuntimeProbeAdmissionError::InvalidUtf8)?;

    if !text.ends_with('\n') {
        return Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    if text.bytes().filter(|byte| *byte == b'\n').count() != 1 {
        return Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    if text.starts_with('\n') || text.starts_with('\r') || text.contains('\r') {
        return Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary);
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
        return Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    let information = ProcessingRuntimeInformationV1::from_json(json_text)
        .map_err(ProcessingRuntimeProbeAdmissionError::Decode)?;
    information
        .validate_compatibility(expected_identity)
        .map_err(ProcessingRuntimeProbeAdmissionError::Incompatible)?;

    Ok(AdmittedProcessingRuntimeInformation { information })
}

pub fn probe_processing_runtime_information(
    executable: &Path,
    expected_identity: RuntimeIdentity,
) -> Result<AdmittedProcessingRuntimeInformation, ProcessingRuntimeProbeExecutionError> {
    probe_processing_runtime_information_with_timeout(
        executable,
        expected_identity,
        RUNTIME_INFORMATION_PROBE_TIMEOUT,
    )
}

pub fn probe_processing_runtime_information_owned(
    executable: &Path,
    expected: &OwnedRuntimeIdentity,
) -> Result<AdmittedProcessingRuntimeInformation, ProcessingRuntimeProbeExecutionError> {
    probe_processing_runtime_information_with_timeout_owned(
        executable,
        expected,
        RUNTIME_INFORMATION_PROBE_TIMEOUT,
    )
}

fn probe_processing_runtime_information_with_timeout_owned(
    executable: &Path,
    expected: &OwnedRuntimeIdentity,
    timeout: Duration,
) -> Result<AdmittedProcessingRuntimeInformation, ProcessingRuntimeProbeExecutionError> {
    let captured = execute_runtime_information_probe(executable, timeout)
        .map_err(map_processing_runtime_probe_transport_error)?;
    if !captured.stderr().is_empty() {
        return Err(ProcessingRuntimeProbeExecutionError::UnexpectedStderr {
            stderr: captured.stderr().to_vec(),
        });
    }
    match admit_processing_runtime_information_probe_owned(expected, captured.stdout()) {
        Ok(admitted) => Ok(admitted),
        Err(error) => Err(ProcessingRuntimeProbeExecutionError::Admission(error)),
    }
}

pub(crate) fn probe_processing_runtime_information_with_timeout(
    executable: &Path,
    expected_identity: RuntimeIdentity,
    timeout: Duration,
) -> Result<AdmittedProcessingRuntimeInformation, ProcessingRuntimeProbeExecutionError> {
    let captured = execute_runtime_information_probe(executable, timeout)
        .map_err(map_processing_runtime_probe_transport_error)?;

    match admit_processing_runtime_information_probe(expected_identity, captured.stdout()) {
        Ok(admitted) => Ok(admitted),
        Err(error) => Err(ProcessingRuntimeProbeExecutionError::Admission(error)),
    }
}

pub fn admit_processing_runtime_information_probe_owned(
    expected: &OwnedRuntimeIdentity,
    stdout: &[u8],
) -> Result<AdmittedProcessingRuntimeInformation, ProcessingRuntimeProbeAdmissionError> {
    if stdout.len() > MAX_RUNTIME_INFORMATION_PROBE_BYTES {
        return Err(ProcessingRuntimeProbeAdmissionError::OutputTooLarge {
            maximum: MAX_RUNTIME_INFORMATION_PROBE_BYTES,
            actual: stdout.len(),
        });
    }

    if stdout.is_empty() {
        return Err(ProcessingRuntimeProbeAdmissionError::EmptyOutput);
    }

    if stdout.iter().any(|byte| *byte == 0) {
        return Err(ProcessingRuntimeProbeAdmissionError::ContainsNul);
    }

    let text =
        std::str::from_utf8(stdout).map_err(ProcessingRuntimeProbeAdmissionError::InvalidUtf8)?;

    if !text.ends_with('\n') {
        return Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    if text.bytes().filter(|byte| *byte == b'\n').count() != 1 {
        return Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    if text.starts_with('\n') || text.starts_with('\r') || text.contains('\r') {
        return Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary);
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
        return Err(ProcessingRuntimeProbeAdmissionError::InvalidOutputBoundary);
    }

    let information = ProcessingRuntimeInformationV1::from_json(json_text)
        .map_err(ProcessingRuntimeProbeAdmissionError::Decode)?;
    information
        .validate_compatibility_owned(expected)
        .map_err(ProcessingRuntimeProbeAdmissionError::IncompatibleOwned)?;

    Ok(AdmittedProcessingRuntimeInformation { information })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;

    use lexicon_core::protocols::http::{HttpCapability, HttpCapabilitySet, HttpSourceContractV1};
    use lexicon_core::runtime::{RuntimeCompatibilityError, RuntimeIdentity, RuntimeInformationV1};
    use lexicon_core::{
        HttpAcquisitionContext,
        protocols::http::runner::{
            RUNTIME_INFORMATION_PROBE_ARGUMENT, try_write_runtime_information_probe,
        },
    };

    use super::{
        AdmittedRuntimeInformation, MAX_RUNTIME_INFORMATION_PROBE_BYTES,
        MAX_RUNTIME_INFORMATION_PROBE_STDERR_BYTES, RuntimeProbeAdmissionError,
        RuntimeProbeExecutionError, admit_http_runtime_information_probe,
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
        assert_eq!(
            admitted.information().required_capabilities(),
            source.required_capabilities()
        );
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
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::EmptyOutput)
        ));
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
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::ContainsNul)
        ));
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output[0] = 0xff;

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::InvalidUtf8(_))
        ));
    }

    #[test]
    fn missing_final_newline_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.pop();

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)
        ));
    }

    #[test]
    fn two_final_newlines_are_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.push(b'\n');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)
        ));
    }

    #[test]
    fn carriage_return_line_ending_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(output.len() - 1, b'\r');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)
        ));
    }

    #[test]
    fn leading_spaces_are_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(0, b' ');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)
        ));
    }

    #[test]
    fn leading_newline_is_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(0, b'\n');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)
        ));
    }

    #[test]
    fn trailing_spaces_before_final_newline_are_rejected() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler);
        let mut output = valid_probe_output(identity, &source, HttpCapabilitySet::empty());
        output.insert(output.len() - 1, b' ');

        let result = admit_http_runtime_information_probe(identity, &output);
        assert!(matches!(
            result,
            Err(RuntimeProbeAdmissionError::InvalidOutputBoundary)
        ));
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
        output.extend_from_slice(&valid_probe_output(
            identity,
            &source,
            HttpCapabilitySet::empty(),
        ));

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
            RuntimeProbeAdmissionError::Incompatible(
                RuntimeCompatibilityError::MissingCapabilities(missing),
            ) => {
                assert_eq!(missing.missing().ordered_capabilities().len(), 1);
                assert_eq!(
                    missing.missing().ordered_capabilities()[0],
                    HttpCapability::ClientCertificateV1
                );
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

    /// `ETXTBSY` ("text file busy") is a known transient race on overlay filesystems
    /// (the default container storage driver): the kernel can briefly still consider a
    /// freshly written-and-chmod'd file open for writing at the moment it is exec'd,
    /// even though the writer has already closed its file handle. It is unrelated to
    /// probe logic; the test fixture always creates a fresh temp file per test.
    fn is_probe_spawn_busy(error: &RuntimeProbeExecutionError) -> bool {
        matches!(
            error,
            RuntimeProbeExecutionError::Spawn { source }
                if source.kind() == std::io::ErrorKind::ExecutableFileBusy
        )
    }

    /// Retries `probe` when it fails to spawn with `ExecutableFileBusy`, up to a small
    /// fixed attempt bound. Any other outcome (success or a different error) is
    /// returned immediately. Exhausting the retry budget returns the last
    /// `ExecutableFileBusy` error unchanged; it is never converted into success, so an
    /// environment that cannot execute the fixture still fails the test.
    fn retry_on_spawn_busy<T>(
        mut probe: impl FnMut() -> Result<T, RuntimeProbeExecutionError>,
    ) -> Result<T, RuntimeProbeExecutionError> {
        const MAX_ATTEMPTS: u32 = 10;
        let mut attempt = 0;
        loop {
            attempt += 1;
            match probe() {
                Err(error) if attempt < MAX_ATTEMPTS && is_probe_spawn_busy(&error) => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
                other => return other,
            }
        }
    }

    #[test]
    fn probe_http_runtime_information_accepts_valid_probe_output() {
        let (_temp, script) = probe_fixture("valid");
        let result = retry_on_spawn_busy(|| {
            super::probe_http_runtime_information(
                &script,
                RuntimeIdentity::http_acquisition("example-source", 1),
            )
        });
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn probe_http_runtime_information_times_out_for_delayed_exit() {
        let (_temp, script) = probe_fixture("delayed-exit");
        let result = retry_on_spawn_busy(|| {
            super::probe_http_runtime_information_with_timeout(
                &script,
                RuntimeIdentity::http_acquisition("example-source", 1),
                std::time::Duration::from_millis(50),
            )
        });
        assert!(matches!(
            result,
            Err(RuntimeProbeExecutionError::Timeout { .. })
        ));
    }

    #[test]
    fn probe_http_runtime_information_rejects_oversized_stdout() {
        let (_temp, script) = probe_fixture("oversized-stdout");
        let result = retry_on_spawn_busy(|| {
            super::probe_http_runtime_information(
                &script,
                RuntimeIdentity::http_acquisition("example-source", 1),
            )
        });
        assert!(matches!(
            result,
            Err(RuntimeProbeExecutionError::StdoutTooLarge { .. })
        ));
    }

    #[test]
    fn probe_http_runtime_information_rejects_nonzero_exit_status() {
        let (_temp, script) = probe_fixture("nonzero-exit");
        let result = retry_on_spawn_busy(|| {
            super::probe_http_runtime_information(
                &script,
                RuntimeIdentity::http_acquisition("example-source", 1),
            )
        });
        assert!(matches!(
            result,
            Err(RuntimeProbeExecutionError::UnsuccessfulExit { .. })
        ));
    }
}
