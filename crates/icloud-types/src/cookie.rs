use crate::prelude::*;

use alloc::collections::BTreeMap;
use core::cmp::Ordering;

use time::{OffsetDateTime, format_description::well_known::Rfc2822};
use url::Url;

use crate::ICloudError;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "std")]
use http::{HeaderMap, header::SET_COOKIE};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CookieIdentity {
    pub domain: String,
    pub path: String,
    pub name: String,
    pub host_only: bool,
}

impl PartialEq for CookieIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.domain == other.domain && self.path == other.path && self.name == other.name
    }
}

impl Eq for CookieIdentity {}

impl PartialOrd for CookieIdentity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CookieIdentity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.domain
            .cmp(&other.domain)
            .then_with(|| self.path.cmp(&other.path))
            .then_with(|| self.name.cmp(&other.name))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StoredCookie {
    pub identity: CookieIdentity,
    pub value: String,
    pub secure: bool,
    pub http_only: bool,
    pub expires_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CookiePair {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CookieJar {
    #[cfg_attr(feature = "serde", serde(with = "cookie_map_serde"))]
    cookies: BTreeMap<CookieIdentity, StoredCookie>,
}

#[cfg(feature = "serde")]
mod cookie_map_serde {
    use alloc::{collections::BTreeMap, vec::Vec};

    use serde::{Deserialize, Serialize, Serializer, de::Deserializer};

    use super::{CookieIdentity, StoredCookie};

    pub(crate) fn serialize<S>(
        cookies: &BTreeMap<CookieIdentity, StoredCookie>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        cookies.values().collect::<Vec<_>>().serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<CookieIdentity, StoredCookie>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Vec::<StoredCookie>::deserialize(deserializer)?
            .into_iter()
            .map(|cookie| (cookie.identity.clone(), cookie))
            .collect())
    }
}

impl CookieJar {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "std")]
    pub fn apply_response_headers(
        &mut self,
        request_url: &Url,
        headers: &HeaderMap,
    ) -> Result<(), ICloudError> {
        for value in &headers.get_all(SET_COOKIE) {
            let raw = value.to_str().map_err(|error| ICloudError::CookieParse {
                message: error.to_string(),
            })?;
            self.apply_set_cookie(request_url, raw)?;
        }
        Ok(())
    }

    pub fn apply_set_cookie(
        &mut self,
        request_url: &Url,
        set_cookie_value: &str,
    ) -> Result<(), ICloudError> {
        let parsed = ParsedCookie::parse(set_cookie_value)?;
        let identity = cookie_identity(request_url, &parsed)?;
        if should_remove_cookie(&parsed) {
            self.cookies.remove(&identity);
            return Ok(());
        }

        let expires_at = cookie_expiration(&parsed);
        let stored = StoredCookie {
            identity: identity.clone(),
            value: parsed.value,
            secure: parsed.secure,
            http_only: parsed.http_only,
            expires_at,
        };
        self.cookies.insert(identity, stored);
        Ok(())
    }

    #[must_use]
    pub fn cookies_for_url(&self, url: &Url) -> Vec<CookiePair> {
        let mut cookies = self
            .cookies
            .values()
            .filter(|cookie| cookie.matches(url))
            .collect::<Vec<_>>();
        cookies.sort_by(|left, right| {
            right
                .identity
                .path
                .len()
                .cmp(&left.identity.path.len())
                .then_with(|| left.identity.name.cmp(&right.identity.name))
        });
        cookies
            .into_iter()
            .map(|cookie| CookiePair {
                name: cookie.identity.name.clone(),
                value: cookie.value.clone(),
            })
            .collect()
    }

    #[must_use]
    pub fn cookie_value(&self, name: &str) -> Option<&str> {
        self.cookies
            .values()
            .find(|cookie| cookie.identity.name == name && !cookie.is_expired())
            .map(|cookie| cookie.value.as_str())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }
}

impl StoredCookie {
    fn matches(&self, url: &Url) -> bool {
        if self.is_expired() {
            return false;
        }

        if self.secure && url.scheme() != "https" {
            return false;
        }

        let Some(host) = url.host_str() else {
            return false;
        };
        let host = host.to_ascii_lowercase();
        if self.identity.host_only {
            if host != self.identity.domain {
                return false;
            }
        } else if host != self.identity.domain
            && !host.ends_with(&format!(".{}", self.identity.domain))
        {
            return false;
        }

        let request_path = normalized_path(url);
        path_matches(self.identity.path.as_str(), request_path.as_str())
    }

    fn is_expired(&self) -> bool {
        self.expires_at
            .zip(current_utc())
            .is_some_and(|(expires_at, now)| expires_at <= now)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedCookie {
    name: String,
    value: String,
    domain: Option<String>,
    path: Option<String>,
    secure: bool,
    http_only: bool,
    max_age_seconds: Option<i64>,
    expires_at: Option<OffsetDateTime>,
}

impl ParsedCookie {
    fn parse(value: &str) -> Result<Self, ICloudError> {
        let mut parts = value.split(';');
        let pair = parts.next().ok_or_else(|| ICloudError::CookieParse {
            message: "set-cookie header is empty".to_owned(),
        })?;
        let (name, cookie_value) =
            pair.split_once('=')
                .ok_or_else(|| ICloudError::CookieParse {
                    message: "set-cookie header does not contain a name/value pair".to_owned(),
                })?;
        let name = name.trim();
        if name.is_empty() {
            return Err(ICloudError::CookieParse {
                message: "cookie name is empty".to_owned(),
            });
        }

        let mut cookie = Self {
            name: name.to_owned(),
            value: cookie_value.trim().to_owned(),
            domain: None,
            path: None,
            secure: false,
            http_only: false,
            max_age_seconds: None,
            expires_at: None,
        };

        for attribute in parts {
            let attribute = attribute.trim();
            if attribute.eq_ignore_ascii_case("secure") {
                cookie.secure = true;
            } else if attribute.eq_ignore_ascii_case("httponly") {
                cookie.http_only = true;
            } else if let Some((name, value)) = attribute.split_once('=') {
                let name = name.trim();
                let value = value.trim();
                if name.eq_ignore_ascii_case("domain") {
                    cookie.domain = Some(value.to_owned());
                } else if name.eq_ignore_ascii_case("path") {
                    cookie.path = Some(value.to_owned());
                } else if name.eq_ignore_ascii_case("max-age") {
                    cookie.max_age_seconds = value.parse::<i64>().ok();
                } else if name.eq_ignore_ascii_case("expires") {
                    cookie.expires_at = OffsetDateTime::parse(value, &Rfc2822).ok();
                }
            }
        }

        Ok(cookie)
    }
}

fn cookie_identity(
    request_url: &Url,
    cookie: &ParsedCookie,
) -> Result<CookieIdentity, ICloudError> {
    let request_host = request_url
        .host_str()
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: "request URL does not contain a host".to_owned(),
        })?
        .to_ascii_lowercase();
    let (domain, host_only) = match cookie.domain.as_deref() {
        Some(domain) => {
            let domain = normalized_cookie_domain(domain)?;
            if !domain_matches_host(request_host.as_str(), domain.as_str()) || !domain.contains('.')
            {
                return Err(ICloudError::CookieParse {
                    message: format!(
                        "cookie domain `{domain}` is invalid for request host `{request_host}`"
                    ),
                });
            }
            (domain, false)
        }
        None => (request_host, true),
    };

    Ok(CookieIdentity {
        domain,
        path: cookie
            .path
            .as_deref()
            .map(str::to_owned)
            .unwrap_or_else(|| default_cookie_path(request_url)),
        name: cookie.name.clone(),
        host_only,
    })
}

fn normalized_cookie_domain(domain: &str) -> Result<String, ICloudError> {
    let domain = domain
        .trim()
        .trim_start_matches('.')
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if domain.is_empty() {
        return Err(ICloudError::CookieParse {
            message: "cookie domain attribute is empty".to_owned(),
        });
    }
    Ok(domain)
}

fn domain_matches_host(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

fn default_cookie_path(url: &Url) -> String {
    let path = normalized_path(url);
    if path == "/" {
        return "/".to_owned();
    }

    match path.rfind('/') {
        Some(0) | None => "/".to_owned(),
        Some(index) => path[..index].to_owned(),
    }
}

fn normalized_path(url: &Url) -> String {
    let path = url.path();
    if path.is_empty() {
        "/".to_owned()
    } else {
        path.to_owned()
    }
}

fn should_remove_cookie(cookie: &ParsedCookie) -> bool {
    cookie.max_age_seconds.is_some_and(|duration| duration <= 0)
        || cookie
            .expires_at
            .zip(current_utc())
            .is_some_and(|(expires, now)| expires <= now)
}

fn cookie_expiration(cookie: &ParsedCookie) -> Option<OffsetDateTime> {
    cookie
        .max_age_seconds
        .and_then(|seconds| current_utc().map(|now| now + time::Duration::seconds(seconds)))
        .or(cookie.expires_at)
}

#[cfg(feature = "std")]
fn current_utc() -> Option<OffsetDateTime> {
    Some(OffsetDateTime::now_utc())
}

#[cfg(not(feature = "std"))]
fn current_utc() -> Option<OffsetDateTime> {
    None
}

fn path_matches(cookie_path: &str, request_path: &str) -> bool {
    if request_path == cookie_path {
        return true;
    }

    if !request_path.starts_with(cookie_path) {
        return false;
    }

    cookie_path.ends_with('/')
        || request_path
            .as_bytes()
            .get(cookie_path.len())
            .is_some_and(|separator| *separator == b'/')
}

#[cfg(test)]
mod tests {
    use http::HeaderValue;

    use super::*;

    #[test]
    fn host_only_cookie_stays_on_original_host() {
        let mut jar = CookieJar::new();
        let request_url = Url::parse("https://setup.icloud.com/setup/ws/1/accountLogin")
            .expect("valid request url");
        let headers = HeaderMap::from_iter([(
            SET_COOKIE,
            HeaderValue::from_static("session=abc; Path=/; HttpOnly; Secure"),
        )]);

        jar.apply_response_headers(&request_url, &headers)
            .expect("cookie should be stored");

        let same_host = Url::parse("https://setup.icloud.com/setup/ws/1/accountLogin")
            .expect("valid request url");
        assert_eq!(
            jar.cookies_for_url(&same_host),
            vec![CookiePair {
                name: "session".to_owned(),
                value: "abc".to_owned(),
            }]
        );

        let other_host =
            Url::parse("https://idmsa.apple.com/appleauth/auth/signin").expect("valid request url");
        assert!(jar.cookies_for_url(&other_host).is_empty());
    }

    #[test]
    fn domain_cookie_matches_subdomains() {
        let mut jar = CookieJar::new();
        let request_url =
            Url::parse("https://idmsa.apple.com/appleauth/auth/signin").expect("valid request url");
        jar.apply_set_cookie(
            &request_url,
            "widget=ready; Domain=.apple.com; Path=/; Secure; HttpOnly",
        )
        .expect("cookie should be parsed");

        let nested = Url::parse("https://portal.apple.com/session").expect("valid url");
        assert_eq!(
            jar.cookies_for_url(&nested),
            vec![CookiePair {
                name: "widget".to_owned(),
                value: "ready".to_owned(),
            }]
        );
    }

    #[test]
    fn rejects_cookie_for_unrelated_domain() {
        let mut jar = CookieJar::new();
        let request_url =
            Url::parse("https://cvws.icloud-content.com/download/file").expect("valid request url");

        let error = jar
            .apply_set_cookie(
                &request_url,
                "session=abc; Domain=.icloud.com; Path=/; Secure; HttpOnly",
            )
            .expect_err("unrelated domain cookie should be rejected");

        assert!(matches!(error, ICloudError::CookieParse { .. }));
        assert!(jar.is_empty());
    }

    #[test]
    fn rejects_cookie_for_top_level_domain() {
        let mut jar = CookieJar::new();
        let request_url =
            Url::parse("https://setup.icloud.com/setup/ws/1/accountLogin").expect("valid url");

        let error = jar
            .apply_set_cookie(&request_url, "session=abc; Domain=.com; Path=/; Secure")
            .expect_err("top-level domain cookie should be rejected");

        assert!(matches!(error, ICloudError::CookieParse { .. }));
        assert!(jar.is_empty());
    }

    #[test]
    fn zero_max_age_deletes_cookie() {
        let mut jar = CookieJar::new();
        let request_url =
            Url::parse("https://setup.icloud.com/setup/ws/1/accountLogin").expect("valid url");
        jar.apply_set_cookie(&request_url, "session=abc; Path=/; Secure")
            .expect("cookie should be parsed");
        jar.apply_set_cookie(&request_url, "session=; Max-Age=0; Path=/; Secure")
            .expect("cookie should be removed");

        assert!(jar.is_empty());
    }

    #[test]
    fn domain_cookie_replaces_matching_host_only_cookie() {
        let mut jar = CookieJar::new();
        let request_url =
            Url::parse("https://setup.icloud.com/setup/ws/1/accountLogin").expect("valid url");
        jar.apply_set_cookie(&request_url, "session=old; Path=/; Secure")
            .expect("host-only cookie should be parsed");
        jar.apply_set_cookie(
            &request_url,
            "session=new; Domain=setup.icloud.com; Path=/; Secure",
        )
        .expect("domain cookie should replace host-only cookie");

        assert_eq!(
            jar.cookies_for_url(&request_url),
            vec![CookiePair {
                name: "session".to_owned(),
                value: "new".to_owned(),
            }]
        );
    }

    #[test]
    fn domain_delete_removes_matching_host_only_cookie() {
        let mut jar = CookieJar::new();
        let request_url =
            Url::parse("https://setup.icloud.com/setup/ws/1/accountLogin").expect("valid url");
        jar.apply_set_cookie(&request_url, "session=abc; Path=/; Secure")
            .expect("host-only cookie should be parsed");
        jar.apply_set_cookie(
            &request_url,
            "session=; Domain=setup.icloud.com; Max-Age=0; Path=/; Secure",
        )
        .expect("domain delete should remove matching host-only cookie");

        assert!(jar.is_empty());
    }

    #[test]
    fn empty_cookie_value_is_preserved_without_expiration() {
        let mut jar = CookieJar::new();
        let request_url =
            Url::parse("https://setup.icloud.com/setup/ws/1/accountLogin").expect("valid url");

        jar.apply_set_cookie(&request_url, "session=; Path=/; Secure")
            .expect("empty cookie should be preserved");

        assert_eq!(
            jar.cookies_for_url(&request_url),
            vec![CookiePair {
                name: "session".to_owned(),
                value: String::new(),
            }]
        );
    }

    #[test]
    fn longer_path_cookie_precedes_root_cookie() {
        let mut jar = CookieJar::new();
        let request_url =
            Url::parse("https://setup.icloud.com/setup/ws/1/accountLogin").expect("valid url");
        jar.apply_set_cookie(&request_url, "session=root; Path=/; Secure")
            .expect("root cookie should be parsed");
        jar.apply_set_cookie(&request_url, "session=narrow; Path=/setup/ws/1/; Secure")
            .expect("narrow cookie should be parsed");

        assert_eq!(
            jar.cookies_for_url(&request_url),
            vec![
                CookiePair {
                    name: "session".to_owned(),
                    value: "narrow".to_owned(),
                },
                CookiePair {
                    name: "session".to_owned(),
                    value: "root".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn expired_cookie_is_removed_even_with_non_empty_value() {
        let mut jar = CookieJar::new();
        let request_url =
            Url::parse("https://setup.icloud.com/setup/ws/1/accountLogin").expect("valid url");
        jar.apply_set_cookie(&request_url, "session=abc; Path=/; Secure")
            .expect("cookie should be parsed");
        jar.apply_set_cookie(
            &request_url,
            "session=abc; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Path=/; Secure",
        )
        .expect("expiry cookie should be parsed");

        assert!(jar.is_empty());
    }

    #[test]
    fn cookie_path_does_not_match_sibling_prefixes() {
        let mut jar = CookieJar::new();
        let request_url = Url::parse("https://setup.icloud.com/foo/index").expect("valid url");
        jar.apply_set_cookie(&request_url, "session=abc; Path=/foo; Secure")
            .expect("cookie should be parsed");

        let child = Url::parse("https://setup.icloud.com/foo/bar").expect("valid url");
        assert_eq!(
            jar.cookies_for_url(&child),
            vec![CookiePair {
                name: "session".to_owned(),
                value: "abc".to_owned(),
            }]
        );

        let sibling = Url::parse("https://setup.icloud.com/foobar").expect("valid url");
        assert!(jar.cookies_for_url(&sibling).is_empty());
    }

    #[test]
    fn default_cookie_path_matches_parent_without_trailing_slash() {
        let mut jar = CookieJar::new();
        let request_url = Url::parse("https://setup.icloud.com/foo/bar").expect("valid url");
        jar.apply_set_cookie(&request_url, "session=abc; Secure")
            .expect("cookie should be parsed");

        let parent = Url::parse("https://setup.icloud.com/foo").expect("valid url");
        assert_eq!(
            jar.cookies_for_url(&parent),
            vec![CookiePair {
                name: "session".to_owned(),
                value: "abc".to_owned(),
            }]
        );
    }

    #[test]
    fn expired_cookies_are_not_returned_from_restored_snapshots() {
        let request_url =
            Url::parse("https://setup.icloud.com/setup/ws/1/accountLogin").expect("valid url");
        let identity = cookie_identity(
            &request_url,
            &ParsedCookie::parse("session=abc; Path=/; Secure").expect("valid cookie"),
        )
        .expect("identity should derive");
        let expired = StoredCookie {
            identity: identity.clone(),
            value: "abc".to_owned(),
            secure: true,
            http_only: false,
            expires_at: Some(OffsetDateTime::now_utc() - time::Duration::hours(1)),
        };
        let mut jar = CookieJar::new();
        jar.cookies.insert(identity, expired);

        assert!(
            jar.cookies_for_url(&request_url).is_empty(),
            "expired restored cookies should be filtered"
        );
        assert_eq!(jar.cookie_value("session"), None);
    }
}
