use crate::prelude::*;

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::str;
use nojson::RawJson;
use url::Url;

use icloud_types::{
    CookiePair, ICloudApiError, ICloudApiErrorCode, ICloudError, JsonError, JsonObject, JsonValue,
};

pub mod header {
    use super::HeaderName;

    pub const ACCEPT: HeaderName = HeaderName::from_static("accept");
    pub const ACCEPT_LANGUAGE: HeaderName = HeaderName::from_static("accept-language");
    pub const CONTENT_TYPE: HeaderName = HeaderName::from_static("content-type");
    pub const COOKIE: HeaderName = HeaderName::from_static("cookie");
    pub const ORIGIN: HeaderName = HeaderName::from_static("origin");
    pub const REFERER: HeaderName = HeaderName::from_static("referer");
    pub const SET_COOKIE: HeaderName = HeaderName::from_static("set-cookie");
    pub const USER_AGENT: HeaderName = HeaderName::from_static("user-agent");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
}

impl Method {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeaderName(&'static str);

impl HeaderName {
    #[must_use]
    pub const fn from_static(name: &'static str) -> Self {
        Self(name)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl From<&'static str> for HeaderName {
    fn from(value: &'static str) -> Self {
        Self::from_static(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderValue(String);

impl HeaderValue {
    #[must_use]
    pub fn from_static(value: &'static str) -> Self {
        Self(value.into())
    }

    pub fn parse(value: &str) -> Result<Self, HeaderValueError> {
        if value.bytes().any(|byte| byte < 0x20 && byte != b'\t') {
            return Err(HeaderValueError);
        }
        Ok(Self(value.into()))
    }

    /// Returns the value as a string.
    ///
    /// The stored value is validated on construction, so this is currently
    /// infallible; the `Result` is kept for parity with `http::HeaderValue`
    /// so callers can stay agnostic about which header type they hold. Prefer
    /// [`as_str`](Self::as_str) when you already know you hold this type.
    pub fn to_str(&self) -> Result<&str, HeaderValueError> {
        Ok(self.0.as_str())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl PartialEq<&str> for HeaderValue {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<str> for HeaderValue {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderValueError;

impl core::fmt::Display for HeaderValueError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("invalid HTTP header value")
    }
}

impl core::error::Error for HeaderValueError {}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeaderMap(Vec<(HeaderName, HeaderValue)>);

impl HeaderMap {
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn insert(&mut self, name: impl Into<HeaderName>, value: HeaderValue) {
        let name = name.into();
        if let Some((_, existing)) = self
            .0
            .iter_mut()
            .find(|(existing, _)| existing.0.eq_ignore_ascii_case(name.0))
        {
            *existing = value;
            return;
        }
        self.0.push((name, value));
    }

    pub fn append(&mut self, name: impl Into<HeaderName>, value: HeaderValue) {
        let name = name.into();
        self.0.push((name, value));
    }

    #[must_use]
    pub fn contains_key(&self, name: HeaderName) -> bool {
        self.0
            .iter()
            .any(|(existing, _)| existing.0.eq_ignore_ascii_case(name.0))
    }

    #[must_use]
    pub fn get(&self, name: impl HeaderLookup) -> Option<&HeaderValue> {
        let name = name.as_header_name();
        self.0
            .iter()
            .find(|(existing, _)| existing.0.eq_ignore_ascii_case(name))
            .map(|(_, value)| value)
    }

    pub fn remove(&mut self, name: HeaderName) {
        self.0
            .retain(|(existing, _)| !existing.0.eq_ignore_ascii_case(name.0));
    }

    pub fn get_all(&self, name: HeaderName) -> impl Iterator<Item = &HeaderValue> {
        self.0
            .iter()
            .filter(move |(existing, _)| existing.0.eq_ignore_ascii_case(name.0))
            .map(|(_, value)| value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (HeaderName, &HeaderValue)> {
        self.0.iter().map(|(name, value)| (*name, value))
    }
}

pub trait HeaderLookup {
    fn as_header_name(&self) -> &str;
}

impl HeaderLookup for HeaderName {
    fn as_header_name(&self) -> &str {
        self.0
    }
}

impl HeaderLookup for &str {
    fn as_header_name(&self) -> &str {
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusCode(u16);

impl StatusCode {
    pub const OK: Self = Self(200);
    pub const CONFLICT: Self = Self(409);
    pub const FORBIDDEN: Self = Self(403);

    #[must_use]
    pub const fn from_u16(status: u16) -> Self {
        Self(status)
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn is_success(self) -> bool {
        self.0 >= 200 && self.0 < 300
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RequestBody {
    #[default]
    Empty,
    Json(Vec<u8>),
    Raw(Vec<u8>),
}

impl RequestBody {
    #[must_use]
    pub fn json(payload: impl Into<Vec<u8>>) -> Self {
        Self::Json(payload.into())
    }

    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Empty => None,
            Self::Json(bytes) | Self::Raw(bytes) => Some(bytes.as_slice()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSpec {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: RequestBody,
    pub cookies: Vec<CookiePair>,
}

impl RequestSpec {
    #[must_use]
    pub fn new(method: Method, url: Url) -> Self {
        Self {
            method,
            url,
            headers: HeaderMap::new(),
            body: RequestBody::Empty,
            cookies: Vec::new(),
        }
    }

    pub fn insert_header(&mut self, name: HeaderName, value: HeaderValue) {
        self.headers.insert(name, value);
    }

    pub fn with_header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.insert_header(name, value);
        self
    }

    /// Sets the request body to `payload` and defaults the `Content-Type` to
    /// `application/json` when one is not already present.
    ///
    /// Returns a `Result` so the signature can stay stable if body validation
    /// is added later; today the operation cannot fail.
    pub fn with_json_body(mut self, payload: impl Into<Vec<u8>>) -> Result<Self, ICloudError> {
        self.body = RequestBody::json(payload);
        if !self.headers.contains_key(header::CONTENT_TYPE) {
            self.headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
        }
        Ok(self)
    }

    pub fn attach_cookies(
        &mut self,
        cookies: impl IntoIterator<Item = CookiePair>,
    ) -> Result<(), ICloudError> {
        self.cookies = cookies.into_iter().collect();
        if let Some(value) = self.cookie_header_value()? {
            self.headers.insert(header::COOKIE, value);
        } else {
            self.headers.remove(header::COOKIE);
        }
        Ok(())
    }

    pub fn cookie_header_value(&self) -> Result<Option<HeaderValue>, ICloudError> {
        if self.cookies.is_empty() {
            return Ok(None);
        }

        let rendered = self
            .cookies
            .iter()
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect::<Vec<_>>()
            .join("; ");
        let header = HeaderValue::parse(rendered.as_str()).map_err(|error| {
            ICloudError::InvalidHeaderValue {
                name: "cookie",
                message: error.to_string(),
            }
        })?;
        Ok(Some(header))
    }
}

#[cfg(test)]
mod tests {
    use crate::{HeaderValue, Method, header::CONTENT_TYPE};
    use icloud_types::{ICloudApiErrorCode, ICloudError};
    use pretty_assertions::assert_eq;

    use super::{RequestSpec, ResponseParts};

    #[test]
    fn json_body_defaults_content_type() {
        let request = RequestSpec::new(
            Method::Post,
            url::Url::parse("https://example.com/request").expect("valid url"),
        )
        .with_json_body(r#"{"hello":"world"}"#)
        .expect("json body should build");

        assert_eq!(
            request.headers.get(CONTENT_TYPE).expect("content type"),
            "application/json",
        );
    }

    #[test]
    fn json_body_keeps_existing_content_type() {
        let request = RequestSpec::new(
            Method::Post,
            url::Url::parse("https://example.com/request").expect("valid url"),
        )
        .with_header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
        .with_json_body(r#"{"hello":"world"}"#)
        .expect("json body should build");

        assert_eq!(
            request.headers.get(CONTENT_TYPE).expect("content type"),
            "text/plain",
        );
    }

    #[test]
    fn ensure_success_interprets_pyicloud_json_error_fields() {
        let response = ResponseParts::new(
            crate::StatusCode::OK,
            crate::HeaderMap::new(),
            br#"{"errorMessage":"Denied","errorCode":"ACCESS_DENIED"}"#.to_vec(),
        );

        let error = response
            .ensure_success()
            .expect_err("json error body should fail");

        let ICloudError::Api(api_error) = error else {
            panic!("expected API error");
        };
        assert_eq!(api_error.code, Some(ICloudApiErrorCode::AccessDenied));
        assert_eq!(
            api_error.reason,
            "Denied. Please wait a few minutes then try again. The remote servers might be trying to throttle requests."
        );
    }

    #[test]
    fn ensure_success_maps_service_not_activated_json_errors() {
        let response = ResponseParts::new(
            crate::StatusCode::OK,
            crate::HeaderMap::new(),
            br#"{"reason":"missing zone","serverErrorCode":"ZONE_NOT_FOUND"}"#.to_vec(),
        );

        let error = response
            .ensure_success()
            .expect_err("service setup error should fail");

        let ICloudError::ServiceNotActivated(api_error) = error else {
            panic!("expected service activation error");
        };
        assert_eq!(api_error.code, Some(ICloudApiErrorCode::ZoneNotFound));
        assert_eq!(
            api_error.reason,
            "Please log into https://icloud.com/ to manually finish setting up your iCloud service"
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseParts {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl ResponseParts {
    #[must_use]
    pub fn new(status: StatusCode, headers: HeaderMap, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    pub fn json_value(&self) -> Result<JsonValue, ICloudError> {
        let body = str::from_utf8(self.body.as_slice()).map_err(JsonError::from)?;
        let raw = RawJson::parse(body).map_err(JsonError::from)?;
        let value = JsonValue::try_from(raw.value()).map_err(JsonError::from)?;
        Ok(value)
    }

    pub fn header_str(&self, name: &'static str) -> Result<Option<&str>, ICloudError> {
        let Some(value) = self.headers.get(name) else {
            return Ok(None);
        };
        value
            .to_str()
            .map(Some)
            .map_err(|error| ICloudError::InvalidHeaderValue {
                name,
                message: error.to_string(),
            })
    }

    pub fn required_header_str(&self, name: &'static str) -> Result<&str, ICloudError> {
        self.header_str(name)?
            .ok_or(ICloudError::MissingHeader { name })
    }

    pub fn ensure_success(&self) -> Result<(), ICloudError> {
        if let Some(error) = parse_json_api_error(self) {
            return Err(error);
        }

        if self.status.is_success() {
            Ok(())
        } else {
            Err(ICloudError::UnexpectedStatus {
                status: self.status.as_u16(),
            })
        }
    }

    /// Parses the body once and validates it in the same pass.
    ///
    /// This is the entry point JSON parsers should use instead of calling
    /// [`ensure_success`](Self::ensure_success) followed by a separate parse:
    /// it decodes the body a single time, surfaces any embedded iCloud API
    /// error, and otherwise enforces a successful HTTP status.
    pub(crate) fn json_value_checked(&self) -> Result<JsonValue, ICloudError> {
        let value = match self.json_value() {
            Ok(value) => value,
            Err(error) => {
                return if self.status.is_success() {
                    Err(error)
                } else {
                    Err(ICloudError::UnexpectedStatus {
                        status: self.status.as_u16(),
                    })
                };
            }
        };

        if let JsonValue::Object(payload) = &value
            && let Some(error) = api_error_from_object(payload)
        {
            return Err(error);
        }

        if self.status.is_success() {
            Ok(value)
        } else {
            Err(ICloudError::UnexpectedStatus {
                status: self.status.as_u16(),
            })
        }
    }
}

fn parse_json_api_error(response: &ResponseParts) -> Option<ICloudError> {
    let JsonValue::Object(payload) = response.json_value().ok()? else {
        return None;
    };
    api_error_from_object(&payload)
}

fn api_error_from_object(payload: &JsonObject) -> Option<ICloudError> {
    let reason = api_error_reason(payload)?;
    let code = api_error_code(payload);
    let mut error = ICloudApiError::new(code, reason);

    match error.code.as_ref() {
        Some(ICloudApiErrorCode::AuthenticationFailed | ICloudApiErrorCode::ZoneNotFound) => {
            error.reason = "Please log into https://icloud.com/ to manually finish setting up your iCloud service"
                .to_owned();
            Some(ICloudError::ServiceNotActivated(error))
        }
        Some(ICloudApiErrorCode::AccessDenied) => {
            error.reason = format!(
                "{}. Please wait a few minutes then try again. The remote servers might be trying to throttle requests.",
                error.reason.trim_end_matches('.')
            );
            Some(ICloudError::Api(error))
        }
        Some(ICloudApiErrorCode::Status(421 | 450 | 500)) => {
            error.reason = "Authentication required for Account.".to_owned();
            Some(ICloudError::Api(error))
        }
        Some(_) | None => Some(ICloudError::Api(error)),
    }
}

fn api_error_reason(payload: &JsonObject) -> Option<String> {
    string_field(payload, "errorMessage")
        .or_else(|| string_field(payload, "reason"))
        .or_else(|| string_field(payload, "errorReason"))
        .or_else(|| match payload.get("error") {
            Some(JsonValue::String(value)) if !value.is_empty() => Some(value.clone()),
            Some(JsonValue::Null) | None => None,
            Some(_) => Some("Unknown reason".to_owned()),
        })
}

fn api_error_code(payload: &JsonObject) -> Option<ICloudApiErrorCode> {
    payload
        .get("errorCode")
        .or_else(|| payload.get("serverErrorCode"))
        .and_then(api_error_code_value)
}

fn api_error_code_value(value: &JsonValue) -> Option<ICloudApiErrorCode> {
    match value {
        JsonValue::String(value) if !value.is_empty() => {
            Some(ICloudApiErrorCode::from_string(value.clone()))
        }
        JsonValue::Number(value) => value
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
            .map(ICloudApiErrorCode::Status)
            .or_else(|| Some(ICloudApiErrorCode::Other(value.to_string()))),
        JsonValue::String(_)
        | JsonValue::Bool(_)
        | JsonValue::Array(_)
        | JsonValue::Object(_)
        | JsonValue::Null => None,
    }
}

fn string_field(payload: &JsonObject, name: &str) -> Option<String> {
    match payload.get(name) {
        Some(JsonValue::String(value)) if !value.is_empty() => Some(value.clone()),
        _ => None,
    }
}
