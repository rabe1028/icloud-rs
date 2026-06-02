//! `reqwest`-backed transport implementations.

use icloud_protocol::{
    HeaderMap, HeaderName, HeaderValue, Method, RequestSpec, ResponseParts, StatusCode, header,
};

#[cfg(feature = "reqwest-async")]
use crate::AsyncTransport;
#[cfg(feature = "reqwest-blocking")]
use crate::BlockingTransport;

#[cfg(feature = "reqwest-async")]
#[derive(Debug, Clone)]
pub struct ReqwestAsyncTransport {
    client: reqwest::Client,
}

#[cfg(feature = "reqwest-async")]
impl ReqwestAsyncTransport {
    #[must_use]
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[cfg(feature = "reqwest-async")]
impl AsyncTransport for ReqwestAsyncTransport {
    type Error = reqwest::Error;

    async fn execute(&self, request: RequestSpec) -> Result<ResponseParts, Self::Error> {
        let response = build_async_request(&self.client, request).send().await?;
        let status = StatusCode::from_u16(response.status().as_u16());
        let headers = protocol_headers_from_http(response.headers());
        let body = response.bytes().await?.to_vec();
        Ok(ResponseParts::new(status, headers, body))
    }
}

#[cfg(feature = "reqwest-blocking")]
#[derive(Debug, Clone)]
pub struct ReqwestBlockingTransport {
    client: reqwest::blocking::Client,
}

#[cfg(feature = "reqwest-blocking")]
impl ReqwestBlockingTransport {
    #[must_use]
    pub fn new(client: reqwest::blocking::Client) -> Self {
        Self { client }
    }
}

#[cfg(feature = "reqwest-blocking")]
impl BlockingTransport for ReqwestBlockingTransport {
    type Error = reqwest::Error;

    fn execute(&self, request: RequestSpec) -> Result<ResponseParts, Self::Error> {
        let response = build_blocking_request(&self.client, request).send()?;
        let status = StatusCode::from_u16(response.status().as_u16());
        let headers = protocol_headers_from_http(response.headers());
        let body = response.bytes()?.to_vec();
        Ok(ResponseParts::new(status, headers, body))
    }
}

#[cfg(feature = "reqwest-async")]
fn build_async_request(client: &reqwest::Client, request: RequestSpec) -> reqwest::RequestBuilder {
    let RequestSpec {
        method,
        url,
        headers,
        body,
        ..
    } = request;
    let mut builder = client
        .request(reqwest_method(method), url)
        .headers(reqwest_headers(headers));
    if let Some(body) = body.as_bytes() {
        builder = builder.body(body.to_vec());
    }
    builder
}

#[cfg(feature = "reqwest-blocking")]
fn build_blocking_request(
    client: &reqwest::blocking::Client,
    request: RequestSpec,
) -> reqwest::blocking::RequestBuilder {
    let RequestSpec {
        method,
        url,
        headers,
        body,
        ..
    } = request;
    let mut builder = client
        .request(reqwest_method(method), url)
        .headers(reqwest_headers(headers));
    if let Some(body) = body.as_bytes() {
        builder = builder.body(body.to_vec());
    }
    builder
}

#[cfg(any(feature = "reqwest-async", feature = "reqwest-blocking"))]
fn reqwest_method(method: Method) -> http::Method {
    match method {
        Method::Get => http::Method::GET,
        Method::Post => http::Method::POST,
        Method::Put => http::Method::PUT,
    }
}

#[cfg(any(feature = "reqwest-async", feature = "reqwest-blocking"))]
fn reqwest_headers(headers: HeaderMap) -> http::HeaderMap {
    let mut converted = http::HeaderMap::new();
    for (name, value) in headers.iter() {
        if let (Ok(name), Ok(value)) = (
            http::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            http::HeaderValue::from_str(value.as_str()),
        ) {
            converted.append(name, value);
        }
    }
    converted
}

#[cfg(any(feature = "reqwest-async", feature = "reqwest-blocking"))]
fn protocol_headers_from_http(headers: &http::HeaderMap) -> HeaderMap {
    let mut converted = HeaderMap::new();
    for (name, value) in headers {
        let Some(name) = protocol_header_name(name.as_str()) else {
            continue;
        };
        if let Ok(value) = value.to_str()
            && let Ok(value) = HeaderValue::parse(value)
        {
            converted.append(name, value);
        }
    }
    converted
}

#[cfg(any(feature = "reqwest-async", feature = "reqwest-blocking"))]
fn protocol_header_name(name: &str) -> Option<HeaderName> {
    match name {
        "content-type" => Some(header::CONTENT_TYPE),
        "set-cookie" => Some(header::SET_COOKIE),
        _ => None,
    }
}
