use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::protocols::http::{HttpCapability, HttpCapabilitySet, HttpSourceContractV1};
use crate::runtime::{RuntimeIdentifierError, RuntimeIdentity, RuntimeOperation, RuntimeProtocol};

pub const RUNTIME_INFORMATION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInformationEncodingError {
    Serialization(String),
}

impl fmt::Display for RuntimeInformationEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(message) => write!(
                formatter,
                "runtime information serialization error: {message}"
            ),
        }
    }
}

impl std::error::Error for RuntimeInformationEncodingError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInformationDecodingError {
    JsonSyntax(String),
    UnknownSchemaVersion(u32),
    UnknownIdentifier { field: &'static str, value: String },
    DuplicateCapability(String),
    InvalidVersion { field: &'static str, value: u32 },
    StructuralDocument(String),
}

impl fmt::Display for RuntimeInformationDecodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsonSyntax(message) => write!(formatter, "invalid JSON: {message}"),
            Self::UnknownSchemaVersion(version) => {
                write!(formatter, "unknown runtime schema version: {version}")
            }
            Self::UnknownIdentifier { field, value } => {
                write!(formatter, "unknown {field} identifier: {value}")
            }
            Self::DuplicateCapability(value) => {
                write!(formatter, "duplicate capability identifier: {value}")
            }
            Self::InvalidVersion { field, value } => {
                write!(formatter, "invalid {field} value: {value}")
            }
            Self::StructuralDocument(message) => {
                write!(formatter, "malformed runtime document: {message}")
            }
        }
    }
}

impl std::error::Error for RuntimeInformationDecodingError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeInformationDocumentV1 {
    schema_version: u32,
    identity: RuntimeIdentityDocumentV1,
    descriptor: RuntimeDescriptorDocumentV1,
    runtime: RuntimeDocumentV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIdentityDocumentV1 {
    source: String,
    protocol: String,
    operation: String,
    source_contract_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeDescriptorDocumentV1 {
    contract_version: u32,
    required_capabilities: Vec<String>,
    resume_handler_registered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeDocumentV1 {
    available_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeInformationV1 {
    identity: RuntimeIdentity,
    descriptor_contract_version: u32,
    required_capabilities: HttpCapabilitySet,
    available_capabilities: HttpCapabilitySet,
    resume_handler_registered: bool,
}

impl RuntimeInformationV1 {
    pub const fn from_http_source(
        identity: RuntimeIdentity,
        source: &HttpSourceContractV1,
        available_capabilities: HttpCapabilitySet,
    ) -> Self {
        Self {
            identity,
            descriptor_contract_version: HttpSourceContractV1::CONTRACT_VERSION,
            required_capabilities: source.required_capabilities(),
            available_capabilities,
            resume_handler_registered: source.resume_handler().is_some(),
        }
    }

    pub const fn identity(&self) -> RuntimeIdentity {
        self.identity
    }

    pub const fn descriptor_contract_version(&self) -> u32 {
        self.descriptor_contract_version
    }

    pub const fn required_capabilities(&self) -> HttpCapabilitySet {
        self.required_capabilities
    }

    pub const fn available_capabilities(&self) -> HttpCapabilitySet {
        self.available_capabilities
    }

    pub const fn validate_capabilities(&self) -> Result<(), MissingHttpCapabilities> {
        if self.required_capabilities.is_subset_of(self.available_capabilities) {
            Ok(())
        } else {
            Err(MissingHttpCapabilities {
                missing: self.required_capabilities.missing_from(self.available_capabilities),
            })
        }
    }

    pub const fn resume_handler_registered(&self) -> bool {
        self.resume_handler_registered
    }

    pub fn to_json(&self) -> Result<String, RuntimeInformationEncodingError> {
        let document = RuntimeInformationDocumentV1 {
            schema_version: RUNTIME_INFORMATION_SCHEMA_VERSION,
            identity: RuntimeIdentityDocumentV1 {
                source: self.identity.source_name().to_string(),
                protocol: self.identity.protocol().identifier().to_string(),
                operation: self.identity.operation().identifier().to_string(),
                source_contract_version: self.identity.source_contract_version(),
            },
            descriptor: RuntimeDescriptorDocumentV1 {
                contract_version: self.descriptor_contract_version,
                required_capabilities: self
                    .required_capabilities
                    .ordered_capabilities()
                    .into_iter()
                    .map(|capability| capability.identifier().to_string())
                    .collect(),
                resume_handler_registered: self.resume_handler_registered,
            },
            runtime: RuntimeDocumentV1 {
                available_capabilities: self
                    .available_capabilities
                    .ordered_capabilities()
                    .into_iter()
                    .map(|capability| capability.identifier().to_string())
                    .collect(),
            },
        };

        serde_json::to_string(&document)
            .map_err(|error| RuntimeInformationEncodingError::Serialization(error.to_string()))
    }

    pub fn from_json(input: &str) -> Result<Self, RuntimeInformationDecodingError> {
        let document = match serde_json::from_str::<RuntimeInformationDocumentV1>(input) {
            Ok(document) => document,
            Err(error) => {
                return match error.classify() {
                    serde_json::error::Category::Syntax => Err(
                        RuntimeInformationDecodingError::JsonSyntax(error.to_string()),
                    ),
                    _ => Err(RuntimeInformationDecodingError::StructuralDocument(
                        error.to_string(),
                    )),
                };
            }
        };

        if document.schema_version != RUNTIME_INFORMATION_SCHEMA_VERSION {
            return Err(RuntimeInformationDecodingError::UnknownSchemaVersion(
                document.schema_version,
            ));
        }

        if document.identity.source_contract_version == 0 {
            return Err(RuntimeInformationDecodingError::InvalidVersion {
                field: "identity.source_contract_version",
                value: document.identity.source_contract_version,
            });
        }

        if document.descriptor.contract_version == 0 {
            return Err(RuntimeInformationDecodingError::InvalidVersion {
                field: "descriptor.contract_version",
                value: document.descriptor.contract_version,
            });
        }

        let protocol =
            RuntimeProtocol::from_identifier(&document.identity.protocol).map_err(|error| {
                match error {
                    RuntimeIdentifierError::UnknownIdentifier { value, .. } => {
                        RuntimeInformationDecodingError::UnknownIdentifier {
                            field: "identity.protocol",
                            value,
                        }
                    }
                }
            })?;

        let operation =
            RuntimeOperation::from_identifier(&document.identity.operation).map_err(|error| {
                match error {
                    RuntimeIdentifierError::UnknownIdentifier { value, .. } => {
                        RuntimeInformationDecodingError::UnknownIdentifier {
                            field: "identity.operation",
                            value,
                        }
                    }
                }
            })?;

        let mut seen_capabilities = BTreeSet::new();
        let mut required_capabilities = HttpCapabilitySet::empty();

        for raw_capability in &document.descriptor.required_capabilities {
            let capability =
                HttpCapability::from_identifier(raw_capability).map_err(|error| match error {
                    RuntimeIdentifierError::UnknownIdentifier { value, .. } => {
                        RuntimeInformationDecodingError::UnknownIdentifier {
                            field: "descriptor.required_capabilities",
                            value,
                        }
                    }
                })?;

            let identifier = capability.identifier().to_string();
            if !seen_capabilities.insert(identifier.clone()) {
                return Err(RuntimeInformationDecodingError::DuplicateCapability(
                    identifier,
                ));
            }

            required_capabilities = required_capabilities.insert(capability);
        }

        let mut seen_runtime_capabilities = BTreeSet::new();
        let mut available_capabilities = HttpCapabilitySet::empty();

        for raw_capability in &document.runtime.available_capabilities {
            let capability =
                HttpCapability::from_identifier(raw_capability).map_err(|error| match error {
                    RuntimeIdentifierError::UnknownIdentifier { value, .. } => {
                        RuntimeInformationDecodingError::UnknownIdentifier {
                            field: "runtime.available_capabilities",
                            value,
                        }
                    }
                })?;

            let identifier = capability.identifier().to_string();
            if !seen_runtime_capabilities.insert(identifier.clone()) {
                return Err(RuntimeInformationDecodingError::DuplicateCapability(
                    identifier,
                ));
            }

            available_capabilities = available_capabilities.insert(capability);
        }

        let source_name = Box::leak(document.identity.source.into_boxed_str());
        let identity = RuntimeIdentity::from_parts(
            source_name,
            protocol,
            operation,
            document.identity.source_contract_version,
        );

        Ok(Self {
            identity,
            descriptor_contract_version: document.descriptor.contract_version,
            required_capabilities,
            available_capabilities,
            resume_handler_registered: document.descriptor.resume_handler_registered,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingHttpCapabilities {
    missing: HttpCapabilitySet,
}

impl MissingHttpCapabilities {
    pub const fn missing(&self) -> HttpCapabilitySet {
        self.missing
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeInformationDecodingError, RuntimeInformationV1};
    use crate::http::HttpCapability;
    use crate::protocols::http::{HttpCapabilitySet, HttpSourceContractV1};
    use crate::runtime::RuntimeIdentity;

    fn acquire_handler(
        _context: &mut crate::HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> crate::protocols::http::AcquisitionResult<()> {
        Ok(())
    }

    fn resume_handler(
        _context: &mut crate::HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> crate::protocols::http::AcquisitionResult<()> {
        Ok(())
    }

    fn failing_acquire(
        _context: &mut crate::HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> crate::protocols::http::AcquisitionResult<()> {
        panic!("acquire should not be invoked while building runtime metadata");
    }

    fn failing_resume(
        _context: &mut crate::HttpAcquisitionContext,
        _args: &[std::ffi::OsString],
    ) -> crate::protocols::http::AcquisitionResult<()> {
        panic!("resume should not be invoked while building runtime metadata");
    }

    #[test]
    fn runtime_information_can_be_constructed_in_const() {
        const IDENTITY: RuntimeIdentity = RuntimeIdentity::http_acquisition("example-source", 1);
        const SOURCE: HttpSourceContractV1 = HttpSourceContractV1::new(acquire_handler);
        const AVAILABLE: HttpCapabilitySet = HttpCapabilitySet::empty();
        const INFO: RuntimeInformationV1 =
            RuntimeInformationV1::from_http_source(IDENTITY, &SOURCE, AVAILABLE);

        assert_eq!(INFO.identity(), IDENTITY);
        assert_eq!(
            INFO.descriptor_contract_version(),
            HttpSourceContractV1::CONTRACT_VERSION
        );
        assert_eq!(INFO.required_capabilities(), HttpCapabilitySet::empty());
        assert_eq!(INFO.available_capabilities(), HttpCapabilitySet::empty());
        assert!(!INFO.resume_handler_registered());
    }

    #[test]
    fn runtime_information_minimal_document_serializes() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let info = RuntimeInformationV1::from_http_source(
            identity,
            &source,
            HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1),
        );

        let json = info.to_json().unwrap();
        assert!(json.contains("\"schema_version\":1"));
        assert!(json.contains("\"source\":\"example-source\""));
        assert!(json.contains("\"protocol\":\"http\""));
        assert!(json.contains("\"operation\":\"acquisition\""));
        assert!(json.contains("\"source_contract_version\":1"));
        assert!(json.contains("\"contract_version\":1"));
        assert!(json.contains("\"required_capabilities\":[\"client-certificate-v1\"]"));
        assert!(json.contains("\"available_capabilities\":[\"client-certificate-v1\"]"));
        assert!(json.contains("\"resume_handler_registered\":false"));
    }

    #[test]
    fn capability_ordering_is_deterministic() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let info = RuntimeInformationV1::from_http_source(
            identity,
            &source,
            HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1),
        );

        let json = info.to_json().unwrap();
        let expected = "\"required_capabilities\":[\"client-certificate-v1\"]";
        assert!(json.contains(expected));
    }

    #[test]
    fn serialization_does_not_invoke_acquire_or_resume_handlers() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(failing_acquire).with_resume(failing_resume);
        let info = RuntimeInformationV1::from_http_source(identity, &source, HttpCapabilitySet::empty());

        let json = info.to_json().unwrap();
        assert!(json.contains("\"resume_handler_registered\":true"));
        assert!(json.contains("\"available_capabilities\":[]"));
        assert!(!json.contains("0x"));
    }

    #[test]
    fn runtime_information_round_trip_preserves_equality() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(acquire_handler)
            .with_resume(resume_handler)
            .requires(HttpCapability::ClientCertificateV1);
        let info = RuntimeInformationV1::from_http_source(
            identity,
            &source,
            HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1),
        );

        let json = info.to_json().unwrap();
        let parsed = RuntimeInformationV1::from_json(&json).unwrap();

        assert_eq!(parsed, info);
        assert_eq!(parsed.identity().source_contract_version(), 1);
        assert_eq!(parsed.descriptor_contract_version(), 1);
    }

    #[test]
    fn invalid_json_is_rejected() {
        assert!(matches!(
            RuntimeInformationV1::from_json("{not json}"),
            Err(RuntimeInformationDecodingError::JsonSyntax(_))
        ));
    }

    #[test]
    fn missing_required_fields_are_rejected() {
        assert!(matches!(
            RuntimeInformationV1::from_json("{\"schema_version\":1,\"identity\":{}}"),
            Err(RuntimeInformationDecodingError::StructuralDocument(_))
        ));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let input = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1,"extra":true},"descriptor":{"contract_version":1,"required_capabilities":["client-certificate-v1"],"resume_handler_registered":false},"runtime":{"available_capabilities":[]}}"#;
        assert!(matches!(
            RuntimeInformationV1::from_json(input),
            Err(RuntimeInformationDecodingError::StructuralDocument(_))
        ));
    }

    #[test]
    fn unknown_schema_versions_are_rejected() {
        let input = r#"{"schema_version":2,"identity":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"descriptor":{"contract_version":1,"required_capabilities":[],"resume_handler_registered":false},"runtime":{"available_capabilities":[]}}"#;
        assert!(matches!(
            RuntimeInformationV1::from_json(input),
            Err(RuntimeInformationDecodingError::UnknownSchemaVersion(2))
        ));
    }

    #[test]
    fn unknown_protocols_are_rejected() {
        let input = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"https","operation":"acquisition","source_contract_version":1},"descriptor":{"contract_version":1,"required_capabilities":[],"resume_handler_registered":false},"runtime":{"available_capabilities":[]}}"#;
        assert!(matches!(
            RuntimeInformationV1::from_json(input),
            Err(RuntimeInformationDecodingError::UnknownIdentifier {
                field: "identity.protocol",
                ..
            })
        ));
    }

    #[test]
    fn unknown_operations_are_rejected() {
        let input = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"resume","source_contract_version":1},"descriptor":{"contract_version":1,"required_capabilities":[],"resume_handler_registered":false},"runtime":{"available_capabilities":[]}}"#;
        assert!(matches!(
            RuntimeInformationV1::from_json(input),
            Err(RuntimeInformationDecodingError::UnknownIdentifier {
                field: "identity.operation",
                ..
            })
        ));
    }

    #[test]
    fn unknown_capabilities_are_rejected() {
        let input = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"descriptor":{"contract_version":1,"required_capabilities":["client-certificate-v2"],"resume_handler_registered":false},"runtime":{"available_capabilities":[]}}"#;
        assert!(matches!(
            RuntimeInformationV1::from_json(input),
            Err(RuntimeInformationDecodingError::UnknownIdentifier {
                field: "descriptor.required_capabilities",
                ..
            })
        ));
    }

    #[test]
    fn duplicate_capabilities_are_rejected() {
        let input = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"descriptor":{"contract_version":1,"required_capabilities":["client-certificate-v1","client-certificate-v1"],"resume_handler_registered":false},"runtime":{"available_capabilities":[]}}"#;
        assert!(matches!(
            RuntimeInformationV1::from_json(input),
            Err(RuntimeInformationDecodingError::DuplicateCapability(_))
        ));
    }

    #[test]
    fn runtime_capability_duplicates_are_rejected() {
        let input = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"descriptor":{"contract_version":1,"required_capabilities":[],"resume_handler_registered":false},"runtime":{"available_capabilities":["client-certificate-v1","client-certificate-v1"]}}"#;
        assert!(matches!(
            RuntimeInformationV1::from_json(input),
            Err(RuntimeInformationDecodingError::DuplicateCapability(_))
        ));
    }

    #[test]
    fn zero_contract_versions_are_rejected() {
        let zero_source = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":0},"descriptor":{"contract_version":1,"required_capabilities":[],"resume_handler_registered":false},"runtime":{"available_capabilities":[]}}"#;
        let zero_descriptor = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"descriptor":{"contract_version":0,"required_capabilities":[],"resume_handler_registered":false},"runtime":{"available_capabilities":[]}}"#;

        assert!(matches!(
            RuntimeInformationV1::from_json(zero_source),
            Err(RuntimeInformationDecodingError::InvalidVersion {
                field: "identity.source_contract_version",
                ..
            })
        ));
        assert!(matches!(
            RuntimeInformationV1::from_json(zero_descriptor),
            Err(RuntimeInformationDecodingError::InvalidVersion {
                field: "descriptor.contract_version",
                ..
            })
        ));
    }

    #[test]
    fn incompatible_document_decodes_but_fails_validation() {
        let input = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"descriptor":{"contract_version":1,"required_capabilities":["client-certificate-v1"],"resume_handler_registered":false},"runtime":{"available_capabilities":[]}}"#;
        let info = RuntimeInformationV1::from_json(input).unwrap();
        assert!(info.validate_capabilities().is_err());
        let error = info.validate_capabilities().unwrap_err();
        assert!(error.missing().contains(HttpCapability::ClientCertificateV1));
    }

    #[test]
    fn different_identity_and_descriptor_versions_survive_round_trip() {
        let info = RuntimeInformationV1::from_http_source(
            RuntimeIdentity::http_acquisition("example-source", 2),
            &HttpSourceContractV1::new(acquire_handler),
            HttpCapabilitySet::empty(),
        );

        let round_trip = RuntimeInformationV1::from_json(&info.to_json().unwrap()).unwrap();
        assert_eq!(round_trip.identity().source_contract_version(), 2);
        assert_eq!(
            round_trip.descriptor_contract_version(),
            HttpSourceContractV1::CONTRACT_VERSION
        );
    }

    #[test]
    fn source_and_descriptor_versions_do_not_need_to_match() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 2);
        let source = HttpSourceContractV1::new(acquire_handler);
        let info = RuntimeInformationV1::from_http_source(identity, &source, HttpCapabilitySet::empty());

        assert_eq!(info.identity().source_contract_version(), 2);
        assert_eq!(
            info.descriptor_contract_version(),
            HttpSourceContractV1::CONTRACT_VERSION
        );
    }

    #[test]
    fn capability_validation_does_not_invoke_acquire() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);
        let source = HttpSourceContractV1::new(failing_acquire)
            .with_resume(failing_resume)
            .requires(HttpCapability::ClientCertificateV1);
        let info = RuntimeInformationV1::from_http_source(identity, &source, HttpCapabilitySet::empty());

        assert!(info.validate_capabilities().is_err());
    }
}
