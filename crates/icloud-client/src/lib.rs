//! High-level iCloud client that drives the [`icloud-protocol`] request
//! builders and parsers over a pluggable transport.
//!
//! Two client surfaces are provided: [`AsyncICloudClient`] (for `async`
//! transports) and [`BlockingICloudClient`] (for synchronous ones). Both share
//! the same type-state authentication lifecycle, encoded in the type parameter
//! `S`:
//!
//! `LoggedOut` → [`sign_in`](AsyncICloudClient::sign_in) →
//! `PendingTwoFactor` (when 2FA is required) →
//! `AuthenticatedPreBootstrap` → [`bootstrap`](AsyncICloudClient::bootstrap) →
//! `Authenticated`.
//!
//! Each state only exposes the operations that are valid for it, so misuse such
//! as issuing a Drive request before bootstrapping is a compile error rather
//! than a runtime failure.
//!
//! [`icloud-protocol`]: icloud_protocol
mod error;
#[cfg(any(feature = "reqwest-async", feature = "reqwest-blocking"))]
mod reqwest_transport;
mod state;
mod support;
mod transport;

use support::{
    account_member_photo, execute_async, execute_blocking, family_member_id,
    findmy_device_or_error, merge_pending_two_factor_response, merge_photo_albums,
    photo_library_not_ready, require_etag, unix_timestamp_ms,
};

#[cfg(feature = "reqwest-async")]
pub use reqwest_transport::ReqwestAsyncTransport;
#[cfg(feature = "reqwest-blocking")]
pub use reqwest_transport::ReqwestBlockingTransport;

use std::num::NonZeroU32;

use secrecy::SecretString;
use time::{Duration, OffsetDateTime};

use icloud_protocol::{
    AccountProtocol, AuthProtocol, CalendarProtocol, ContactsProtocol, DriveProtocol,
    FindMyProtocol, HeaderMap, PhotosProtocol, RemindersProtocol, RequestSpec, ResponseParts,
    SignInOutcome, StatusCode, UbiquityProtocol, header, parse_account_bootstrap,
    parse_account_devices, parse_account_storage, parse_calendar_event_detail,
    parse_calendar_events, parse_contacts_page, parse_contacts_startup, parse_drive_app_libraries,
    parse_drive_container, parse_drive_container_with_seed, parse_drive_download,
    parse_drive_upload_receipt, parse_drive_upload_ticket, parse_family_members,
    parse_findmy_devices, parse_legacy_trusted_devices, parse_photo_album_count,
    parse_photo_album_folders, parse_photo_album_page, parse_photo_albums,
    parse_photo_library_state, parse_reminders, parse_sign_in_response, parse_ubiquity_children,
    parse_ubiquity_folder, parse_ubiquity_node,
};
use icloud_types::{
    AccountDevice, AccountMemberPhoto, AccountStorage, AuthState, AuthStatus, AuthenticatedSession,
    CalendarEventDetail, CalendarEventsResponse, ClientContext, ClientIdentity,
    ClientStateSnapshot, ContactCard, ContactsPage, ContactsStartup, CookieJar, Credentials,
    DriveContainer, DriveContainerContents, DriveDownload, DriveFile, DriveNode, DriveUploadFile,
    FamilyMember, FindMyDevice, FindMyDevicesResponse, FindMyLocation, FindMyLostModeCommand,
    FindMyMessageCommand, FindMyRefreshOptions, FindMySoundCommand, ICloudError, JsonValue,
    LegacyTrustedDevice, PhotoAlbumPage, PhotoAlbumSummary, PhotoAsset, PhotoAssetVersionKind,
    PhotoLibraryState, ReminderList, RemindersResponse, ServiceId, TrustedPhoneNumber,
    UbiquityFile, UbiquityFolder, UbiquityNode, UbiquityNodeId,
};

pub use error::ClientError;
pub use state::{
    Authenticated, AuthenticatedPreBootstrap, LoggedOut, PendingTwoFactor, RestoredAsyncClient,
    RestoredBlockingClient,
};
pub use transport::{AsyncTransport, BlockingTransport};

#[derive(Debug, Clone)]
pub struct AsyncICloudClient<T, S = LoggedOut> {
    core: ClientCore<T, S>,
}

#[derive(Debug, Clone)]
pub struct BlockingICloudClient<T, S = LoggedOut> {
    core: ClientCore<T, S>,
}

#[derive(Debug, Clone)]
struct ClientCore<T, S> {
    context: ClientContext,
    identity: ClientIdentity,
    transport: T,
    cookie_jar: CookieJar,
    state: S,
}

#[derive(Debug, Clone)]
pub enum AsyncSignInResult<T> {
    PendingTwoFactor(AsyncICloudClient<T, PendingTwoFactor>),
    Authenticated(AsyncICloudClient<T, AuthenticatedPreBootstrap>),
}

#[derive(Debug, Clone)]
pub enum BlockingSignInResult<T> {
    PendingTwoFactor(BlockingICloudClient<T, PendingTwoFactor>),
    Authenticated(BlockingICloudClient<T, AuthenticatedPreBootstrap>),
}

impl<T> AsyncICloudClient<T, LoggedOut> {
    #[must_use]
    pub fn new(context: ClientContext, identity: ClientIdentity, transport: T) -> Self {
        Self {
            core: ClientCore {
                context,
                identity,
                transport,
                cookie_jar: CookieJar::new(),
                state: LoggedOut,
            },
        }
    }

    #[must_use]
    pub fn random_identity(context: ClientContext, transport: T) -> Self {
        Self::new(context, ClientIdentity::random(), transport)
    }

    #[must_use]
    pub fn restore(
        context: ClientContext,
        transport: T,
        snapshot: ClientStateSnapshot,
    ) -> RestoredAsyncClient<T> {
        match snapshot.auth_state {
            AuthState::LoggedOut => RestoredAsyncClient::LoggedOut(AsyncICloudClient {
                core: ClientCore {
                    context,
                    identity: ClientIdentity::new(snapshot.client_id),
                    transport,
                    cookie_jar: snapshot.cookie_jar,
                    state: LoggedOut,
                },
            }),
            AuthState::PendingTwoFactor(pending) => {
                RestoredAsyncClient::PendingTwoFactor(AsyncICloudClient {
                    core: ClientCore {
                        context,
                        identity: ClientIdentity::new(snapshot.client_id),
                        transport,
                        cookie_jar: snapshot.cookie_jar,
                        state: PendingTwoFactor { pending },
                    },
                })
            }
            AuthState::AuthenticatedPreBootstrap(session) => {
                RestoredAsyncClient::AuthenticatedPreBootstrap(AsyncICloudClient {
                    core: ClientCore {
                        context,
                        identity: ClientIdentity::new(snapshot.client_id),
                        transport,
                        cookie_jar: snapshot.cookie_jar,
                        state: AuthenticatedPreBootstrap { session },
                    },
                })
            }
            AuthState::Authenticated(session) => {
                RestoredAsyncClient::Authenticated(AsyncICloudClient {
                    core: ClientCore {
                        context,
                        identity: ClientIdentity::new(snapshot.client_id),
                        transport,
                        cookie_jar: snapshot.cookie_jar,
                        state: Authenticated { session },
                    },
                })
            }
        }
    }
}

impl<T> BlockingICloudClient<T, LoggedOut> {
    #[must_use]
    pub fn new(context: ClientContext, identity: ClientIdentity, transport: T) -> Self {
        Self {
            core: ClientCore {
                context,
                identity,
                transport,
                cookie_jar: CookieJar::new(),
                state: LoggedOut,
            },
        }
    }

    #[must_use]
    pub fn random_identity(context: ClientContext, transport: T) -> Self {
        Self::new(context, ClientIdentity::random(), transport)
    }

    #[must_use]
    pub fn restore(
        context: ClientContext,
        transport: T,
        snapshot: ClientStateSnapshot,
    ) -> RestoredBlockingClient<T> {
        match snapshot.auth_state {
            AuthState::LoggedOut => RestoredBlockingClient::LoggedOut(BlockingICloudClient {
                core: ClientCore {
                    context,
                    identity: ClientIdentity::new(snapshot.client_id),
                    transport,
                    cookie_jar: snapshot.cookie_jar,
                    state: LoggedOut,
                },
            }),
            AuthState::PendingTwoFactor(pending) => {
                RestoredBlockingClient::PendingTwoFactor(BlockingICloudClient {
                    core: ClientCore {
                        context,
                        identity: ClientIdentity::new(snapshot.client_id),
                        transport,
                        cookie_jar: snapshot.cookie_jar,
                        state: PendingTwoFactor { pending },
                    },
                })
            }
            AuthState::AuthenticatedPreBootstrap(session) => {
                RestoredBlockingClient::AuthenticatedPreBootstrap(BlockingICloudClient {
                    core: ClientCore {
                        context,
                        identity: ClientIdentity::new(snapshot.client_id),
                        transport,
                        cookie_jar: snapshot.cookie_jar,
                        state: AuthenticatedPreBootstrap { session },
                    },
                })
            }
            AuthState::Authenticated(session) => {
                RestoredBlockingClient::Authenticated(BlockingICloudClient {
                    core: ClientCore {
                        context,
                        identity: ClientIdentity::new(snapshot.client_id),
                        transport,
                        cookie_jar: snapshot.cookie_jar,
                        state: Authenticated { session },
                    },
                })
            }
        }
    }
}

macro_rules! impl_common_methods {
    ($client:ident) => {
        impl<T, S> $client<T, S>
        where
            S: state::StateSnapshot,
        {
            /// Captures the current typed client state.
            ///
            /// Snapshots contain cookies and authentication secrets. Enable the
            /// `serde` feature to encode them directly; encryption at rest remains
            /// the caller's responsibility.
            #[must_use]
            pub fn snapshot(&self) -> ClientStateSnapshot {
                self.core.snapshot()
            }

            #[must_use]
            pub fn auth_status(&self) -> AuthStatus {
                self.core.state.snapshot().status()
            }

            #[must_use]
            pub fn context(&self) -> &ClientContext {
                &self.core.context
            }

            #[must_use]
            pub fn identity(&self) -> &ClientIdentity {
                &self.core.identity
            }

            #[must_use]
            pub fn cookie_jar(&self) -> &CookieJar {
                &self.core.cookie_jar
            }

            #[must_use]
            pub fn state(&self) -> &S {
                &self.core.state
            }
        }
    };
}

impl_common_methods!(AsyncICloudClient);
impl_common_methods!(BlockingICloudClient);

/// Generates the authentication state-transition surface (sign-in, 2FA,
/// bootstrap, session validation) for both the async and the blocking
/// client from a single source.
macro_rules! impl_auth_transitions {
    ($client:ident, $transport:ident, ($($a:tt)*), ($($w:tt)*), $exec:ident, $result:ident) => {
impl<T> $client<T, LoggedOut>
where
    T: $transport,
{
    pub $($a)* fn sign_in(
        self,
        credentials: &Credentials,
    ) -> Result<$result<T>, ClientError<T::Error>> {
        self.sign_in_with_optional_trust_token(credentials, None)
             $($w)*
    }

    pub $($a)* fn sign_in_with_trust_token(
        self,
        credentials: &Credentials,
        trust_token: &str,
    ) -> Result<$result<T>, ClientError<T::Error>> {
        self.sign_in_with_optional_trust_token(credentials, Some(trust_token))
             $($w)*
    }

    $($a)* fn sign_in_with_optional_trust_token(
        mut self,
        credentials: &Credentials,
        trust_token: Option<&str>,
    ) -> Result<$result<T>, ClientError<T::Error>> {
        let protocol = AuthProtocol::new(&self.core.context, &self.core.identity);
        let request = protocol.sign_in(credentials, trust_token)?;
        let response = $exec(&mut self.core, request) $($w)* ?;
        match parse_sign_in_response(credentials.apple_id.as_str(), &response)? {
            SignInOutcome::PendingTwoFactor(pending) => {
                Ok($result::PendingTwoFactor($client {
                    core: self.core.transition(PendingTwoFactor { pending }),
                }))
            }
            SignInOutcome::Authenticated(mut session) => {
                if session.trust_token.is_none() {
                    session.trust_token =
                        trust_token.map(|token| SecretString::from(token.to_owned()));
                }
                Ok($result::Authenticated($client {
                    core: self.core.transition(AuthenticatedPreBootstrap { session }),
                }))
            }
        }
    }
}

impl<T> $client<T, PendingTwoFactor>
where
    T: $transport,
{
    #[must_use]
    pub fn trusted_phone_numbers(&self) -> &[TrustedPhoneNumber] {
        self.core.state.pending.trusted_phone_numbers.as_slice()
    }

    #[must_use]
    pub fn has_trusted_devices(&self) -> bool {
        self.core.state.pending.has_trusted_devices
    }

    pub $($a)* fn request_sms_verification_code(
        &mut self,
        phone_number: &TrustedPhoneNumber,
    ) -> Result<(), ClientError<T::Error>> {
        let request = {
            let protocol = AuthProtocol::new(&self.core.context, &self.core.identity);
            protocol.request_sms_code(&self.core.state.pending, phone_number)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        merge_pending_two_factor_response(&mut self.core.state.pending, &response)?;
        Ok(())
    }

    pub $($a)* fn submit_trusted_device_code(
        mut self,
        code: &str,
    ) -> Result<$client<T, AuthenticatedPreBootstrap>, ClientError<T::Error>> {
        let mut pending = self.core.state.pending.clone();

        let verify_request = {
            let protocol = AuthProtocol::new(&self.core.context, &self.core.identity);
            protocol.verify_trusted_device_code(&pending, code)?
        };
        let verify_response = $exec(&mut self.core, verify_request) $($w)* ?;
        verify_response.ensure_success()?;
        merge_pending_two_factor_response(&mut pending, &verify_response)?;

        let trust_request = {
            let protocol = AuthProtocol::new(&self.core.context, &self.core.identity);
            protocol.trust_session(&pending)?
        };
        let trust_response = $exec(&mut self.core, trust_request) $($w)* ?;
        trust_response.ensure_success()?;
        merge_pending_two_factor_response(&mut pending, &trust_response)?;

        let session = pending
            .seed
            .try_into_established(Some(pending.challenge.clone()))?;
        Ok($client {
            core: self.core.transition(AuthenticatedPreBootstrap { session }),
        })
    }

    pub $($a)* fn submit_sms_verification_code(
        mut self,
        phone_number: &TrustedPhoneNumber,
        code: &str,
    ) -> Result<$client<T, AuthenticatedPreBootstrap>, ClientError<T::Error>> {
        let mut pending = self.core.state.pending.clone();

        let verify_request = {
            let protocol = AuthProtocol::new(&self.core.context, &self.core.identity);
            protocol.verify_sms_code(&pending, phone_number, code)?
        };
        let verify_response = $exec(&mut self.core, verify_request) $($w)* ?;
        verify_response.ensure_success()?;
        merge_pending_two_factor_response(&mut pending, &verify_response)?;

        let trust_request = {
            let protocol = AuthProtocol::new(&self.core.context, &self.core.identity);
            protocol.trust_session(&pending)?
        };
        let trust_response = $exec(&mut self.core, trust_request) $($w)* ?;
        trust_response.ensure_success()?;
        merge_pending_two_factor_response(&mut pending, &trust_response)?;

        let session = pending
            .seed
            .try_into_established(Some(pending.challenge.clone()))?;
        Ok($client {
            core: self.core.transition(AuthenticatedPreBootstrap { session }),
        })
    }
}

impl<T> $client<T, AuthenticatedPreBootstrap>
where
    T: $transport,
{
    pub $($a)* fn bootstrap(
        mut self,
    ) -> Result<$client<T, Authenticated>, ClientError<T::Error>> {
        let protocol = AuthProtocol::new(&self.core.context, &self.core.identity);
        let request = protocol.account_login(&self.core.state.session)?;
        let response = $exec(&mut self.core, request) $($w)* ?;
        let bootstrap = parse_account_bootstrap(&response)?;
        let established = self.core.state.session.clone();
        Ok($client {
            core: self.core.transition(Authenticated {
                session: AuthenticatedSession {
                    established,
                    bootstrap,
                },
            }),
        })
    }

    pub $($a)* fn validate_session(
        mut self,
    ) -> Result<$client<T, Authenticated>, ClientError<T::Error>> {
        let protocol = AuthProtocol::new(&self.core.context, &self.core.identity);
        let request = protocol.validate_session()?;
        let response = $exec(&mut self.core, request) $($w)* ?;
        let bootstrap = parse_account_bootstrap(&response)?;
        let established = self.core.state.session.clone();
        Ok($client {
            core: self.core.transition(Authenticated {
                session: AuthenticatedSession {
                    established,
                    bootstrap,
                },
            }),
        })
    }
}

    };
}

impl_auth_transitions!(
    AsyncICloudClient, AsyncTransport, (async), (.await), execute_async, AsyncSignInResult
);

impl_auth_transitions!(
    BlockingICloudClient,
    BlockingTransport,
    (),
    (),
    execute_blocking,
    BlockingSignInResult
);

/// Generates the `Authenticated` client surface for both the async and the
/// blocking client from a single source. The async-ness markers `($($a)*)`
/// (`async`/empty) and `($($w)*)` (`.await`/empty) plus the executor and the
/// matching free helper names are substituted per invocation.
macro_rules! impl_authenticated_methods {
    ($client:ident, $transport:ident, ($($a:tt)*), ($($w:tt)*), $exec:ident,
     $load_children:ident, $load_ubi:ident, $photo_state:ident,
     $photos_ready:ident, $load_albums:ident) => {
        impl<T> $client<T, Authenticated>
        where
            T: $transport,
        {
    /// Re-validates the current session and refreshes the cached bootstrap in
    /// place, keeping the client in the [`Authenticated`] state.
    pub $($a)* fn refresh_session(&mut self) -> Result<(), ClientError<T::Error>> {
        let request = {
            let protocol = AuthProtocol::new(&self.core.context, &self.core.identity);
            protocol.validate_session()?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        self.core.state.session.bootstrap = parse_account_bootstrap(&response)?;
        Ok(())
    }

    pub $($a)* fn legacy_trusted_devices(
        &mut self,
    ) -> Result<Vec<LegacyTrustedDevice>, ClientError<T::Error>> {
        let request = {
            let protocol = AuthProtocol::new(&self.core.context, &self.core.identity);
            protocol.legacy_trusted_devices(self.core.dsid())?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_legacy_trusted_devices(&response)?)
    }

    pub $($a)* fn request_legacy_verification_code(
        &mut self,
        device: &LegacyTrustedDevice,
    ) -> Result<bool, ClientError<T::Error>> {
        let request = {
            let protocol = AuthProtocol::new(&self.core.context, &self.core.identity);
            protocol.send_legacy_verification_code(self.core.dsid(), device)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        Ok(response_success_flag(&response).unwrap_or(true))
    }

    pub $($a)* fn submit_legacy_verification_code(
        &mut self,
        device: &LegacyTrustedDevice,
        code: &str,
    ) -> Result<(), ClientError<T::Error>> {
        let request = {
            let protocol = AuthProtocol::new(&self.core.context, &self.core.identity);
            protocol.validate_legacy_verification_code(self.core.dsid(), device, code)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        self.refresh_session() $($w)* ?;
        Ok(())
    }

    pub $($a)* fn refresh_findmy_devices(
        &mut self,
        options: &FindMyRefreshOptions,
    ) -> Result<FindMyDevicesResponse, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.findmy_protocol()?;
            protocol.refresh_client(options)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_findmy_devices(&response)?)
    }

    pub $($a)* fn refresh_findmy_device(
        &mut self,
        device_id: &str,
    ) -> Result<FindMyDevice, ClientError<T::Error>> {
        let response = self
            .refresh_findmy_devices(&FindMyRefreshOptions::for_device(device_id))
             $($w)* ?;
        Ok(findmy_device_or_error(response, device_id)?)
    }

    pub $($a)* fn refresh_findmy_location(
        &mut self,
        device_id: &str,
    ) -> Result<Option<FindMyLocation>, ClientError<T::Error>> {
        let device = self.refresh_findmy_device(device_id) $($w)* ?;
        Ok(device.location)
    }

    pub $($a)* fn play_findmy_sound(
        &mut self,
        command: &FindMySoundCommand,
    ) -> Result<(), ClientError<T::Error>> {
        let request = {
            let protocol = self.core.findmy_protocol()?;
            protocol.play_sound(command)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        Ok(())
    }

    pub $($a)* fn send_findmy_message(
        &mut self,
        command: &FindMyMessageCommand,
    ) -> Result<(), ClientError<T::Error>> {
        let request = {
            let protocol = self.core.findmy_protocol()?;
            protocol.send_message(command)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        Ok(())
    }

    pub $($a)* fn enter_findmy_lost_mode(
        &mut self,
        command: &FindMyLostModeCommand,
    ) -> Result<(), ClientError<T::Error>> {
        let request = {
            let protocol = self.core.findmy_protocol()?;
            protocol.lost_device(command)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        Ok(())
    }

    pub $($a)* fn drive_root(&mut self) -> Result<DriveContainer, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.drive_protocol()?;
            protocol.retrieve_root()?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_drive_container(&response)?)
    }

    pub $($a)* fn drive_app_libraries(
        &mut self,
    ) -> Result<Vec<DriveContainer>, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.drive_protocol()?;
            protocol.retrieve_app_libraries()?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_drive_app_libraries(&response)?)
    }

    pub $($a)* fn drive_children(
        &mut self,
        container: &DriveContainer,
    ) -> Result<Vec<DriveNode>, ClientError<T::Error>> {
        $load_children(&mut self.core, container) $($w)*
    }

    pub $($a)* fn drive_child_by_name(
        &mut self,
        container: &DriveContainer,
        name: &str,
    ) -> Result<DriveNode, ClientError<T::Error>> {
        let children = self.drive_children(container) $($w)* ?;
        children
            .into_iter()
            .find(|child| child.name() == name)
            .ok_or_else(|| {
                ICloudError::DriveChildNotFound {
                    name: name.to_owned(),
                }
                .into()
            })
    }

    /// Downloads the contents of a Drive file.
    ///
    /// Empty files (`size == 0`) are short-circuited: iCloud exposes no content
    /// URL for them, so a synthetic empty `200 OK` [`ResponseParts`] is returned
    /// without performing any network request.
    pub $($a)* fn download_drive_file(
        &mut self,
        file: &DriveFile,
    ) -> Result<ResponseParts, ClientError<T::Error>> {
        if file.size == 0 {
            return Ok(ResponseParts::new(
                StatusCode::OK,
                HeaderMap::new(),
                Vec::new(),
            ));
        }

        let request = {
            let protocol = self.core.drive_protocol()?;
            protocol.download_by_id(file.document_id(), file.metadata.zone.as_str())?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        let download = parse_drive_download(&response)?;
        let request = {
            let protocol = self.core.drive_protocol()?;
            protocol.download_content(download.target.token())
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        Ok(response)
    }

    pub $($a)* fn resolve_drive_file_download(
        &mut self,
        file: &DriveFile,
    ) -> Result<DriveDownload, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.drive_protocol()?;
            protocol.download_by_id(file.document_id(), file.metadata.zone.as_str())?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_drive_download(&response)?)
    }

    pub $($a)* fn create_drive_folder(
        &mut self,
        parent: &DriveContainer,
        name: &str,
    ) -> Result<icloud_types::JsonValue, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.drive_protocol()?;
            protocol.create_folders(parent.drivewsid(), name)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        Ok(response.json_value()?)
    }

    pub $($a)* fn rename_drive_node(
        &mut self,
        node: &DriveNode,
        name: &str,
    ) -> Result<icloud_types::JsonValue, ClientError<T::Error>> {
        let etag = require_etag(node)?;
        let request = {
            let protocol = self.core.drive_protocol()?;
            protocol.rename_item(&node.metadata().drivewsid, etag, name)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        Ok(response.json_value()?)
    }

    pub $($a)* fn trash_drive_node(
        &mut self,
        node: &DriveNode,
    ) -> Result<icloud_types::JsonValue, ClientError<T::Error>> {
        let etag = require_etag(node)?;
        let request = {
            let protocol = self.core.drive_protocol()?;
            protocol.move_item_to_trash(&node.metadata().drivewsid, etag)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        Ok(response.json_value()?)
    }

    pub $($a)* fn upload_drive_file(
        &mut self,
        folder: &DriveContainer,
        file: &DriveUploadFile,
    ) -> Result<icloud_types::JsonValue, ClientError<T::Error>> {
        let upload_token = self.core.drive_upload_token()?;
        let upload_request = {
            let protocol = self.core.drive_protocol()?;
            protocol.begin_upload(upload_token.as_str(), folder.metadata.zone.as_str(), file)?
        };
        let upload_response = $exec(&mut self.core, upload_request) $($w)* ?;
        let ticket = parse_drive_upload_ticket(&upload_response)?;

        let content_request = {
            let protocol = self.core.drive_protocol()?;
            protocol.upload_content(&ticket.content_url, file)?
        };
        let content_response = $exec(&mut self.core, content_request) $($w)* ?;
        let receipt = parse_drive_upload_receipt(&content_response)?;

        let timestamp_ms = unix_timestamp_ms()?;
        let commit_request = {
            let protocol = self.core.drive_protocol()?;
            protocol.commit_upload(
                folder.document_id(),
                folder.metadata.zone.as_str(),
                &ticket,
                &receipt,
                file,
                timestamp_ms,
            )?
        };
        let commit_response = $exec(&mut self.core, commit_request) $($w)* ?;
        commit_response.ensure_success()?;
        Ok(commit_response.json_value()?)
    }

    pub $($a)* fn ubiquity_root(&mut self) -> Result<UbiquityFolder, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.ubiquity_protocol()?;
            protocol.retrieve_root()?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_ubiquity_folder(&response)?)
    }

    pub $($a)* fn ubiquity_node(
        &mut self,
        node_id: &UbiquityNodeId,
    ) -> Result<UbiquityNode, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.ubiquity_protocol()?;
            protocol.retrieve_node(node_id)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_ubiquity_node(&response)?)
    }

    pub $($a)* fn ubiquity_children(
        &mut self,
        folder: &UbiquityFolder,
    ) -> Result<Vec<UbiquityNode>, ClientError<T::Error>> {
        $load_ubi(&mut self.core, folder) $($w)*
    }

    pub $($a)* fn ubiquity_child_by_name(
        &mut self,
        folder: &UbiquityFolder,
        name: &str,
    ) -> Result<UbiquityNode, ClientError<T::Error>> {
        let children = self.ubiquity_children(folder) $($w)* ?;
        children
            .into_iter()
            .find(|child| child.name() == name)
            .ok_or_else(|| {
                ICloudError::UbiquityChildNotFound {
                    name: name.to_owned(),
                }
                .into()
            })
    }

    pub $($a)* fn download_ubiquity_file(
        &mut self,
        file: &UbiquityFile,
    ) -> Result<ResponseParts, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.ubiquity_protocol()?;
            protocol.download_file(file.item_id())?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        Ok(response)
    }

    pub $($a)* fn photos_state(&mut self) -> Result<PhotoLibraryState, ClientError<T::Error>> {
        $photo_state(&mut self.core) $($w)*
    }

    pub $($a)* fn calendar_events(
        &mut self,
    ) -> Result<CalendarEventsResponse, ClientError<T::Error>> {
        let start = OffsetDateTime::now_utc();
        self.calendar_events_between(start, start + Duration::days(30))
             $($w)*
    }

    pub $($a)* fn calendar_events_between(
        &mut self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> Result<CalendarEventsResponse, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.calendar_protocol()?;
            protocol.events(start, end)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_calendar_events(&response)?)
    }

    pub $($a)* fn calendar_event_detail(
        &mut self,
        calendar_guid: &str,
        event_guid: &str,
    ) -> Result<CalendarEventDetail, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.calendar_protocol()?;
            protocol.event_detail(calendar_guid, event_guid)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_calendar_event_detail(&response)?)
    }

    pub $($a)* fn account_devices(&mut self) -> Result<Vec<AccountDevice>, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.account_protocol()?;
            protocol.devices()?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_account_devices(&response)?)
    }

    pub $($a)* fn family_members(&mut self) -> Result<Vec<FamilyMember>, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.account_protocol()?;
            protocol.family()?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_family_members(&response)?)
    }

    pub $($a)* fn family_member_photo(
        &mut self,
        member: &FamilyMember,
    ) -> Result<AccountMemberPhoto, ClientError<T::Error>> {
        self.family_member_photo_by_id(family_member_id(member)?)
             $($w)*
    }

    pub $($a)* fn family_member_photo_by_id(
        &mut self,
        member_id: &str,
    ) -> Result<AccountMemberPhoto, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.account_protocol()?;
            protocol.family_member_photo(member_id)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        Ok(account_member_photo(member_id, response))
    }

    pub $($a)* fn account_storage(&mut self) -> Result<AccountStorage, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.account_protocol()?;
            protocol.storage()?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_account_storage(&response)?)
    }

    pub $($a)* fn contacts_startup(&mut self) -> Result<ContactsStartup, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.contacts_protocol()?;
            protocol.startup()?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_contacts_startup(&response)?)
    }

    pub $($a)* fn contacts_page(
        &mut self,
        startup: &ContactsStartup,
        limit: usize,
        offset: usize,
    ) -> Result<ContactsPage, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.contacts_protocol()?;
            protocol.contacts(
                startup.pref_token.as_str(),
                startup.sync_token.as_str(),
                limit,
                offset,
            )?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_contacts_page(&response)?)
    }

    pub $($a)* fn contacts_all(&mut self) -> Result<Vec<ContactCard>, ClientError<T::Error>> {
        let startup = self.contacts_startup() $($w)* ?;
        Ok(self.contacts_page(&startup, 0, 0) $($w)* ?.contacts)
    }

    pub $($a)* fn reminders(&mut self) -> Result<RemindersResponse, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.reminders_protocol()?;
            protocol.refresh()?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_reminders(&response)?)
    }

    pub $($a)* fn reminder_lists(&mut self) -> Result<Vec<ReminderList>, ClientError<T::Error>> {
        Ok(self.reminders() $($w)* ?.lists)
    }

    pub $($a)* fn photo_albums(&mut self) -> Result<Vec<PhotoAlbumSummary>, ClientError<T::Error>> {
        $photos_ready(&mut self.core) $($w)* ?;
        Ok(merge_photo_albums(
            $load_albums(&mut self.core) $($w)* ?,
        ))
    }

    pub $($a)* fn all_photos_album(&mut self) -> Result<PhotoAlbumSummary, ClientError<T::Error>> {
        $photos_ready(&mut self.core) $($w)* ?;
        Ok(PhotoAlbumSummary::all_photos())
    }

    pub $($a)* fn photo_album_count(
        &mut self,
        album: &PhotoAlbumSummary,
    ) -> Result<u64, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.photos_protocol()?;
            protocol.album_count(album)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_photo_album_count(&response)?)
    }

    pub $($a)* fn photo_album_page(
        &mut self,
        album: &PhotoAlbumSummary,
        start_rank: u64,
        page_size: NonZeroU32,
    ) -> Result<PhotoAlbumPage, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.photos_protocol()?;
            protocol.album_page(album, start_rank, page_size)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        Ok(parse_photo_album_page(
            &response, album, start_rank, page_size,
        )?)
    }

    pub $($a)* fn download_photo_version(
        &mut self,
        asset: &PhotoAsset,
        version: PhotoAssetVersionKind,
    ) -> Result<ResponseParts, ClientError<T::Error>> {
        let version_data =
            asset
                .version(version)
                .ok_or_else(|| ICloudError::PhotoVersionUnavailable {
                    kind: version.as_str().to_owned(),
                })?;
        let request = {
            let protocol = self.core.photos_protocol()?;
            protocol.download_version(version_data)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        Ok(response)
    }

    pub $($a)* fn delete_photo(
        &mut self,
        asset: &PhotoAsset,
    ) -> Result<icloud_types::JsonValue, ClientError<T::Error>> {
        let request = {
            let protocol = self.core.photos_protocol()?;
            protocol.delete_asset(asset)?
        };
        let response = $exec(&mut self.core, request) $($w)* ?;
        response.ensure_success()?;
        Ok(response.json_value()?)
    }
        }
    };
}

impl_authenticated_methods!(
    AsyncICloudClient, AsyncTransport, (async), (.await), execute_async,
    load_drive_children_async, load_ubiquity_children_async, fetch_photo_library_state_async,
    ensure_photos_ready_async, load_photo_albums_async
);

impl_authenticated_methods!(
    BlockingICloudClient,
    BlockingTransport,
    (),
    (),
    execute_blocking,
    load_drive_children_blocking,
    load_ubiquity_children_blocking,
    fetch_photo_library_state_blocking,
    ensure_photos_ready_blocking,
    load_photo_albums_blocking
);

impl<T, S> ClientCore<T, S>
where
    S: state::StateSnapshot,
{
    fn snapshot(&self) -> ClientStateSnapshot {
        ClientStateSnapshot {
            client_id: self.identity.client_id.clone(),
            cookie_jar: self.cookie_jar.clone(),
            auth_state: self.state.snapshot(),
        }
    }

    fn transition<Next>(self, state: Next) -> ClientCore<T, Next> {
        ClientCore {
            context: self.context,
            identity: self.identity,
            transport: self.transport,
            cookie_jar: self.cookie_jar,
            state,
        }
    }
}

impl<T, S> ClientCore<T, S> {
    fn prepare_request(&self, request: &mut RequestSpec) -> Result<(), ICloudError> {
        request.attach_cookies(self.cookie_jar.cookies_for_url(&request.url))
    }

    fn ingest_response(
        &mut self,
        request_url: &url::Url,
        response: &ResponseParts,
    ) -> Result<(), ICloudError> {
        for value in response.headers.get_all(header::SET_COOKIE) {
            let value = value
                .to_str()
                .map_err(|error| ICloudError::InvalidHeaderValue {
                    name: "set-cookie",
                    message: error.to_string(),
                })?;
            self.cookie_jar.apply_set_cookie(request_url, value)?;
        }
        Ok(())
    }
}

fn response_success_flag(response: &ResponseParts) -> Option<bool> {
    let JsonValue::Object(payload) = response.json_value().ok()? else {
        return None;
    };
    payload.get("success").and_then(JsonValue::as_bool)
}

impl<T> ClientCore<T, Authenticated> {
    fn dsid(&self) -> &str {
        self.state.session.bootstrap.dsid.as_str()
    }

    fn service_root(
        &self,
        service_id: ServiceId,
        service_name: &'static str,
    ) -> Result<&url::Url, ICloudError> {
        self.state
            .session
            .bootstrap
            .webservices
            .get(&service_id)
            .map(|service| &service.url)
            .ok_or(ICloudError::MissingService {
                service: service_name,
            })
    }

    fn account_protocol(&self) -> Result<AccountProtocol<'_>, ICloudError> {
        Ok(AccountProtocol::new(
            self.service_root(ServiceId::Account, "account")?,
            self.identity.client_id.as_str(),
            self.dsid(),
        ))
    }

    fn findmy_protocol(&self) -> Result<FindMyProtocol<'_>, ICloudError> {
        Ok(FindMyProtocol::new(
            self.service_root(ServiceId::FindMy, "findme")?,
            self.identity.client_id.as_str(),
            self.dsid(),
        ))
    }

    fn drive_protocol(&self) -> Result<DriveProtocol<'_>, ICloudError> {
        Ok(DriveProtocol::new(
            self.service_root(ServiceId::Drive, "drivews")?,
            self.service_root(ServiceId::Docs, "docws")?,
            self.identity.client_id.as_str(),
            self.dsid(),
        ))
    }

    fn ubiquity_protocol(&self) -> Result<UbiquityProtocol<'_>, ICloudError> {
        Ok(UbiquityProtocol::new(
            self.service_root(ServiceId::Ubiquity, "ubiquity")?,
            self.dsid(),
        ))
    }

    fn photos_protocol(&self) -> Result<PhotosProtocol<'_>, ICloudError> {
        Ok(PhotosProtocol::new(
            self.service_root(ServiceId::CkDatabase, "ckdatabasews")?,
            self.identity.client_id.as_str(),
            self.dsid(),
        ))
    }

    fn calendar_protocol(&self) -> Result<CalendarProtocol<'_>, ICloudError> {
        Ok(CalendarProtocol::new(
            self.service_root(ServiceId::Calendar, "calendar")?,
            self.identity.client_id.as_str(),
            self.dsid(),
        ))
    }

    fn contacts_protocol(&self) -> Result<ContactsProtocol<'_>, ICloudError> {
        Ok(ContactsProtocol::new(
            self.service_root(ServiceId::Contacts, "contacts")?,
            self.identity.client_id.as_str(),
            self.dsid(),
        ))
    }

    fn reminders_protocol(&self) -> Result<RemindersProtocol<'_>, ICloudError> {
        Ok(RemindersProtocol::new(
            self.service_root(ServiceId::Reminders, "reminders")?,
            self.identity.client_id.as_str(),
            self.dsid(),
        ))
    }

    fn drive_upload_token(&self) -> Result<String, ICloudError> {
        let cookie_value = self
            .cookie_jar
            .cookie_value("X-APPLE-WEBAUTH-VALIDATE")
            .ok_or(ICloudError::MissingCookie {
                name: "X-APPLE-WEBAUTH-VALIDATE",
            })?;
        DriveProtocol::extract_validate_token(cookie_value)
    }
}

/// Generates the shared `Authenticated` data-loading helpers for both the
/// async and the blocking client from a single source.
macro_rules! impl_authenticated_helpers {
    ($transport:ident, ($($a:tt)*), ($($w:tt)*), $exec:ident,
     $load_children:ident, $load_ubi:ident, $photo_state:ident,
     $photos_ready:ident, $load_albums:ident) => {
$($a)* fn $load_children<T>(
    core: &mut ClientCore<T, Authenticated>,
    container: &DriveContainer,
) -> Result<Vec<DriveNode>, ClientError<T::Error>>
where
    T: $transport,
{
    match &container.contents {
        DriveContainerContents::Loaded(children) => Ok(children.clone()),
        DriveContainerContents::Unavailable(unavailable) => {
            Err(ICloudError::DriveContainerUnavailable {
                status: unavailable.status.clone(),
            }
            .into())
        }
        DriveContainerContents::Omitted => {
            let request = {
                let protocol = core.drive_protocol()?;
                protocol.retrieve_item_details(container.drivewsid())?
            };
            let response = $exec(core, request) $($w)* ?;
            let refreshed = parse_drive_container_with_seed(&response, container)?;
            match refreshed.contents {
                DriveContainerContents::Loaded(children) => Ok(children),
                DriveContainerContents::Unavailable(unavailable) => {
                    Err(ICloudError::DriveContainerUnavailable {
                        status: unavailable.status,
                    }
                    .into())
                }
                DriveContainerContents::Omitted => Err(ICloudError::InvalidResponse {
                    message: "drive container detail response did not include child items"
                        .to_owned(),
                }
                .into()),
            }
        }
    }
}


$($a)* fn $load_ubi<T>(
    core: &mut ClientCore<T, Authenticated>,
    folder: &UbiquityFolder,
) -> Result<Vec<UbiquityNode>, ClientError<T::Error>>
where
    T: $transport,
{
    let request = {
        let protocol = core.ubiquity_protocol()?;
        protocol.retrieve_children(folder.item_id())?
    };
    let response = $exec(core, request) $($w)* ?;
    Ok(parse_ubiquity_children(&response)?)
}


$($a)* fn $photo_state<T>(
    core: &mut ClientCore<T, Authenticated>,
) -> Result<PhotoLibraryState, ClientError<T::Error>>
where
    T: $transport,
{
    let request = {
        let protocol = core.photos_protocol()?;
        protocol.indexing_state()?
    };
    let response = $exec(core, request) $($w)* ?;
    Ok(parse_photo_library_state(&response)?)
}


$($a)* fn $photos_ready<T>(
    core: &mut ClientCore<T, Authenticated>,
) -> Result<(), ClientError<T::Error>>
where
    T: $transport,
{
    let state = $photo_state(core) $($w)* ?;
    if state.is_ready() {
        Ok(())
    } else {
        Err(photo_library_not_ready(state).into())
    }
}


$($a)* fn $load_albums<T>(
    core: &mut ClientCore<T, Authenticated>,
) -> Result<Vec<PhotoAlbumSummary>, ClientError<T::Error>>
where
    T: $transport,
{
    let mut pending = vec![None::<String>];
    let mut albums = Vec::new();
    while let Some(parent_id) = pending.pop() {
        let request = {
            let protocol = core.photos_protocol()?;
            protocol.query_albums(parent_id.as_deref())?
        };
        let response = $exec(core, request) $($w)* ?;
        albums.extend(parse_photo_albums(&response)?);
        pending.extend(parse_photo_album_folders(&response)?.into_iter().map(Some));
    }
    Ok(albums)
}

    };
}

impl_authenticated_helpers!(
    AsyncTransport, (async), (.await), execute_async,
    load_drive_children_async, load_ubiquity_children_async, fetch_photo_library_state_async,
    ensure_photos_ready_async, load_photo_albums_async
);

impl_authenticated_helpers!(
    BlockingTransport,
    (),
    (),
    execute_blocking,
    load_drive_children_blocking,
    load_ubiquity_children_blocking,
    fetch_photo_library_state_blocking,
    ensure_photos_ready_blocking,
    load_photo_albums_blocking
);

#[cfg(test)]
mod tests;
