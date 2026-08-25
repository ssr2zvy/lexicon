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

    const fn bit_for(capability: HttpCapability) -> u8 {
        match capability {
            HttpCapability::ClientCertificateV1 => 1 << 0,
        }
    }
}
