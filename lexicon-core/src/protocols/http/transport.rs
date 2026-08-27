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
    pub(crate) fn new() -> Result<Self, HttpTransportFailure> {
        let client = Client::builder()
            .redirect(Policy::none())
            .gzip(false)
            .brotli(false)
            .deflate(false)
            .zstd(false)
            .build()
            .map_err(|_| HttpTransportFailure::Configuration)?;

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

        let response = builder.send().map_err(|_| HttpTransportFailure::Io)?;
        Ok(HttpTransportResponse::from_response(response))
    }
}

pub(crate) struct HttpTransportResponse {
    pub(crate) status: u16,
    pub(crate) version: Option<String>,
    pub(crate) headers: Vec<(String, Vec<u8>)>,
    pub(crate) body: Box<dyn Read + Send>,
}

impl HttpTransportResponse {
    fn from_response(response: Response) -> Self {
        let status = response.status().as_u16();
        let version = Some(format!("{:?}", response.version()));

        let mut headers = Vec::new();
        for (name, value) in response.headers() {
            headers.push((name.to_string(), value.as_bytes().to_vec()));
        }

        Self {
            status,
            version,
            headers,
            body: Box::new(response),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpTransportFailure {
    Configuration,
    RequestBuild,
    Io,
}

impl fmt::Display for HttpTransportFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration => formatter.write_str("HTTP transport configuration failed"),
            Self::RequestBuild => formatter.write_str("HTTP transport request construction failed"),
            Self::Io => formatter.write_str("HTTP transport exchange failed"),
        }
    }
}

impl std::error::Error for HttpTransportFailure {}
