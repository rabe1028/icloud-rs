use crate::prelude::*;

use secrecy::{ExposeSecret, SecretString};

use crate::{CookieJar, ICloudError, WebServiceMap};

fn secret_eq(a: &SecretString, b: &SecretString) -> bool {
    a.expose_secret() == b.expose_secret()
}

fn secret_opt_eq(a: &Option<SecretString>, b: &Option<SecretString>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => secret_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub struct Credentials {
    pub apple_id: String,
    pub password: SecretString,
}

impl Credentials {
    #[must_use]
    pub fn new(apple_id: impl Into<String>, password: SecretString) -> Self {
        Self {
            apple_id: apple_id.into(),
            password,
        }
    }

    #[must_use]
    pub fn expose_password(&self) -> &str {
        secrecy::ExposeSecret::expose_secret(&self.password)
    }
}

#[derive(Debug, Clone)]
pub struct ChallengeHeaders {
    pub scnt: SecretString,
    pub session_id: String,
}

impl PartialEq for ChallengeHeaders {
    fn eq(&self, other: &Self) -> bool {
        self.session_id == other.session_id && secret_eq(&self.scnt, &other.scnt)
    }
}

impl Eq for ChallengeHeaders {}

#[derive(Debug, Clone, Default)]
pub struct SessionSeed {
    pub session_token: Option<SecretString>,
    pub account_country_code: Option<String>,
    pub trust_token: Option<SecretString>,
}

impl PartialEq for SessionSeed {
    fn eq(&self, other: &Self) -> bool {
        secret_opt_eq(&self.session_token, &other.session_token)
            && self.account_country_code == other.account_country_code
            && secret_opt_eq(&self.trust_token, &other.trust_token)
    }
}

impl Eq for SessionSeed {}

impl SessionSeed {
    pub fn merge(&mut self, newer: SessionSeed) {
        if newer.session_token.is_some() {
            self.session_token = newer.session_token;
        }
        if newer.account_country_code.is_some() {
            self.account_country_code = newer.account_country_code;
        }
        if newer.trust_token.is_some() {
            self.trust_token = newer.trust_token;
        }
    }

    pub fn try_into_established(
        self,
        challenge: Option<ChallengeHeaders>,
    ) -> Result<EstablishedSession, ICloudError> {
        let session_token = self.session_token.ok_or(ICloudError::MissingSessionField {
            name: "X-Apple-Session-Token",
        })?;
        let account_country_code =
            self.account_country_code
                .ok_or(ICloudError::MissingSessionField {
                    name: "X-Apple-ID-Account-Country",
                })?;
        Ok(EstablishedSession {
            session_token,
            account_country_code,
            trust_token: self.trust_token,
            challenge,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTwoFactorAuth {
    pub apple_id: String,
    pub challenge: ChallengeHeaders,
    pub seed: SessionSeed,
}

#[derive(Debug, Clone)]
pub struct EstablishedSession {
    pub session_token: SecretString,
    pub account_country_code: String,
    pub trust_token: Option<SecretString>,
    pub challenge: Option<ChallengeHeaders>,
}

impl PartialEq for EstablishedSession {
    fn eq(&self, other: &Self) -> bool {
        secret_eq(&self.session_token, &other.session_token)
            && self.account_country_code == other.account_country_code
            && secret_opt_eq(&self.trust_token, &other.trust_token)
            && self.challenge == other.challenge
    }
}

impl Eq for EstablishedSession {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountBootstrap {
    pub dsid: String,
    pub hsa_challenge_required: bool,
    pub is_trusted_browser: bool,
    pub account_name: Option<String>,
    pub webservices: WebServiceMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedSession {
    pub established: EstablishedSession,
    pub bootstrap: AccountBootstrap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthState {
    LoggedOut,
    PendingTwoFactor(PendingTwoFactorAuth),
    AuthenticatedPreBootstrap(EstablishedSession),
    Authenticated(AuthenticatedSession),
}

/// Serializable persistence is intentionally not provided by the core crates.
/// Applications should decide how to encode, encrypt, and store snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientStateSnapshot {
    pub client_id: String,
    pub cookie_jar: CookieJar,
    pub auth_state: AuthState,
}
