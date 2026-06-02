use alloc::{borrow::ToOwned, format, string::String};

use url::Url;
#[cfg(feature = "std")]
use uuid::Uuid;

use crate::ICloudError;

const DEFAULT_USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) ",
    "AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15"
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointSet {
    pub auth_endpoint: Url,
    pub setup_endpoint: Url,
    pub home_endpoint: Url,
}

impl EndpointSet {
    pub fn production() -> Result<Self, ICloudError> {
        Ok(Self {
            auth_endpoint: Url::parse("https://idmsa.apple.com/appleauth/auth/")?,
            setup_endpoint: Url::parse("https://setup.icloud.com/setup/ws/1/")?,
            home_endpoint: Url::parse("https://www.icloud.com")?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientProfile {
    pub widget_key: String,
    pub oauth_client_id: String,
    pub oauth_client_type: String,
    pub oauth_redirect_uri: Url,
    pub oauth_require_grant_code: bool,
    pub oauth_response_mode: String,
    pub oauth_response_type: String,
    pub user_agent: String,
    pub origin: Url,
    pub referer: Url,
    pub accept_language: String,
}

impl ClientProfile {
    pub fn pyicloud(widget_key: impl Into<String>) -> Result<Self, ICloudError> {
        let origin = Url::parse("https://www.icloud.com")?;
        Ok(Self {
            widget_key: widget_key.into(),
            oauth_client_id: "d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d"
                .to_owned(),
            oauth_client_type: "firstPartyAuth".to_owned(),
            oauth_redirect_uri: origin.clone(),
            oauth_require_grant_code: true,
            oauth_response_mode: "web_message".to_owned(),
            oauth_response_type: "code".to_owned(),
            user_agent: DEFAULT_USER_AGENT.to_owned(),
            referer: origin.clone(),
            origin,
            accept_language: "en-US,en;q=0.9".to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientContext {
    pub endpoints: EndpointSet,
    pub profile: ClientProfile,
}

impl ClientContext {
    pub fn production(widget_key: impl Into<String>) -> Result<Self, ICloudError> {
        Ok(Self {
            endpoints: EndpointSet::production()?,
            profile: ClientProfile::pyicloud(widget_key)?,
        })
    }

    pub fn auth_url(&self, path: &str) -> Result<Url, ICloudError> {
        join_path(&self.endpoints.auth_endpoint, path)
    }

    pub fn setup_url(&self, path: &str) -> Result<Url, ICloudError> {
        join_path(&self.endpoints.setup_endpoint, path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientIdentity {
    pub client_id: String,
}

impl ClientIdentity {
    #[must_use]
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
        }
    }

    /// Builds an identity with a generated client id suffix.
    ///
    /// With the `std` feature the suffix is a random UUID v4. Under `no_std`
    /// there is no entropy source, so the suffix falls back to a process-local
    /// monotonic counter: it is unique within a single process run but is
    /// neither random nor stable across restarts. Supply your own id via
    /// [`ClientIdentity::new`] when uniqueness across runs matters.
    #[must_use]
    pub fn random() -> Self {
        Self {
            client_id: format!("auth-{}", random_client_id_suffix()),
        }
    }

    #[must_use]
    pub fn as_header_value(&self) -> &str {
        self.client_id.as_str()
    }
}

#[cfg(feature = "std")]
fn random_client_id_suffix() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(not(feature = "std"))]
fn random_client_id_suffix() -> String {
    use core::sync::atomic::{AtomicU32, Ordering};

    static NEXT_ID: AtomicU32 = AtomicU32::new(1);
    format!("no-std-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

fn join_path(base: &Url, path: &str) -> Result<Url, ICloudError> {
    let normalized = path.trim_start_matches('/');
    Ok(base.join(normalized)?)
}
