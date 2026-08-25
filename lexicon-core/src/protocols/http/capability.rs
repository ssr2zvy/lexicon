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

    pub const fn is_subset_of(self, available: HttpCapabilitySet) -> bool {
        (self.bits & available.bits) == self.bits
    }

    pub const fn missing_from(self, available: HttpCapabilitySet) -> Self {
        Self {
            bits: self.bits & !available.bits,
        }
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

    #[test]
    fn capability_set_subset_and_missing_checks_are_correct() {
        let required = HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1);
        let available = HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1);
        let empty = HttpCapabilitySet::empty();

        assert!(required.is_subset_of(available));
        assert_eq!(required.missing_from(available), HttpCapabilitySet::empty());
        assert!(!required.is_subset_of(empty));
        assert_eq!(
            required.missing_from(empty),
            HttpCapabilitySet::empty().insert(HttpCapability::ClientCertificateV1)
        );
    }

    #[test]
    fn repeated_capability_insertion_does_not_create_duplicates() {
        let set = HttpCapabilitySet::empty()
            .insert(HttpCapability::ClientCertificateV1)
            .insert(HttpCapability::ClientCertificateV1);

        assert_eq!(
            set.ordered_capabilities(),
            vec![HttpCapability::ClientCertificateV1]
        );
        assert!(set.contains(HttpCapability::ClientCertificateV1));
    }
}
