use std::fmt;

use serde::{Deserialize, Serialize};

use super::ProcessingSourceContractV1;
use crate::runtime::{RuntimeIdentifierError, RuntimeIdentity, RuntimeOperation, RuntimeProtocol};

pub const PROCESSING_RUNTIME_INFORMATION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessingRuntimeInformationEncodingError {
    Serialization(String),
}

impl fmt::Display for ProcessingRuntimeInformationEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(message) => {
                write!(
                    formatter,
                    "processing runtime information serialization error: {message}"
                )
            }
        }
    }
}

impl std::error::Error for ProcessingRuntimeInformationEncodingError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessingRuntimeInformationDecodingError {
    JsonSyntax(String),
    UnknownSchemaVersion(u32),
    UnknownIdentifier { field: &'static str, value: String },
    WrongProtocol { actual: RuntimeProtocol },
    WrongOperation { actual: RuntimeOperation },
    InvalidVersion { field: &'static str, value: u32 },
    StructuralDocument(String),
}

impl fmt::Display for ProcessingRuntimeInformationDecodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsonSyntax(message) => write!(formatter, "invalid JSON: {message}"),
            Self::UnknownSchemaVersion(version) => {
                write!(
                    formatter,
                    "unknown processing runtime schema version: {version}"
                )
            }
            Self::UnknownIdentifier { field, value } => {
                write!(formatter, "unknown {field} identifier: {value}")
            }
            Self::WrongProtocol { actual } => {
                write!(
                    formatter,
                    "processing runtime information requires HTTP protocol, actual: {actual:?}"
                )
            }
            Self::WrongOperation { actual } => {
                write!(
                    formatter,
                    "processing runtime information requires processing operation, actual: {actual:?}"
                )
            }
            Self::InvalidVersion { field, value } => {
                write!(formatter, "invalid {field} value: {value}")
            }
            Self::StructuralDocument(message) => {
                write!(formatter, "malformed processing document: {message}")
            }
        }
    }
}

impl std::error::Error for ProcessingRuntimeInformationDecodingError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessingRuntimeInformationDocumentV1 {
    schema_version: u32,
    identity: ProcessingRuntimeIdentityDocumentV1,
    descriptor: ProcessingRuntimeDescriptorDocumentV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessingRuntimeIdentityDocumentV1 {
    source: String,
    protocol: String,
    operation: String,
    source_contract_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessingRuntimeDescriptorDocumentV1 {
    contract_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessingRuntimeInformationV1 {
    identity: RuntimeIdentity,
    descriptor_contract_version: u32,
}

impl ProcessingRuntimeInformationV1 {
    fn from_parts_unchecked(identity: RuntimeIdentity, descriptor_contract_version: u32) -> Self {
        Self {
            identity,
            descriptor_contract_version,
        }
    }

    pub const fn identity(&self) -> RuntimeIdentity {
        self.identity
    }

    pub const fn descriptor_contract_version(&self) -> u32 {
        self.descriptor_contract_version
    }

    pub fn from_processing_source(
        identity: RuntimeIdentity,
        source: &ProcessingSourceContractV1,
    ) -> Result<Self, ProcessingRuntimeInformationConstructionError> {
        let _ = source;

        if identity.protocol() != RuntimeProtocol::Http {
            return Err(
                ProcessingRuntimeInformationConstructionError::WrongProtocol {
                    actual: identity.protocol(),
                },
            );
        }

        if identity.operation() != RuntimeOperation::Processing {
            return Err(
                ProcessingRuntimeInformationConstructionError::WrongOperation {
                    actual: identity.operation(),
                },
            );
        }

        if identity.source_contract_version() != ProcessingSourceContractV1::CONTRACT_VERSION {
            return Err(
                ProcessingRuntimeInformationConstructionError::IdentityContractVersionMismatch {
                    identity_version: identity.source_contract_version(),
                    descriptor_version: ProcessingSourceContractV1::CONTRACT_VERSION,
                },
            );
        }

        Ok(Self {
            identity,
            descriptor_contract_version: ProcessingSourceContractV1::CONTRACT_VERSION,
        })
    }

    pub fn to_json(&self) -> Result<String, ProcessingRuntimeInformationEncodingError> {
        let document = ProcessingRuntimeInformationDocumentV1 {
            schema_version: PROCESSING_RUNTIME_INFORMATION_SCHEMA_VERSION,
            identity: ProcessingRuntimeIdentityDocumentV1 {
                source: self.identity.source_name().to_string(),
                protocol: self.identity.protocol().identifier().to_string(),
                operation: self.identity.operation().identifier().to_string(),
                source_contract_version: self.identity.source_contract_version(),
            },
            descriptor: ProcessingRuntimeDescriptorDocumentV1 {
                contract_version: self.descriptor_contract_version,
            },
        };

        serde_json::to_string(&document).map_err(|error| {
            ProcessingRuntimeInformationEncodingError::Serialization(error.to_string())
        })
    }

    pub fn from_json(input: &str) -> Result<Self, ProcessingRuntimeInformationDecodingError> {
        if let Some(field) = detect_duplicate_object_keys(input) {
            return Err(
                ProcessingRuntimeInformationDecodingError::StructuralDocument(format!(
                    "duplicate field '{field}'"
                )),
            );
        }

        let document = match serde_json::from_str::<ProcessingRuntimeInformationDocumentV1>(input) {
            Ok(document) => document,
            Err(error) => {
                return match error.classify() {
                    serde_json::error::Category::Syntax => Err(
                        ProcessingRuntimeInformationDecodingError::JsonSyntax(error.to_string()),
                    ),
                    _ => Err(
                        ProcessingRuntimeInformationDecodingError::StructuralDocument(
                            error.to_string(),
                        ),
                    ),
                };
            }
        };

        if document.schema_version != PROCESSING_RUNTIME_INFORMATION_SCHEMA_VERSION {
            return Err(
                ProcessingRuntimeInformationDecodingError::UnknownSchemaVersion(
                    document.schema_version,
                ),
            );
        }

        if document.identity.source_contract_version == 0 {
            return Err(ProcessingRuntimeInformationDecodingError::InvalidVersion {
                field: "identity.source_contract_version",
                value: document.identity.source_contract_version,
            });
        }

        if document.descriptor.contract_version == 0 {
            return Err(ProcessingRuntimeInformationDecodingError::InvalidVersion {
                field: "descriptor.contract_version",
                value: document.descriptor.contract_version,
            });
        }

        let protocol =
            RuntimeProtocol::from_identifier(&document.identity.protocol).map_err(|error| {
                match error {
                    RuntimeIdentifierError::UnknownIdentifier { value, .. } => {
                        ProcessingRuntimeInformationDecodingError::UnknownIdentifier {
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
                        ProcessingRuntimeInformationDecodingError::UnknownIdentifier {
                            field: "identity.operation",
                            value,
                        }
                    }
                }
            })?;

        if protocol != RuntimeProtocol::Http {
            return Err(ProcessingRuntimeInformationDecodingError::WrongProtocol {
                actual: protocol,
            });
        }

        if operation != RuntimeOperation::Processing {
            return Err(ProcessingRuntimeInformationDecodingError::WrongOperation {
                actual: operation,
            });
        }

        let source_name = Box::leak(document.identity.source.into_boxed_str());
        let identity = RuntimeIdentity::from_parts(
            source_name,
            protocol,
            operation,
            document.identity.source_contract_version,
        );

        Ok(Self::from_parts_unchecked(
            identity,
            document.descriptor.contract_version,
        ))
    }

    pub fn validate_compatibility(
        &self,
        expected_identity: RuntimeIdentity,
    ) -> Result<(), ProcessingRuntimeCompatibilityError> {
        if self.identity() != expected_identity {
            return Err(ProcessingRuntimeCompatibilityError::IdentityMismatch {
                expected: expected_identity,
                actual: self.identity(),
            });
        }

        if self.descriptor_contract_version() != self.identity().source_contract_version() {
            return Err(
                ProcessingRuntimeCompatibilityError::DescriptorContractVersionMismatch {
                    identity_version: self.identity().source_contract_version(),
                    descriptor_version: self.descriptor_contract_version(),
                },
            );
        }

        Ok(())
    }
}

fn detect_duplicate_object_keys(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut stack: Vec<(bool, std::collections::HashSet<String>)> = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'{' => {
                stack.push((true, std::collections::HashSet::new()));
                index += 1;
            }
            b'}' => {
                if !stack.is_empty() {
                    stack.pop();
                }
                index += 1;
            }
            b',' => {
                if let Some((state, _)) = stack.last_mut() {
                    *state = true;
                }
                index += 1;
            }
            b':' => {
                if let Some((state, _)) = stack.last_mut() {
                    *state = false;
                }
                index += 1;
            }
            b'"' => {
                let Some((next_index, value)) = parse_json_string(&bytes[index..]) else {
                    return None;
                };

                if let Some((expecting_key, seen_keys)) = stack.last_mut() {
                    if *expecting_key {
                        if !seen_keys.insert(value.clone()) {
                            return Some(value);
                        }
                    }
                }
                index += next_index;
            }
            b' ' | b'\n' | b'\r' | b'\t' => {
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    None
}

fn parse_json_string(input: &[u8]) -> Option<(usize, String)> {
    let mut index = 1;
    let mut escaped = false;
    let mut buffer = Vec::new();

    while index < input.len() {
        let byte = input[index];

        if escaped {
            buffer.push(byte);
            escaped = false;
            index += 1;
            continue;
        }

        match byte {
            b'\\' => {
                escaped = true;
                index += 1;
            }
            b'"' => {
                let value = String::from_utf8(buffer).ok()?;
                return Some((index + 1, value));
            }
            _ => {
                buffer.push(byte);
                index += 1;
            }
        }
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingRuntimeInformationConstructionError {
    WrongProtocol {
        actual: RuntimeProtocol,
    },
    WrongOperation {
        actual: RuntimeOperation,
    },
    IdentityContractVersionMismatch {
        identity_version: u32,
        descriptor_version: u32,
    },
}

impl fmt::Display for ProcessingRuntimeInformationConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongProtocol { actual } => {
                write!(
                    formatter,
                    "processing runtime information requires HTTP protocol, actual: {actual:?}"
                )
            }
            Self::WrongOperation { actual } => {
                write!(
                    formatter,
                    "processing runtime information requires processing operation, actual: {actual:?}"
                )
            }
            Self::IdentityContractVersionMismatch {
                identity_version,
                descriptor_version,
            } => write!(
                formatter,
                "processing runtime information identity/version mismatch: identity version {identity_version}, descriptor version {descriptor_version}"
            ),
        }
    }
}

impl std::error::Error for ProcessingRuntimeInformationConstructionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingRuntimeCompatibilityError {
    IdentityMismatch {
        expected: RuntimeIdentity,
        actual: RuntimeIdentity,
    },
    DescriptorContractVersionMismatch {
        identity_version: u32,
        descriptor_version: u32,
    },
}

impl fmt::Display for ProcessingRuntimeCompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityMismatch { expected, actual } => write!(
                formatter,
                "processing runtime identity mismatch: expected {expected:?}, actual {actual:?}"
            ),
            Self::DescriptorContractVersionMismatch {
                identity_version,
                descriptor_version,
            } => write!(
                formatter,
                "processing descriptor contract version mismatch: identity version {identity_version}, descriptor version {descriptor_version}"
            ),
        }
    }
}

impl std::error::Error for ProcessingRuntimeCompatibilityError {}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{
        PROCESSING_RUNTIME_INFORMATION_SCHEMA_VERSION, ProcessingRuntimeCompatibilityError,
        ProcessingRuntimeInformationConstructionError, ProcessingRuntimeInformationDecodingError,
        ProcessingRuntimeInformationV1,
    };
    use crate::processing::{ProcessingContext, ProcessingResult, ProcessingSourceContractV1};
    use crate::runtime::{RuntimeIdentity, RuntimeOperation, RuntimeProtocol};

    static CONSTRUCTION_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

    fn process_handler(
        _context: &mut ProcessingContext,
        _args: &[OsString],
    ) -> ProcessingResult<()> {
        CONSTRUCTION_CALL_COUNT.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn private_handler(
        _context: &mut ProcessingContext,
        _args: &[OsString],
    ) -> ProcessingResult<()> {
        Ok(())
    }

    fn processing_identity(version: u32) -> RuntimeIdentity {
        RuntimeIdentity::http_processing("example-source", version)
    }

    fn acquisition_identity() -> RuntimeIdentity {
        RuntimeIdentity::http_acquisition("example-source", 1)
    }

    #[test]
    fn valid_processing_identity_and_descriptor_construct_successfully() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let result =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source);

        assert!(result.is_ok(), "result: {result:?}");
    }

    #[test]
    fn information_preserves_source_identity() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let identity = processing_identity(1);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(identity, &source).unwrap();

        assert_eq!(info.identity(), identity);
    }

    #[test]
    fn information_reports_http_protocol() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();

        assert_eq!(info.identity().protocol(), RuntimeProtocol::Http);
    }

    #[test]
    fn information_reports_processing_operation() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();

        assert_eq!(info.identity().operation(), RuntimeOperation::Processing);
    }

    #[test]
    fn descriptor_contract_version_is_one() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();

        assert_eq!(
            info.descriptor_contract_version(),
            ProcessingSourceContractV1::CONTRACT_VERSION
        );
        assert_eq!(info.descriptor_contract_version(), 1);
    }

    #[test]
    fn processing_runtime_information_is_copy() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();
        let duplicate = info;
        let _ = (info, duplicate);
    }

    #[test]
    fn construction_does_not_invoke_process_handler() {
        CONSTRUCTION_CALL_COUNT.store(0, Ordering::Relaxed);
        let source = ProcessingSourceContractV1::new(process_handler);
        let _ =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source);

        assert_eq!(CONSTRUCTION_CALL_COUNT.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn acquisition_identity_returns_wrong_operation() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let result =
            ProcessingRuntimeInformationV1::from_processing_source(acquisition_identity(), &source);

        assert_eq!(
            result,
            Err(
                ProcessingRuntimeInformationConstructionError::WrongOperation {
                    actual: RuntimeOperation::Acquisition,
                }
            )
        );
    }

    #[test]
    fn non_matching_identity_contract_version_returns_identity_contract_version_mismatch() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let result =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(2), &source);

        assert_eq!(
            result,
            Err(
                ProcessingRuntimeInformationConstructionError::IdentityContractVersionMismatch {
                    identity_version: 2,
                    descriptor_version: ProcessingSourceContractV1::CONTRACT_VERSION,
                }
            )
        );
    }

    #[test]
    fn validation_against_same_processing_identity_succeeds() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();

        assert!(info.validate_compatibility(processing_identity(1)).is_ok());
    }

    #[test]
    fn validation_against_another_source_returns_identity_mismatch() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();

        assert!(matches!(
            info.validate_compatibility(RuntimeIdentity::http_processing("other-source", 1)),
            Err(ProcessingRuntimeCompatibilityError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn validation_against_acquisition_identity_returns_identity_mismatch() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();

        assert!(matches!(
            info.validate_compatibility(acquisition_identity()),
            Err(ProcessingRuntimeCompatibilityError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn descriptor_version_disagreement_returns_descriptor_contract_version_mismatch() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();
        let mutated = ProcessingRuntimeInformationV1 {
            identity: info.identity(),
            descriptor_contract_version: 2,
        };

        assert!(matches!(
            mutated.validate_compatibility(info.identity()),
            Err(ProcessingRuntimeCompatibilityError::DescriptorContractVersionMismatch { .. })
        ));
    }

    #[test]
    fn construction_and_validation_do_not_mutate_descriptor() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let original_ptr = source.process_handler() as *const ();
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();
        let validated = info.validate_compatibility(processing_identity(1));

        assert!(validated.is_ok());
        assert_eq!(source.process_handler() as *const (), original_ptr);
    }

    #[test]
    fn construction_and_validation_do_not_invoke_process_handler() {
        CONSTRUCTION_CALL_COUNT.store(0, Ordering::Relaxed);
        let source = ProcessingSourceContractV1::new(process_handler);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();
        let _ = info.validate_compatibility(processing_identity(1));

        assert_eq!(CONSTRUCTION_CALL_COUNT.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn private_handler_works_behind_public_descriptor_constant() {
        const PRIVATE_SOURCE: ProcessingSourceContractV1 =
            ProcessingSourceContractV1::new(private_handler);

        let mut context = ProcessingContext::new_for_tests();
        let args = [OsString::from("alpha")];
        let result = PRIVATE_SOURCE.process_handler()(&mut context, &args);

        assert!(result.is_ok(), "result: {result:?}");
    }

    #[test]
    fn native_source_arguments_are_not_involved_in_information_construction() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let _ = source.process_handler();
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();

        assert_eq!(info.identity().source_contract_version(), 1);
    }

    #[test]
    fn valid_processing_information_serializes_successfully() {
        let info = ProcessingRuntimeInformationV1::from_processing_source(
            processing_identity(1),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap();
        assert!(info.to_json().is_ok());
    }

    #[test]
    fn processing_schema_version_is_one() {
        let info = ProcessingRuntimeInformationV1::from_processing_source(
            processing_identity(1),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap();
        let json = info.to_json().unwrap();
        assert!(json.contains("\"schema_version\":1"));
        assert_eq!(PROCESSING_RUNTIME_INFORMATION_SCHEMA_VERSION, 1);
    }

    #[test]
    fn processing_protocol_and_operation_are_stable() {
        let info = ProcessingRuntimeInformationV1::from_processing_source(
            processing_identity(1),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap();
        let json = info.to_json().unwrap();
        assert!(json.contains("\"protocol\":\"http\""));
        assert!(json.contains("\"operation\":\"processing\""));
    }

    #[test]
    fn source_identity_is_preserved_in_json_round_trip() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let identity = processing_identity(1);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(identity, &source).unwrap();
        let json = info.to_json().unwrap();
        let parsed = ProcessingRuntimeInformationV1::from_json(&json).unwrap();
        assert_eq!(parsed.identity(), info.identity());
        assert_eq!(parsed.identity().source_name(), "example-source");
    }

    #[test]
    fn source_contract_version_is_preserved() {
        let info = ProcessingRuntimeInformationV1::from_processing_source(
            processing_identity(1),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap();
        let json = info.to_json().unwrap();
        let parsed = ProcessingRuntimeInformationV1::from_json(&json).unwrap();
        assert_eq!(parsed.identity().source_contract_version(), 1);
    }

    #[test]
    fn descriptor_contract_version_is_preserved() {
        let info = ProcessingRuntimeInformationV1::from_processing_source(
            processing_identity(1),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap();
        let json = info.to_json().unwrap();
        let parsed = ProcessingRuntimeInformationV1::from_json(&json).unwrap();
        assert_eq!(
            parsed.descriptor_contract_version(),
            info.descriptor_contract_version()
        );
    }

    #[test]
    fn encoding_does_not_add_a_newline() {
        let info = ProcessingRuntimeInformationV1::from_processing_source(
            processing_identity(1),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap();
        let json = info.to_json().unwrap();
        assert!(!json.ends_with('\n'));
        assert!(!json.contains("\n"));
    }

    #[test]
    fn encoding_does_not_invoke_process_handler() {
        CONSTRUCTION_CALL_COUNT.store(0, Ordering::Relaxed);
        let source = ProcessingSourceContractV1::new(process_handler);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();
        let _ = info.to_json();
        assert_eq!(CONSTRUCTION_CALL_COUNT.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn json_round_trip_preserves_equality() {
        let info = ProcessingRuntimeInformationV1::from_processing_source(
            processing_identity(1),
            &ProcessingSourceContractV1::new(process_handler),
        )
        .unwrap();
        let round_trip =
            ProcessingRuntimeInformationV1::from_json(&info.to_json().unwrap()).unwrap();
        assert_eq!(round_trip, info);
    }

    #[test]
    fn invalid_json_is_rejected() {
        let result = ProcessingRuntimeInformationV1::from_json("{not valid}");
        assert!(matches!(
            result,
            Err(ProcessingRuntimeInformationDecodingError::JsonSyntax(_))
        ));
    }

    #[test]
    fn duplicate_fields_are_rejected() {
        let json = r#"{"schema_version":1,"schema_version":2,"identity":{"source":"example-source","protocol":"http","operation":"processing","source_contract_version":1},"descriptor":{"contract_version":1}}"#;
        let result = ProcessingRuntimeInformationV1::from_json(json);
        assert!(matches!(
            result,
            Err(ProcessingRuntimeInformationDecodingError::StructuralDocument(_))
        ));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let json = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"processing","source_contract_version":1},"descriptor":{"contract_version":1},"extra":true}"#;
        let result = ProcessingRuntimeInformationV1::from_json(json);
        assert!(matches!(
            result,
            Err(ProcessingRuntimeInformationDecodingError::StructuralDocument(_))
        ));
    }

    #[test]
    fn missing_fields_are_rejected() {
        let json = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"processing"}}"#;
        let result = ProcessingRuntimeInformationV1::from_json(json);
        assert!(matches!(
            result,
            Err(ProcessingRuntimeInformationDecodingError::StructuralDocument(_))
        ));
    }

    #[test]
    fn unknown_schema_versions_are_rejected() {
        let json = r#"{"schema_version":999,"identity":{"source":"example-source","protocol":"http","operation":"processing","source_contract_version":1},"descriptor":{"contract_version":1}}"#;
        let result = ProcessingRuntimeInformationV1::from_json(json);
        assert!(matches!(
            result,
            Err(ProcessingRuntimeInformationDecodingError::UnknownSchemaVersion(999))
        ));
    }

    #[test]
    fn unknown_protocol_identifiers_are_rejected() {
        let json = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"https","operation":"processing","source_contract_version":1},"descriptor":{"contract_version":1}}"#;
        let result = ProcessingRuntimeInformationV1::from_json(json);
        assert!(matches!(
            result,
            Err(
                ProcessingRuntimeInformationDecodingError::UnknownIdentifier {
                    field: "identity.protocol",
                    ..
                }
            )
        ));
    }

    #[test]
    fn unknown_operation_identifiers_are_rejected() {
        let json = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"resume","source_contract_version":1},"descriptor":{"contract_version":1}}"#;
        let result = ProcessingRuntimeInformationV1::from_json(json);
        assert!(matches!(
            result,
            Err(
                ProcessingRuntimeInformationDecodingError::UnknownIdentifier {
                    field: "identity.operation",
                    ..
                }
            )
        ));
    }

    #[test]
    fn acquisition_operation_is_rejected_as_wrong_operation() {
        let json = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"acquisition","source_contract_version":1},"descriptor":{"contract_version":1}}"#;
        let result = ProcessingRuntimeInformationV1::from_json(json);
        assert!(matches!(
            result,
            Err(ProcessingRuntimeInformationDecodingError::WrongOperation {
                actual: RuntimeOperation::Acquisition
            })
        ));
    }

    #[test]
    fn zero_identity_contract_version_is_rejected() {
        let json = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"processing","source_contract_version":0},"descriptor":{"contract_version":1}}"#;
        let result = ProcessingRuntimeInformationV1::from_json(json);
        assert!(matches!(
            result,
            Err(ProcessingRuntimeInformationDecodingError::InvalidVersion {
                field: "identity.source_contract_version",
                ..
            })
        ));
    }

    #[test]
    fn zero_descriptor_contract_version_is_rejected() {
        let json = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"processing","source_contract_version":1},"descriptor":{"contract_version":0}}"#;
        let result = ProcessingRuntimeInformationV1::from_json(json);
        assert!(matches!(
            result,
            Err(ProcessingRuntimeInformationDecodingError::InvalidVersion {
                field: "descriptor.contract_version",
                ..
            })
        ));
    }

    #[test]
    fn structurally_valid_incompatible_versions_decode() {
        let json = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"processing","source_contract_version":2},"descriptor":{"contract_version":1}}"#;
        let info = ProcessingRuntimeInformationV1::from_json(json).unwrap();
        assert_eq!(info.identity().source_contract_version(), 2);
        assert_eq!(info.descriptor_contract_version(), 1);
    }

    #[test]
    fn structurally_valid_incompatible_versions_fail_compatibility_validation() {
        let json = r#"{"schema_version":1,"identity":{"source":"example-source","protocol":"http","operation":"processing","source_contract_version":2},"descriptor":{"contract_version":1}}"#;
        let info = ProcessingRuntimeInformationV1::from_json(json).unwrap();
        let result = info.validate_compatibility(processing_identity(1));
        assert!(matches!(
            result,
            Err(ProcessingRuntimeCompatibilityError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn private_unchecked_constructor_is_not_publicly_accessible() {
        let source = ProcessingSourceContractV1::new(process_handler);
        let info =
            ProcessingRuntimeInformationV1::from_processing_source(processing_identity(1), &source)
                .unwrap();
        assert_eq!(info.identity().source_contract_version(), 1);
    }
}
