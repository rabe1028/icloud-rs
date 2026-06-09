use crate::prelude::*;

use crate::{
    HeaderMap, HeaderName, HeaderValue, Method,
    header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, ORIGIN, REFERER, USER_AGENT},
};
use secrecy::{ExposeSecret, SecretString};

use icloud_types::{
    AccountBootstrap, ChallengeHeaders, ClientContext, ClientIdentity, Credentials,
    EstablishedSession, ICloudError, JsonObject, JsonValue, LegacyTrustedDevice,
    PendingTwoFactorAuth, ServiceId, ServiceStatus, SessionSeed, TrustedPhoneNumber,
    TrustedPhoneNumberId, WebService, WebServiceMap,
};

use crate::{RequestSpec, ResponseParts};

const HEADER_SCNT: &str = "scnt";
const HEADER_SESSION_ID: &str = "x-apple-id-session-id";
const HEADER_SESSION_TOKEN: &str = "x-apple-session-token";
const HEADER_ACCOUNT_COUNTRY: &str = "x-apple-id-account-country";
const HEADER_TRUST_TOKEN: &str = "x-apple-twosv-trust-token";
const HEADER_OAUTH_CLIENT_ID: &str = "x-apple-oauth-client-id";
const HEADER_OAUTH_CLIENT_TYPE: &str = "x-apple-oauth-client-type";
const HEADER_OAUTH_REDIRECT_URI: &str = "x-apple-oauth-redirect-uri";
const HEADER_OAUTH_REQUIRE_GRANT_CODE: &str = "x-apple-oauth-require-grant-code";
const HEADER_OAUTH_RESPONSE_MODE: &str = "x-apple-oauth-response-mode";
const HEADER_OAUTH_RESPONSE_TYPE: &str = "x-apple-oauth-response-type";
const HEADER_OAUTH_STATE: &str = "x-apple-oauth-state";
const HEADER_WIDGET_KEY: &str = "x-apple-widget-key";
const HEADER_REQUESTED_WITH: &str = "x-requested-with";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInOutcome {
    PendingTwoFactor(PendingTwoFactorAuth),
    Authenticated(EstablishedSession),
}

#[derive(Debug, Clone)]
pub struct AuthProtocol<'a> {
    context: &'a ClientContext,
    identity: &'a ClientIdentity,
}

impl<'a> AuthProtocol<'a> {
    #[must_use]
    pub fn new(context: &'a ClientContext, identity: &'a ClientIdentity) -> Self {
        Self { context, identity }
    }

    pub fn sign_in(
        &self,
        credentials: &Credentials,
        trust_token: Option<&str>,
    ) -> Result<RequestSpec, ICloudError> {
        let trust_tokens = trust_token
            .map(crate::json::quote)
            .into_iter()
            .collect::<Vec<_>>()
            .join(",");
        let payload = format!(
            "{{{},{},{},\"trustTokens\":[{}]}}",
            crate::json::string_member("accountName", credentials.apple_id.as_str()),
            crate::json::string_member("password", credentials.expose_password()),
            crate::json::bool_member("rememberMe", true),
            trust_tokens
        );
        let mut request = RequestSpec::new(Method::Post, self.context.auth_url("signin")?);
        request.headers = self.auth_headers()?;
        request.insert_header(
            HeaderName::from_static(HEADER_REQUESTED_WITH),
            HeaderValue::from_static("XMLHttpRequest"),
        );
        request
            .url
            .query_pairs_mut()
            .append_pair("isRememberMeEnabled", "true");
        request = request.with_json_body(payload)?;
        Ok(request)
    }

    pub fn verify_trusted_device_code(
        &self,
        pending: &PendingTwoFactorAuth,
        code: &str,
    ) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(
            Method::Post,
            self.context.auth_url("verify/trusteddevice/securitycode")?,
        );
        request.headers = self.auth_headers()?;
        add_challenge_headers(&mut request.headers, &pending.challenge)?;
        request.insert_header(ACCEPT, HeaderValue::from_static("application/json"));
        request = request.with_json_body(format!(
            "{{\"securityCode\":{{{}}}}}",
            crate::json::string_member("code", code)
        ))?;
        Ok(request)
    }

    pub fn request_sms_code(
        &self,
        pending: &PendingTwoFactorAuth,
        phone_number: &TrustedPhoneNumber,
    ) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(Method::Put, self.context.auth_url("verify/phone")?);
        request.headers = self.auth_headers()?;
        add_challenge_headers(&mut request.headers, &pending.challenge)?;
        request.insert_header(ACCEPT, HeaderValue::from_static("application/json"));
        request = request.with_json_body(format!(
            "{{{},\"mode\":\"sms\"}}",
            phone_number_member(phone_number)
        ))?;
        Ok(request)
    }

    pub fn request_trusted_device_code(
        &self,
        pending: &PendingTwoFactorAuth,
    ) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(
            Method::Put,
            self.context.auth_url("verify/trusteddevice/securitycode")?,
        );
        request.headers = self.auth_headers()?;
        add_challenge_headers(&mut request.headers, &pending.challenge)?;
        Ok(request)
    }

    pub fn verify_sms_code(
        &self,
        pending: &PendingTwoFactorAuth,
        phone_number: &TrustedPhoneNumber,
        code: &str,
    ) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(
            Method::Post,
            self.context.auth_url("verify/phone/securitycode")?,
        );
        request.headers = self.auth_headers()?;
        add_challenge_headers(&mut request.headers, &pending.challenge)?;
        request.insert_header(
            ACCEPT,
            HeaderValue::from_static("application/json, text/plain"),
        );
        request = request.with_json_body(format!(
            "{{{},{},\"mode\":{}}}",
            phone_number_member(phone_number),
            crate::json::member(
                "securityCode",
                format!("{{{}}}", crate::json::string_member("code", code))
            ),
            crate::json::quote(phone_number.push_mode.as_deref().unwrap_or("sms"))
        ))?;
        Ok(request)
    }

    pub fn trust_session(
        &self,
        pending: &PendingTwoFactorAuth,
    ) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(Method::Get, self.context.auth_url("2sv/trust")?);
        request.headers = self.auth_headers()?;
        add_challenge_headers(&mut request.headers, &pending.challenge)?;
        Ok(request)
    }

    pub fn legacy_trusted_devices(&self, dsid: &str) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(Method::Get, self.context.setup_url("listDevices")?);
        request.headers = self.setup_headers()?;
        request.url.query_pairs_mut().append_pair("dsid", dsid);
        Ok(request)
    }

    pub fn send_legacy_verification_code(
        &self,
        dsid: &str,
        device: &LegacyTrustedDevice,
    ) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(
            Method::Post,
            self.context.setup_url("sendVerificationCode")?,
        );
        request.headers = self.setup_headers()?;
        request.url.query_pairs_mut().append_pair("dsid", dsid);
        request = request.with_json_body(crate::json::render_object(&device.raw))?;
        Ok(request)
    }

    pub fn validate_legacy_verification_code(
        &self,
        dsid: &str,
        device: &LegacyTrustedDevice,
        code: &str,
    ) -> Result<RequestSpec, ICloudError> {
        let mut payload = device.raw.clone();
        payload.insert(
            "verificationCode".to_owned(),
            JsonValue::String(code.to_owned()),
        );
        payload.insert("trustBrowser".to_owned(), JsonValue::Bool(true));

        let mut request = RequestSpec::new(
            Method::Post,
            self.context.setup_url("validateVerificationCode")?,
        );
        request.headers = self.setup_headers()?;
        request.url.query_pairs_mut().append_pair("dsid", dsid);
        request = request.with_json_body(crate::json::render_object(&payload))?;
        Ok(request)
    }

    pub fn account_login(&self, session: &EstablishedSession) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(Method::Post, self.context.setup_url("accountLogin")?);
        request.headers = self.setup_headers()?;
        request = request.with_json_body(format!(
            "{{{},{},{},{}}}",
            crate::json::string_member("accountCountryCode", session.account_country_code.as_str()),
            crate::json::string_member("dsWebAuthToken", session.session_token.expose_secret()),
            crate::json::bool_member("extended_login", true),
            crate::json::string_member(
                "trustToken",
                session
                    .trust_token
                    .as_ref()
                    .map(ExposeSecret::expose_secret)
                    .unwrap_or_default()
            )
        ))?;
        Ok(request)
    }

    pub fn validate_session(&self) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(Method::Post, self.context.setup_url("validate")?);
        request.headers = self.setup_headers()?;
        request.body = crate::RequestBody::Raw(b"null".to_vec());
        Ok(request)
    }

    fn auth_headers(&self) -> Result<HeaderMap, ICloudError> {
        let mut headers = HeaderMap::new();
        let origin = header_url_value(&self.context.profile.origin)?;
        let oauth_redirect_uri = header_url_value(&self.context.profile.oauth_redirect_uri)?;
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            ACCEPT_LANGUAGE,
            header_value(
                self.context.profile.accept_language.as_str(),
                ACCEPT_LANGUAGE.as_str(),
            )?,
        );
        headers.insert(
            USER_AGENT,
            header_value(
                self.context.profile.user_agent.as_str(),
                USER_AGENT.as_str(),
            )?,
        );
        headers.insert(ORIGIN, header_value(origin.as_str(), ORIGIN.as_str())?);
        headers.insert(
            REFERER,
            header_value(self.context.profile.referer.as_str(), REFERER.as_str())?,
        );
        headers.insert(
            HeaderName::from_static(HEADER_OAUTH_CLIENT_ID),
            header_value(
                self.context.profile.oauth_client_id.as_str(),
                HEADER_OAUTH_CLIENT_ID,
            )?,
        );
        headers.insert(
            HeaderName::from_static(HEADER_OAUTH_CLIENT_TYPE),
            header_value(
                self.context.profile.oauth_client_type.as_str(),
                HEADER_OAUTH_CLIENT_TYPE,
            )?,
        );
        headers.insert(
            HeaderName::from_static(HEADER_OAUTH_REDIRECT_URI),
            header_value(oauth_redirect_uri.as_str(), HEADER_OAUTH_REDIRECT_URI)?,
        );
        headers.insert(
            HeaderName::from_static(HEADER_OAUTH_REQUIRE_GRANT_CODE),
            HeaderValue::from_static(if self.context.profile.oauth_require_grant_code {
                "true"
            } else {
                "false"
            }),
        );
        headers.insert(
            HeaderName::from_static(HEADER_OAUTH_RESPONSE_MODE),
            header_value(
                self.context.profile.oauth_response_mode.as_str(),
                HEADER_OAUTH_RESPONSE_MODE,
            )?,
        );
        headers.insert(
            HeaderName::from_static(HEADER_OAUTH_RESPONSE_TYPE),
            header_value(
                self.context.profile.oauth_response_type.as_str(),
                HEADER_OAUTH_RESPONSE_TYPE,
            )?,
        );
        headers.insert(
            HeaderName::from_static(HEADER_OAUTH_STATE),
            header_value(self.identity.client_id.as_str(), HEADER_OAUTH_STATE)?,
        );
        headers.insert(
            HeaderName::from_static(HEADER_WIDGET_KEY),
            header_value(self.context.profile.widget_key.as_str(), HEADER_WIDGET_KEY)?,
        );
        Ok(headers)
    }

    fn setup_headers(&self) -> Result<HeaderMap, ICloudError> {
        let mut headers = self.auth_headers()?;
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        Ok(headers)
    }
}

pub fn parse_sign_in_response(
    apple_id: &str,
    response: &ResponseParts,
) -> Result<SignInOutcome, ICloudError> {
    match response.status.as_u16() {
        200..=299 => Ok(SignInOutcome::Authenticated(
            parse_session_seed(response)?
                .try_into_established(extract_optional_challenge(response)?)?,
        )),
        409 => {
            let challenge = ChallengeHeaders {
                scnt: SecretString::from(response.required_header_str(HEADER_SCNT)?.to_owned()),
                session_id: response.required_header_str(HEADER_SESSION_ID)?.to_owned(),
            };
            Ok(SignInOutcome::PendingTwoFactor(PendingTwoFactorAuth {
                apple_id: apple_id.to_owned(),
                challenge,
                seed: parse_session_seed(response)?,
                trusted_phone_numbers: parse_pending_trusted_phone_numbers(response),
                has_trusted_devices: parse_pending_has_trusted_devices(response),
            }))
        }
        _ => Err(ICloudError::UnexpectedStatus {
            status: response.status.as_u16(),
        }),
    }
}

pub fn parse_legacy_trusted_devices(
    response: &ResponseParts,
) -> Result<Vec<LegacyTrustedDevice>, ICloudError> {
    let payload = crate::json::object(response)?;
    let Some(JsonValue::Array(devices)) = payload.get("devices") else {
        return crate::json::invalid("missing trusted devices");
    };

    devices
        .iter()
        .map(|device| {
            let JsonValue::Object(raw) = device else {
                return crate::json::invalid("trusted device entry was not an object");
            };
            Ok(LegacyTrustedDevice {
                device_name: crate::json::string_field(raw, "deviceName"),
                phone_number: crate::json::string_field(raw, "phoneNumber"),
                raw: raw.clone(),
            })
        })
        .collect()
}

pub fn parse_session_seed(response: &ResponseParts) -> Result<SessionSeed, ICloudError> {
    Ok(SessionSeed {
        session_token: response
            .header_str(HEADER_SESSION_TOKEN)?
            .map(|value| SecretString::from(value.to_owned())),
        account_country_code: response
            .header_str(HEADER_ACCOUNT_COUNTRY)?
            .map(str::to_owned),
        trust_token: response
            .header_str(HEADER_TRUST_TOKEN)?
            .map(|value| SecretString::from(value.to_owned())),
    })
}

pub fn parse_challenge_headers(
    response: &ResponseParts,
) -> Result<Option<ChallengeHeaders>, ICloudError> {
    extract_optional_challenge(response)
}

pub fn parse_account_bootstrap(response: &ResponseParts) -> Result<AccountBootstrap, ICloudError> {
    let payload = crate::json::object(response)?;
    let webservices = payload
        .get("webservices")
        .and_then(icloud_types::JsonValue::as_object)
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: "missing webservices".to_owned(),
        })?
        .iter()
        .map(|(service_id, wire)| {
            let Some(wire) = wire.as_object() else {
                return Err(ICloudError::InvalidResponse {
                    message: "webservice entry was not an object".to_owned(),
                });
            };
            Ok((
                ServiceId::from(service_id.clone()),
                WebService {
                    url: url::Url::parse(
                        crate::json::string_field(wire, "url")
                            .ok_or_else(|| ICloudError::InvalidResponse {
                                message: "missing webservice url".to_owned(),
                            })?
                            .as_str(),
                    )?,
                    status: ServiceStatus::from(
                        crate::json::string_field(wire, "status").unwrap_or_default(),
                    ),
                },
            ))
        })
        .collect::<Result<WebServiceMap, ICloudError>>()?;
    let ds_info = payload
        .get("dsInfo")
        .and_then(icloud_types::JsonValue::as_object);

    Ok(AccountBootstrap {
        dsid: ds_info
            .and_then(|info| crate::json::string_field(info, "dsid"))
            .ok_or(ICloudError::MissingSessionField { name: "dsid" })?,
        hsa_challenge_required: crate::json::bool_field(&payload, "hsaChallengeRequired")
            .unwrap_or(false),
        is_trusted_browser: crate::json::bool_field(&payload, "isTrustedBrowser")
            .or_else(|| crate::json::bool_field(&payload, "hsaTrustedBrowser"))
            .unwrap_or(false),
        account_name: crate::json::string_field(&payload, "accountName"),
        webservices,
    })
}

fn parse_pending_payload(response: &ResponseParts) -> Option<JsonObject> {
    match response.json_value().ok()? {
        JsonValue::Object(value) => Some(value),
        _ => None,
    }
}

fn parse_pending_has_trusted_devices(response: &ResponseParts) -> bool {
    parse_pending_payload(response)
        .as_ref()
        .and_then(|payload| crate::json::bool_field(payload, "hasTrustedDevices"))
        .unwrap_or(true)
}

fn parse_pending_trusted_phone_numbers(response: &ResponseParts) -> Vec<TrustedPhoneNumber> {
    let Some(payload) = parse_pending_payload(response) else {
        return Vec::new();
    };

    let mut phones = Vec::new();
    push_phone_from_object(&mut phones, &payload, "trustedPhoneNumber");

    if let Some(JsonValue::Object(verification)) = payload.get("phoneNumberVerification") {
        push_phone_from_object(&mut phones, verification, "trustedPhoneNumber");
        if let Some(JsonValue::Array(values)) = verification.get("trustedPhoneNumbers") {
            phones.extend(values.iter().filter_map(trusted_phone_number_from_value));
        }
    }

    if let Some(JsonValue::Array(values)) = payload.get("trustedPhoneNumbers") {
        phones.extend(values.iter().filter_map(trusted_phone_number_from_value));
    }

    phones.dedup_by(|left, right| left.id == right.id);
    phones
}

fn push_phone_from_object(phones: &mut Vec<TrustedPhoneNumber>, raw: &JsonObject, field: &str) {
    if let Some(phone) = raw.get(field).and_then(trusted_phone_number_from_value) {
        phones.push(phone);
    }
}

fn trusted_phone_number_from_value(value: &JsonValue) -> Option<TrustedPhoneNumber> {
    let JsonValue::Object(raw) = value else {
        return None;
    };
    let id = match raw.get("id")? {
        JsonValue::Number(value) => value.as_u64().map(TrustedPhoneNumberId::Number)?,
        JsonValue::String(value) => TrustedPhoneNumberId::String(value.clone()),
        _ => return None,
    };

    Some(TrustedPhoneNumber {
        id,
        push_mode: crate::json::string_field(raw, "pushMode"),
        non_fteu: crate::json::bool_field(raw, "nonFTEU"),
        obfuscated_number: crate::json::string_field(raw, "obfuscatedNumber")
            .or_else(|| crate::json::string_field(raw, "obfuscatedPhoneNumber")),
        number_with_dial_code: crate::json::string_field(raw, "numberWithDialCode")
            .or_else(|| crate::json::string_field(raw, "phoneNumber")),
    })
}

fn phone_number_member(phone_number: &TrustedPhoneNumber) -> String {
    crate::json::member("phoneNumber", phone_number_payload(phone_number))
}

fn phone_number_payload(phone_number: &TrustedPhoneNumber) -> String {
    let mut fields = vec![crate::json::member(
        "id",
        match &phone_number.id {
            TrustedPhoneNumberId::Number(value) => value.to_string(),
            TrustedPhoneNumberId::String(value) => crate::json::quote(value),
        },
    )];
    if let Some(non_fteu) = phone_number.non_fteu {
        fields.push(crate::json::bool_member("nonFTEU", non_fteu));
    }
    format!("{{{}}}", fields.join(","))
}

fn add_challenge_headers(
    headers: &mut HeaderMap,
    challenge: &ChallengeHeaders,
) -> Result<(), ICloudError> {
    headers.insert(
        HeaderName::from_static(HEADER_SCNT),
        header_value(challenge.scnt.expose_secret(), HEADER_SCNT)?,
    );
    headers.insert(
        HeaderName::from_static(HEADER_SESSION_ID),
        header_value(challenge.session_id.as_str(), HEADER_SESSION_ID)?,
    );
    Ok(())
}

fn extract_optional_challenge(
    response: &ResponseParts,
) -> Result<Option<ChallengeHeaders>, ICloudError> {
    let scnt = response.header_str(HEADER_SCNT)?.map(str::to_owned);
    let session_id = response.header_str(HEADER_SESSION_ID)?.map(str::to_owned);
    match (scnt, session_id) {
        (Some(scnt), Some(session_id)) => Ok(Some(ChallengeHeaders {
            scnt: SecretString::from(scnt),
            session_id,
        })),
        (None, None) => Ok(None),
        _ => Err(ICloudError::InvalidResponse {
            message: "challenge headers were only partially present".to_owned(),
        }),
    }
}

fn header_value(value: &str, name: &'static str) -> Result<HeaderValue, ICloudError> {
    HeaderValue::parse(value).map_err(|error| ICloudError::InvalidHeaderValue {
        name,
        message: error.to_string(),
    })
}

fn header_url_value(url: &url::Url) -> Result<String, ICloudError> {
    if url.host_str().is_none() {
        return Err(ICloudError::InvalidResponse {
            message: "header URL does not contain a host".to_owned(),
        });
    }

    let rendered = url.as_str();
    if url.path() == "/" && url.query().is_none() && url.fragment().is_none() {
        Ok(rendered.trim_end_matches('/').to_owned())
    } else {
        Ok(rendered.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use crate::{HeaderMap, HeaderValue, StatusCode};
    use pretty_assertions::assert_eq;

    use super::*;

    fn context() -> ClientContext {
        ClientContext::production(
            "d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d",
        )
        .expect("production context")
    }

    fn identity() -> ClientIdentity {
        ClientIdentity::new("auth-test-client")
    }

    #[test]
    fn sign_in_request_matches_pyicloud_contract() {
        let context = context();
        let identity = identity();
        let protocol = AuthProtocol::new(&context, &identity);
        let credentials = Credentials::new(
            "person@example.com",
            secrecy::SecretString::from("top-secret".to_owned()),
        );

        let request = protocol
            .sign_in(&credentials, Some("trust-token"))
            .expect("request should be built");

        assert_eq!(request.method, Method::Post);
        assert_eq!(
            request.url.as_str(),
            "https://idmsa.apple.com/appleauth/auth/signin?isRememberMeEnabled=true"
        );
        assert_eq!(
            request
                .headers
                .get(HEADER_OAUTH_STATE)
                .and_then(|value| value.to_str().ok()),
            Some("auth-test-client"),
        );
        assert_eq!(
            request
                .headers
                .get(HEADER_WIDGET_KEY)
                .and_then(|value| value.to_str().ok()),
            Some("d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d"),
        );
        assert_eq!(
            request
                .headers
                .get(ORIGIN)
                .and_then(|value| value.to_str().ok()),
            Some("https://www.icloud.com"),
        );
        assert_eq!(
            request
                .headers
                .get(HEADER_OAUTH_REDIRECT_URI)
                .and_then(|value| value.to_str().ok()),
            Some("https://www.icloud.com"),
        );
        let body = String::from_utf8(request.body.as_bytes().expect("json body").to_vec())
            .expect("request body is valid utf8");
        assert_eq!(
            body,
            r#"{"accountName":"person@example.com","password":"top-secret","rememberMe":true,"trustTokens":["trust-token"]}"#,
        );
    }

    #[test]
    fn china_mainland_sign_in_keeps_pyicloud_oauth_redirect_uri() {
        let context = ClientContext::china_mainland(
            "d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d",
        )
        .expect("china mainland context");
        let identity = identity();
        let protocol = AuthProtocol::new(&context, &identity);
        let credentials = Credentials::new(
            "person@example.com",
            secrecy::SecretString::from("top-secret".to_owned()),
        );

        let request = protocol
            .sign_in(&credentials, None)
            .expect("request should be built");

        assert_eq!(
            request.url.as_str(),
            "https://idmsa.apple.com.cn/appleauth/auth/signin?isRememberMeEnabled=true"
        );
        assert_eq!(
            request
                .headers
                .get(ORIGIN)
                .and_then(|value| value.to_str().ok()),
            Some("https://www.icloud.com.cn"),
        );
        assert_eq!(
            request
                .headers
                .get(HEADER_OAUTH_REDIRECT_URI)
                .and_then(|value| value.to_str().ok()),
            Some("https://www.icloud.com"),
        );
    }

    #[test]
    fn sign_in_response_with_409_becomes_pending_two_factor() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(HEADER_SCNT),
            HeaderValue::from_static("challenge-scnt"),
        );
        headers.insert(
            HeaderName::from_static(HEADER_SESSION_ID),
            HeaderValue::from_static("session-id"),
        );
        headers.insert(
            HeaderName::from_static(HEADER_SESSION_TOKEN),
            HeaderValue::from_static("session-token"),
        );
        headers.insert(
            HeaderName::from_static(HEADER_ACCOUNT_COUNTRY),
            HeaderValue::from_static("JPN"),
        );

        let response = ResponseParts::new(StatusCode::CONFLICT, headers, Vec::new());
        let outcome =
            parse_sign_in_response("person@example.com", &response).expect("response should parse");

        assert_eq!(
            outcome,
            SignInOutcome::PendingTwoFactor(PendingTwoFactorAuth {
                apple_id: "person@example.com".to_owned(),
                challenge: ChallengeHeaders {
                    scnt: SecretString::from("challenge-scnt".to_owned()),
                    session_id: "session-id".to_owned(),
                },
                seed: SessionSeed {
                    session_token: Some(SecretString::from("session-token".to_owned())),
                    account_country_code: Some("JPN".to_owned()),
                    trust_token: None,
                },
                trusted_phone_numbers: Vec::new(),
                has_trusted_devices: true,
            }),
        );
    }

    #[test]
    fn sign_in_response_extracts_trusted_phone_numbers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(HEADER_SCNT),
            HeaderValue::from_static("challenge-scnt"),
        );
        headers.insert(
            HeaderName::from_static(HEADER_SESSION_ID),
            HeaderValue::from_static("session-id"),
        );
        headers.insert(
            HeaderName::from_static(HEADER_SESSION_TOKEN),
            HeaderValue::from_static("session-token"),
        );
        headers.insert(
            HeaderName::from_static(HEADER_ACCOUNT_COUNTRY),
            HeaderValue::from_static("JPN"),
        );
        let body = br#"{
            "hasTrustedDevices": false,
            "phoneNumberVerification": {
                "trustedPhoneNumbers": [
                    {
                        "id": 1,
                        "pushMode": "sms",
                        "nonFTEU": true,
                        "obfuscatedNumber": "+81 ******1234"
                    }
                ]
            }
        }"#;

        let response = ResponseParts::new(StatusCode::CONFLICT, headers, body.to_vec());
        let SignInOutcome::PendingTwoFactor(pending) =
            parse_sign_in_response("person@example.com", &response).expect("response should parse")
        else {
            panic!("pending 2fa expected");
        };

        assert!(!pending.has_trusted_devices);
        assert_eq!(
            pending.trusted_phone_numbers,
            vec![TrustedPhoneNumber {
                id: TrustedPhoneNumberId::Number(1),
                push_mode: Some("sms".to_owned()),
                non_fteu: Some(true),
                obfuscated_number: Some("+81 ******1234".to_owned()),
                number_with_dial_code: None,
            }],
        );
    }

    #[test]
    fn sms_verification_requests_match_apple_contract() {
        let context = context();
        let identity = identity();
        let protocol = AuthProtocol::new(&context, &identity);
        let pending = PendingTwoFactorAuth {
            apple_id: "person@example.com".to_owned(),
            challenge: ChallengeHeaders {
                scnt: SecretString::from("challenge-scnt".to_owned()),
                session_id: "session-id".to_owned(),
            },
            seed: SessionSeed::default(),
            trusted_phone_numbers: Vec::new(),
            has_trusted_devices: true,
        };
        let phone = TrustedPhoneNumber {
            id: TrustedPhoneNumberId::Number(7),
            push_mode: Some("sms".to_owned()),
            non_fteu: Some(false),
            obfuscated_number: None,
            number_with_dial_code: None,
        };

        let request = protocol
            .request_sms_code(&pending, &phone)
            .expect("sms request should build");
        assert_eq!(request.method, Method::Put);
        assert_eq!(
            request.url.as_str(),
            "https://idmsa.apple.com/appleauth/auth/verify/phone"
        );
        assert_eq!(
            String::from_utf8(request.body.as_bytes().expect("json body").to_vec())
                .expect("utf8 body"),
            r#"{"phoneNumber":{"id":7,"nonFTEU":false},"mode":"sms"}"#,
        );

        let request = protocol
            .verify_sms_code(&pending, &phone, "123456")
            .expect("sms verification should build");
        assert_eq!(request.method, Method::Post);
        assert_eq!(
            request.url.as_str(),
            "https://idmsa.apple.com/appleauth/auth/verify/phone/securitycode"
        );
        assert_eq!(
            String::from_utf8(request.body.as_bytes().expect("json body").to_vec())
                .expect("utf8 body"),
            r#"{"phoneNumber":{"id":7,"nonFTEU":false},"securityCode":{"code":"123456"},"mode":"sms"}"#,
        );
    }

    #[test]
    fn request_trusted_device_code_uses_expected_method_and_headers() {
        let context = context();
        let identity = identity();
        let protocol = AuthProtocol::new(&context, &identity);
        let pending = PendingTwoFactorAuth {
            apple_id: "person@example.com".to_owned(),
            challenge: ChallengeHeaders {
                scnt: SecretString::from("challenge-scnt".to_owned()),
                session_id: "session-id".to_owned(),
            },
            seed: SessionSeed::default(),
            trusted_phone_numbers: Vec::new(),
            has_trusted_devices: true,
        };

        let request = protocol
            .request_trusted_device_code(&pending)
            .expect("trusted-device request should build");

        assert_eq!(request.method, Method::Put);
        assert_eq!(
            request.url.as_str(),
            "https://idmsa.apple.com/appleauth/auth/verify/trusteddevice/securitycode"
        );
        assert!(
            request.body.as_bytes().is_none_or(|body| body.is_empty()),
            "trusted-device code requests must not send a body"
        );
        assert_eq!(
            request
                .headers
                .get("scnt")
                .and_then(|value| value.to_str().ok()),
            Some("challenge-scnt")
        );
        assert_eq!(
            request
                .headers
                .get("x-apple-id-session-id")
                .and_then(|value| value.to_str().ok()),
            Some("session-id")
        );
    }

    #[test]
    fn legacy_trusted_devices_keep_raw_payload() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            br#"{"devices":[{"deviceName":"MacBook Pro","id":"device-1","extra":true},{"phoneNumber":"+81 ******1234","id":2}]}"#.to_vec(),
        );

        let devices = parse_legacy_trusted_devices(&response).expect("devices should parse");

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].display_label(), Some("MacBook Pro"));
        assert_eq!(devices[1].display_label(), Some("+81 ******1234"));
        assert_eq!(devices[0].raw["extra"], JsonValue::Bool(true));
    }

    #[test]
    fn bootstrap_response_parses_dynamic_webservices() {
        let fixture = include_bytes!("../tests/fixtures/auth/account_login_success.json");
        let response = ResponseParts::new(StatusCode::OK, HeaderMap::new(), fixture.to_vec());

        let bootstrap = parse_account_bootstrap(&response).expect("bootstrap should parse");

        assert_eq!(
            bootstrap.account_name.as_deref(),
            Some("person@example.com")
        );
        assert_eq!(bootstrap.dsid, "123456789");
        assert_eq!(bootstrap.is_trusted_browser, true);
        assert_eq!(
            bootstrap
                .webservices
                .get(&ServiceId::FindMy)
                .map(|service| service.url.as_str()),
            Some("https://p43-fmip.icloud.com/"),
        );
    }

    #[test]
    fn bootstrap_response_accepts_hsa_trusted_browser_alias() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            br#"{"dsInfo":{"dsid":"123"},"hsaTrustedBrowser":true,"webservices":{}}"#.to_vec(),
        );

        let bootstrap = parse_account_bootstrap(&response).expect("bootstrap should parse");

        assert_eq!(bootstrap.dsid, "123");
        assert!(bootstrap.is_trusted_browser);
    }

    #[test]
    fn bootstrap_response_requires_dsid() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            br#"{"webservices":{}}"#.to_vec(),
        );

        let error =
            parse_account_bootstrap(&response).expect_err("missing dsid should be rejected");

        assert_eq!(error.to_string(), "missing session field `dsid`");
    }
}
