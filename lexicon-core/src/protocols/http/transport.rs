use std::fmt;
use std::io::Read;
use std::sync::Arc;

use reqwest::blocking::{Client, Response};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};

use super::request::FinalizedHttpRequest;

pub(crate) trait HttpTransport: Send + Sync {
    fn execute(
        &self,
        request: &FinalizedHttpRequest,
    ) -> Result<HttpTransportResponse, HttpTransportFailure>;
}

pub(crate) struct ReqwestHttpTransport {
    client: Client,
}

impl ReqwestHttpTransport {
    pub(crate) fn new() -> Result<Self, HttpTransportConfigurationError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .build()
            .map_err(HttpTransportConfigurationError::new)?;

        Ok(Self { client })
    }
}

impl HttpTransport for ReqwestHttpTransport {
    fn execute(
        &self,
        request: &FinalizedHttpRequest,
    ) -> Result<HttpTransportResponse, HttpTransportFailure> {
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
    } else if error.is_body() {
        HttpTransportFailure::BodyWrite
    } else if error.is_request() || error.is_builder() {
        HttpTransportFailure::RequestBuild
    } else if error.is_connect() {
        HttpTransportFailure::Connect
    } else {
        HttpTransportFailure::ExchangeIo
    }
}

pub(crate) struct HttpTransportResponse {
    pub(crate) status: u16,
    pub(crate) version: Option<StoredHttpVersion>,
    pub(crate) headers: Vec<(String, Vec<u8>)>,
    pub(crate) body: Box<dyn Read + Send>,
    pub(crate) location_header: HttpLocationHeader,
}

impl HttpTransportResponse {
    fn from_response(response: Response) -> Self {
        let status = response.status().as_u16();
        let version = StoredHttpVersion::from_reqwest(response.version());

        let mut headers = Vec::new();
        let mut location_header = HttpLocationHeader::Missing;

        for (name, value) in response.headers() {
            let name_lower = name.as_str().to_ascii_lowercase();
            if name_lower == "location" && matches!(location_header, HttpLocationHeader::Missing) {
                location_header = match std::str::from_utf8(value.as_bytes()) {
                    Ok(text) => HttpLocationHeader::Present(text.to_string()),
                    Err(_) => HttpLocationHeader::InvalidEncoding,
                };
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

pub(crate) enum HttpLocationHeader {
    Missing,
    InvalidEncoding,
    Present(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredHttpVersion {
    Http09,
    Http10,
    Http11,
    Http2,
    Http3,
}

impl StoredHttpVersion {
    fn from_reqwest(version: reqwest::Version) -> Option<Self> {
        match version {
            reqwest::Version::HTTP_09 => Some(Self::Http09),
            reqwest::Version::HTTP_10 => Some(Self::Http10),
            reqwest::Version::HTTP_11 => Some(Self::Http11),
            reqwest::Version::HTTP_2 => Some(Self::Http2),
            reqwest::Version::HTTP_3 => Some(Self::Http3),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpTransportConfigurationError {
    source: Arc<reqwest::Error>,
}

impl HttpTransportConfigurationError {
    fn new(source: reqwest::Error) -> Self {
        Self {
            source: Arc::new(source),
        }
    }
}

impl fmt::Display for HttpTransportConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HTTP transport configuration failed")
    }
}

impl std::error::Error for HttpTransportConfigurationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpTransportFailure {
    Configuration,
    RequestBuild,
    Connect,
    Timeout,
    BodyWrite,
    ExchangeIo,
}

impl HttpTransportFailure {
    pub fn retryable(self) -> bool {
        matches!(self, Self::Connect | Self::Timeout | Self::ExchangeIo)
    }

    pub fn stable_class(self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::RequestBuild => "request_build",
            Self::Connect => "connect",
            Self::Timeout => "timeout",
            Self::BodyWrite => "body_write",
            Self::ExchangeIo => "exchange_io",
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
        }
    }
}

impl std::error::Error for HttpTransportFailure {}
