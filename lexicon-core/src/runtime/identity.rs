use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeIdentifierError {
    UnknownIdentifier { kind: &'static str, value: String },
}

impl RuntimeIdentifierError {
    pub fn unknown(kind: &'static str, value: impl Into<String>) -> Self {
        Self::UnknownIdentifier {
            kind,
            value: value.into(),
        }
    }
}

impl fmt::Display for RuntimeIdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownIdentifier { kind, value } => {
                write!(formatter, "unknown {kind} identifier: {value}")
            }
        }
    }
}

impl std::error::Error for RuntimeIdentifierError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RuntimeProtocol {
    Http,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RuntimeOperation {
    Acquisition,
}

impl RuntimeProtocol {
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Http => "http",
        }
    }

    pub fn from_identifier(value: &str) -> Result<Self, RuntimeIdentifierError> {
        match value {
            "http" => Ok(Self::Http),
            _ => Err(RuntimeIdentifierError::unknown("protocol", value)),
        }
    }
}

impl RuntimeOperation {
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Acquisition => "acquisition",
        }
    }

    pub fn from_identifier(value: &str) -> Result<Self, RuntimeIdentifierError> {
        match value {
            "acquisition" => Ok(Self::Acquisition),
            _ => Err(RuntimeIdentifierError::unknown("operation", value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimeIdentity {
    source_name: &'static str,
    protocol: RuntimeProtocol,
    operation: RuntimeOperation,
    source_contract_version: u32,
}

impl RuntimeIdentity {
    pub const fn from_parts(
        source_name: &'static str,
        protocol: RuntimeProtocol,
        operation: RuntimeOperation,
        source_contract_version: u32,
    ) -> Self {
        Self {
            source_name,
            protocol,
            operation,
            source_contract_version,
        }
    }

    pub const fn http_acquisition(source_name: &'static str, source_contract_version: u32) -> Self {
        Self::from_parts(
            source_name,
            RuntimeProtocol::Http,
            RuntimeOperation::Acquisition,
            source_contract_version,
        )
    }

    pub const fn source_name(&self) -> &'static str {
        self.source_name
    }

    pub const fn protocol(&self) -> RuntimeProtocol {
        self.protocol
    }

    pub const fn operation(&self) -> RuntimeOperation {
        self.operation
    }

    pub const fn source_contract_version(&self) -> u32 {
        self.source_contract_version
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeIdentifierError, RuntimeIdentity, RuntimeOperation, RuntimeProtocol};

    #[test]
    fn runtime_identity_http_acquisition_works_in_const() {
        const IDENTITY: RuntimeIdentity = RuntimeIdentity::http_acquisition("example-source", 1);

        let actual = IDENTITY;
        assert_eq!(actual.source_name(), "example-source");
        assert_eq!(actual.protocol(), RuntimeProtocol::Http);
        assert_eq!(actual.operation(), RuntimeOperation::Acquisition);
        assert_eq!(actual.source_contract_version(), 1);
    }

    #[test]
    fn runtime_identity_accessors_return_expected_values() {
        let identity = RuntimeIdentity::http_acquisition("example-source", 1);

        assert_eq!(identity.source_name(), "example-source");
        assert_eq!(identity.protocol(), RuntimeProtocol::Http);
        assert_eq!(identity.operation(), RuntimeOperation::Acquisition);
        assert_eq!(identity.source_contract_version(), 1);
    }

    #[test]
    fn runtime_protocol_identifier_is_stable() {
        assert_eq!(RuntimeProtocol::Http.identifier(), "http");
    }

    #[test]
    fn runtime_operation_identifier_is_stable() {
        assert_eq!(RuntimeOperation::Acquisition.identifier(), "acquisition");
    }

    #[test]
    fn runtime_protocol_from_identifier_accepts_known_value() {
        assert_eq!(
            RuntimeProtocol::from_identifier("http"),
            Ok(RuntimeProtocol::Http)
        );
    }

    #[test]
    fn runtime_operation_from_identifier_accepts_known_value() {
        assert_eq!(
            RuntimeOperation::from_identifier("acquisition"),
            Ok(RuntimeOperation::Acquisition)
        );
    }

    #[test]
    fn runtime_identifier_parse_rejects_unknown_values() {
        assert!(matches!(
            RuntimeProtocol::from_identifier("https"),
            Err(RuntimeIdentifierError::UnknownIdentifier { .. })
        ));
        assert!(matches!(
            RuntimeOperation::from_identifier("resume"),
            Err(RuntimeIdentifierError::UnknownIdentifier { .. })
        ));
    }

    #[test]
    fn runtime_identity_types_are_the_same() {
        let left: crate::runtime::RuntimeIdentity =
            crate::http::RuntimeIdentity::http_acquisition("example-source", 1);
        let right: crate::http::RuntimeIdentity =
            crate::runtime::RuntimeIdentity::http_acquisition("example-source", 1);

        assert_eq!(left, right);
    }

    #[test]
    fn runtime_identity_equality_matches_fields() {
        let left = RuntimeIdentity::http_acquisition("example-source", 1);
        let right = RuntimeIdentity::http_acquisition("example-source", 1);
        let different_name = RuntimeIdentity::http_acquisition("other-source", 1);
        let different_version = RuntimeIdentity::http_acquisition("example-source", 2);

        assert_eq!(left, right);
        assert_ne!(left, different_name);
        assert_ne!(left, different_version);
    }
}
