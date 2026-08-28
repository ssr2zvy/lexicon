use std::collections::HashSet;
use std::fmt;

use reqwest::header::{HeaderName, HeaderValue};
use serde::Serialize;
use url::Url;

use super::policy::{HttpPolicyError, HttpRedirectPolicy, HttpRetryPolicy};
use super::transaction::{HttpLogicalRequestKey, HttpLogicalRequestKeyError};

#[derive(Debug, Clone)]
pub struct HttpRequest {
    method: String,
    url: String,
    headers: Vec<RequestHeader>,
    query: Vec<RequestQueryParameter>,
    body: RequestBody,
    logical_key: Option<HttpLogicalRequestKey>,
    retry_policy: HttpRetryPolicy,
    redirect_policy: HttpRedirectPolicy,
    /// Names marked via `sensitive_query_name()`, matched ASCII case-insensitively.
    explicit_sensitive_query_names: Vec<String>,
}

#[derive(Debug, Clone)]
struct RequestHeader {
    name: String,
    value: Vec<u8>,
    sensitive: bool,
}

#[derive(Debug, Clone)]
struct RequestQueryParameter {
    name: String,
    value: String,
    sensitive: bool,
}

#[derive(Debug, Clone)]
enum RequestBody {
    None,
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone)]
pub(crate) struct FinalizedHttpRequest {
    pub(crate) method: String,
    pub(crate) url: Url,
    pub(crate) redacted_url: String,
    pub(crate) headers: Vec<FinalizedHeader>,
    pub(crate) body: Option<Vec<u8>>,
    pub(crate) logical_key: Option<HttpLogicalRequestKey>,
    pub(crate) retry_policy: HttpRetryPolicy,
    pub(crate) redirect_policy: HttpRedirectPolicy,
    pub(crate) sensitive_query_names: HashSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct FinalizedHeader {
    pub(crate) name: String,
    pub(crate) value: Vec<u8>,
    pub(crate) sensitive: bool,
}

impl HttpRequest {
    pub fn get(url: impl AsRef<str>) -> Result<Self, HttpRequestError> {
        Self::new("GET", url)
    }

    pub fn post(url: impl AsRef<str>) -> Result<Self, HttpRequestError> {
        Self::new("POST", url)
    }

    pub fn put(url: impl AsRef<str>) -> Result<Self, HttpRequestError> {
        Self::new("PUT", url)
    }

    pub fn patch(url: impl AsRef<str>) -> Result<Self, HttpRequestError> {
        Self::new("PATCH", url)
    }

    pub fn delete(url: impl AsRef<str>) -> Result<Self, HttpRequestError> {
        Self::new("DELETE", url)
    }

    pub fn head(url: impl AsRef<str>) -> Result<Self, HttpRequestError> {
        Self::new("HEAD", url)
    }

    pub fn new(method: impl AsRef<str>, url: impl AsRef<str>) -> Result<Self, HttpRequestError> {
        let method = method.as_ref().trim().to_ascii_uppercase();
        if method.is_empty() {
            return Err(HttpRequestError::InvalidMethod);
        }

        Ok(Self {
            method,
            url: url.as_ref().to_string(),
            headers: Vec::new(),
            query: Vec::new(),
            body: RequestBody::None,
            logical_key: None,
            retry_policy: HttpRetryPolicy::none(),
            redirect_policy: HttpRedirectPolicy::none(),
            explicit_sensitive_query_names: Vec::new(),
        })
    }

    pub fn header(
        mut self,
        name: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<Self, HttpRequestError> {
        self.push_header(name.as_ref(), value.as_ref(), false)?;
        Ok(self)
    }

    pub fn sensitive_header(
        mut self,
        name: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<Self, HttpRequestError> {
        self.push_header(name.as_ref(), value.as_ref(), true)?;
        Ok(self)
    }

    pub fn sensitive_header_from_env(
        mut self,
        name: impl AsRef<str>,
        environment_variable: impl AsRef<str>,
    ) -> Result<Self, HttpRequestError> {
        let variable_name = environment_variable.as_ref();
        match std::env::var_os(variable_name) {
            None => return Err(HttpRequestError::EnvironmentVariableUnavailable),
            Some(os_value) => {
                let value = os_value
                    .into_string()
                    .map_err(|_| HttpRequestError::EnvironmentVariableNotUtf8)?;
                self.push_header(name.as_ref(), &value, true)?;
            }
        }
        Ok(self)
    }

    /// Marks every existing or appended query field with `name` as sensitive
    /// for persisted metadata. ASCII case-insensitive matching.
    pub fn sensitive_query_name(mut self, name: impl AsRef<str>) -> Result<Self, HttpRequestError> {
        let key = name.as_ref().trim().to_ascii_lowercase();
        if key.is_empty() {
            return Err(HttpRequestError::InvalidQueryParameter);
        }
        // Mark any already-accumulated query parameters with this name as sensitive.
        for param in &mut self.query {
            if param.name.to_ascii_lowercase() == key {
                param.sensitive = true;
            }
        }
        // Record the name so finalize() will also mark future/URL-embedded params.
        self.explicit_sensitive_query_names.push(key);
        Ok(self)
    }

    pub fn query_parameter(
        mut self,
        name: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<Self, HttpRequestError> {
        self.push_query(name.as_ref(), value.as_ref(), false)?;
        Ok(self)
    }

    pub fn sensitive_query_parameter(
        mut self,
        name: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<Self, HttpRequestError> {
        self.push_query(name.as_ref(), value.as_ref(), true)?;
        Ok(self)
    }

    pub fn body_bytes(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = RequestBody::Bytes(body.into());
        self
    }

    pub fn text(mut self, text: impl AsRef<str>) -> Self {
        self.body = RequestBody::Bytes(text.as_ref().as_bytes().to_vec());
        self
    }

    pub fn json<T: Serialize>(mut self, value: &T) -> Result<Self, HttpRequestError> {
        let bytes = serde_json::to_vec(value).map_err(HttpRequestError::JsonSerialization)?;
        self.body = RequestBody::Bytes(bytes);
        Ok(self)
    }

    pub fn logical_key(mut self, key: impl AsRef<str>) -> Result<Self, HttpRequestError> {
        self.logical_key = Some(
            HttpLogicalRequestKey::new(key).map_err(HttpRequestError::InvalidLogicalKey)?,
        );
        Ok(self)
    }

    pub fn retry_policy(mut self, policy: HttpRetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    pub fn redirect_policy(mut self, policy: HttpRedirectPolicy) -> Self {
        self.redirect_policy = policy;
        self
    }

    pub(crate) fn finalize(self) -> Result<FinalizedHttpRequest, HttpRequestError> {
        if self.retry_policy.maximum_attempts() == 0 {
            return Err(HttpRequestError::Policy(HttpPolicyError::InvalidRetryMaximumAttempts));
        }
        if matches!(self.redirect_policy, HttpRedirectPolicy::Follow { maximum: 0 }) {
            return Err(HttpRequestError::Policy(HttpPolicyError::InvalidRedirectMaximum));
        }

        let mut url = Url::parse(&self.url).map_err(HttpRequestError::InvalidUrl)?;
        match url.scheme() {
            "http" | "https" => {}
            _ => return Err(HttpRequestError::UnsupportedScheme),
        }

        {
            let mut query_pairs = url.query_pairs_mut();
            for query in &self.query {
                query_pairs.append_pair(&query.name, &query.value);
            }
        }

        let mut sensitive_query_names = HashSet::new();
        for query in &self.query {
            if query.sensitive {
                sensitive_query_names.insert(query.name.to_ascii_lowercase());
            }
        }
        // Include names marked via sensitive_query_name().
        for name in &self.explicit_sensitive_query_names {
            sensitive_query_names.insert(name.clone());
        }

        let redacted_url = redact_url(&url, &sensitive_query_names);

        let headers = self
            .headers
            .into_iter()
            .map(|header| FinalizedHeader {
                name: header.name,
                value: header.value,
                sensitive: header.sensitive,
            })
            .collect();

        let body = match self.body {
            RequestBody::None => None,
            RequestBody::Bytes(bytes) => Some(bytes),
        };

        Ok(FinalizedHttpRequest {
            method: self.method,
            url,
            redacted_url,
            headers,
            body,
            logical_key: self.logical_key,
            retry_policy: self.retry_policy,
            redirect_policy: self.redirect_policy,
            sensitive_query_names,
        })
    }

    fn push_header(
        &mut self,
        name: &str,
        value: &str,
        sensitive: bool,
    ) -> Result<(), HttpRequestError> {
        HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| HttpRequestError::InvalidHeaderName)?;
        HeaderValue::from_str(value).map_err(|_| HttpRequestError::InvalidHeaderValue)?;

        self.headers.push(RequestHeader {
            name: name.to_string(),
            value: value.as_bytes().to_vec(),
            sensitive,
        });
        Ok(())
    }

    fn push_query(&mut self, name: &str, value: &str, sensitive: bool) -> Result<(), HttpRequestError> {
        if name.is_empty() {
            return Err(HttpRequestError::InvalidQueryParameter);
        }

        self.query.push(RequestQueryParameter {
            name: name.to_string(),
            value: value.to_string(),
            sensitive,
        });
        Ok(())
    }
}

pub(crate) fn redact_url(url: &Url, sensitive_query_names: &HashSet<String>) -> String {
    let mut redacted = url.clone();
    let pairs: Vec<(String, String)> = redacted
        .query_pairs()
        .map(|(name, value)| {
            let key = name.to_ascii_lowercase();
            if sensitive_query_names.contains(&key) {
                (name.to_string(), "<redacted>".to_string())
            } else {
                (name.to_string(), value.to_string())
            }
        })
        .collect();

    redacted.set_query(None);
    {
        let mut q = redacted.query_pairs_mut();
        for (name, value) in pairs {
            q.append_pair(&name, &value);
        }
    }
    redacted.to_string()
}

#[derive(Debug)]
pub enum HttpRequestError {
    InvalidMethod,
    InvalidUrl(url::ParseError),
    UnsupportedScheme,
    InvalidHeaderName,
    InvalidHeaderValue,
    /// The requested environment variable is not set.
    EnvironmentVariableUnavailable,
    /// The requested environment variable is set but not valid UTF-8.
    EnvironmentVariableNotUtf8,
    JsonSerialization(serde_json::Error),
    InvalidLogicalKey(HttpLogicalRequestKeyError),
    InvalidQueryParameter,
    Policy(HttpPolicyError),
}

impl fmt::Display for HttpRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMethod => formatter.write_str("invalid HTTP method"),
            Self::InvalidUrl(_) => formatter.write_str("invalid HTTP URL"),
            Self::UnsupportedScheme => formatter.write_str("unsupported HTTP URL scheme"),
            Self::InvalidHeaderName => formatter.write_str("invalid HTTP header name"),
            Self::InvalidHeaderValue => formatter.write_str("invalid HTTP header value"),
            Self::EnvironmentVariableUnavailable => {
                formatter.write_str("environment variable for sensitive header is not set")
            }
            Self::EnvironmentVariableNotUtf8 => {
                formatter.write_str("environment variable for sensitive header is not valid UTF-8")
            }
            Self::JsonSerialization(_) => formatter.write_str("failed to serialize JSON request body"),
            Self::InvalidLogicalKey(_) => formatter.write_str("invalid logical request key"),
            Self::InvalidQueryParameter => formatter.write_str("invalid HTTP query parameter"),
            Self::Policy(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for HttpRequestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidUrl(error) => Some(error),
            Self::JsonSerialization(error) => Some(error),
            Self::InvalidLogicalKey(error) => Some(error),
            Self::Policy(error) => Some(error),
            _ => None,
        }
    }
}
