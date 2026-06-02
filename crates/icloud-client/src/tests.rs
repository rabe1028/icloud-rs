use std::{
    collections::VecDeque,
    convert::Infallible,
    sync::{Arc, Mutex},
};

use icloud_protocol::HeaderValue;
use pretty_assertions::assert_eq;
use time::macros::datetime;

use super::*;

#[derive(Debug, Default, Clone)]
struct MockAsyncTransport {
    responses: Arc<Mutex<VecDeque<ResponseParts>>>,
    requests: Arc<Mutex<Vec<RequestSpec>>>,
}

impl MockAsyncTransport {
    fn with_responses(responses: Vec<ResponseParts>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl AsyncTransport for MockAsyncTransport {
    type Error = Infallible;

    async fn execute(&self, request: RequestSpec) -> Result<ResponseParts, Self::Error> {
        self.requests.lock().expect("requests mutex").push(request);
        Ok(self
            .responses
            .lock()
            .expect("responses mutex")
            .pop_front()
            .expect("queued response"))
    }
}

#[derive(Debug, Default, Clone)]
struct MockBlockingTransport {
    responses: Arc<Mutex<VecDeque<ResponseParts>>>,
    requests: Arc<Mutex<Vec<RequestSpec>>>,
}

impl MockBlockingTransport {
    fn with_responses(responses: Vec<ResponseParts>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl BlockingTransport for MockBlockingTransport {
    type Error = Infallible;

    fn execute(&self, request: RequestSpec) -> Result<ResponseParts, Self::Error> {
        self.requests.lock().expect("requests mutex").push(request);
        Ok(self
            .responses
            .lock()
            .expect("responses mutex")
            .pop_front()
            .expect("queued response"))
    }
}

fn context() -> ClientContext {
    ClientContext::production("d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d")
        .expect("production context")
}

fn credentials() -> Credentials {
    Credentials::new(
        "person@example.com",
        secrecy::SecretString::from("top-secret".to_owned()),
    )
}

fn pending_sign_in_response() -> ResponseParts {
    let mut headers = HeaderMap::new();
    headers.insert("scnt", HeaderValue::from_static("challenge-scnt"));
    headers.insert(
        "x-apple-id-session-id",
        HeaderValue::from_static("session-id"),
    );
    headers.insert(
        "x-apple-session-token",
        HeaderValue::from_static("session-token"),
    );
    headers.insert(
        "x-apple-id-account-country",
        HeaderValue::from_static("JPN"),
    );
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_static("session=abc; Path=/; Secure; HttpOnly"),
    );
    ResponseParts::new(StatusCode::CONFLICT, headers, Vec::new())
}

fn trust_response() -> ResponseParts {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-apple-session-token",
        HeaderValue::from_static("session-token"),
    );
    headers.insert(
        "x-apple-id-account-country",
        HeaderValue::from_static("JPN"),
    );
    headers.insert(
        "x-apple-twosv-trust-token",
        HeaderValue::from_static("trust-token"),
    );
    ResponseParts::new(StatusCode::OK, headers, Vec::new())
}

fn trusted_device_verify_response_with_rotated_challenge() -> ResponseParts {
    let mut headers = HeaderMap::new();
    headers.insert("scnt", HeaderValue::from_static("rotated-scnt"));
    headers.insert(
        "x-apple-id-session-id",
        HeaderValue::from_static("rotated-session-id"),
    );
    headers.insert(
        "x-apple-session-token",
        HeaderValue::from_static("session-token"),
    );
    headers.insert(
        "x-apple-id-account-country",
        HeaderValue::from_static("JPN"),
    );
    ResponseParts::new(StatusCode::OK, headers, Vec::new())
}

fn authenticated_sign_in_response_without_trust_header() -> ResponseParts {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-apple-session-token",
        HeaderValue::from_static("session-token"),
    );
    headers.insert(
        "x-apple-id-account-country",
        HeaderValue::from_static("JPN"),
    );
    ResponseParts::new(StatusCode::OK, headers, Vec::new())
}

fn account_login_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/auth/account_login_success.json").to_vec(),
    )
}

fn ubiquity_root_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/ubiquity/root.json").to_vec(),
    )
}

fn ubiquity_children_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/ubiquity/children.json").to_vec(),
    )
}

fn ubiquity_file_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/ubiquity/file.json").to_vec(),
    )
}

fn photos_ready_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/photos/indexing_finished.json").to_vec(),
    )
}

fn photos_indexing_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/photos/indexing_in_progress.json").to_vec(),
    )
}

fn contacts_startup_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/contacts/startup.json").to_vec(),
    )
}

fn contacts_page_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/contacts/contacts.json").to_vec(),
    )
}

fn calendar_events_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/calendar/events.json").to_vec(),
    )
}

fn calendar_event_detail_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/calendar/event_detail.json").to_vec(),
    )
}

fn reminders_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/reminders/reminders.json").to_vec(),
    )
}

fn account_devices_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/account/devices.json").to_vec(),
    )
}

fn family_members_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/account/family.json").to_vec(),
    )
}

fn account_storage_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/account/storage.json").to_vec(),
    )
}

fn photos_albums_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/photos/custom_albums.json").to_vec(),
    )
}

fn photos_album_count_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/photos/album_count.json").to_vec(),
    )
}

fn photos_album_page_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/photos/album_page.json").to_vec(),
    )
}

fn photos_folder_albums_response() -> ResponseParts {
    ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/photos/folder_children.json").to_vec(),
    )
}

fn authenticated_client_with_transport(
    transport: MockBlockingTransport,
) -> BlockingICloudClient<MockBlockingTransport, Authenticated> {
    match BlockingICloudClient::restore(
        context(),
        transport,
        ClientStateSnapshot {
            client_id: "auth-test-client".to_owned(),
            cookie_jar: {
                let mut jar = CookieJar::new();
                jar.apply_set_cookie(
                        &url::Url::parse("https://setup.icloud.com/setup/ws/1/accountLogin")
                            .expect("valid url"),
                        "X-APPLE-WEBAUTH-VALIDATE=x=ignored:t=upload-token:s=signature; Path=/; Secure; HttpOnly",
                    )
                    .expect("cookie should parse");
                jar
            },
            auth_state: AuthState::Authenticated(AuthenticatedSession {
                established: icloud_types::EstablishedSession {
                    session_token: SecretString::from("session-token".to_owned()),
                    account_country_code: "JPN".to_owned(),
                    trust_token: Some(SecretString::from("trust-token".to_owned())),
                    challenge: None,
                },
                bootstrap: parse_account_bootstrap(&account_login_response())
                    .expect("bootstrap fixture should parse"),
            }),
        },
    ) {
        RestoredBlockingClient::Authenticated(client) => client,
        _ => panic!("unexpected restored variant"),
    }
}

fn authenticated_async_client_with_transport(
    transport: MockAsyncTransport,
) -> AsyncICloudClient<MockAsyncTransport, Authenticated> {
    match AsyncICloudClient::restore(
        context(),
        transport,
        ClientStateSnapshot {
            client_id: "auth-test-client".to_owned(),
            cookie_jar: {
                let mut jar = CookieJar::new();
                jar.apply_set_cookie(
                        &url::Url::parse("https://setup.icloud.com/setup/ws/1/accountLogin")
                            .expect("valid url"),
                        "X-APPLE-WEBAUTH-VALIDATE=x=ignored:t=upload-token:s=signature; Path=/; Secure; HttpOnly",
                    )
                    .expect("cookie should parse");
                jar
            },
            auth_state: AuthState::Authenticated(AuthenticatedSession {
                established: icloud_types::EstablishedSession {
                    session_token: SecretString::from("session-token".to_owned()),
                    account_country_code: "JPN".to_owned(),
                    trust_token: Some(SecretString::from("trust-token".to_owned())),
                    challenge: None,
                },
                bootstrap: parse_account_bootstrap(&account_login_response())
                    .expect("bootstrap fixture should parse"),
            }),
        },
    ) {
        RestoredAsyncClient::Authenticated(client) => client,
        _ => panic!("unexpected restored variant"),
    }
}

#[tokio::test]
async fn async_client_transitions_through_auth_states() {
    let transport = MockAsyncTransport::with_responses(vec![
        pending_sign_in_response(),
        ResponseParts::new(StatusCode::OK, HeaderMap::new(), Vec::new()),
        trust_response(),
        account_login_response(),
    ]);
    let client = AsyncICloudClient::new(
        context(),
        ClientIdentity::new("auth-test-client"),
        transport,
    );

    let pending = match client
        .sign_in(&credentials())
        .await
        .expect("sign in succeeds")
    {
        AsyncSignInResult::PendingTwoFactor(client) => client,
        AsyncSignInResult::Authenticated(_) => panic!("challenge expected"),
    };
    assert_eq!(pending.cookie_jar().is_empty(), false);

    let prebootstrap = pending
        .submit_trusted_device_code("123456")
        .await
        .expect("challenge completes");
    assert_eq!(
        prebootstrap
            .state()
            .session
            .trust_token
            .as_ref()
            .map(secrecy::ExposeSecret::expose_secret),
        Some("trust-token"),
    );

    let authenticated = prebootstrap.bootstrap().await.expect("bootstrap succeeds");
    assert_eq!(
        authenticated
            .state()
            .session
            .bootstrap
            .webservices
            .get(&icloud_types::ServiceId::Drive)
            .map(|service| service.url.as_str()),
        Some("https://p43-drivews.icloud.com/"),
    );
}

#[test]
fn blocking_client_uses_rotated_challenge_headers_for_trust_request() {
    let transport = MockBlockingTransport::with_responses(vec![
        pending_sign_in_response(),
        trusted_device_verify_response_with_rotated_challenge(),
        trust_response(),
    ]);
    let client = BlockingICloudClient::new(
        context(),
        ClientIdentity::new("auth-test-client"),
        transport.clone(),
    );

    let pending = match client.sign_in(&credentials()).expect("sign in succeeds") {
        BlockingSignInResult::PendingTwoFactor(client) => client,
        BlockingSignInResult::Authenticated(_) => panic!("challenge expected"),
    };
    let prebootstrap = pending
        .submit_trusted_device_code("123456")
        .expect("challenge completes");

    assert_eq!(
        prebootstrap.state().session.challenge,
        Some(icloud_types::ChallengeHeaders {
            scnt: SecretString::from("rotated-scnt".to_owned()),
            session_id: "rotated-session-id".to_owned(),
        }),
    );

    let requests = transport.requests.lock().expect("requests mutex");
    let trust_request = requests.get(2).expect("trust request should be recorded");
    assert_eq!(
        trust_request
            .headers
            .get("scnt")
            .and_then(|value| value.to_str().ok()),
        Some("rotated-scnt"),
    );
    assert_eq!(
        trust_request
            .headers
            .get("x-apple-id-session-id")
            .and_then(|value| value.to_str().ok()),
        Some("rotated-session-id"),
    );
}

#[test]
fn blocking_client_snapshot_preserves_typed_state() {
    let transport = MockBlockingTransport::with_responses(vec![pending_sign_in_response()]);
    let client = BlockingICloudClient::new(
        context(),
        ClientIdentity::new("auth-test-client"),
        transport,
    );

    let pending = match client.sign_in(&credentials()).expect("sign in succeeds") {
        BlockingSignInResult::PendingTwoFactor(client) => client,
        BlockingSignInResult::Authenticated(_) => panic!("challenge expected"),
    };
    let snapshot = pending.snapshot();

    assert_eq!(snapshot.client_id, "auth-test-client");
    assert_eq!(
        snapshot.auth_state,
        AuthState::PendingTwoFactor(icloud_types::PendingTwoFactorAuth {
            apple_id: "person@example.com".to_owned(),
            challenge: icloud_types::ChallengeHeaders {
                scnt: SecretString::from("challenge-scnt".to_owned()),
                session_id: "session-id".to_owned(),
            },
            seed: icloud_types::SessionSeed {
                session_token: Some(SecretString::from("session-token".to_owned())),
                account_country_code: Some("JPN".to_owned()),
                trust_token: None,
            },
        }),
    );
}

#[test]
fn blocking_sign_in_preserves_supplied_trust_token_when_response_omits_it() {
    let transport = MockBlockingTransport::with_responses(vec![
        authenticated_sign_in_response_without_trust_header(),
    ]);
    let client = BlockingICloudClient::new(
        context(),
        ClientIdentity::new("auth-test-client"),
        transport,
    );

    let authenticated = match client
        .sign_in_with_trust_token(&credentials(), "provided-trust-token")
        .expect("sign in should succeed")
    {
        BlockingSignInResult::Authenticated(client) => client,
        BlockingSignInResult::PendingTwoFactor(_) => {
            panic!("trusted sign in should not challenge")
        }
    };

    assert_eq!(
        authenticated
            .state()
            .session
            .trust_token
            .as_ref()
            .map(secrecy::ExposeSecret::expose_secret),
        Some("provided-trust-token"),
    );
}

#[test]
fn blocking_client_restores_authenticated_prebootstrap_variant() {
    let transport = MockBlockingTransport::default();
    let restored = BlockingICloudClient::restore(
        context(),
        transport,
        ClientStateSnapshot {
            client_id: "auth-restore".to_owned(),
            cookie_jar: CookieJar::new(),
            auth_state: AuthState::AuthenticatedPreBootstrap(icloud_types::EstablishedSession {
                session_token: SecretString::from("session-token".to_owned()),
                account_country_code: "JPN".to_owned(),
                trust_token: Some(SecretString::from("trust-token".to_owned())),
                challenge: None,
            }),
        },
    );

    match restored {
        RestoredBlockingClient::AuthenticatedPreBootstrap(client) => {
            assert_eq!(client.identity().client_id, "auth-restore");
            assert_eq!(client.state().session.account_country_code, "JPN");
        }
        _ => panic!("unexpected restored variant"),
    }
}

#[test]
fn cookie_header_is_attached_before_dispatch() {
    let transport = MockBlockingTransport::with_responses(vec![pending_sign_in_response()]);
    let client = BlockingICloudClient::restore(
        context(),
        transport,
        ClientStateSnapshot {
            client_id: "auth-test-client".to_owned(),
            cookie_jar: {
                let mut jar = CookieJar::new();
                jar.apply_set_cookie(
                    &url::Url::parse("https://idmsa.apple.com/appleauth/auth/signin")
                        .expect("valid url"),
                    "seed=1; Domain=.apple.com; Path=/; Secure; HttpOnly",
                )
                .expect("cookie should parse");
                jar
            },
            auth_state: AuthState::LoggedOut,
        },
    );

    let client = match client {
        RestoredBlockingClient::LoggedOut(client) => client,
        _ => panic!("unexpected restored variant"),
    };
    let _ = client.sign_in(&credentials()).expect("sign in succeeds");
}

#[test]
fn blocking_client_refreshes_findmy_devices() {
    let transport = MockBlockingTransport::with_responses(vec![ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/findmy/refresh_client.json").to_vec(),
    )]);
    let mut client = match BlockingICloudClient::restore(
        context(),
        transport,
        ClientStateSnapshot {
            client_id: "auth-test-client".to_owned(),
            cookie_jar: CookieJar::new(),
            auth_state: AuthState::Authenticated(AuthenticatedSession {
                established: icloud_types::EstablishedSession {
                    session_token: SecretString::from("session-token".to_owned()),
                    account_country_code: "JPN".to_owned(),
                    trust_token: Some(SecretString::from("trust-token".to_owned())),
                    challenge: None,
                },
                bootstrap: parse_account_bootstrap(&account_login_response())
                    .expect("bootstrap fixture should parse"),
            }),
        },
    ) {
        RestoredBlockingClient::Authenticated(client) => client,
        _ => panic!("unexpected restored variant"),
    };

    let devices = client
        .refresh_findmy_devices(&FindMyRefreshOptions::default())
        .expect("refresh should succeed");
    assert_eq!(devices.devices.len(), 1);
    assert_eq!(devices.devices[0].id, "device-1");
}

#[test]
fn blocking_client_refreshes_a_specific_findmy_device() {
    let transport = MockBlockingTransport::with_responses(vec![ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/findmy/refresh_client.json").to_vec(),
    )]);
    let mut client = authenticated_client_with_transport(transport.clone());

    let device = client
        .refresh_findmy_device("device-1")
        .expect("device refresh should succeed");

    assert_eq!(device.id, "device-1");
    let requests = transport.requests.lock().expect("requests mutex");
    let body = String::from_utf8(
        requests[0]
            .body
            .as_bytes()
            .expect("json body should exist")
            .to_vec(),
    )
    .expect("utf8 request");
    assert_eq!(
        body,
        r#"{"clientContext":{"fmly":true,"shouldLocate":true,"selectedDevice":"device-1","deviceListVersion":1}}"#,
    );
}

#[test]
fn blocking_client_refreshes_findmy_location() {
    let transport = MockBlockingTransport::with_responses(vec![ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/findmy/refresh_client.json").to_vec(),
    )]);
    let mut client = authenticated_client_with_transport(transport);

    let location = client
        .refresh_findmy_location("device-1")
        .expect("location refresh should succeed")
        .expect("device should have a location");

    assert_eq!(
        (location.latitude, location.longitude),
        (35.681236, 139.767125)
    );
}

#[test]
fn blocking_client_reports_missing_findmy_device() {
    let transport = MockBlockingTransport::with_responses(vec![ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/findmy/refresh_client.json").to_vec(),
    )]);
    let mut client = authenticated_client_with_transport(transport);

    let error = client
        .refresh_findmy_device("missing-device")
        .expect_err("missing device should fail");

    assert_eq!(
        error.to_string(),
        "find my device `missing-device` does not exist"
    );
}

#[test]
fn blocking_client_traverses_drive_tree() {
    let transport = MockBlockingTransport::with_responses(vec![
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/root.json").to_vec(),
        ),
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/folder.json").to_vec(),
        ),
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/subfolder.json").to_vec(),
        ),
    ]);
    let mut client = authenticated_client_with_transport(transport);

    let root = client.drive_root().expect("root should load");
    let root_children = client
        .drive_children(&root)
        .expect("root children should be available");
    assert_eq!(
        root_children
            .iter()
            .map(DriveNode::name)
            .collect::<Vec<_>>(),
        vec!["Preview", "pyiCloud"],
    );

    let folder = client
        .drive_child_by_name(&root, "pyiCloud")
        .expect("pyiCloud folder should exist");
    let folder = folder
        .as_container()
        .expect("pyiCloud should be a container");
    let subfolder = client
        .drive_child_by_name(folder, "Test")
        .expect("Test folder should exist");
    let subfolder = subfolder
        .as_container()
        .expect("Test should be a container");
    let file = client
        .drive_child_by_name(subfolder, "Scanned document 1.pdf")
        .expect("file should exist");
    let file = file.as_file().expect("expected a file node");

    assert_eq!(file.size, 21_644_358);
    assert_eq!(file.name(), "Scanned document 1.pdf");
}

#[test]
fn blocking_client_reports_invalid_app_library() {
    let transport = MockBlockingTransport::with_responses(vec![
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/root.json").to_vec(),
        ),
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/invalid_app_library.json").to_vec(),
        ),
    ]);
    let mut client = authenticated_client_with_transport(transport);

    let root = client.drive_root().expect("root should load");
    let preview = client
        .drive_child_by_name(&root, "Preview")
        .expect("Preview app library should exist");
    let preview = preview
        .as_container()
        .expect("Preview should be a container");
    let error = client
        .drive_children(preview)
        .expect_err("invalid app library should fail");

    assert_eq!(
        error.to_string(),
        "drive container contents unavailable: ID_INVALID",
    );
}

#[test]
fn blocking_client_downloads_drive_file_contents() {
    let transport = MockBlockingTransport::with_responses(vec![
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/root.json").to_vec(),
        ),
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/folder.json").to_vec(),
        ),
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/subfolder.json").to_vec(),
        ),
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/download_by_id.json").to_vec(),
        ),
        ResponseParts::new(StatusCode::OK, HeaderMap::new(), b"%PDF-test".to_vec()),
    ]);
    let mut client = authenticated_client_with_transport(transport);

    let root = client.drive_root().expect("root should load");
    let folder = client
        .drive_child_by_name(&root, "pyiCloud")
        .expect("pyiCloud should exist");
    let folder = folder
        .as_container()
        .expect("pyiCloud should be a container");
    let subfolder = client
        .drive_child_by_name(folder, "Test")
        .expect("Test should exist");
    let subfolder = subfolder
        .as_container()
        .expect("Test should be a container");
    let file = client
        .drive_child_by_name(subfolder, "Scanned document 1.pdf")
        .expect("file should exist");
    let file = file.as_file().expect("expected a file");

    let response = client
        .download_drive_file(file)
        .expect("download should succeed");
    assert_eq!(response.body, b"%PDF-test".to_vec());
}

#[tokio::test]
async fn async_client_reports_failed_drive_content_download() {
    let transport = MockAsyncTransport::with_responses(vec![
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/download_by_id.json").to_vec(),
        ),
        ResponseParts::new(
            StatusCode::FORBIDDEN,
            HeaderMap::new(),
            b"forbidden".to_vec(),
        ),
    ]);
    let mut client = authenticated_async_client_with_transport(transport);
    let file = DriveFile {
        metadata: icloud_types::DriveNodeMetadata {
            drivewsid: icloud_types::DriveWsId::from("FILE::com.apple.CloudDocs::blocked"),
            docwsid: icloud_types::DriveDocumentId::from("blocked"),
            zone: "com.apple.CloudDocs".to_owned(),
            name: "blocked.pdf".to_owned(),
            parent_id: None,
            etag: Some("1".to_owned()),
        },
        base_name: "blocked".to_owned(),
        extension: Some("pdf".to_owned()),
        hidden_extension: false,
        size: 1,
        date_changed: None,
        date_modified: None,
        date_last_open: None,
        extra: icloud_types::JsonObject::new(),
    };

    let error = client
        .download_drive_file(&file)
        .await
        .expect_err("failed content response should be surfaced");

    assert_eq!(error.to_string(), "unexpected HTTP status 403");
}

#[test]
fn blocking_client_reports_failed_drive_content_download() {
    let transport = MockBlockingTransport::with_responses(vec![
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/download_by_id.json").to_vec(),
        ),
        ResponseParts::new(
            StatusCode::FORBIDDEN,
            HeaderMap::new(),
            b"forbidden".to_vec(),
        ),
    ]);
    let mut client = authenticated_client_with_transport(transport);
    let file = DriveFile {
        metadata: icloud_types::DriveNodeMetadata {
            drivewsid: icloud_types::DriveWsId::from("FILE::com.apple.CloudDocs::blocked"),
            docwsid: icloud_types::DriveDocumentId::from("blocked"),
            zone: "com.apple.CloudDocs".to_owned(),
            name: "blocked.pdf".to_owned(),
            parent_id: None,
            etag: Some("1".to_owned()),
        },
        base_name: "blocked".to_owned(),
        extension: Some("pdf".to_owned()),
        hidden_extension: false,
        size: 1,
        date_changed: None,
        date_modified: None,
        date_last_open: None,
        extra: icloud_types::JsonObject::new(),
    };

    let error = client
        .download_drive_file(&file)
        .expect_err("failed content response should be surfaced");

    assert_eq!(error.to_string(), "unexpected HTTP status 403");
}

#[test]
fn blocking_client_uses_file_zone_for_drive_download_lookup() {
    let transport = MockBlockingTransport::with_responses(vec![ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        include_bytes!("../tests/fixtures/drive/download_by_id.json").to_vec(),
    )]);
    let mut client = authenticated_client_with_transport(transport.clone());
    let file = DriveFile {
        metadata: icloud_types::DriveNodeMetadata {
            drivewsid: icloud_types::DriveWsId::from("FILE::com.apple.Preview::preview-doc"),
            docwsid: icloud_types::DriveDocumentId::from("preview-doc"),
            zone: "com.apple.Preview".to_owned(),
            name: "Preview.note".to_owned(),
            parent_id: None,
            etag: Some("1".to_owned()),
        },
        base_name: "Preview".to_owned(),
        extension: Some("note".to_owned()),
        hidden_extension: false,
        size: 1,
        date_changed: None,
        date_modified: None,
        date_last_open: None,
        extra: icloud_types::JsonObject::new(),
    };

    let _ = client
        .resolve_drive_file_download(&file)
        .expect("download lookup should succeed");

    let requests = transport.requests.lock().expect("requests mutex");
    let request = requests
        .first()
        .expect("download lookup request should be recorded");
    assert_eq!(
        request.url.as_str(),
        "https://p43-docws.icloud.com/ws/com.apple.Preview/download/by_id?clientId=auth-test-client&dsid=123456789&document_id=preview-doc",
    );
}

#[test]
fn blocking_client_uploads_drive_file_via_three_step_flow() {
    let transport = MockBlockingTransport::with_responses(vec![
            ResponseParts::new(
                StatusCode::OK,
                HeaderMap::new(),
                include_bytes!("../tests/fixtures/drive/folder.json").to_vec(),
            ),
            ResponseParts::new(
                StatusCode::OK,
                HeaderMap::new(),
                br#"[{"document_id":"new-document","url":"https://content.icloud.com/upload"}]"#
                    .to_vec(),
            ),
            ResponseParts::new(
                StatusCode::OK,
                HeaderMap::new(),
                br#"{"singleFile":{"fileChecksum":"checksum","wrappingKey":"wrap","referenceChecksum":"ref","receipt":"receipt","size":5}}"#
                    .to_vec(),
            ),
            ResponseParts::new(
                StatusCode::OK,
                HeaderMap::new(),
                br#"{"status":"ok"}"#.to_vec(),
            ),
        ]);
    let mut client = authenticated_client_with_transport(transport);
    let folder = client.drive_root().expect("folder should parse");
    let upload =
        DriveUploadFile::new("hello.txt", b"hello".to_vec()).with_content_type("text/plain");

    let response = client
        .upload_drive_file(&folder, &upload)
        .expect("upload should succeed");

    assert_eq!(response["status"], "ok");
}

#[test]
fn blocking_client_traverses_ubiquity_tree() {
    let transport = MockBlockingTransport::with_responses(vec![
        ubiquity_root_response(),
        ubiquity_children_response(),
        ubiquity_children_response(),
    ]);
    let mut client = authenticated_client_with_transport(transport);

    let root = client.ubiquity_root().expect("root should load");
    assert_eq!(root.item_id().as_str(), "0");

    let children = client
        .ubiquity_children(&root)
        .expect("children should be available");
    assert_eq!(
        children.iter().map(UbiquityNode::name).collect::<Vec<_>>(),
        vec!["Documents", "notes.txt", "corrupt.bin"],
    );

    let note = client
        .ubiquity_child_by_name(&root, "notes.txt")
        .expect("notes.txt should exist");
    let note = note.as_file().expect("notes.txt should be a file");
    assert_eq!(note.item_id().as_str(), "notes-file");
    assert_eq!(note.size, Some(12));
}

#[test]
fn blocking_client_loads_specific_ubiquity_node() {
    let transport = MockBlockingTransport::with_responses(vec![ubiquity_file_response()]);
    let mut client = authenticated_client_with_transport(transport);

    let node = client
        .ubiquity_node(&UbiquityNodeId::from("notes-file"))
        .expect("node should load");

    assert_eq!(node.name(), "notes.txt");
    assert!(node.as_file().is_some());
}

#[test]
fn blocking_client_downloads_ubiquity_file_contents() {
    let transport = MockBlockingTransport::with_responses(vec![
        ubiquity_root_response(),
        ubiquity_children_response(),
        ResponseParts::new(StatusCode::OK, HeaderMap::new(), b"hello world".to_vec()),
    ]);
    let mut client = authenticated_client_with_transport(transport.clone());

    let root = client.ubiquity_root().expect("root should load");
    let note = client
        .ubiquity_child_by_name(&root, "notes.txt")
        .expect("notes.txt should exist");
    let note = note.as_file().expect("notes.txt should be a file");

    let response = client
        .download_ubiquity_file(note)
        .expect("download should succeed");

    assert_eq!(response.body, b"hello world".to_vec());
    let requests = transport.requests.lock().expect("requests mutex");
    let request = requests
        .last()
        .expect("download request should be recorded");
    assert_eq!(
        request.url.as_str(),
        "https://p43-ubiquityws.icloud.com/ws/123456789/file/notes-file",
    );
}

#[test]
fn blocking_client_lists_photo_albums_after_ready_check() {
    let transport = MockBlockingTransport::with_responses(vec![
        photos_ready_response(),
        photos_albums_response(),
        photos_folder_albums_response(),
    ]);
    let mut client = authenticated_client_with_transport(transport.clone());

    let albums = client.photo_albums().expect("albums should load");

    assert_eq!(
        albums.first().map(|album| album.name.as_str()),
        Some("All Photos")
    );
    assert!(albums.iter().any(|album| album.name == "Vacation"));
    assert!(albums.iter().any(|album| album.name == "Nested Vacation"));
    assert!(!albums.iter().any(|album| album.name == "Trips"));
    assert_eq!(albums.len(), PhotoAlbumSummary::smart_albums().len() + 2);
    assert_eq!(
        albums
            .iter()
            .find(|album| album.name == "Vacation")
            .expect("vacation album should exist")
            .query
            .direction,
        icloud_types::PhotoAlbumDirection::Descending,
    );

    let requests = transport.requests.lock().expect("requests mutex");
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[0].url.as_str(),
        "https://p43-ckdatabasews.icloud.com/database/1/com.apple.photos.cloud/production/private/records/query?clientId=auth-test-client&dsid=123456789&remapEnums=true&getCurrentSyncToken=true",
    );
}

#[test]
fn blocking_client_rejects_photo_library_before_indexing_finishes() {
    let transport = MockBlockingTransport::with_responses(vec![photos_indexing_response()]);
    let mut client = authenticated_client_with_transport(transport);

    let error = client
        .photo_albums()
        .expect_err("indexing photo library should be rejected");

    assert_eq!(error.to_string(), "photo library is not ready: INDEXING");
}

#[test]
fn blocking_client_loads_all_contacts() {
    let transport = MockBlockingTransport::with_responses(vec![
        contacts_startup_response(),
        contacts_page_response(),
    ]);
    let mut client = authenticated_client_with_transport(transport.clone());

    let contacts = client.contacts_all().expect("contacts should load");

    assert_eq!(contacts.len(), 2);
    assert_eq!(contacts[0].display_name.as_deref(), Some("Ada Lovelace"));
    assert_eq!(
        contacts[1].company_name.as_deref(),
        Some("Analytical Engines")
    );

    let requests = transport.requests.lock().expect("requests mutex");
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].url.as_str(),
        "https://p43-contacts.icloud.com/co/startup?clientId=auth-test-client&dsid=123456789&clientVersion=2.1&locale=en_US&order=last%2Cfirst&includePhoneNumbers=true",
    );
    assert_eq!(
        requests[1].url.as_str(),
        "https://p43-contacts.icloud.com/co/contacts?clientId=auth-test-client&dsid=123456789&clientVersion=2.1&locale=en_US&order=last%2Cfirst&includePhoneNumbers=true&prefToken=pref-token-1&syncToken=sync-token-1&limit=0&offset=0",
    );
}

#[test]
fn blocking_client_loads_calendar_events() {
    let transport = MockBlockingTransport::with_responses(vec![calendar_events_response()]);
    let mut client = authenticated_client_with_transport(transport.clone());

    let events = client
        .calendar_events_between(
            datetime!(2024-06-01 0:00 UTC),
            datetime!(2024-07-01 0:00 UTC),
        )
        .expect("calendar events should load");

    assert_eq!(events.events.len(), 2);
    assert_eq!(events.events[0].guid, "event-1");
    assert_eq!(events.events[1].notes.as_deref(), Some("Out of office"));

    let requests = transport.requests.lock().expect("requests mutex");
    assert_eq!(
        requests[0].url.as_str(),
        "https://p43-calendarws.icloud.com/ca/events?clientId=auth-test-client&dsid=123456789&lang=en-us&usertz=UTC&startDate=2024-06-01&endDate=2024-07-01",
    );
}

#[test]
fn blocking_client_loads_reminders() {
    let transport = MockBlockingTransport::with_responses(vec![reminders_response()]);
    let mut client = authenticated_client_with_transport(transport.clone());

    let reminders = client.reminders().expect("reminders should load");

    assert_eq!(reminders.lists.len(), 2);
    assert_eq!(reminders.lists[0].reminders[0].guid, "task-1");

    let requests = transport.requests.lock().expect("requests mutex");
    assert_eq!(
        requests[0].url.as_str(),
        "https://p43-remindersws.icloud.com/rd/startup?clientId=auth-test-client&dsid=123456789&clientVersion=4.0&lang=en-us&usertz=UTC",
    );
}

#[test]
fn blocking_client_loads_account_devices() {
    let transport = MockBlockingTransport::with_responses(vec![account_devices_response()]);
    let mut client = authenticated_client_with_transport(transport.clone());

    let devices = client.account_devices().expect("devices should load");

    assert_eq!(devices.len(), 2);
    assert_eq!(
        devices[0].model_display_name.as_deref(),
        Some("MacBook Pro")
    );

    let requests = transport.requests.lock().expect("requests mutex");
    assert_eq!(
        requests[0].url.as_str(),
        "https://p43-setup.icloud.com/setup/web/device/getDevices?clientId=auth-test-client&dsid=123456789",
    );
}

#[test]
fn blocking_client_downloads_family_member_photo() {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    let transport = MockBlockingTransport::with_responses(vec![
        family_members_response(),
        ResponseParts::new(StatusCode::OK, headers, b"jpeg-photo".to_vec()),
    ]);
    let mut client = authenticated_client_with_transport(transport.clone());

    let member = client
        .family_members()
        .expect("family should load")
        .into_iter()
        .next()
        .expect("member should exist");
    let photo = client
        .family_member_photo(&member)
        .expect("photo should download");

    assert_eq!(photo.member_id, "member-1");
    assert_eq!(photo.content_type.as_deref(), Some("image/jpeg"));
    assert_eq!(photo.body, b"jpeg-photo".to_vec());

    let requests = transport.requests.lock().expect("requests mutex");
    assert_eq!(
        requests[1].url.as_str(),
        "https://p43-setup.icloud.com/setup/web/family/getMemberPhoto?clientId=auth-test-client&dsid=123456789&memberId=member-1",
    );
}

#[test]
fn blocking_client_rejects_family_member_photo_without_dsid() {
    let transport = MockBlockingTransport::default();
    let mut client = authenticated_client_with_transport(transport);
    let member = FamilyMember {
        dsid: None,
        original_invitation_email: None,
        full_name: Some("Missing DSID".to_owned()),
        age_classification: None,
        apple_id_for_purchases: None,
        apple_id: None,
        family_id: None,
        first_name: None,
        last_name: None,
        has_parental_privileges: None,
        has_screen_time_enabled: None,
        has_ask_to_buy_enabled: None,
        has_share_purchases_enabled: None,
        share_my_location_enabled_family_members: Vec::new(),
        has_share_my_location_enabled: None,
        dsid_for_purchases: None,
        extra: icloud_types::JsonObject::new(),
    };

    let error = client
        .family_member_photo(&member)
        .expect_err("missing dsid should fail");

    assert_eq!(
        error.to_string(),
        "invalid response: family member is missing `dsid`"
    );
}

#[test]
fn blocking_client_counts_and_deletes_photo_assets() {
    let transport = MockBlockingTransport::with_responses(vec![
        photos_album_count_response(),
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            br#"{"status":"ok"}"#.to_vec(),
        ),
    ]);
    let mut client = authenticated_client_with_transport(transport.clone());
    let asset = icloud_protocol::parse_photo_album_page(
        &photos_album_page_response(),
        &PhotoAlbumSummary::all_photos(),
        0,
        NonZeroU32::new(2).expect("non-zero"),
    )
    .expect("fixture should parse")
    .assets
    .into_iter()
    .next()
    .expect("sample asset");

    let count = client
        .photo_album_count(&PhotoAlbumSummary::all_photos())
        .expect("count should load");
    let response = client.delete_photo(&asset).expect("delete should succeed");

    assert_eq!(count, 2);
    assert_eq!(response["status"], "ok");

    let requests = transport.requests.lock().expect("requests mutex");
    assert_eq!(
        requests[0].url.as_str(),
        "https://p43-ckdatabasews.icloud.com/database/1/com.apple.photos.cloud/production/private/internal/records/query/batch?clientId=auth-test-client&dsid=123456789&remapEnums=true&getCurrentSyncToken=true",
    );
    let body = ResponseParts::new(
        StatusCode::OK,
        HeaderMap::new(),
        requests[1]
            .body
            .as_bytes()
            .expect("delete request should have a body")
            .to_vec(),
    )
    .json_value()
    .expect("delete request body should be valid json");
    assert_eq!(
        body["operations"][0]["record"]["recordName"],
        "asset-photo-1"
    );
    assert_eq!(
        body["operations"][0]["record"]["recordChangeTag"],
        "asset-tag-1",
    );
}

#[tokio::test]
async fn async_client_pages_and_downloads_photos() {
    let transport = MockAsyncTransport::with_responses(vec![
        photos_ready_response(),
        photos_album_page_response(),
        ResponseParts::new(StatusCode::OK, HeaderMap::new(), b"jpeg-bytes".to_vec()),
    ]);
    let mut client = authenticated_async_client_with_transport(transport);

    let album = client
        .all_photos_album()
        .await
        .expect("all photos album should be available");
    let page = client
        .photo_album_page(&album, 0, NonZeroU32::new(2).expect("non-zero"))
        .await
        .expect("page should load");
    let response = client
        .download_photo_version(&page.assets[0], PhotoAssetVersionKind::Original)
        .await
        .expect("download should succeed");

    assert_eq!(page.assets.len(), 2);
    assert_eq!(page.assets[0].filename, "IMG_0001.JPG");
    assert_eq!(page.next_offset, Some(3));
    assert_eq!(response.body, b"jpeg-bytes".to_vec());
}

#[tokio::test]
async fn async_client_loads_all_contacts() {
    let transport = MockAsyncTransport::with_responses(vec![
        contacts_startup_response(),
        contacts_page_response(),
    ]);
    let mut client = authenticated_async_client_with_transport(transport);

    let contacts = client.contacts_all().await.expect("contacts should load");

    assert_eq!(contacts.len(), 2);
    assert_eq!(contacts[0].emails[0].address, "ada@example.com");
    assert_eq!(contacts[1].phones[0].number, "+1-555-0100");
}

#[tokio::test]
async fn async_client_loads_calendar_event_detail() {
    let transport = MockAsyncTransport::with_responses(vec![calendar_event_detail_response()]);
    let mut client = authenticated_async_client_with_transport(transport.clone());

    let detail = client
        .calendar_event_detail("calendar-1", "event-1")
        .await
        .expect("calendar detail should load");

    assert_eq!(detail.event.guid, "event-1");
    assert_eq!(detail.attendees.len(), 2);
    assert_eq!(detail.attendees[0].status.as_deref(), Some("ACCEPTED"));

    let requests = transport.requests.lock().expect("requests mutex");
    assert_eq!(
        requests[0].url.as_str(),
        "https://p43-calendarws.icloud.com/ca/eventdetail/calendar-1/event-1?clientId=auth-test-client&dsid=123456789&lang=en-us&usertz=UTC",
    );
}

#[tokio::test]
async fn async_client_loads_reminder_lists() {
    let transport = MockAsyncTransport::with_responses(vec![reminders_response()]);
    let mut client = authenticated_async_client_with_transport(transport);

    let lists = client.reminder_lists().await.expect("lists should load");

    assert_eq!(lists.len(), 2);
    assert_eq!(lists[1].guid, "list-2");
    assert_eq!(lists[1].reminders[0].guid, "task-2");
}

#[tokio::test]
async fn async_client_loads_account_storage() {
    let transport = MockAsyncTransport::with_responses(vec![account_storage_response()]);
    let mut client = authenticated_async_client_with_transport(transport);

    let storage = client.account_storage().await.expect("storage should load");

    assert_eq!(storage.usage.used_storage_in_bytes, 5_368_709_120);
    assert_eq!(storage.usage.available_storage_in_bytes(), 16_106_127_360);
    assert_eq!(storage.usages_by_media.len(), 2);
}
