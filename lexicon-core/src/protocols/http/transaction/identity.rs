use std::fmt;

const HTTP_TRANSACTION_ID_LENGTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HttpTransactionIdentity {
    id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpTransactionIdentityError {
    Empty,
    InvalidLength { found: usize },
    InvalidCharacter,
}

impl HttpTransactionIdentity {
    pub(crate) fn new() -> Result<Self, HttpTransactionIdentityError> {
        Self::from_validated(uuid::Uuid::new_v4().simple().to_string())
    }

    pub(crate) fn from_validated(
        id: impl Into<String>,
    ) -> Result<Self, HttpTransactionIdentityError> {
        let id = id.into();
        if id.is_empty() {
            return Err(HttpTransactionIdentityError::Empty);
        }
        if id.len() != HTTP_TRANSACTION_ID_LENGTH {
            return Err(HttpTransactionIdentityError::InvalidLength { found: id.len() });
        }
        if !id.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(HttpTransactionIdentityError::InvalidCharacter);
        }
        Ok(Self { id })
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl fmt::Display for HttpTransactionIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.id)
    }
}

impl fmt::Display for HttpTransactionIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("HTTP transaction identity is empty"),
            Self::InvalidLength { .. } => {
                formatter.write_str("HTTP transaction identity has invalid length")
            }
            Self::InvalidCharacter => {
                formatter.write_str("HTTP transaction identity has invalid characters")
            }
        }
    }
}

impl std::error::Error for HttpTransactionIdentityError {}
