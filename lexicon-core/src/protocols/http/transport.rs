use std::fmt;
use std::io::Read;

use reqwest::blocking::{Client, Response};
use reqwest::redirect::Policy;

use super::request::FinalizedHttpRequest;

pub(crate) trait HttpTransport: Send + Sync {
    fn execute(&self, request: &FinalizedHttpRequest) -> Result<HttpTransportResponse, HttpTransportFailure>;
}

pub(crate) struct ReqwestHttpTransport {
    client: Client,
}

impl ReqwestHttpTransport {
    pub(crate) fn new() -> Result<Self, HttpTransportConfigurationError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .build()
            .map_err(|e| HttpTransportConfigurationError(e.to_string()))?;

        Ok(Self { client })
    }
}

impl HttpTransport for ReqwestHttpTransport {
    fn execute(&self, request: &FinalizedHttpRequest) -> Result<HttpTransportResponse, HttpTransportFailure> {
        let method = reqwest::Method::from_bytes(request.method.as_bytes())
            .map_err(|_| HttpTransportFailure::RequestBuild)?;
        let mut builder = self.client.request(method, request.url.clone());

        for header in &request.headers {
            let name = reqwest::header::HeaderName::from_bytes(header.name.as_bytes())
                .map_err(|_| HttpTransportFailure::RequestBuild)?;
            let value = reqwest::header::HeaderValue::from_bytes(&header.value)
                .map_err(|_| HttpTransportFailure::RequestBuild)?;
            builder = builder.header(name, value);
        }

        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }

        let response = builder.send().map_err(classify_send_error)?;
        Ok(HttpTransportResponse::from_response(response))
    }
}

fn classify_send_error(error: reqwest::Error) -> HttpTransportFailure {
    if error.is_timeout() {
        HttpTransportFailure::Timeout
    } else if error.is_connect() {
        HttpTransportFailure::Connect
    } else if error.is_request() || error.is_builder() {
        HttpTransportFailure::RequestBuild
    } else {
        HttpTransportFailure::ExchangeIo
    }
}

pub(crate) struct HttpTransportResponse {
    pub(crate) status: u16,
    pub(crate) version: Option<String>,
    pub(crate) headers: Vec<(String, Vec<u8>)>,
    pub(crate) body: Box<dyn Read + Send>,
    /// Raw Location header value from the actual transport response, for redirect control.
    pub(crate) location_header: Option<String>,
}

impl HttpTransportResponse {
    fn from_response(response: Response) -> Self {
        let status = response.status().as_u16();
        let version = Some(format!("{:?}", response.version()));

        let mut headers = Vec::new();
        let mut location_header: Option<String> = None;

        for (name, value) in response.headers() {
            let name_lower = name.as_str().to_ascii_lowercase();
            if name_lower == "location" && location_header.is_none() {
                if let Ok(text) = std::str::from_utf8(value.as_bytes()) {
                    location_header = Some(text.to_string());
                }
            }
            headers.push((name.to_string(), value.as_bytes().to_vec()));
        }

        Self {
            status,
            version,
            headers,
            body: Box::new(response),
            location_header,
        }
    }
}

/// Opaque transport configuration error. Does not expose internal details.
#[derive(Debug, Clone)]
pub struct HttpTransportConfigurationError(pub(crate) String);

impl fmt::Display for HttpTransportConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HTTP transport configuration failed")
    }
}

impl std::error::Error for HttpTransportConfigurationError {}

/// Stable typed transport failure classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpTransportFailure {
    /// Client configuration prevented the request from being built.
    Configuration,
    /// Request construction failed (invalid method, header, or body conversion).
    RequestBuild,
    /// Connection to the server could not be established.
    Connect,
    /// The request or connection timed out.
    Timeout,
    /// An I/O error occurred while writing the request body.
    BodyWrite,
    /// An I/O error occurred during the HTTP exchange.
    ExchangeIo,
    /// A TLS/SSL error occurred during the exchange.
    Tls,
}

impl HttpTransportFailure {
    /// Returns true only for explicitly classified transient exchange failures.
    /// Unknown or non-transient failures are not retryable.
    pub fn retryable(self) -> bool {
        matches!(self, Self::Connect | Self::Timeout | Self::ExchangeIo)
    }

    /// Returns a stable string label for persistence in transaction metadata.
    pub fn stable_class(self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::RequestBuild => "request_build",
            Self::Connect => "connect",
            Self::Timeout => "timeout",
            Self::BodyWrite => "body_write",
            Self::ExchangeIo => "exchange_io",
            Self::Tls => "tls",
        }
    }
}

impl fmt::Display for HttpTransportFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration => formatter.write_str("HTTP transport configuration failed"),
            Self::RequestBuild => formatter.write_str("HTTP transport request construction failed"),
            Self::Connect => formatter.write_str("HTTP transport connection failed"),
            Self::Timeout => formatter.write_str("HTTP transport timed out"),
            Self::BodyWrite => formatter.write_str("HTTP transport body write failed"),
            Self::ExchangeIo => formatter.write_str("HTTP transport exchange failed"),
            Self::Tls => formatter.write_str("HTTP transport TLS error"),
        }
    }
}

impl std::error::Error for HttpTransportFailure {}
