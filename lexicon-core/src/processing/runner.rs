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
use crate::session::{
    CoreRunnerSessionError, SessionDataPaths, SessionOperationRoot, SessionStore,
    bind_runtime_session, decode_runtime_context_from_env,
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
    Session(CoreRunnerSessionError),
    /// An internal processing lifecycle invariant was violated.
    Lifecycle(crate::processing::ProcessingLifecycleError),
    /// Setup failed and the terminal failure state was persisted successfully.
    Setup(crate::processing::ProcessingSetupError),
    /// Setup failed and terminal persistence also failed; both are retained.
    SetupAndPersistence(crate::processing::ProcessingSetupAndPersistenceFailure),
    Handler(ProcessingError),
    /// The handler failed and the Core-owned rollback also failed.
    HandlerAndRollbackFailure {
        handler_error: ProcessingError,
        rollback_error: crate::processing::ProcessingDatabaseTransactionError,
        terminal_persistence_error: Option<crate::session::SessionStoreError>,
    },
    /// The handler failed and terminal session persistence also failed.
    HandlerAndPersistenceFailure {
        handler_error: ProcessingError,
        session_error: crate::session::SessionStoreError,
    },
    /// Terminal session persistence failed with no handler failure involved.
    TerminalPersistence {
        session_error: crate::session::SessionStoreError,
    },
    /// A database transaction operation failed and the database did not commit.
    DatabaseTransaction(crate::processing::ProcessingDatabaseTransactionError),
    /// The database did not commit and terminal failure persistence also failed.
    DatabaseCommitAndPersistenceFailure {
        commit_error: crate::processing::ProcessingDatabaseTransactionError,
        persistence_error: crate::session::SessionStoreError,
    },
    /// SQLite could not prove whether the commit became durable.
    DatabaseCommitOutcomeUncertain(crate::processing::ProcessingDatabaseCommitOutcomeUncertain),
    /// SQLite committed but the session cannot be reported successful.
    DatabasePartialCommit(crate::processing::ProcessingDatabasePartialCommit),
    /// The SQLite sidecar policy was violated; the database did not commit.
    DatabaseSidecar {
        handler_error: Option<ProcessingError>,
        sidecar_error: crate::processing::ProcessingDatabaseSidecarError,
        terminal_persistence_error: Option<crate::session::SessionStoreError>,
    },
}

impl ProcessingRuntimeInvocationExecutionError {
    /// The retained source handler failure, when this outcome involved one.
    pub fn handler_error(&self) -> Option<&ProcessingError> {
        match self {
            Self::Handler(error)
            | Self::HandlerAndRollbackFailure {
                handler_error: error,
                ..
            }
            | Self::HandlerAndPersistenceFailure {
                handler_error: error,
                ..
            } => Some(error),
            Self::DatabaseSidecar { handler_error, .. } => handler_error.as_ref(),
            _ => None,
        }
    }

    /// The retained setup failure, when this outcome involved one.
    pub fn setup_error(&self) -> Option<&crate::processing::ProcessingSetupError> {
        match self {
            Self::Setup(error) => Some(error),
            Self::SetupAndPersistence(failure) => Some(failure.setup_error()),
            _ => None,
        }
    }

    /// The retained database transaction failure, when this outcome involved one.
    pub fn database_transaction_error(
        &self,
    ) -> Option<&crate::processing::ProcessingDatabaseTransactionError> {
        match self {
            Self::DatabaseTransaction(error)
            | Self::DatabaseCommitAndPersistenceFailure {
                commit_error: error,
                ..
            }
            | Self::HandlerAndRollbackFailure {
                rollback_error: error,
                ..
            } => Some(error),
            _ => None,
        }
    }

    /// The retained sidecar failure, when this outcome involved one.
    pub fn sidecar_error(&self) -> Option<&crate::processing::ProcessingDatabaseSidecarError> {
        match self {
            Self::DatabaseSidecar { sidecar_error, .. } => Some(sidecar_error),
            Self::DatabasePartialCommit(partial) => partial.sidecar_error(),
            _ => None,
        }
    }

    /// The retained committed-database outcome, when this outcome involved one.
    pub fn database_partial_commit(
        &self,
    ) -> Option<&crate::processing::ProcessingDatabasePartialCommit> {
        match self {
            Self::DatabasePartialCommit(partial) => Some(partial),
            _ => None,
        }
    }

    /// The retained uncertain-commit outcome, when this outcome involved one.
    pub fn database_commit_outcome_uncertain(
        &self,
    ) -> Option<&crate::processing::ProcessingDatabaseCommitOutcomeUncertain> {
        match self {
            Self::DatabaseCommitOutcomeUncertain(outcome) => Some(outcome),
            _ => None,
        }
    }

    /// Every retained session-persistence failure, however this outcome arose.
    pub fn session_persistence_error(&self) -> Option<&crate::session::SessionStoreError> {
        match self {
            Self::SetupAndPersistence(failure) => Some(failure.persistence_error()),
            Self::HandlerAndRollbackFailure {
                terminal_persistence_error,
                ..
            }
            | Self::DatabaseSidecar {
                terminal_persistence_error,
                ..
            } => terminal_persistence_error.as_ref(),
            Self::HandlerAndPersistenceFailure { session_error, .. }
            | Self::TerminalPersistence { session_error } => Some(session_error),
            Self::DatabaseCommitAndPersistenceFailure {
                persistence_error, ..
            } => Some(persistence_error),
            Self::DatabasePartialCommit(partial) => partial.session_persistence_error(),
            Self::DatabaseCommitOutcomeUncertain(outcome) => outcome.session_persistence_error(),
            _ => None,
        }
    }
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
            Self::Session(_) => {
                formatter.write_str("processing runtime session initialization error")
            }
            Self::Lifecycle(_) => {
                formatter.write_str("processing runtime lifecycle invariant violated")
            }
            Self::Setup(_) => formatter.write_str("processing setup failed"),
            Self::SetupAndPersistence(_) => formatter.write_str(
                "processing setup failed and terminal session persistence also failed",
            ),
            Self::Handler(_) => formatter.write_str("processing handler error"),
            Self::HandlerAndRollbackFailure { .. } => formatter.write_str(
                "processing handler error followed by rollback and/or terminal persistence failure",
            ),
            Self::HandlerAndPersistenceFailure { .. } => formatter.write_str(
                "processing handler error; terminal session state persistence also failed",
            ),
            Self::TerminalPersistence { .. } => {
                formatter.write_str("terminal session state persistence failed")
            }
            Self::DatabaseTransaction(_) => {
                formatter.write_str("processing database transaction failed")
            }
            Self::DatabaseCommitAndPersistenceFailure { .. } => formatter.write_str(
                "processing database commit failed and terminal session failure persistence also failed",
            ),
            Self::DatabaseCommitOutcomeUncertain(_) => {
                formatter.write_str("processing database commit outcome is uncertain")
            }
            Self::DatabasePartialCommit(_) => formatter.write_str(
                "processing database committed but the session cannot be reported successful",
            ),
            Self::DatabaseSidecar { .. } => {
                formatter.write_str("processing database sidecar policy was violated")
            }
        }
    }
}

impl std::error::Error for ProcessingRuntimeInvocationExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Session(error) => Some(error),
            Self::Lifecycle(error) => Some(error),
            Self::Setup(error) => Some(error),
            Self::SetupAndPersistence(error) => Some(error),
            Self::Handler(error) => Some(error),
            Self::HandlerAndRollbackFailure { handler_error, .. }
            | Self::HandlerAndPersistenceFailure { handler_error, .. } => Some(handler_error),
            Self::TerminalPersistence { session_error } => Some(session_error),
            Self::DatabaseTransaction(error) => Some(error),
            Self::DatabaseCommitAndPersistenceFailure { commit_error, .. } => Some(commit_error),
            Self::DatabaseCommitOutcomeUncertain(error) => Some(error),
            Self::DatabasePartialCommit(error) => Some(error),
            Self::DatabaseSidecar { sidecar_error, .. } => Some(sidecar_error),
        }
    }
}

// --- Running lifecycle ownership ---

/// Owns the proven `Running` processing session for the rest of the invocation.
///
/// The running lifecycle is non-optional and is consumed exactly once by whichever
/// terminal operation applies. Ordinary paths therefore never need `expect`,
/// `unwrap`, or an internal-state assertion to reach the owner.
struct RunningProcessingExecution<'store> {
    running: crate::session::RunningRuntimeSession<'store>,
    project: crate::session::ProjectIdentity,
    runtime: crate::runtime::OwnedRuntimeIdentity,
    session: crate::session::SessionIdentity,
}

impl<'store> RunningProcessingExecution<'store> {
    fn new(
        running: crate::session::RunningRuntimeSession<'store>,
        project: crate::session::ProjectIdentity,
        runtime: crate::runtime::OwnedRuntimeIdentity,
        session: crate::session::SessionIdentity,
    ) -> Self {
        Self {
            running,
            project,
            runtime,
            session,
        }
    }

    fn project(&self) -> &crate::session::ProjectIdentity {
        &self.project
    }

    fn runtime(&self) -> &crate::runtime::OwnedRuntimeIdentity {
        &self.runtime
    }

    fn session(&self) -> &crate::session::SessionIdentity {
        &self.session
    }

    /// Record a setup failure, preserving the original typed error even when
    /// terminal persistence also fails.
    fn fail_setup(
        self,
        setup_error: crate::processing::ProcessingSetupError,
    ) -> ProcessingRuntimeInvocationExecutionError {
        let code = setup_error.failure_code();
        let diagnostic = setup_error.diagnostic().to_string();
        match self.running.fail_runtime(code, Some(diagnostic)) {
            Ok(_) => ProcessingRuntimeInvocationExecutionError::Setup(setup_error),
            Err(persistence_error) => {
                ProcessingRuntimeInvocationExecutionError::SetupAndPersistence(
                    crate::processing::ProcessingSetupAndPersistenceFailure::new(
                        setup_error,
                        persistence_error,
                    ),
                )
            }
        }
    }

    /// Record a Core-authored runtime failure with a stable code and bounded diagnostic.
    fn fail_runtime(
        self,
        code: crate::session::SessionFailureCode,
        diagnostic: &'static str,
    ) -> Option<crate::session::SessionStoreError> {
        self.running
            .fail_runtime(code, Some(diagnostic.to_string()))
            .err()
    }

    /// Record an ordinary source failure. No source-authored text is persisted.
    fn fail_source(self) -> Option<crate::session::SessionStoreError> {
        self.running.fail_source().err()
    }

    /// Record successful completion.
    fn complete(self) -> Option<crate::session::SessionStoreError> {
        self.running.complete().err()
    }
}

/// Run a processing runtime invocation with full session lifecycle.
///
/// Authoritative sequence:
/// ```text
/// parse invocation
/// → admit processing invocation
/// → decode managed context
/// → open processing SessionStore
/// → bind processing session
/// → enter Running with a non-optional owner
/// → validate exact raw/acquisition/processed roots
/// → enumerate raw entries
/// → strictly admit finalized transactions
/// → load typed acquisition-session cache
/// → validate every transaction against its session
/// → build deterministic catalog
/// → derive exact database path
/// → validate main and sidecar paths
/// → open SQLite with explicit flags
/// → configure and verify baseline pragmas
/// → BEGIN IMMEDIATE
/// → construct fully checked ProcessingContext
/// → verify transaction is active
/// → invoke source handler
/// → verify transaction remains active
/// → success:  COMMIT, durability, sidecar validation, session Succeeded
/// → source failure: ROLLBACK, sidecar validation, session Failed
/// → setup/runtime failure: preserve the primary typed error, persist a stable code
/// → partial or uncertain commit: preserve database provenance, never report success
/// ```
pub fn run_processing_runtime_invocation(
    arguments: &[OsString],
    compiled_identity: RuntimeIdentity,
    source: &ProcessingSourceContractV1,
) -> Result<(), ProcessingRuntimeInvocationExecutionError> {
    use crate::processing::{
        ProcessingDatabasePartialCommit, ProcessingDatabasePartialCommitCause,
        ProcessingDatabasePartialCommitPhase, ProcessingDatabaseTransactionError,
        ProcessingSetupError, ProcessingTransactionBoundaryPhase,
    };
    use crate::session::SessionFailureCode;

    let parsed = parse_runtime_invocation(arguments)
        .map_err(ProcessingRuntimeInvocationExecutionError::Transport)?;

    let admitted = admit_processing_runtime_invocation(parsed, compiled_identity, source)
        .map_err(ProcessingRuntimeInvocationExecutionError::Admission)?;

    let (envelope, source_arguments, handler) = admitted.into_parts();

    // Decode runtime context and compare identities against admitted envelope.
    let context_document = decode_runtime_context_from_env(
        envelope.project(),
        &envelope.runtime().into_owned_identity(),
        envelope.session(),
    )
    .map_err(|e| {
        ProcessingRuntimeInvocationExecutionError::Session(CoreRunnerSessionError::ContextDecode(e))
    })?;

    let operation_root =
        SessionOperationRoot::new(context_document.paths.operation_root().to_path_buf()).map_err(
            |e| {
                ProcessingRuntimeInvocationExecutionError::Session(
                    CoreRunnerSessionError::StoreOpen(e),
                )
            },
        )?;

    let store = SessionStore::open(operation_root).map_err(|e| {
        ProcessingRuntimeInvocationExecutionError::Session(CoreRunnerSessionError::StoreOpen(e))
    })?;

    let bound = bind_runtime_session(&store, &envelope).map_err(|err| {
        ProcessingRuntimeInvocationExecutionError::Session(CoreRunnerSessionError::SessionBinding(
            err,
        ))
    })?;
    let running = bound.enter_running().map_err(|e| {
        ProcessingRuntimeInvocationExecutionError::Session(
            CoreRunnerSessionError::TransitionToRunning(e),
        )
    })?;

    // The running lifecycle is owned outright from here on; no ordinary path needs to
    // reach for it through an `Option`.
    let execution = RunningProcessingExecution::new(
        running,
        context_document.project.clone(),
        context_document.runtime.clone(),
        envelope.session().clone(),
    );

    let data_paths = SessionDataPaths::from_context_paths(&context_document.paths);
    let protocol_root = context_document.paths.protocol_root().to_path_buf();
    let processed_root = context_document
        .paths
        .processed_data_directory()
        .to_path_buf();

    let discovered_transactions =
        match super::transactions::discover_http_transactions_for_processing(
            &context_document.project,
            &context_document.runtime,
            &protocol_root,
            context_document.paths.raw_data_directory(),
        ) {
            Ok(value) => value,
            Err(error) => {
                return Err(
                    execution.fail_setup(ProcessingSetupError::TransactionDiscovery(error))
                );
            }
        };

    let database_path = match derive_processing_database_path(
        &protocol_root,
        &processed_root,
        &context_document.runtime,
    ) {
        Ok(value) => value,
        Err(error) => {
            return Err(execution.fail_setup(ProcessingSetupError::DatabasePath(error)));
        }
    };

    let database = match open_processing_database(&protocol_root, &processed_root, &database_path) {
        Ok(value) => value,
        Err(error) => {
            return Err(execution.fail_setup(ProcessingSetupError::DatabaseOpen(error)));
        }
    };

    let mut context = match ProcessingContext::new(
        data_paths,
        context_document.project.clone(),
        context_document.runtime.clone(),
        envelope.session().clone(),
        discovered_transactions,
        database_path.clone(),
        database,
    ) {
        Ok(value) => value,
        Err(error) => {
            return Err(execution.fail_setup(ProcessingSetupError::ContextConstruction(error)));
        }
    };

    // The Core-owned transaction must be active before source code runs.
    if let Err(violation) =
        context.require_transaction_active(ProcessingTransactionBoundaryPhase::BeforeHandler)
    {
        context.mark_transaction_ended_outside_core();
        drop(context);
        return Err(execution.fail_setup(ProcessingSetupError::TransactionBoundary(violation)));
    }

    // Invoke the selected handler.
    let handler_result = match handler {
        AdmittedProcessingHandler::Process(f) => f(&mut context, &source_arguments),
    };

    // The source may have ended the Core-owned transaction. Detect that before any
    // commit or rollback decision, and never report success afterwards.
    if let Err(violation) =
        context.require_transaction_active(ProcessingTransactionBoundaryPhase::AfterHandler)
    {
        context.mark_transaction_ended_outside_core();
        drop(context);

        if violation.possible_database_partial_commit() {
            let partial = ProcessingDatabasePartialCommit::new(
                execution.project().clone(),
                execution.runtime().clone(),
                execution.session().clone(),
                database_path.clone(),
                ProcessingDatabasePartialCommitPhase::SourceTransactionBoundaryLoss,
                ProcessingDatabasePartialCommitCause::TransactionBoundary(violation),
            );
            let persistence = execution.fail_runtime(
                SessionFailureCode::ProcessingDatabaseTransactionFailed,
                "processing source ended the Core-owned database transaction",
            );
            let partial = match persistence {
                Some(error) => partial.with_session_persistence_error(error),
                None => partial,
            };
            return Err(ProcessingRuntimeInvocationExecutionError::DatabasePartialCommit(
                partial,
            ));
        }

        return Err(execution.fail_setup(ProcessingSetupError::TransactionBoundary(violation)));
    }

    match handler_result {
        Ok(()) => {
            match context.commit_database() {
                Ok(()) => {}
                Err(ProcessingDatabaseTransactionError::Commit(sqlite_error)) => {
                    // SQLite does not always prove that a failed COMMIT left no durable
                    // changes. If the transaction is still active, nothing committed. If
                    // it is gone, the outcome is genuinely uncertain.
                    let still_active = context.database_transaction_active();
                    if !still_active {
                        context.mark_transaction_ended_outside_core();
                    }
                    drop(context);

                    if still_active {
                        let commit_error =
                            ProcessingDatabaseTransactionError::Commit(sqlite_error);
                        return Err(match execution.fail_runtime(
                            SessionFailureCode::ProcessingDatabaseTransactionFailed,
                            "processing database commit failed",
                        ) {
                            Some(persistence_error) => {
                                ProcessingRuntimeInvocationExecutionError::DatabaseCommitAndPersistenceFailure {
                                    commit_error,
                                    persistence_error,
                                }
                            }
                            None => ProcessingRuntimeInvocationExecutionError::DatabaseTransaction(
                                commit_error,
                            ),
                        });
                    }

                    let uncertain =
                        crate::processing::ProcessingDatabaseCommitOutcomeUncertain::new(
                            execution.project().clone(),
                            execution.runtime().clone(),
                            execution.session().clone(),
                            database_path.clone(),
                            sqlite_error,
                        );
                    let persistence = execution.fail_runtime(
                        SessionFailureCode::ProcessingDatabaseTransactionFailed,
                        "processing database commit outcome is uncertain",
                    );
                    let uncertain = match persistence {
                        Some(error) => uncertain.with_session_persistence_error(error),
                        None => uncertain,
                    };
                    return Err(
                        ProcessingRuntimeInvocationExecutionError::DatabaseCommitOutcomeUncertain(
                            uncertain,
                        ),
                    );
                }
                Err(state_error) => {
                    drop(context);
                    return Err(match execution.fail_runtime(
                        SessionFailureCode::ProcessingDatabaseTransactionFailed,
                        "processing database transaction state is invalid",
                    ) {
                        Some(persistence_error) => {
                            ProcessingRuntimeInvocationExecutionError::DatabaseCommitAndPersistenceFailure {
                                commit_error: state_error,
                                persistence_error,
                            }
                        }
                        None => ProcessingRuntimeInvocationExecutionError::DatabaseTransaction(
                            state_error,
                        ),
                    });
                }
            }

            // Close the connection before durability and sidecar validation so the
            // rollback journal is released and the checks observe the final state.
            drop(context);

            // The database is committed from here on. Core never claims a rollback.
            if let Err(durability_error) =
                finalize_database_durability(&processed_root, &database_path)
            {
                let partial = ProcessingDatabasePartialCommit::new(
                    execution.project().clone(),
                    execution.runtime().clone(),
                    execution.session().clone(),
                    database_path.clone(),
                    ProcessingDatabasePartialCommitPhase::PostCommitDurability,
                    ProcessingDatabasePartialCommitCause::Durability(durability_error),
                );
                let persistence = execution.fail_runtime(
                    SessionFailureCode::ProcessingDatabaseTransactionFailed,
                    "processing database durability failed after commit",
                );
                let partial = match persistence {
                    Some(error) => partial.with_session_persistence_error(error),
                    None => partial,
                };
                return Err(
                    ProcessingRuntimeInvocationExecutionError::DatabasePartialCommit(partial),
                );
            }

            if let Err(sidecar_error) =
                validate_database_sidecars(&database_path, SidecarValidationPhase::AfterTransaction)
            {
                let partial = ProcessingDatabasePartialCommit::new(
                    execution.project().clone(),
                    execution.runtime().clone(),
                    execution.session().clone(),
                    database_path.clone(),
                    ProcessingDatabasePartialCommitPhase::PostCommitSidecarValidation,
                    ProcessingDatabasePartialCommitCause::Sidecar(sidecar_error),
                );
                let persistence = execution.fail_runtime(
                    SessionFailureCode::ProcessingDatabaseTransactionFailed,
                    "processing database sidecar validation failed after commit",
                );
                let partial = match persistence {
                    Some(error) => partial.with_session_persistence_error(error),
                    None => partial,
                };
                return Err(
                    ProcessingRuntimeInvocationExecutionError::DatabasePartialCommit(partial),
                );
            }

            // Only now is the session allowed to become Succeeded.
            let project = execution.project().clone();
            let runtime = execution.runtime().clone();
            let session = execution.session().clone();
            match execution.complete() {
                None => Ok(()),
                Some(session_error) => Err(
                    ProcessingRuntimeInvocationExecutionError::DatabasePartialCommit(
                        ProcessingDatabasePartialCommit::new(
                            project,
                            runtime,
                            session,
                            database_path,
                            ProcessingDatabasePartialCommitPhase::SessionCompletionPersistence,
                            ProcessingDatabasePartialCommitCause::SessionPersistence(session_error),
                        ),
                    ),
                ),
            }
        }
        Err(processing_error) => {
            let rollback_result = context.rollback_database();
            drop(context);

            if let Err(rollback_error) = rollback_result {
                let terminal_persistence_error = execution.fail_source();
                return Err(
                    ProcessingRuntimeInvocationExecutionError::HandlerAndRollbackFailure {
                        handler_error: processing_error,
                        rollback_error,
                        terminal_persistence_error,
                    },
                );
            }

            if let Err(sidecar_error) =
                validate_database_sidecars(&database_path, SidecarValidationPhase::AfterTransaction)
            {
                let terminal_persistence_error = execution.fail_source();
                return Err(ProcessingRuntimeInvocationExecutionError::DatabaseSidecar {
                    handler_error: Some(processing_error),
                    sidecar_error,
                    terminal_persistence_error,
                });
            }

            match execution.fail_source() {
                Some(session_error) => Err(
                    ProcessingRuntimeInvocationExecutionError::HandlerAndPersistenceFailure {
                        handler_error: processing_error,
                        session_error,
                    },
                ),
                None => Err(ProcessingRuntimeInvocationExecutionError::Handler(
                    processing_error,
                )),
            }
        }
    }
}

/// Derive and admit the canonical processing database path.
///
/// Requires `processed_root == protocol_root/data/processed` and
/// `database_path == processed_root/<runtime-source>.sqlite3`.
fn derive_processing_database_path(
    protocol_root: &std::path::Path,
    processed_root: &std::path::Path,
    runtime: &crate::runtime::OwnedRuntimeIdentity,
) -> Result<std::path::PathBuf, crate::processing::ProcessingDatabasePathError> {
    use crate::processing::ProcessingDatabasePathError;
    use crate::protocols::http::transaction::error::{
        HttpManagedPathValidationMode, validate_managed_path,
    };

    let expected_processed_root = protocol_root.join("data").join("processed");
    if processed_root != expected_processed_root {
        return Err(ProcessingDatabasePathError::ProcessedRootDisagreement {
            expected: expected_processed_root,
            actual: processed_root.to_path_buf(),
        });
    }

    validate_managed_path(
        protocol_root,
        protocol_root,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(ProcessingDatabasePathError::ManagedPath)?;
    validate_managed_path(
        protocol_root,
        processed_root,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(ProcessingDatabasePathError::ManagedPath)?;

    let database_path =
        crate::processing::context::processing_database_path(processed_root, runtime);
    if database_path.parent() != Some(processed_root) {
        return Err(ProcessingDatabasePathError::DatabaseNameDisagreement {
            expected: processed_root
                .join(crate::processing::context::processing_database_file_name(runtime)),
            actual: database_path,
        });
    }

    validate_managed_path(
        protocol_root,
        &database_path,
        existing_or_creatable_regular_file(&database_path),
    )
    .map_err(ProcessingDatabasePathError::ManagedPath)?;

    validate_database_sidecars(&database_path, SidecarValidationPhase::BeforeOpen)
        .map_err(ProcessingDatabasePathError::Sidecar)?;

    Ok(database_path)
}

/// Open the processing database with explicit flags and a verified baseline.
///
/// URI filename interpretation is deliberately not enabled, so an alternate filename
/// can never be embedded in the managed path.
fn open_processing_database(
    protocol_root: &std::path::Path,
    processed_root: &std::path::Path,
    database_path: &std::path::Path,
) -> Result<rusqlite::Connection, crate::processing::ProcessingDatabaseOpenError> {
    use crate::processing::{ProcessingDatabaseOpenError, ProcessingDatabasePathError};
    use crate::protocols::http::transaction::error::{
        HttpManagedPathValidationMode, validate_managed_path,
    };

    validate_managed_path(
        protocol_root,
        processed_root,
        HttpManagedPathValidationMode::ExistingDirectory,
    )
    .map_err(ProcessingDatabasePathError::ManagedPath)
    .map_err(ProcessingDatabaseOpenError::Path)?;

    let database_existed = database_path.exists();
    validate_managed_path(
        protocol_root,
        database_path,
        existing_or_creatable_regular_file(database_path),
    )
    .map_err(ProcessingDatabasePathError::ManagedPath)
    .map_err(ProcessingDatabaseOpenError::Path)?;

    let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
        | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
        | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = rusqlite::Connection::open_with_flags(database_path, flags)
        .map_err(ProcessingDatabaseOpenError::ConnectionOpen)?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(ProcessingDatabaseOpenError::BusyTimeoutConfiguration)?;

    // Baseline configuration must be applied before the transaction begins, because
    // SQLite refuses journal-mode changes inside an active transaction.
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = DELETE;")
        .map_err(ProcessingDatabaseOpenError::BaselineConfiguration)?;
    verify_persistent_baseline_configuration(&connection)
        .map_err(ProcessingDatabaseOpenError::Configuration)?;

    connection
        .execute_batch("BEGIN IMMEDIATE;")
        .map_err(ProcessingDatabaseOpenError::BaselineConfiguration)?;
    verify_transaction_active(&connection).map_err(ProcessingDatabaseOpenError::Configuration)?;

    // The database file now certainly exists; re-validate and make its directory entry
    // durable when this invocation created it.
    validate_managed_path(
        protocol_root,
        database_path,
        HttpManagedPathValidationMode::ExistingRegularFile,
    )
    .map_err(ProcessingDatabasePathError::ManagedPath)
    .map_err(ProcessingDatabaseOpenError::Path)?;

    if !database_existed {
        initialize_database_durability(processed_root, database_path)
            .map_err(ProcessingDatabaseOpenError::Durability)?;
    }

    Ok(connection)
}

fn existing_or_creatable_regular_file(
    path: &std::path::Path,
) -> crate::protocols::http::transaction::error::HttpManagedPathValidationMode {
    use crate::protocols::http::transaction::error::HttpManagedPathValidationMode;
    if path.exists() {
        HttpManagedPathValidationMode::ExistingRegularFile
    } else {
        HttpManagedPathValidationMode::CreatableRegularFile
    }
}

/// Read back and verify the persistent SQLite settings the supported route requires.
///
/// SQLite may ignore a pragma or apply a different effective value, so the effective
/// configuration is verified rather than assumed.
fn verify_persistent_baseline_configuration(
    connection: &rusqlite::Connection,
) -> Result<(), crate::processing::ProcessingDatabaseConfigurationError> {
    use crate::processing::{ProcessingDatabaseConfigurationError, ProcessingDatabaseSetting};

    let foreign_keys: i64 = connection
        .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
        .map_err(|source| ProcessingDatabaseConfigurationError::Readback {
            setting: ProcessingDatabaseSetting::ForeignKeys,
            source,
        })?;
    if foreign_keys != 1 {
        return Err(ProcessingDatabaseConfigurationError::Disagreement {
            setting: ProcessingDatabaseSetting::ForeignKeys,
            expected: "1",
            actual: foreign_keys.to_string(),
        });
    }

    // WAL stays disabled: the supported route requires DELETE journaling.
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
        .map_err(|source| ProcessingDatabaseConfigurationError::Readback {
            setting: ProcessingDatabaseSetting::JournalMode,
            source,
        })?;
    if !journal_mode.eq_ignore_ascii_case("delete") {
        return Err(ProcessingDatabaseConfigurationError::Disagreement {
            setting: ProcessingDatabaseSetting::JournalMode,
            expected: "delete",
            actual: journal_mode,
        });
    }

    Ok(())
}

/// Verify that `BEGIN IMMEDIATE` actually left a transaction open.
fn verify_transaction_active(
    connection: &rusqlite::Connection,
) -> Result<(), crate::processing::ProcessingDatabaseConfigurationError> {
    use crate::processing::{ProcessingDatabaseConfigurationError, ProcessingDatabaseSetting};

    if connection.is_autocommit() {
        return Err(ProcessingDatabaseConfigurationError::Disagreement {
            setting: ProcessingDatabaseSetting::TransactionActive,
            expected: "active",
            actual: "autocommit".to_string(),
        });
    }
    Ok(())
}

/// Make a newly created database file and its parent directory entry durable.
fn initialize_database_durability(
    processed_root: &std::path::Path,
    database_path: &std::path::Path,
) -> Result<(), crate::processing::ProcessingDatabaseDurabilityError> {
    finalize_database_durability(processed_root, database_path)
}

/// Make the database file and the processed-data directory durable.
///
/// Uses the shared cross-platform Core durability boundary.
fn finalize_database_durability(
    processed_root: &std::path::Path,
    database_path: &std::path::Path,
) -> Result<(), crate::processing::ProcessingDatabaseDurabilityError> {
    use crate::processing::{ProcessingDatabaseDurabilityError, ProcessingDurabilityPhase};
    use crate::protocols::http::transaction::recorder::{sync_directory, sync_regular_file};

    sync_regular_file(database_path).map_err(|source| ProcessingDatabaseDurabilityError::Sync {
        phase: ProcessingDurabilityPhase::DatabaseFileSync,
        path: database_path.to_path_buf(),
        source,
    })?;
    sync_directory(processed_root).map_err(|source| ProcessingDatabaseDurabilityError::Sync {
        phase: ProcessingDurabilityPhase::ProcessedDirectorySync,
        path: processed_root.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// When the SQLite sidecar policy is evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidecarValidationPhase {
    /// Before the database is opened.
    BeforeOpen,
    /// After the Core-owned transaction has finished and the connection is closed.
    AfterTransaction,
}

/// Enforce the allowed SQLite sidecar policy beside the canonical database.
///
/// Only the transient DELETE-mode rollback journal is ever permitted, and only while a
/// transaction is in flight. Symlinks, wrong file types, and `-wal`/`-shm` files are
/// rejected. Nothing is ever deleted: an unexpected user file whose name resembles a
/// SQLite sidecar is reported, not removed.
fn validate_database_sidecars(
    database_path: &std::path::Path,
    phase: SidecarValidationPhase,
) -> Result<(), crate::processing::ProcessingDatabaseSidecarError> {
    use crate::processing::{ProcessingDatabaseSidecarError, ProcessingDatabaseSidecarKind};

    for kind in [
        ProcessingDatabaseSidecarKind::RollbackJournal,
        ProcessingDatabaseSidecarKind::WriteAheadLog,
        ProcessingDatabaseSidecarKind::SharedMemory,
    ] {
        let path = sidecar_path(database_path, kind);
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(source) => {
                return Err(ProcessingDatabaseSidecarError::Inspection { kind, path, source });
            }
        };

        if metadata.file_type().is_symlink() {
            return Err(ProcessingDatabaseSidecarError::Symlink { kind, path });
        }
        if !metadata.is_file() {
            return Err(ProcessingDatabaseSidecarError::WrongFileType { kind, path });
        }

        match kind {
            ProcessingDatabaseSidecarKind::WriteAheadLog
            | ProcessingDatabaseSidecarKind::SharedMemory => {
                return Err(ProcessingDatabaseSidecarError::ForbiddenSidecarPresent { kind, path });
            }
            ProcessingDatabaseSidecarKind::RollbackJournal => match phase {
                // A journal left by an interrupted writer is legitimate; SQLite recovers
                // it when the database is opened.
                SidecarValidationPhase::BeforeOpen => {}
                SidecarValidationPhase::AfterTransaction => {
                    return Err(ProcessingDatabaseSidecarError::RollbackJournalNotCleanedUp {
                        path,
                    });
                }
            },
        }
    }

    Ok(())
}

/// Derive the SQLite sidecar path for the canonical database file.
fn sidecar_path(
    database_path: &std::path::Path,
    kind: crate::processing::ProcessingDatabaseSidecarKind,
) -> std::path::PathBuf {
    let mut name = database_path.as_os_str().to_os_string();
    name.push(kind.suffix());
    std::path::PathBuf::from(name)
}

// A prior `execution_tests` module here (matching-process dispatch, source-argument
// fidelity, and success/handler-error assertions) predated `run_processing_runtime_invocation`
// growing full session-store/lease/env-context binding. Those tests never set up a
// `SessionStore`, a `Prepared` session record, a held lease, or the `LEXICON_RUNTIME_CONTEXT_V1`
// env var, so every test that expected the handler to be reached failed with
// `Session(ContextDecode(MissingEnvironmentVariable))`, and were removed rather than left
// disabled with `#[ignore]`. The tests immediately below are the subset that never reached
// session-context decoding in the first place (pure transport/admission rejection paths) and
// remain valid, unmodified coverage.
//
// Full handler-dispatch coverage is restored further down using
// `crate::session::test_support::RuntimeInvocationFixture`, which builds the real minimum
// session-store/lease/runtime-context environment (plus the raw/processed data directories
// and real SQLite database the processing path requires) that the production path needs.
#[cfg(test)]
mod execution_tests {
    use std::ffi::OsString;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::processing::{
        ProcessingContext, ProcessingError, ProcessingResult, ProcessingSourceContractV1,
    };
    use crate::runtime::{
        ProjectInvocationIdentity, RuntimeExecutionMode, RuntimeIdentity,
        RuntimeInvocationEnvelopeV1, RuntimeSupervisionMode, SessionInvocationIdentity,
    };
    use crate::session::test_support::RuntimeInvocationFixture;

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

    // Test 1: malformed transport returns Transport error
    #[test]
    fn malformed_transport_returns_transport_error() {
        let args = vec![OsString::from("--not-invocation-flag")];
        let err = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(|_: &mut ProcessingContext, _: &[OsString]| Ok(())),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Transport(_)
        ));
    }

    // Test 2: probe arguments return transport error
    #[test]
    fn probe_arguments_return_transport_error() {
        use crate::runtime::RUNTIME_INFORMATION_PROBE_ARGUMENT;
        let args = vec![OsString::from(RUNTIME_INFORMATION_PROBE_ARGUMENT)];
        let err = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(|_: &mut ProcessingContext, _: &[OsString]| Ok(())),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Transport(_)
        ));
    }

    // Test 3: identity mismatch returns Admission error
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
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 4: wrong compiled operation returns Admission error
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
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 5: descriptor-version mismatch returns Admission error
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
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 6: processing/resume remains rejected before handler invocation
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

    // Test 7: transport failure does not invoke processing
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
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Transport(_)
        ));
    }

    // Test 8: admission failure does not invoke processing
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
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Admission(_)
        ));
    }

    // Test 9: error formatting does not expose source arguments
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
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            Ok(())
        }
        let args = encode(&example_envelope(), &[]);
        let err = run_processing_runtime_invocation(
            &args,
            RuntimeIdentity::http_processing("wrong-source", 1),
            &ProcessingSourceContractV1::new(process),
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
    // lease, real `data/raw` and `data/processed` directories, and a valid
    // `LEXICON_RUNTIME_CONTEXT_V1` environment via `RuntimeInvocationFixture`, then drives
    // `run_processing_runtime_invocation` through the unmodified production path (including
    // the real SQLite open/commit/rollback sequence) so the process handler is genuinely
    // reached.

    // Test 11: a matching invocation reaches the process handler exactly once with a
    // real, mutable `ProcessingContext`.
    #[test]
    fn matching_invocation_reaches_process_handler_exactly_once_with_real_context() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn process(context: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            assert!(
                context
                    .protocol_root()
                    .to_string_lossy()
                    .contains("example-source")
            );
            Ok(())
        }

        let fixture = RuntimeInvocationFixture::foreground_run(example_identity());
        let args = fixture.build_argv(&[]);

        let result = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
        );

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    // Test 12: background supervision also reaches the handler.
    #[test]
    fn background_invocation_reaches_handler() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        let fixture = RuntimeInvocationFixture::background_run(example_identity());
        let args = fixture.build_argv(&[]);

        let result = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
        );

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    // Test 13: source-argument order and OS representation survive dispatch unchanged.
    #[test]
    fn source_argument_fidelity_is_preserved_across_dispatch() {
        use std::sync::OnceLock;
        static SEEN: OnceLock<Vec<OsString>> = OnceLock::new();
        fn process(_ctx: &mut ProcessingContext, args: &[OsString]) -> ProcessingResult<()> {
            let _ = SEEN.set(args.to_vec());
            Ok(())
        }

        let fixture = RuntimeInvocationFixture::foreground_run(example_identity());
        let source_args = vec![
            OsString::from("alpha"),
            OsString::from("alpha"),
            OsString::from(""),
            OsString::from("--looks-like-a-flag"),
        ];
        let args = fixture.build_argv(&source_args);

        let result = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
        );

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(SEEN.get().unwrap(), &source_args);
    }

    // Test 14: a source-authored failure maps to `Handler(_)` with exactly one dispatch
    // and the database rolled back rather than committed.
    #[test]
    fn source_authored_failure_returns_handler_error_without_reinvocation() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Err(ProcessingError::source_message("processing failed"))
        }

        let fixture = RuntimeInvocationFixture::foreground_run(example_identity());
        let args = fixture.build_argv(&[]);

        let err = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Handler(_)
        ));
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    // Test 15: a successful handler moves the session Prepared -> Running -> Succeeded.
    #[test]
    fn session_transitions_to_succeeded_after_successful_handler() {
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            Ok(())
        }

        let fixture = RuntimeInvocationFixture::foreground_run(example_identity());
        let args = fixture.build_argv(&[]);

        let result = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
        );
        assert!(result.is_ok(), "{result:?}");

        let record = fixture.store().load(fixture.session()).unwrap();
        assert_eq!(record.state(), crate::session::SessionState::Succeeded);
    }

    // Test 16: a source-authored failure moves the session Prepared -> Running -> Failed.
    #[test]
    fn session_transitions_to_failed_after_source_authored_failure() {
        fn process(_ctx: &mut ProcessingContext, _args: &[OsString]) -> ProcessingResult<()> {
            Err(ProcessingError::source_message("processing failed"))
        }

        let fixture = RuntimeInvocationFixture::foreground_run(example_identity());
        let args = fixture.build_argv(&[]);

        let result = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process),
        );
        assert!(result.is_err());

        let record = fixture.store().load(fixture.session()).unwrap();
        assert_eq!(record.state(), crate::session::SessionState::Failed);
    }

    // Test 17: session/lease identities are validated before dispatch — an envelope
    // session that was never prepared/leased in this fixture's store is rejected before
    // the handler is ever invoked.
    #[test]
    fn session_identity_mismatch_is_rejected_before_handler_dispatch() {
        fn process_must_not_be_called(
            _ctx: &mut ProcessingContext,
            _args: &[OsString],
        ) -> ProcessingResult<()> {
            panic!("process must not be called when the envelope session was never prepared");
        }

        // Establishes a real session-store/lease/env-context environment for a
        // different session id than the one encoded below.
        let _fixture = RuntimeInvocationFixture::foreground_run(example_identity());

        let foreign_envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            example_identity(),
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

        let err = run_processing_runtime_invocation(
            &args,
            example_identity(),
            &ProcessingSourceContractV1::new(process_must_not_be_called),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            ProcessingRuntimeInvocationExecutionError::Session(_)
        ));
    }
}
