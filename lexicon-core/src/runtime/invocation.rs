use std::fmt;

use crate::runtime::{RuntimeIdentity, RuntimeOperation};

pub const RUNTIME_INVOCATION_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInvocationIdentifierError {
    UnknownIdentifier {
        kind: &'static str,
        value: String,
    },
}

impl RuntimeInvocationIdentifierError {
    pub fn unknown(kind: &'static str, value: impl Into<String>) -> Self {
        Self::UnknownIdentifier {
            kind,
            value: value.into(),
        }
    }
}

impl fmt::Display for RuntimeInvocationIdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownIdentifier { kind, value } => {
                write!(formatter, "unknown {kind} identifier: {value}")
            }
        }
    }
}

impl std::error::Error for RuntimeInvocationIdentifierError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInvocationValueError {
    InvalidValue {
        field: &'static str,
        value: String,
    },
}

impl RuntimeInvocationValueError {
    pub fn invalid(field: &'static str, value: impl Into<String>) -> Self {
        Self::InvalidValue {
            field,
            value: value.into(),
        }
    }
}

impl fmt::Display for RuntimeInvocationValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue { field, value } => {
                write!(formatter, "invalid {field} value: {value}")
            }
        }
    }
}

impl std::error::Error for RuntimeInvocationValueError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInvocationConstructionError {
    InvalidSourceContractVersion { value: u32 },
    UnsupportedExecutionMode {
        runtime: RuntimeIdentity,
        execution_mode: RuntimeExecutionMode,
    },
}

impl fmt::Display for RuntimeInvocationConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSourceContractVersion { value } => {
                write!(formatter, "invalid source contract version: {value}")
            }
            Self::UnsupportedExecutionMode {
                runtime,
                execution_mode,
            } => {
                write!(
                    formatter,
                    "runtime identity {:?} does not support execution mode {:?}",
                    runtime, execution_mode
                )
            }
        }
    }
}

impl std::error::Error for RuntimeInvocationConstructionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeExecutionMode {
    Run,
    Resume,
}

impl RuntimeExecutionMode {
    pub const fn identifier(&self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Resume => "resume",
        }
    }

    pub fn from_identifier(value: &str) -> Result<Self, RuntimeInvocationIdentifierError> {
        match value {
            "run" => Ok(Self::Run),
            "resume" => Ok(Self::Resume),
            _ => Err(RuntimeInvocationIdentifierError::unknown(
                "execution mode",
                value,
            )),
        }
    }

    fn supports_operation(self, operation: RuntimeOperation) -> bool {
        match operation {
            RuntimeOperation::Acquisition => true,
            RuntimeOperation::Processing => matches!(self, Self::Run),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeSupervisionMode {
    Foreground,
    Background,
}

impl RuntimeSupervisionMode {
    pub const fn identifier(&self) -> &'static str {
        match self {
            Self::Foreground => "foreground",
            Self::Background => "background",
        }
    }

    pub fn from_identifier(value: &str) -> Result<Self, RuntimeInvocationIdentifierError> {
        match value {
            "foreground" => Ok(Self::Foreground),
            "background" => Ok(Self::Background),
            _ => Err(RuntimeInvocationIdentifierError::unknown(
                "supervision mode",
                value,
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectInvocationIdentity {
    name: String,
}

impl ProjectInvocationIdentity {
    pub fn new(name: impl Into<String>) -> Result<Self, RuntimeInvocationValueError> {
        let name = name.into();
        validate_safe_component(&name, "project name")?;
        Ok(Self { name })
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionInvocationIdentity {
    id: String,
}

impl SessionInvocationIdentity {
    pub fn new(id: impl Into<String>) -> Result<Self, RuntimeInvocationValueError> {
        let id = id.into();
        validate_safe_component(&id, "session id")?;
        Ok(Self { id })
    }

    pub fn id(&self) -> &str {
        self.id.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInvocationEnvelopeV1 {
    project: ProjectInvocationIdentity,
    runtime: RuntimeIdentity,
    session: SessionInvocationIdentity,
    execution_mode: RuntimeExecutionMode,
    supervision_mode: RuntimeSupervisionMode,
}

impl RuntimeInvocationEnvelopeV1 {
    pub fn new(
        project: ProjectInvocationIdentity,
        runtime: RuntimeIdentity,
        session: SessionInvocationIdentity,
        execution_mode: RuntimeExecutionMode,
        supervision_mode: RuntimeSupervisionMode,
    ) -> Result<Self, RuntimeInvocationConstructionError> {
        if runtime.source_contract_version() == 0 {
            return Err(RuntimeInvocationConstructionError::InvalidSourceContractVersion {
                value: runtime.source_contract_version(),
            });
        }

        if !execution_mode.supports_operation(runtime.operation()) {
            return Err(RuntimeInvocationConstructionError::UnsupportedExecutionMode {
                runtime,
                execution_mode,
            });
        }

        Ok(Self {
            project,
            runtime,
            session,
            execution_mode,
            supervision_mode,
        })
    }

    pub fn project(&self) -> &ProjectInvocationIdentity {
        &self.project
    }

    pub const fn runtime(&self) -> RuntimeIdentity {
        self.runtime
    }

    pub fn session(&self) -> &SessionInvocationIdentity {
        &self.session
    }

    pub const fn execution_mode(&self) -> RuntimeExecutionMode {
        self.execution_mode
    }

    pub const fn supervision_mode(&self) -> RuntimeSupervisionMode {
        self.supervision_mode
    }
}

fn validate_safe_component(value: &str, field: &'static str) -> Result<(), RuntimeInvocationValueError> {
    if value.is_empty() {
        return Err(RuntimeInvocationValueError::invalid(field, value));
    }

    if value == "." || value == ".." {
        return Err(RuntimeInvocationValueError::invalid(field, value));
    }

    if value == "/" || value == "\\" {
        return Err(RuntimeInvocationValueError::invalid(field, value));
    }

    if value.contains('/') || value.contains('\\') {
        return Err(RuntimeInvocationValueError::invalid(field, value));
    }

    if value.contains('\0') || value.chars().any(|character| character.is_ascii_control()) {
        return Err(RuntimeInvocationValueError::invalid(field, value));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectInvocationIdentity, RuntimeExecutionMode, RuntimeInvocationConstructionError,
        RuntimeInvocationEnvelopeV1, RuntimeInvocationIdentifierError, RuntimeSupervisionMode,
        SessionInvocationIdentity,
    };
    use crate::runtime::RuntimeIdentity;

    #[test]
    fn runtime_execution_mode_identifiers_are_stable() {
        assert_eq!(RuntimeExecutionMode::Run.identifier(), "run");
        assert_eq!(RuntimeExecutionMode::Resume.identifier(), "resume");
        assert_eq!(RuntimeExecutionMode::from_identifier("run"), Ok(RuntimeExecutionMode::Run));
        assert_eq!(
            RuntimeExecutionMode::from_identifier("resume"),
            Ok(RuntimeExecutionMode::Resume)
        );
    }

    #[test]
    fn runtime_execution_mode_from_identifier_rejects_case_and_whitespace_variants() {
        assert!(matches!(
            RuntimeExecutionMode::from_identifier("Run"),
            Err(RuntimeInvocationIdentifierError::UnknownIdentifier { .. })
        ));
        assert!(matches!(
            RuntimeExecutionMode::from_identifier(" run "),
            Err(RuntimeInvocationIdentifierError::UnknownIdentifier { .. })
        ));
    }

    #[test]
    fn runtime_supervision_mode_identifiers_are_stable() {
        assert_eq!(RuntimeSupervisionMode::Foreground.identifier(), "foreground");
        assert_eq!(RuntimeSupervisionMode::Background.identifier(), "background");
        assert_eq!(
            RuntimeSupervisionMode::from_identifier("foreground"),
            Ok(RuntimeSupervisionMode::Foreground)
        );
        assert_eq!(
            RuntimeSupervisionMode::from_identifier("background"),
            Ok(RuntimeSupervisionMode::Background)
        );
    }

    #[test]
    fn project_invocation_identity_accepts_safe_names_and_rejects_invalid_values() {
        assert_eq!(
            ProjectInvocationIdentity::new("example-project").unwrap().name(),
            "example-project"
        );

        assert!(ProjectInvocationIdentity::new("").is_err());
        assert!(ProjectInvocationIdentity::new(".").is_err());
        assert!(ProjectInvocationIdentity::new("..").is_err());
        assert!(ProjectInvocationIdentity::new("/").is_err());
        assert!(ProjectInvocationIdentity::new("\\").is_err());
        assert!(ProjectInvocationIdentity::new("foo/bar").is_err());
        assert!(ProjectInvocationIdentity::new("bad\u{0000}name").is_err());
        assert!(ProjectInvocationIdentity::new("bad\nname").is_err());
    }

    #[test]
    fn session_invocation_identity_uses_same_validation_rules() {
        assert_eq!(SessionInvocationIdentity::new("session-1").unwrap().id(), "session-1");
        assert!(SessionInvocationIdentity::new("..").is_err());
        assert!(SessionInvocationIdentity::new("session\\1").is_err());
    }

    #[test]
    fn runtime_invocation_envelope_accepts_supported_modes() {
        let identity = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("project-x").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-1").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();

        assert_eq!(identity.project().name(), "project-x");
        assert_eq!(identity.runtime(), RuntimeIdentity::http_acquisition("example-source", 1));
        assert_eq!(identity.session().id(), "session-1");
        assert_eq!(identity.execution_mode(), RuntimeExecutionMode::Run);
        assert_eq!(identity.supervision_mode(), RuntimeSupervisionMode::Foreground);

        let resume = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("project-y").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-2").unwrap(),
            RuntimeExecutionMode::Resume,
            RuntimeSupervisionMode::Background,
        )
        .unwrap();
        assert_eq!(resume.execution_mode(), RuntimeExecutionMode::Resume);

        let processing = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("project-z").unwrap(),
            RuntimeIdentity::http_processing("example-source", 1),
            SessionInvocationIdentity::new("session-3").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        assert_eq!(processing.runtime().operation(), crate::runtime::RuntimeOperation::Processing);
    }

    #[test]
    fn runtime_invocation_envelope_rejects_invalid_contract_and_resume_processing() {
        let invalid_version = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("project-a").unwrap(),
            RuntimeIdentity::http_processing("example-source", 0),
            SessionInvocationIdentity::new("session-a").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        );
        assert!(matches!(
            invalid_version,
            Err(RuntimeInvocationConstructionError::InvalidSourceContractVersion { .. })
        ));

        let invalid_processing_resume = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("project-b").unwrap(),
            RuntimeIdentity::http_processing("example-source", 1),
            SessionInvocationIdentity::new("session-b").unwrap(),
            RuntimeExecutionMode::Resume,
            RuntimeSupervisionMode::Foreground,
        );
        assert!(matches!(
            invalid_processing_resume,
            Err(RuntimeInvocationConstructionError::UnsupportedExecutionMode { .. })
        ));
    }
}
