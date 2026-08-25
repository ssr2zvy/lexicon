use crate::runtime::RuntimeIdentifierError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpCapability {
    ClientCertificateV1,
}

impl HttpCapability {
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::ClientCertificateV1 => "client-certificate-v1",
        }
    }

    pub fn from_identifier(value: &str) -> Result<Self, RuntimeIdentifierError> {
        match value {
            "client-certificate-v1" => Ok(Self::ClientCertificateV1),
            _ => Err(RuntimeIdentifierError::unknown("capability", value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpCapabilitySet {
    bits: u8,
}

impl HttpCapabilitySet {
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn insert(self, capability: HttpCapability) -> Self {
        Self {
            bits: self.bits | Self::bit_for(capability),
        }
    }

    pub const fn contains(self, capability: HttpCapability) -> bool {
        (self.bits & Self::bit_for(capability)) != 0
    }

    pub fn ordered_capabilities(self) -> Vec<HttpCapability> {
        let mut capabilities = Vec::new();
        if self.contains(HttpCapability::ClientCertificateV1) {
            capabilities.push(HttpCapability::ClientCertificateV1);
        }
        capabilities
    }

    const fn bit_for(capability: HttpCapability) -> u8 {
        match capability {
            HttpCapability::ClientCertificateV1 => 1 << 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HttpCapability, HttpCapabilitySet};
    use crate::runtime::RuntimeIdentifierError;

    #[test]
    fn capability_identifier_is_stable() {
        assert_eq!(
            HttpCapability::ClientCertificateV1.identifier(),
            "client-certificate-v1"
        );
    }

    #[test]
    fn capability_from_identifier_accepts_known_value() {
        assert_eq!(
            HttpCapability::from_identifier("client-certificate-v1"),
            Ok(HttpCapability::ClientCertificateV1)
        );
    }

    #[test]
    fn capability_from_identifier_rejects_unknown_value() {
        assert!(matches!(
            HttpCapability::from_identifier("client-certificate-v2"),
            Err(RuntimeIdentifierError::UnknownIdentifier { .. })
        ));
    }

    #[test]
    fn capability_ordering_is_deterministic() {
        let required = HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1);
        assert_eq!(
            required.ordered_capabilities(),
            vec![HttpCapability::ClientCertificateV1]
        );
    }
}
