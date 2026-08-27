use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRedirectPolicy {
    None,
    Follow { maximum: u32 },
}

impl HttpRedirectPolicy {
    pub const DEFAULT_MAXIMUM_REDIRECTS: u32 = 10;

    pub const fn none() -> Self {
        Self::None
    }

    pub fn follow(maximum: u32) -> Result<Self, HttpPolicyError> {
        if maximum == 0 {
            return Err(HttpPolicyError::InvalidRedirectMaximum);
        }
        Ok(Self::Follow { maximum })
    }

    pub(crate) fn max_redirects(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Follow { maximum } => maximum,
        }
    }
}

impl Default for HttpRedirectPolicy {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRetryPolicy {
    maximum_attempts: u32,
    retryable_transport_failures: bool,
    retryable_statuses: Vec<u16>,
}

impl HttpRetryPolicy {
    pub const DEFAULT_MAXIMUM_ATTEMPTS: u32 = 3;

    pub const fn none() -> Self {
        Self {
            maximum_attempts: 1,
            retryable_transport_failures: false,
            retryable_statuses: Vec::new(),
        }
    }

    pub fn transient(maximum_attempts: u32) -> Result<Self, HttpPolicyError> {
        if maximum_attempts == 0 {
            return Err(HttpPolicyError::InvalidRetryMaximumAttempts);
        }

        Ok(Self {
            maximum_attempts,
            retryable_transport_failures: true,
            retryable_statuses: vec![408, 425, 429, 500, 502, 503, 504],
        })
    }

    pub fn custom(
        maximum_attempts: u32,
        retryable_transport_failures: bool,
        retryable_statuses: impl Into<Vec<u16>>,
    ) -> Result<Self, HttpPolicyError> {
        if maximum_attempts == 0 {
            return Err(HttpPolicyError::InvalidRetryMaximumAttempts);
        }

        Ok(Self {
            maximum_attempts,
            retryable_transport_failures,
            retryable_statuses: retryable_statuses.into(),
        })
    }

    pub const fn maximum_attempts(&self) -> u32 {
        self.maximum_attempts
    }

    pub const fn retryable_transport_failures(&self) -> bool {
        self.retryable_transport_failures
    }

    pub fn retryable_statuses(&self) -> &[u16] {
        &self.retryable_statuses
    }

    pub(crate) fn should_retry_status(&self, status: u16) -> bool {
        self.retryable_statuses.contains(&status)
    }
}

impl Default for HttpRetryPolicy {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpPolicyError {
    InvalidRedirectMaximum,
    InvalidRetryMaximumAttempts,
}

impl fmt::Display for HttpPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRedirectMaximum => formatter.write_str(
                "invalid redirect policy: maximum redirects must be greater than zero",
            ),
            Self::InvalidRetryMaximumAttempts => formatter.write_str(
                "invalid retry policy: maximum attempts must be greater than zero",
            ),
        }
    }
}

impl std::error::Error for HttpPolicyError {}
