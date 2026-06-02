use crate::prelude::*;

use core::fmt;

use nojson::JsonParseError;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ICloudApiErrorCode {
    AccessDenied,
    AuthenticationFailed,
    Status(u16),
    ZoneNotFound,
    Other(String),
}

impl ICloudApiErrorCode {
    #[must_use]
    pub fn from_string(value: String) -> Self {
        match value.as_str() {
            "ACCESS_DENIED" => Self::AccessDenied,
            "AUTHENTICATION_FAILED" => Self::AuthenticationFailed,
            "ZONE_NOT_FOUND" => Self::ZoneNotFound,
            _ => Self::Other(value),
        }
    }
}

impl fmt::Display for ICloudApiErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccessDenied => f.write_str("ACCESS_DENIED"),
            Self::AuthenticationFailed => f.write_str("AUTHENTICATION_FAILED"),
            Self::Status(status) => write!(f, "{status}"),
            Self::ZoneNotFound => f.write_str("ZONE_NOT_FOUND"),
            Self::Other(value) => f.write_str(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ICloudApiError {
    pub code: Option<ICloudApiErrorCode>,
    pub reason: String,
}

impl ICloudApiError {
    #[must_use]
    pub fn new(code: Option<ICloudApiErrorCode>, reason: String) -> Self {
        Self { code, reason }
    }
}

impl fmt::Display for ICloudApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.code {
            Some(code) => write!(f, "`{code}`: {}", self.reason),
            None => f.write_str(self.reason.as_str()),
        }
    }
}

impl core::error::Error for ICloudApiError {}

/// Error raised while decoding a JSON response body.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum JsonError {
    #[error("response body is not valid UTF-8")]
    Utf8(#[from] core::str::Utf8Error),
    #[error("failed to parse JSON: {0}")]
    Parse(#[from] JsonParseError),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ICloudError {
    #[error("iCloud API error: {0}")]
    Api(#[source] ICloudApiError),
    #[error("iCloud service is not activated: {0}")]
    ServiceNotActivated(#[source] ICloudApiError),
    #[error("failed to decode JSON response: {0}")]
    Json(#[from] JsonError),
    #[error("failed to parse URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("missing response header `{name}`")]
    MissingHeader { name: &'static str },
    #[error("missing cookie `{name}`")]
    MissingCookie { name: &'static str },
    #[error("missing session field `{name}`")]
    MissingSessionField { name: &'static str },
    #[error("missing required service `{service}` in authenticated bootstrap")]
    MissingService { service: &'static str },
    #[error("invalid response: {message}")]
    InvalidResponse { message: String },
    #[error("unexpected HTTP status {status}")]
    UnexpectedStatus { status: u16 },
    #[error("drive child `{name}` does not exist")]
    DriveChildNotFound { name: String },
    #[error("find my device `{id}` does not exist")]
    FindMyDeviceNotFound { id: String },
    #[error("photo library is not ready: {state}")]
    PhotosLibraryNotReady { state: String },
    #[error("photo version `{kind}` is not available")]
    PhotoVersionUnavailable { kind: String },
    #[error("ubiquity child `{name}` does not exist")]
    UbiquityChildNotFound { name: String },
    #[error("drive container contents unavailable: {status}")]
    DriveContainerUnavailable { status: String },
    #[error("drive download response does not contain a data or package token")]
    DriveDownloadTokenUnavailable,
    #[error("failed to parse set-cookie header: {message}")]
    CookieParse { message: String },
    #[error("invalid header value for `{name}`: {message}")]
    InvalidHeaderValue { name: &'static str, message: String },
}
