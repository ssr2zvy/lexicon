use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::runtime::{RuntimeIdentifierError, RuntimeIdentity, RuntimeOperation, RuntimeProtocol};

pub const RUNTIME_INVOCATION_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInvocationEncodingError {
    Serialization(String),
}

impl fmt::Display for RuntimeInvocationEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(message) => {
                write!(
                    formatter,
                    "runtime invocation serialization error: {message}"
                )
            }
        }
    }
}

impl std::error::Error for RuntimeInvocationEncodingError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInvocationDecodingError {
    JsonSyntax(String),
    UnknownSchemaVersion(u32),
    UnknownIdentifier { field: &'static str, value: String },
    InvalidProjectIdentity(RuntimeInvocationValueError),
    InvalidSessionIdentity(RuntimeInvocationValueError),
    InvalidVersion { field: &'static str, value: u32 },
    InvalidConstruction(RuntimeInvocationConstructionError),
    StructuralDocument(String),
}

impl fmt::Display for RuntimeInvocationDecodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsonSyntax(message) => write!(formatter, "invalid JSON: {message}"),
            Self::UnknownSchemaVersion(version) => {
                write!(
                    formatter,
                    "unknown runtime invocation schema version: {version}"
                )
            }
            Self::UnknownIdentifier { field, value } => {
                write!(formatter, "unknown {field} identifier: {value}")
            }
            Self::InvalidProjectIdentity(error) => {
                write!(formatter, "invalid project identity: {error}")
            }
            Self::InvalidSessionIdentity(error) => {
                write!(formatter, "invalid session identity: {error}")
            }
            Self::InvalidVersion { field, value } => {
                write!(formatter, "invalid {field} value: {value}")
            }
            Self::InvalidConstruction(error) => write!(
                formatter,
                "invalid runtime invocation construction: {error}"
            ),
            Self::StructuralDocument(message) => {
                write!(
                    formatter,
                    "malformed runtime invocation document: {message}"
                )
            }
        }
    }
}

impl std::error::Error for RuntimeInvocationDecodingError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeInvocationDocumentV1 {
    schema_version: u32,
    project: ProjectDocumentV1,
    runtime: RuntimeDocumentV1,
    session: SessionDocumentV1,
    execution: ExecutionDocumentV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectDocumentV1 {
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeDocumentV1 {
    source: String,
    protocol: String,
    operation: String,
    source_contract_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionDocumentV1 {
    id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionDocumentV1 {
    mode: String,
    supervision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInvocationIdentifierError {
    UnknownIdentifier { kind: &'static str, value: String },
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
    InvalidValue { field: &'static str, value: String },
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
    InvalidSourceContractVersion {
        value: u32,
    },
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
            return Err(
                RuntimeInvocationConstructionError::InvalidSourceContractVersion {
                    value: runtime.source_contract_version(),
                },
            );
        }

        if !execution_mode.supports_operation(runtime.operation()) {
            return Err(
                RuntimeInvocationConstructionError::UnsupportedExecutionMode {
                    runtime,
                    execution_mode,
                },
            );
        }

        Ok(Self {
            project,
            runtime,
            session,
            execution_mode,
            supervision_mode,
        })
    }

    pub fn to_json(&self) -> Result<String, RuntimeInvocationEncodingError> {
        let document = RuntimeInvocationDocumentV1 {
            schema_version: RUNTIME_INVOCATION_PROTOCOL_VERSION,
            project: ProjectDocumentV1 {
                name: self.project.name().to_string(),
            },
            runtime: RuntimeDocumentV1 {
                source: self.runtime.source_name().to_string(),
                protocol: self.runtime.protocol().identifier().to_string(),
                operation: self.runtime.operation().identifier().to_string(),
                source_contract_version: self.runtime.source_contract_version(),
            },
            session: SessionDocumentV1 {
                id: self.session.id().to_string(),
            },
            execution: ExecutionDocumentV1 {
                mode: self.execution_mode.identifier().to_string(),
                supervision: self.supervision_mode.identifier().to_string(),
            },
        };

        serde_json::to_string(&document)
            .map_err(|error| RuntimeInvocationEncodingError::Serialization(error.to_string()))
    }

    pub fn from_json(input: &str) -> Result<Self, RuntimeInvocationDecodingError> {
        let document = serde_json::from_str::<RuntimeInvocationDocumentV1>(input).map_err(
            |error| match error.classify() {
                serde_json::error::Category::Syntax => {
                    RuntimeInvocationDecodingError::JsonSyntax(error.to_string())
                }
                _ => RuntimeInvocationDecodingError::StructuralDocument(error.to_string()),
            },
        )?;

        reject_duplicate_object_keys(input)?;

        if document.schema_version != RUNTIME_INVOCATION_PROTOCOL_VERSION {
            return Err(RuntimeInvocationDecodingError::UnknownSchemaVersion(
                document.schema_version,
            ));
        }

        if document.runtime.source_contract_version == 0 {
            return Err(RuntimeInvocationDecodingError::InvalidVersion {
                field: "runtime.source_contract_version",
                value: document.runtime.source_contract_version,
            });
        }

        let project = ProjectInvocationIdentity::new(document.project.name)
            .map_err(RuntimeInvocationDecodingError::InvalidProjectIdentity)?;
        let session = SessionInvocationIdentity::new(document.session.id)
            .map_err(RuntimeInvocationDecodingError::InvalidSessionIdentity)?;

        let protocol =
            RuntimeProtocol::from_identifier(&document.runtime.protocol).map_err(|error| {
                match error {
                    RuntimeIdentifierError::UnknownIdentifier { value, .. } => {
                        RuntimeInvocationDecodingError::UnknownIdentifier {
                            field: "runtime.protocol",
                            value,
                        }
                    }
                }
            })?;

        let operation =
            RuntimeOperation::from_identifier(&document.runtime.operation).map_err(|error| {
                match error {
                    RuntimeIdentifierError::UnknownIdentifier { value, .. } => {
                        RuntimeInvocationDecodingError::UnknownIdentifier {
                            field: "runtime.operation",
                            value,
                        }
                    }
                }
            })?;

        let execution_mode = RuntimeExecutionMode::from_identifier(&document.execution.mode)
            .map_err(|error| match error {
                RuntimeInvocationIdentifierError::UnknownIdentifier { value, .. } => {
                    RuntimeInvocationDecodingError::UnknownIdentifier {
                        field: "execution.mode",
                        value,
                    }
                }
            })?;

        let supervision_mode = RuntimeSupervisionMode::from_identifier(
            &document.execution.supervision,
        )
        .map_err(|error| match error {
            RuntimeInvocationIdentifierError::UnknownIdentifier { value, .. } => {
                RuntimeInvocationDecodingError::UnknownIdentifier {
                    field: "execution.supervision",
                    value,
                }
            }
        })?;

        let source_name = Box::leak(document.runtime.source.into_boxed_str());
        let runtime = RuntimeIdentity::from_parts(
            source_name,
            protocol,
            operation,
            document.runtime.source_contract_version,
        );

        Self::new(project, runtime, session, execution_mode, supervision_mode)
            .map_err(RuntimeInvocationDecodingError::InvalidConstruction)
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

fn reject_duplicate_object_keys(input: &str) -> Result<(), RuntimeInvocationDecodingError> {
    let mut parser = JsonKeyDetector::new(input);
    parser.parse_value()?;
    parser.finish()
}

struct JsonKeyDetector<'a> {
    input: &'a [u8],
    index: usize,
}

impl<'a> JsonKeyDetector<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            index: 0,
        }
    }

    fn parse_value(&mut self) -> Result<(), RuntimeInvocationDecodingError> {
        self.skip_whitespace();
        if self.index >= self.input.len() {
            return Err(RuntimeInvocationDecodingError::StructuralDocument(
                "empty runtime invocation document".to_string(),
            ));
        }

        match self.input[self.index] {
            b'{' => self.parse_object(),
            b'[' => self.parse_array(),
            b'"' => {
                self.parse_string()?;
                Ok(())
            }
            b'-' | b'0'..=b'9' => {
                self.parse_number();
                Ok(())
            }
            b't' => self.parse_literal("true"),
            b'f' => self.parse_literal("false"),
            b'n' => self.parse_literal("null"),
            other => Err(RuntimeInvocationDecodingError::StructuralDocument(format!(
                "unexpected JSON token: {}",
                other as char
            ))),
        }
    }

    fn parse_array(&mut self) -> Result<(), RuntimeInvocationDecodingError> {
        self.consume(b'[')?;
        self.skip_whitespace();
        if self.consume_if(b']') {
            return Ok(());
        }

        loop {
            self.parse_value()?;
            self.skip_whitespace();
            if self.consume_if(b',') {
                self.skip_whitespace();
                if self.consume_if(b']') {
                    return Err(RuntimeInvocationDecodingError::StructuralDocument(
                        "trailing comma in JSON array".to_string(),
                    ));
                }
                continue;
            }
            if self.consume_if(b']') {
                return Ok(());
            }
            return Err(RuntimeInvocationDecodingError::StructuralDocument(
                "missing JSON array terminator".to_string(),
            ));
        }
    }

    fn parse_object(&mut self) -> Result<(), RuntimeInvocationDecodingError> {
        self.consume(b'{')?;
        self.skip_whitespace();
        if self.consume_if(b'}') {
            return Ok(());
        }

        let mut seen = HashSet::new();
        loop {
            let key = self.parse_string()?;
            if !seen.insert(key.clone()) {
                return Err(RuntimeInvocationDecodingError::StructuralDocument(format!(
                    "duplicate field: {key}"
                )));
            }
            self.skip_whitespace();
            self.consume(b':')?;
            self.parse_value()?;
            self.skip_whitespace();
            if self.consume_if(b',') {
                self.skip_whitespace();
                if self.consume_if(b'}') {
                    return Err(RuntimeInvocationDecodingError::StructuralDocument(
                        "trailing comma in JSON object".to_string(),
                    ));
                }
                continue;
            }
            if self.consume_if(b'}') {
                return Ok(());
            }
            return Err(RuntimeInvocationDecodingError::StructuralDocument(
                "missing JSON object terminator".to_string(),
            ));
        }
    }

    fn parse_string(&mut self) -> Result<String, RuntimeInvocationDecodingError> {
        self.consume(b'"')?;
        let mut value = String::new();
        while let Some(byte) = self.peek() {
            match byte {
                b'"' => {
                    self.index += 1;
                    return Ok(value);
                }
                b'\\' => {
                    self.index += 1;
                    let escaped = self.peek().ok_or_else(|| {
                        RuntimeInvocationDecodingError::StructuralDocument(
                            "unterminated escape sequence in JSON string".to_string(),
                        )
                    })?;
                    match escaped {
                        b'"' | b'\\' | b'/' => {
                            value.push(escaped as char);
                            self.index += 1;
                        }
                        b'b' => {
                            value.push('\u{0008}');
                            self.index += 1;
                        }
                        b'f' => {
                            value.push('\u{000C}');
                            self.index += 1;
                        }
                        b'n' => {
                            value.push('\n');
                            self.index += 1;
                        }
                        b'r' => {
                            value.push('\r');
                            self.index += 1;
                        }
                        b't' => {
                            value.push('\t');
                            self.index += 1;
                        }
                        b'u' => {
                            self.index += 1;
                            let mut code = 0u32;
                            for _ in 0..4 {
                                let digit = self.peek().ok_or_else(|| {
                                    RuntimeInvocationDecodingError::StructuralDocument(
                                        "unterminated unicode escape in JSON string".to_string(),
                                    )
                                })?;
                                let value_digit = match digit {
                                    b'0'..=b'9' => (digit - b'0') as u32,
                                    b'a'..=b'f' => (digit - b'a' + 10) as u32,
                                    b'A'..=b'F' => (digit - b'A' + 10) as u32,
                                    _ => {
                                        return Err(
                                            RuntimeInvocationDecodingError::StructuralDocument(
                                                "invalid unicode escape in JSON string".to_string(),
                                            ),
                                        );
                                    }
                                };
                                code = (code << 4) | value_digit;
                                self.index += 1;
                            }
                            value.push(char::from_u32(code).ok_or_else(|| {
                                RuntimeInvocationDecodingError::StructuralDocument(
                                    "invalid Unicode codepoint in JSON string".to_string(),
                                )
                            })?);
                        }
                        _ => {
                            return Err(RuntimeInvocationDecodingError::StructuralDocument(
                                "unsupported escape sequence in JSON string".to_string(),
                            ));
                        }
                    }
                }
                other if other < 0x20 => {
                    return Err(RuntimeInvocationDecodingError::StructuralDocument(
                        "unescaped control character in JSON string".to_string(),
                    ));
                }
                other => {
                    value.push(other as char);
                    self.index += 1;
                }
            }
        }

        Err(RuntimeInvocationDecodingError::StructuralDocument(
            "unterminated JSON string".to_string(),
        ))
    }

    fn parse_number(&mut self) {
        while let Some(byte) = self.peek() {
            match byte {
                b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E' => {
                    self.index += 1;
                }
                _ => break,
            }
        }
    }

    fn parse_literal(&mut self, expected: &str) -> Result<(), RuntimeInvocationDecodingError> {
        let end = self.index + expected.len();
        if end > self.input.len() || &self.input[self.index..end] != expected.as_bytes() {
            return Err(RuntimeInvocationDecodingError::StructuralDocument(format!(
                "invalid JSON literal: {expected}"
            )));
        }
        self.index = end;
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        while let Some(byte) = self.peek() {
            if byte.is_ascii_whitespace() {
                self.index += 1;
            } else {
                break;
            }
        }
    }

    fn finish(mut self) -> Result<(), RuntimeInvocationDecodingError> {
        self.skip_whitespace();
        if self.index != self.input.len() {
            return Err(RuntimeInvocationDecodingError::StructuralDocument(
                "unexpected trailing JSON content".to_string(),
            ));
        }
        Ok(())
    }

    fn consume(&mut self, expected: u8) -> Result<(), RuntimeInvocationDecodingError> {
        self.skip_whitespace();
        if self.peek() == Some(expected) {
            self.index += 1;
            Ok(())
        } else {
            Err(RuntimeInvocationDecodingError::StructuralDocument(format!(
                "expected JSON token {}",
                expected as char
            )))
        }
    }

    fn consume_if(&mut self, expected: u8) -> bool {
        self.skip_whitespace();
        if self.peek() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.index).copied()
    }
}

fn validate_safe_component(
    value: &str,
    field: &'static str,
) -> Result<(), RuntimeInvocationValueError> {
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
        RuntimeInvocationDecodingError, RuntimeInvocationEnvelopeV1,
        RuntimeInvocationIdentifierError, RuntimeSupervisionMode, SessionInvocationIdentity,
    };
    use crate::runtime::RuntimeIdentity;

    #[test]
    fn runtime_execution_mode_identifiers_are_stable() {
        assert_eq!(RuntimeExecutionMode::Run.identifier(), "run");
        assert_eq!(RuntimeExecutionMode::Resume.identifier(), "resume");
        assert_eq!(
            RuntimeExecutionMode::from_identifier("run"),
            Ok(RuntimeExecutionMode::Run)
        );
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
        assert_eq!(
            RuntimeSupervisionMode::Foreground.identifier(),
            "foreground"
        );
        assert_eq!(
            RuntimeSupervisionMode::Background.identifier(),
            "background"
        );
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
            ProjectInvocationIdentity::new("example-project")
                .unwrap()
                .name(),
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
        assert_eq!(
            SessionInvocationIdentity::new("session-1").unwrap().id(),
            "session-1"
        );
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
        assert_eq!(
            identity.runtime(),
            RuntimeIdentity::http_acquisition("example-source", 1)
        );
        assert_eq!(identity.session().id(), "session-1");
        assert_eq!(identity.execution_mode(), RuntimeExecutionMode::Run);
        assert_eq!(
            identity.supervision_mode(),
            RuntimeSupervisionMode::Foreground
        );

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
        assert_eq!(
            processing.runtime().operation(),
            crate::runtime::RuntimeOperation::Processing
        );
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

    #[test]
    fn runtime_invocation_json_contract_serializes_expected_fields() {
        let acquire_run = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("telugu-lexicon").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-000001").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();

        let acquire_run_json = acquire_run.to_json().unwrap();
        assert_eq!(
            acquire_run_json,
            "{\"schema_version\":1,\"project\":{\"name\":\"telugu-lexicon\"},\"runtime\":{\"source\":\"example-source\",\"protocol\":\"http\",\"operation\":\"acquisition\",\"source_contract_version\":1},\"session\":{\"id\":\"session-000001\"},\"execution\":{\"mode\":\"run\",\"supervision\":\"foreground\"}}"
        );
        assert_eq!(
            acquire_run_json.parse::<serde_json::Value>().unwrap()["schema_version"],
            1
        );
        assert!(!acquire_run_json.ends_with('\n'));
        assert!(!acquire_run_json.contains("args"));
        assert!(!acquire_run_json.contains("arguments"));
        assert!(!acquire_run_json.contains("source_args"));
        assert!(!acquire_run_json.contains("command_line"));
        assert!(!acquire_run_json.contains("/workspace"));
        assert!(!acquire_run_json.contains("/tmp"));

        let acquire_resume = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("telugu-lexicon").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-000002").unwrap(),
            RuntimeExecutionMode::Resume,
            RuntimeSupervisionMode::Background,
        )
        .unwrap();
        let acquire_resume_json = acquire_resume.to_json().unwrap();
        assert!(acquire_resume_json.contains("\"operation\":\"acquisition\""));
        assert!(acquire_resume_json.contains("\"mode\":\"resume\""));
        assert!(acquire_resume_json.contains("\"supervision\":\"background\""));

        let processing_run = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("telugu-lexicon").unwrap(),
            RuntimeIdentity::http_processing("example-source", 1),
            SessionInvocationIdentity::new("session-000003").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        let processing_run_json = processing_run.to_json().unwrap();
        assert!(processing_run_json.contains("\"operation\":\"processing\""));
        assert!(processing_run_json.contains("\"mode\":\"run\""));
    }

    #[test]
    fn runtime_invocation_json_round_trips_preserve_envelope_identity() {
        let acquire_run = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("telugu-lexicon").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-000001").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        let parsed =
            RuntimeInvocationEnvelopeV1::from_json(&acquire_run.to_json().unwrap()).unwrap();
        assert_eq!(parsed, acquire_run);

        let acquire_resume = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("telugu-lexicon").unwrap(),
            RuntimeIdentity::http_acquisition("example-source", 1),
            SessionInvocationIdentity::new("session-000002").unwrap(),
            RuntimeExecutionMode::Resume,
            RuntimeSupervisionMode::Background,
        )
        .unwrap();
        let parsed_resume =
            RuntimeInvocationEnvelopeV1::from_json(&acquire_resume.to_json().unwrap()).unwrap();
        assert_eq!(parsed_resume, acquire_resume);

        let processing_run = RuntimeInvocationEnvelopeV1::new(
            ProjectInvocationIdentity::new("telugu-lexicon").unwrap(),
            RuntimeIdentity::http_processing("example-source", 1),
            SessionInvocationIdentity::new("session-000003").unwrap(),
            RuntimeExecutionMode::Run,
            RuntimeSupervisionMode::Foreground,
        )
        .unwrap();
        let parsed_processing =
            RuntimeInvocationEnvelopeV1::from_json(&processing_run.to_json().unwrap()).unwrap();
        assert_eq!(parsed_processing, processing_run);
    }

    #[test]
    fn runtime_invocation_json_rejects_invalid_inputs() {
        let bad_json = "{not valid json}";
        assert!(matches!(
            RuntimeInvocationEnvelopeV1::from_json(bad_json),
            Err(RuntimeInvocationDecodingError::JsonSyntax(_))
        ));

        let duplicate = r#"{"schema_version":1,"schema_version":2,"project":{"name":"telugu-lexicon"},"runtime":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"session":{"id":"session-000001"},"execution":{"mode":"run","supervision":"foreground"}}"#;
        assert!(matches!(
            RuntimeInvocationEnvelopeV1::from_json(duplicate),
            Err(RuntimeInvocationDecodingError::StructuralDocument(_))
        ));

        let unknown_top_level = r#"{"schema_version":1,"project":{"name":"telugu-lexicon"},"runtime":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"session":{"id":"session-000001"},"execution":{"mode":"run","supervision":"foreground"},"extra":"value"}"#;
        assert!(matches!(
            RuntimeInvocationEnvelopeV1::from_json(unknown_top_level),
            Err(RuntimeInvocationDecodingError::StructuralDocument(_))
        ));

        let unknown_nested = r#"{"schema_version":1,"project":{"name":"telugu-lexicon","path":"/tmp"},"runtime":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"session":{"id":"session-000001"},"execution":{"mode":"run","supervision":"foreground"}}"#;
        assert!(matches!(
            RuntimeInvocationEnvelopeV1::from_json(unknown_nested),
            Err(RuntimeInvocationDecodingError::StructuralDocument(_))
        ));

        let missing_field = r#"{"schema_version":1,"project":{"name":"telugu-lexicon"},"runtime":{"source":"example-source","protocol":"http","operation":"acquisition"},"session":{"id":"session-000001"},"execution":{"mode":"run","supervision":"foreground"}}"#;
        assert!(matches!(
            RuntimeInvocationEnvelopeV1::from_json(missing_field),
            Err(RuntimeInvocationDecodingError::StructuralDocument(_))
        ));

        let unknown_schema = r#"{"schema_version":2,"project":{"name":"telugu-lexicon"},"runtime":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"session":{"id":"session-000001"},"execution":{"mode":"run","supervision":"foreground"}}"#;
        assert!(matches!(
            RuntimeInvocationEnvelopeV1::from_json(unknown_schema),
            Err(RuntimeInvocationDecodingError::UnknownSchemaVersion(2))
        ));

        let invalid_project = r#"{"schema_version":1,"project":{"name":"/bad"},"runtime":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"session":{"id":"session-000001"},"execution":{"mode":"run","supervision":"foreground"}}"#;
        assert!(matches!(
            RuntimeInvocationEnvelopeV1::from_json(invalid_project),
            Err(RuntimeInvocationDecodingError::InvalidProjectIdentity(_))
        ));

        let invalid_version = r#"{"schema_version":1,"project":{"name":"telugu-lexicon"},"runtime":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":0},"session":{"id":"session-000001"},"execution":{"mode":"run","supervision":"foreground"}}"#;
        assert!(matches!(
            RuntimeInvocationEnvelopeV1::from_json(invalid_version),
            Err(RuntimeInvocationDecodingError::InvalidVersion { .. })
        ));
    }
}
