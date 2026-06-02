use crate::prelude::*;

use secrecy::{ExposeSecret, SecretString};

use crate::{CookieJar, ICloudError, JsonObject, WebServiceMap};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "serde")]
mod secret_serde {
    use alloc::string::String;

    use secrecy::{ExposeSecret, SecretString};
    use serde::{Deserialize, Serializer, de::Deserializer};

    pub(crate) fn serialize<S>(secret: &SecretString, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(secret.expose_secret())
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<SecretString, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(SecretString::from)
    }

    pub(crate) mod option {
        use alloc::string::String;

        use secrecy::{ExposeSecret, SecretString};
        use serde::{Deserialize, Serialize, Serializer, de::Deserializer};

        pub(crate) fn serialize<S>(
            secret: &Option<SecretString>,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            secret
                .as_ref()
                .map(ExposeSecret::expose_secret)
                .serialize(serializer)
        }

        pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<SecretString>, D::Error>
        where
            D: Deserializer<'de>,
        {
            Option::<String>::deserialize(deserializer).map(|secret| secret.map(SecretString::from))
        }
    }
}

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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Credentials {
    pub apple_id: String,
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "secret_serde::serialize",
            deserialize_with = "secret_serde::deserialize"
        )
    )]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ChallengeHeaders {
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "secret_serde::serialize",
            deserialize_with = "secret_serde::deserialize"
        )
    )]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SessionSeed {
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "secret_serde::option::serialize",
            deserialize_with = "secret_serde::option::deserialize"
        )
    )]
    pub session_token: Option<SecretString>,
    pub account_country_code: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "secret_serde::option::serialize",
            deserialize_with = "secret_serde::option::deserialize"
        )
    )]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TrustedPhoneNumberId {
    Number(u64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TrustedPhoneNumber {
    pub id: TrustedPhoneNumberId,
    pub push_mode: Option<String>,
    pub non_fteu: Option<bool>,
    pub obfuscated_number: Option<String>,
    pub number_with_dial_code: Option<String>,
}

impl TrustedPhoneNumber {
    #[must_use]
    pub fn display_label(&self) -> Option<&str> {
        self.number_with_dial_code
            .as_deref()
            .or(self.obfuscated_number.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PendingTwoFactorAuth {
    pub apple_id: String,
    pub challenge: ChallengeHeaders,
    pub seed: SessionSeed,
    pub trusted_phone_numbers: Vec<TrustedPhoneNumber>,
    pub has_trusted_devices: bool,
}

impl PendingTwoFactorAuth {
    #[must_use]
    pub fn delivery_options(&self) -> Vec<AuthDeliveryOption> {
        let mut options = Vec::new();
        if self.has_trusted_devices {
            options.push(AuthDeliveryOption::TrustedDevice);
        }
        options.extend(
            self.trusted_phone_numbers
                .iter()
                .cloned()
                .map(AuthDeliveryOption::TrustedPhoneNumber),
        );
        options
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AuthDeliveryOption {
    TrustedDevice,
    TrustedPhoneNumber(TrustedPhoneNumber),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LegacyTrustedDevice {
    pub device_name: Option<String>,
    pub phone_number: Option<String>,
    pub raw: JsonObject,
}

impl LegacyTrustedDevice {
    #[must_use]
    pub fn display_label(&self) -> Option<&str> {
        self.device_name.as_deref().or(self.phone_number.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AuthStatus {
    LoggedOut,
    PendingAuth(PendingAuthStatus),
    AuthenticatedPreBootstrap,
    Authenticated {
        trusted_session: bool,
        hsa_challenge_required: bool,
        account_name: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PendingAuthStatus {
    pub apple_id: String,
    pub delivery_options: Vec<AuthDeliveryOption>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EstablishedSession {
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "secret_serde::serialize",
            deserialize_with = "secret_serde::deserialize"
        )
    )]
    pub session_token: SecretString,
    pub account_country_code: String,
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "secret_serde::option::serialize",
            deserialize_with = "secret_serde::option::deserialize"
        )
    )]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AccountBootstrap {
    pub dsid: String,
    pub hsa_challenge_required: bool,
    pub is_trusted_browser: bool,
    pub account_name: Option<String>,
    pub webservices: WebServiceMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AuthenticatedSession {
    pub established: EstablishedSession,
    pub bootstrap: AccountBootstrap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AuthState {
    LoggedOut,
    PendingTwoFactor(PendingTwoFactorAuth),
    AuthenticatedPreBootstrap(EstablishedSession),
    Authenticated(AuthenticatedSession),
}

impl AuthState {
    #[must_use]
    pub fn status(&self) -> AuthStatus {
        match self {
            Self::LoggedOut => AuthStatus::LoggedOut,
            Self::PendingTwoFactor(pending) => AuthStatus::PendingAuth(PendingAuthStatus {
                apple_id: pending.apple_id.clone(),
                delivery_options: pending.delivery_options(),
            }),
            Self::AuthenticatedPreBootstrap(_) => AuthStatus::AuthenticatedPreBootstrap,
            Self::Authenticated(session) => AuthStatus::Authenticated {
                trusted_session: session.bootstrap.is_trusted_browser,
                hsa_challenge_required: session.bootstrap.hsa_challenge_required,
                account_name: session.bootstrap.account_name.clone(),
            },
        }
    }
}

/// Serializable snapshot of a client session.
///
/// This value contains secrets: cookies, session tokens, trust tokens, and
/// challenge headers. With the `serde` feature enabled callers can encode and
/// decode it directly, but encrypting and protecting the stored bytes is the
/// caller's responsibility.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ClientStateSnapshot {
    pub client_id: String,
    pub cookie_jar: CookieJar,
    pub auth_state: AuthState,
}
