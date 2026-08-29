use std::ffi::OsString;
use std::fmt;

use crate::runtime::{
    ParsedRuntimeInvocation, RuntimeExecutionMode, RuntimeIdentity, RuntimeInvocationEnvelopeV1,
    RuntimeOperation, RuntimeProtocol,
};

use super::{ProcessDataFn, ProcessingSourceContractV1};

#[derive(Clone, Copy, Debug)]
pub enum AdmittedProcessingHandler {
    Process(ProcessDataFn),
}

impl AdmittedProcessingHandler {
    pub const fn execution_mode(&self) -> RuntimeExecutionMode {
        match self {
            Self::Process(_) => RuntimeExecutionMode::Run,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessingRuntimeInvocationAdmissionError {
    WrongCompiledProtocol {
        actual: RuntimeProtocol,
    },
    WrongCompiledOperation {
        actual: RuntimeOperation,
    },
    IdentityMismatch {
        compiled: RuntimeIdentity,
        envelope: RuntimeIdentity,
    },
    DescriptorContractVersionMismatch {
        identity_version: u32,
        descriptor_version: u32,
    },
}

impl fmt::Display for ProcessingRuntimeInvocationAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongCompiledProtocol { actual } => write!(
                formatter,
                "compiled runtime protocol mismatch: expected {:?}, actual {:?}",
                RuntimeProtocol::Http,
                actual
            ),
            Self::WrongCompiledOperation { actual } => write!(
                formatter,
                "compiled runtime operation mismatch: expected {:?}, actual {:?}",
                RuntimeOperation::Processing,
                actual
            ),
            Self::IdentityMismatch { compiled, envelope } => write!(
                formatter,
                "runtime identity mismatch: compiled {compiled:?}, envelope {envelope:?}"
            ),
            Self::DescriptorContractVersionMismatch {
                identity_version,
                descriptor_version,
            } => write!(
                formatter,
                "descriptor contract version mismatch: identity version {identity_version}, descriptor version {descriptor_version}"
            ),
        }
    }
}

impl std::error::Error for ProcessingRuntimeInvocationAdmissionError {}

#[derive(Clone, Debug)]
pub struct AdmittedProcessingRuntimeInvocation {
    envelope: RuntimeInvocationEnvelopeV1,
    source_arguments: Vec<OsString>,
    handler: AdmittedProcessingHandler,
}

impl AdmittedProcessingRuntimeInvocation {
    pub fn envelope(&self) -> &RuntimeInvocationEnvelopeV1 {
        &self.envelope
    }

    pub fn source_arguments(&self) -> &[OsString] {
        &self.source_arguments
    }

    pub const fn handler(&self) -> AdmittedProcessingHandler {
        self.handler
    }

    pub fn into_parts(
        self,
    ) -> (
        RuntimeInvocationEnvelopeV1,
        Vec<OsString>,
        AdmittedProcessingHandler,
    ) {
        (self.envelope, self.source_arguments, self.handler)
    }
}

pub fn admit_processing_runtime_invocation(
    parsed: ParsedRuntimeInvocation,
    compiled_identity: RuntimeIdentity,
    source: &ProcessingSourceContractV1,
) -> Result<AdmittedProcessingRuntimeInvocation, ProcessingRuntimeInvocationAdmissionError> {
    if compiled_identity.protocol() != RuntimeProtocol::Http {
        return Err(
            ProcessingRuntimeInvocationAdmissionError::WrongCompiledProtocol {
                actual: compiled_identity.protocol(),
            },
        );
    }

    if compiled_identity.operation() != RuntimeOperation::Processing {
        return Err(
            ProcessingRuntimeInvocationAdmissionError::WrongCompiledOperation {
                actual: compiled_identity.operation(),
            },
        );
    }

    let (envelope, source_arguments) = parsed.into_parts();

    if envelope.runtime() != compiled_identity {
        return Err(
            ProcessingRuntimeInvocationAdmissionError::IdentityMismatch {
                compiled: compiled_identity,
                envelope: envelope.runtime(),
            },
        );
    }

    if compiled_identity.source_contract_version() != ProcessingSourceContractV1::CONTRACT_VERSION {
        return Err(
            ProcessingRuntimeInvocationAdmissionError::DescriptorContractVersionMismatch {
                identity_version: compiled_identity.source_contract_version(),
                descriptor_version: ProcessingSourceContractV1::CONTRACT_VERSION,
            },
        );
    }

    let handler = match envelope.execution_mode() {
        RuntimeExecutionMode::Run => AdmittedProcessingHandler::Process(source.process_handler()),
        RuntimeExecutionMode::Resume => {
            unreachable!("processing runtime invocations do not support resume execution mode")
        }
    };

    Ok(AdmittedProcessingRuntimeInvocation {
        envelope,
        source_arguments,
        handler,
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::processing::{ProcessingContext, ProcessingResult, ProcessingSourceContractV1};
    use crate::runtime::{
        ProjectInvocationIdentity, RuntimeExecutionMode, RuntimeIdentity,
        RuntimeInvocationEnvelopeV1, RuntimeOperation, RuntimeProtocol, RuntimeSupervisionMode,
        SessionInvocationIdentity,
    };

    use super::{
        AdmittedProcessingHandler, ProcessingRuntimeInvocationAdmissionError,
        admit_processing_runtime_invocation,
    };

    fn process_handler(
        _context: &mut ProcessingContext,
        _args: &[OsString],
    ) -> ProcessingResult<()> {
        Ok(())
    }

    fn counting_handler(
        _context: &mut ProcessingContext,
        _args: &[OsString],
    ) -> ProcessingResult<()> {
        static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
        CALL_COUNT.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn example_envelope() -> RuntimeInvocationEnvelopeV1 {
        RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            RuntimeIdentity::http_processing("example-source", 1),
            SessionInvocationIdentity::new("session-123").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap()
    }

    #[test]
    fn matching_http_processing_invocation_is_admitted() {
        let parsed = crate::runtime::parse_runtime_invocation(&[
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(example_envelope().to_json().unwrap()),
            OsString::from("--"),
            OsString::from("alpha"),
            OsString::from("beta"),
        ])
        .unwrap();

        let admitted = admit_processing_runtime_invocation(
            parsed,
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap();

        assert_eq!(admitted.envelope(), &example_envelope());
        assert_eq!(
            admitted.source_arguments(),
            &[OsString::from("alpha"), OsString::from("beta")]
        );
        assert_eq!(
            admitted.handler().execution_mode(),
            RuntimeExecutionMode::Run
        );
        assert_eq!(
            admitted.envelope().supervision_mode(),
            RuntimeSupervisionMode::Foreground
        );
    }

    #[test]
    fn selected_handler_matches_the_registered_function_pointer() {
        let expected: crate::processing::ProcessDataFn = process_handler;
        let parsed = crate::runtime::parse_runtime_invocation(&[
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(example_envelope().to_json().unwrap()),
            OsString::from("--"),
        ])
        .unwrap();

        let admitted = admit_processing_runtime_invocation(
            parsed,
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(expected),
        )
        .unwrap();

        let selected = match admitted.handler() {
            AdmittedProcessingHandler::Process(handler) => handler,
        };

        assert!(std::ptr::fn_addr_eq(selected, expected));
    }

    #[test]
    fn protocol_check_happens_before_operation_check() {
        let parsed = crate::runtime::parse_runtime_invocation(&[
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(example_envelope().to_json().unwrap()),
            OsString::from("--"),
        ])
        .unwrap();

        let error = admit_processing_runtime_invocation(
            parsed,
            RuntimeIdentity::from_parts(
                "example-source",
                RuntimeProtocol::Http,
                RuntimeOperation::Acquisition,
                1,
            ),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProcessingRuntimeInvocationAdmissionError::WrongCompiledOperation { .. }
        ));
    }

    #[test]
    fn identity_mismatch_returns_typed_error() {
        let parsed = crate::runtime::parse_runtime_invocation(&[
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(example_envelope().to_json().unwrap()),
            OsString::from("--"),
        ])
        .unwrap();

        let error = admit_processing_runtime_invocation(
            parsed,
            RuntimeIdentity::http_processing("different-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProcessingRuntimeInvocationAdmissionError::IdentityMismatch { .. }
        ));
    }

    #[test]
    fn descriptor_version_mismatch_returns_typed_error() {
        let envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            RuntimeIdentity::http_processing("example-source", 2),
            SessionInvocationIdentity::new("session-123").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        let parsed = crate::runtime::parse_runtime_invocation(&[
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(envelope.to_json().unwrap()),
            OsString::from("--"),
        ])
        .unwrap();

        let error = admit_processing_runtime_invocation(
            parsed,
            RuntimeIdentity::http_processing("example-source", 2),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProcessingRuntimeInvocationAdmissionError::DescriptorContractVersionMismatch { .. }
        ));
    }

    #[test]
    fn processing_resume_mode_is_rejected_by_existing_envelope_model() {
        let result = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            RuntimeIdentity::http_processing("example-source", 1),
            SessionInvocationIdentity::new("session-123").unwrap(),
            RuntimeExecutionMode::Resume,
            RuntimeSupervisionMode::Foreground,
        );

        assert!(result.is_err());
    }

    #[test]
    fn failed_admission_does_not_invoke_the_selected_handler() {
        let parsed = crate::runtime::parse_runtime_invocation(&[
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(example_envelope().to_json().unwrap()),
            OsString::from("--"),
        ])
        .unwrap();

        let error = admit_processing_runtime_invocation(
            parsed,
            RuntimeIdentity::http_processing("different-source", 1),
            &ProcessingSourceContractV1::new(counting_handler),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProcessingRuntimeInvocationAdmissionError::IdentityMismatch { .. }
        ));
    }

    #[test]
    fn error_text_hides_source_values_and_identity_fields() {
        let parsed = crate::runtime::parse_runtime_invocation(&[
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(example_envelope().to_json().unwrap()),
            OsString::from("--"),
            OsString::from("alpha"),
            OsString::from("--lexicon-runtime-info"),
            OsString::from("gamma"),
        ])
        .unwrap();

        let error = admit_processing_runtime_invocation(
            parsed,
            RuntimeIdentity::http_processing("other-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap_err();

        let message = error.to_string();
        assert!(!message.contains("example-project"));
        assert!(!message.contains("session-123"));
        assert!(!message.contains("alpha"));
        assert!(!message.contains("--lexicon-runtime-info"));
        assert!(!message.contains("{\"schema_version\""));
    }

    #[cfg(unix)]
    #[test]
    fn unix_source_arguments_are_preserved_byte_for_byte() {
        use std::os::unix::ffi::OsStringExt;

        let original = [
            OsString::from_vec(vec![b'a', 0x80, b'c']),
            OsString::from_vec(vec![0xFF, 0xFE, 0xFD]),
        ];
        let envelope = example_envelope();
        let parsed = crate::runtime::parse_runtime_invocation(&[
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(envelope.to_json().unwrap()),
            OsString::from("--"),
            original[0].clone(),
            original[1].clone(),
        ])
        .unwrap();

        let admitted = admit_processing_runtime_invocation(
            parsed,
            RuntimeIdentity::http_processing("example-source", 1),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap();

        assert_eq!(admitted.source_arguments(), &original);
    }
}
