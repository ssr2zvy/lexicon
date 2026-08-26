use std::ffi::OsString;
use std::fmt;

use crate::runtime::{
    MissingHttpCapabilities, ParsedRuntimeInvocation, RuntimeExecutionMode, RuntimeIdentity,
    RuntimeInvocationEnvelopeV1, RuntimeOperation, RuntimeProtocol,
};

use super::{HttpAcquireFn, HttpCapabilitySet, HttpResumeFn, HttpSourceContractV1};

#[derive(Clone, Copy, Debug)]
pub enum AdmittedHttpHandler {
    Acquire(HttpAcquireFn),
    Resume(HttpResumeFn),
}

impl AdmittedHttpHandler {
    pub const fn execution_mode(&self) -> RuntimeExecutionMode {
        match self {
            Self::Acquire(_) => RuntimeExecutionMode::Run,
            Self::Resume(_) => RuntimeExecutionMode::Resume,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpRuntimeInvocationAdmissionError {
    CompiledProtocolMismatch {
        expected: RuntimeProtocol,
        actual: RuntimeProtocol,
    },
    CompiledOperationMismatch {
        expected: RuntimeOperation,
        actual: RuntimeOperation,
    },
    IdentityMismatch {
        expected: RuntimeIdentity,
        actual: RuntimeIdentity,
    },
    DescriptorContractVersionMismatch {
        identity_version: u32,
        descriptor_version: u32,
    },
    MissingCapabilities(MissingHttpCapabilities),
    ResumeHandlerUnavailable,
}

impl fmt::Display for HttpRuntimeInvocationAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompiledProtocolMismatch { expected, actual } => write!(
                formatter,
                "compiled runtime protocol mismatch: expected {:?}, actual {:?}",
                expected, actual
            ),
            Self::CompiledOperationMismatch { expected, actual } => write!(
                formatter,
                "compiled runtime operation mismatch: expected {:?}, actual {:?}",
                expected, actual
            ),
            Self::IdentityMismatch { expected, actual } => write!(
                formatter,
                "runtime identity mismatch: expected {expected:?}, actual {actual:?}"
            ),
            Self::DescriptorContractVersionMismatch {
                identity_version,
                descriptor_version,
            } => write!(
                formatter,
                "descriptor contract version mismatch: identity version {identity_version}, descriptor version {descriptor_version}"
            ),
            Self::MissingCapabilities(error) => write!(formatter, "{error}"),
            Self::ResumeHandlerUnavailable => formatter.write_str(
                "resume handler is unavailable for the selected HTTP runtime invocation",
            ),
        }
    }
}

impl std::error::Error for HttpRuntimeInvocationAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MissingCapabilities(error) => Some(error),
            Self::CompiledProtocolMismatch { .. }
            | Self::CompiledOperationMismatch { .. }
            | Self::IdentityMismatch { .. }
            | Self::DescriptorContractVersionMismatch { .. }
            | Self::ResumeHandlerUnavailable => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AdmittedHttpRuntimeInvocation {
    envelope: RuntimeInvocationEnvelopeV1,
    source_arguments: Vec<OsString>,
    handler: AdmittedHttpHandler,
    available_capabilities: HttpCapabilitySet,
}

impl AdmittedHttpRuntimeInvocation {
    pub fn envelope(&self) -> &RuntimeInvocationEnvelopeV1 {
        &self.envelope
    }

    pub fn source_arguments(&self) -> &[OsString] {
        &self.source_arguments
    }

    pub const fn handler(&self) -> AdmittedHttpHandler {
        self.handler
    }

    pub const fn available_capabilities(&self) -> HttpCapabilitySet {
        self.available_capabilities
    }

    pub fn into_parts(
        self,
    ) -> (
        RuntimeInvocationEnvelopeV1,
        Vec<OsString>,
        AdmittedHttpHandler,
        HttpCapabilitySet,
    ) {
        (
            self.envelope,
            self.source_arguments,
            self.handler,
            self.available_capabilities,
        )
    }
}

pub fn admit_http_runtime_invocation(
    parsed: ParsedRuntimeInvocation,
    compiled_identity: RuntimeIdentity,
    source: &HttpSourceContractV1,
    available_capabilities: HttpCapabilitySet,
) -> Result<AdmittedHttpRuntimeInvocation, HttpRuntimeInvocationAdmissionError> {
    if compiled_identity.protocol() != RuntimeProtocol::Http {
        return Err(
            HttpRuntimeInvocationAdmissionError::CompiledProtocolMismatch {
                expected: RuntimeProtocol::Http,
                actual: compiled_identity.protocol(),
            },
        );
    }

    if compiled_identity.operation() != RuntimeOperation::Acquisition {
        return Err(
            HttpRuntimeInvocationAdmissionError::CompiledOperationMismatch {
                expected: RuntimeOperation::Acquisition,
                actual: compiled_identity.operation(),
            },
        );
    }

    let (envelope, source_arguments) = parsed.into_parts();

    if envelope.runtime() != compiled_identity {
        return Err(HttpRuntimeInvocationAdmissionError::IdentityMismatch {
            expected: compiled_identity,
            actual: envelope.runtime(),
        });
    }

    if compiled_identity.source_contract_version() != HttpSourceContractV1::CONTRACT_VERSION {
        return Err(
            HttpRuntimeInvocationAdmissionError::DescriptorContractVersionMismatch {
                identity_version: compiled_identity.source_contract_version(),
                descriptor_version: HttpSourceContractV1::CONTRACT_VERSION,
            },
        );
    }

    let required_capabilities = source.required_capabilities();
    if !required_capabilities.is_subset_of(available_capabilities) {
        return Err(HttpRuntimeInvocationAdmissionError::MissingCapabilities(
            MissingHttpCapabilities::new(
                required_capabilities.missing_from(available_capabilities),
            ),
        ));
    }

    let handler = match envelope.execution_mode() {
        RuntimeExecutionMode::Run => AdmittedHttpHandler::Acquire(source.acquire()),
        RuntimeExecutionMode::Resume => {
            let resume = source
                .resume_handler()
                .ok_or(HttpRuntimeInvocationAdmissionError::ResumeHandlerUnavailable)?;
            AdmittedHttpHandler::Resume(resume)
        }
    };

    Ok(AdmittedHttpRuntimeInvocation {
        envelope,
        source_arguments,
        handler,
        available_capabilities,
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use crate::runtime::{
        ProjectInvocationIdentity, RuntimeExecutionMode, RuntimeIdentity,
        RuntimeInvocationEnvelopeV1, RuntimeSupervisionMode, SessionInvocationIdentity,
    };

    use super::{
        AdmittedHttpHandler, HttpRuntimeInvocationAdmissionError, admit_http_runtime_invocation,
    };
    use crate::protocols::http::{
        HttpAcquisitionContext, HttpCapability, HttpCapabilitySet, HttpSourceContractV1,
    };

    fn acquire_handler(
        _context: &mut HttpAcquisitionContext,
        _arguments: &[OsString],
    ) -> crate::protocols::http::AcquisitionResult<()> {
        Ok(())
    }

    fn resume_handler(
        _context: &mut HttpAcquisitionContext,
        _arguments: &[OsString],
    ) -> crate::protocols::http::AcquisitionResult<()> {
        Ok(())
    }

    fn example_envelope() -> RuntimeInvocationEnvelopeV1 {
        RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-123").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap()
    }

    #[test]
    fn admitted_handler_execution_mode_matches_selected_function_type() {
        let acquire = HttpSourceContractV1::new(acquire_handler).acquire();
        let resume = HttpSourceContractV1::new(acquire_handler)
            .with_resume(resume_handler)
            .resume_handler()
            .unwrap();

        assert_eq!(
            AdmittedHttpHandler::Acquire(acquire).execution_mode(),
            RuntimeExecutionMode::Run
        );
        assert_eq!(
            AdmittedHttpHandler::Resume(resume).execution_mode(),
            RuntimeExecutionMode::Resume
        );
    }

    #[test]
    fn admitting_http_runtime_invocation_preserves_source_arguments_and_handler() {
        let parsed = crate::runtime::parse_runtime_invocation(&[
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(example_envelope().to_json().unwrap()),
            OsString::from("--"),
            OsString::from("alpha"),
            OsString::from("beta"),
        ])
        .unwrap();

        let admitted = admit_http_runtime_invocation(
            parsed,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
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
            admitted.available_capabilities(),
            HttpCapabilitySet::empty()
        );
    }

    #[test]
    fn admitting_runtime_invocation_checks_protocol_before_operation() {
        let parsed = crate::runtime::parse_runtime_invocation(&[
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(example_envelope().to_json().unwrap()),
            OsString::from("--"),
        ])
        .unwrap();

        let error = admit_http_runtime_invocation(
            parsed,
            RuntimeIdentity::from_parts(
                "example-source",
                crate::runtime::RuntimeProtocol::Http,
                crate::runtime::RuntimeOperation::Processing,
                1,
            ),
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            HttpRuntimeInvocationAdmissionError::CompiledOperationMismatch { .. }
        ));
    }

    #[test]
    fn resume_handler_is_required_for_resume_execution_mode() {
        let envelope = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("example-project").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-123").unwrap(),
            RuntimeExecutionMode::Resume,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();

        let parsed = crate::runtime::parse_runtime_invocation(&[
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(envelope.to_json().unwrap()),
            OsString::from("--"),
        ])
        .unwrap();

        let error = admit_http_runtime_invocation(
            parsed,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            HttpRuntimeInvocationAdmissionError::ResumeHandlerUnavailable
        ));
    }

    #[test]
    fn capability_shortfall_is_reported_with_missing_capabilities() {
        let parsed = crate::runtime::parse_runtime_invocation(&[
            OsString::from("--lexicon-invocation-v1"),
            OsString::from(example_envelope().to_json().unwrap()),
            OsString::from("--"),
        ])
        .unwrap();

        let error = admit_http_runtime_invocation(
            parsed,
            RuntimeIdentity::http_acquisition("example-source", 1),
            &HttpSourceContractV1::new(acquire_handler)
                .requires(HttpCapability::ClientCertificateV1),
            HttpCapabilitySet::empty(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            HttpRuntimeInvocationAdmissionError::MissingCapabilities(_)
        ));
    }
}
