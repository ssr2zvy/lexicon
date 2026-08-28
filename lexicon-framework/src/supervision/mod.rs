//! Internal operator-host invocation protocol.
//!
//! This is a small, versioned, internal protocol used only to re-execute the
//! current `lexicon` binary in the reserved `__operator-host` role for
//! background execution (contract.md section 3, "Background execution").
//! It is not a public framework API and must not be treated as a stable
//! external interface.
//!
//! It is deliberately distinct from `RuntimeInvocationEnvelopeV1` (the
//! source-runtime invocation contract), the session record schema, and the
//! source contract version, per the distinct-compatibility-surfaces
//! requirement in contract.md / specs.md section 16.
//!
//! Raw source arguments are never included in this reference. They continue
//! to travel only as the operator-host process's own trailing argv after
//! `--`, exactly as the ordinary `lexicon data ... -- <source-args>` path
//! already works. Everything else needed to relocate the exact session an
//! initiating process already prepared is either carried here or is
//! independently re-derivable by the operator host (for example, the project
//! is re-discovered from the operator host's own working directory, exactly
//! as the initiating process originally discovered it).

use std::fmt;

use lexicon_core::session::SessionIdentity;

use crate::data::request::DataOperation;

/// Schema version for the operator-host invocation reference.
///
/// Distinct from `RUNTIME_INVOCATION_PROTOCOL_VERSION`, the session schema
/// version, and the source contract version.
pub const OPERATOR_HOST_INVOCATION_SCHEMA_VERSION: u32 = 1;

/// A reference the operator host uses to relocate the exact session an
/// initiating process already prepared with `RuntimeSupervisionMode::Background`.
///
/// This is transported only as the operator-host process's own argv (via
/// [`OperatorHostInvocationV1::to_json`] encoded into a single argument); it
/// is not itself written to durable storage as a separate file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorHostInvocationV1 {
    source_name: String,
    operation: DataOperation,
    session: SessionIdentity,
}

impl OperatorHostInvocationV1 {
    pub fn new(source_name: impl Into<String>, operation: DataOperation, session: SessionIdentity) -> Self {
        Self {
            source_name: source_name.into(),
            operation,
            session,
        }
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn operation(&self) -> DataOperation {
        self.operation
    }

    pub fn session(&self) -> &SessionIdentity {
        &self.session
    }

    pub fn to_json(&self) -> Result<String, OperatorHostInvocationEncodingError> {
        let document = OperatorHostInvocationDocumentV1 {
            schema_version: OPERATOR_HOST_INVOCATION_SCHEMA_VERSION,
            source_name: self.source_name.clone(),
            operation: operation_identifier(self.operation).to_string(),
            session_id: self.session.id().to_string(),
        };

        serde_json::to_string(&document)
            .map_err(|error| OperatorHostInvocationEncodingError::Serialization(error.to_string()))
    }

    pub fn from_json(input: &str) -> Result<Self, OperatorHostInvocationDecodingError> {
        let document = serde_json::from_str::<OperatorHostInvocationDocumentV1>(input).map_err(
            |error| match error.classify() {
                serde_json::error::Category::Syntax => {
                    OperatorHostInvocationDecodingError::JsonSyntax(error.to_string())
                }
                _ => OperatorHostInvocationDecodingError::StructuralDocument(error.to_string()),
            },
        )?;

        if document.schema_version != OPERATOR_HOST_INVOCATION_SCHEMA_VERSION {
            return Err(OperatorHostInvocationDecodingError::UnknownSchemaVersion(
                document.schema_version,
            ));
        }

        let operation = operation_from_identifier(&document.operation).ok_or_else(|| {
            OperatorHostInvocationDecodingError::UnknownOperation(document.operation.clone())
        })?;

        let session = SessionIdentity::new(document.session_id.clone())
            .map_err(|error| OperatorHostInvocationDecodingError::InvalidSessionIdentity(error.to_string()))?;

        Ok(Self {
            source_name: document.source_name,
            operation,
            session,
        })
    }
}

fn operation_identifier(operation: DataOperation) -> &'static str {
    match operation {
        DataOperation::Acquisition => "acquisition",
        DataOperation::Processing => "processing",
    }
}

fn operation_from_identifier(value: &str) -> Option<DataOperation> {
    match value {
        "acquisition" => Some(DataOperation::Acquisition),
        "processing" => Some(DataOperation::Processing),
        _ => None,
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorHostInvocationDocumentV1 {
    schema_version: u32,
    source_name: String,
    operation: String,
    session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorHostInvocationEncodingError {
    Serialization(String),
}

impl fmt::Display for OperatorHostInvocationEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(message) => {
                write!(formatter, "operator-host invocation serialization error: {message}")
            }
        }
    }
}

impl std::error::Error for OperatorHostInvocationEncodingError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorHostInvocationDecodingError {
    JsonSyntax(String),
    StructuralDocument(String),
    UnknownSchemaVersion(u32),
    UnknownOperation(String),
    InvalidSessionIdentity(String),
}

impl fmt::Display for OperatorHostInvocationDecodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsonSyntax(message) => write!(formatter, "invalid JSON: {message}"),
            Self::StructuralDocument(message) => {
                write!(formatter, "malformed operator-host invocation document: {message}")
            }
            Self::UnknownSchemaVersion(version) => write!(
                formatter,
                "unknown operator-host invocation schema version: {version}"
            ),
            Self::UnknownOperation(value) => {
                write!(formatter, "unknown operator-host invocation operation: {value}")
            }
            Self::InvalidSessionIdentity(message) => {
                write!(formatter, "invalid operator-host invocation session identity: {message}")
            }
        }
    }
}

impl std::error::Error for OperatorHostInvocationDecodingError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let reference = OperatorHostInvocationV1::new(
            "example-source",
            DataOperation::Acquisition,
            SessionIdentity::new("session-abc").unwrap(),
        );

        let json = reference.to_json().unwrap();
        let decoded = OperatorHostInvocationV1::from_json(&json).unwrap();

        assert_eq!(decoded, reference);
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let json = r#"{"schema_version":999,"source_name":"s","operation":"acquisition","session_id":"abc"}"#;
        let result = OperatorHostInvocationV1::from_json(json);
        assert!(matches!(
            result,
            Err(OperatorHostInvocationDecodingError::UnknownSchemaVersion(999))
        ));
    }

    #[test]
    fn rejects_unknown_operation() {
        let json = r#"{"schema_version":1,"source_name":"s","operation":"bogus","session_id":"abc"}"#;
        let result = OperatorHostInvocationV1::from_json(json);
        assert!(matches!(
            result,
            Err(OperatorHostInvocationDecodingError::UnknownOperation(_))
        ));
    }

    #[test]
    fn processing_operation_round_trips() {
        let reference = OperatorHostInvocationV1::new(
            "example-source",
            DataOperation::Processing,
            SessionIdentity::new("session-xyz").unwrap(),
        );
        let json = reference.to_json().unwrap();
        let decoded = OperatorHostInvocationV1::from_json(&json).unwrap();
        assert_eq!(decoded.operation(), DataOperation::Processing);
    }
}
