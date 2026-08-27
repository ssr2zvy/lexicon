use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HttpTransactionIdentity {
    id: String,
}

impl HttpTransactionIdentity {
    pub(crate) fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().simple().to_string(),
        }
    }

    pub(crate) fn from_string(id: String) -> Self {
        Self { id }
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
