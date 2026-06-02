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
mod state;
mod transport;

use std::num::NonZeroU32;

use secrecy::SecretString;
use thiserror::Error;
use time::{Duration, OffsetDateTime};

use icloud_protocol::{
    AccountProtocol, AuthProtocol, CalendarProtocol, ContactsProtocol, DriveProtocol,
    FindMyProtocol, HeaderMap, PhotosProtocol, RemindersProtocol, RequestSpec, ResponseParts,
    SignInOutcome, StatusCode, UbiquityProtocol, header, parse_account_bootstrap,
    parse_account_devices, parse_account_storage, parse_calendar_event_detail,
    parse_calendar_events, parse_challenge_headers, parse_contacts_page, parse_contacts_startup,
    parse_drive_app_libraries, parse_drive_container, parse_drive_container_with_seed,
    parse_drive_download, parse_drive_upload_receipt, parse_drive_upload_ticket,
    parse_family_members, parse_findmy_devices, parse_photo_album_count, parse_photo_album_folders,
    parse_photo_album_page, parse_photo_albums, parse_photo_library_state, parse_reminders,
    parse_session_seed, parse_sign_in_response, parse_ubiquity_children, parse_ubiquity_folder,
    parse_ubiquity_node,
};
#[cfg(any(feature = "reqwest-async", feature = "reqwest-blocking"))]
use icloud_protocol::{HeaderName, HeaderValue, Method};
use icloud_types::{
    AccountDevice, AccountMemberPhoto, AccountStorage, AuthState, AuthenticatedSession,
    CalendarEventDetail, CalendarEventsResponse, ClientContext, ClientIdentity,
    ClientStateSnapshot, ContactCard, ContactsPage, ContactsStartup, CookieJar, Credentials,
    DriveContainer, DriveContainerContents, DriveDownload, DriveFile, DriveNode, DriveUploadFile,
    FamilyMember, FindMyDevice, FindMyDevicesResponse, FindMyLocation, FindMyLostModeCommand,
    FindMyMessageCommand, FindMyRefreshOptions, FindMySoundCommand, ICloudError,
    PendingTwoFactorAuth, PhotoAlbumPage, PhotoAlbumSummary, PhotoAsset, PhotoAssetVersionKind,
    PhotoLibraryState, ReminderList, RemindersResponse, ServiceId, UbiquityFile, UbiquityFolder,
    UbiquityNode, UbiquityNodeId,
};

pub use state::{
    Authenticated, AuthenticatedPreBootstrap, LoggedOut, PendingTwoFactor, RestoredAsyncClient,
    RestoredBlockingClient,
};
pub use transport::{AsyncTransport, BlockingTransport};

#[derive(Debug, Error)]
pub enum ClientError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[error(transparent)]
    ICloud(#[from] ICloudError),
    #[error("transport error: {0}")]
    Transport(#[source] E),
}

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
            /// The core crates intentionally do not provide snapshot persistence or a
            /// serialization format. Applications should decide how to encode,
            /// encrypt, and store this value.
            #[must_use]
            pub fn snapshot(&self) -> ClientStateSnapshot {
                self.core.snapshot()
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

async fn execute_async<T, S>(
    core: &mut ClientCore<T, S>,
    mut request: RequestSpec,
) -> Result<ResponseParts, ClientError<T::Error>>
where
    T: AsyncTransport,
{
    core.prepare_request(&mut request)?;
    let request_url = request.url.clone();
    let response = core
        .transport
        .execute(request)
        .await
        .map_err(ClientError::Transport)?;
    core.ingest_response(&request_url, &response)?;
    Ok(response)
}

fn execute_blocking<T, S>(
    core: &mut ClientCore<T, S>,
    mut request: RequestSpec,
) -> Result<ResponseParts, ClientError<T::Error>>
where
    T: BlockingTransport,
{
    core.prepare_request(&mut request)?;
    let request_url = request.url.clone();
    let response = core
        .transport
        .execute(request)
        .map_err(ClientError::Transport)?;
    core.ingest_response(&request_url, &response)?;
    Ok(response)
}

fn merge_pending_two_factor_response(
    pending: &mut PendingTwoFactorAuth,
    response: &ResponseParts,
) -> Result<(), ICloudError> {
    pending.seed.merge(parse_session_seed(response)?);
    if let Some(challenge) = parse_challenge_headers(response)? {
        pending.challenge = challenge;
    }
    Ok(())
}

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

fn require_etag(node: &DriveNode) -> Result<&str, ICloudError> {
    node.metadata()
        .etag
        .as_deref()
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: "drive node is missing an `etag`".to_owned(),
        })
}

fn findmy_device_or_error(
    response: FindMyDevicesResponse,
    device_id: &str,
) -> Result<FindMyDevice, ICloudError> {
    response
        .into_device(device_id)
        .ok_or_else(|| ICloudError::FindMyDeviceNotFound {
            id: device_id.to_owned(),
        })
}

fn merge_photo_albums(custom_albums: Vec<PhotoAlbumSummary>) -> Vec<PhotoAlbumSummary> {
    let mut albums = PhotoAlbumSummary::smart_albums();
    albums.extend(custom_albums);
    albums
}

fn photo_library_not_ready(state: PhotoLibraryState) -> ICloudError {
    ICloudError::PhotosLibraryNotReady {
        state: state.as_str().to_owned(),
    }
}

fn family_member_id(member: &FamilyMember) -> Result<&str, ICloudError> {
    member
        .dsid
        .as_deref()
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: "family member is missing `dsid`".to_owned(),
        })
}

fn account_member_photo(member_id: &str, response: ResponseParts) -> AccountMemberPhoto {
    let content_type = response
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    AccountMemberPhoto {
        member_id: member_id.to_owned(),
        body: response.body,
        content_type,
    }
}

fn unix_timestamp_ms() -> Result<i64, ICloudError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| ICloudError::InvalidResponse {
            message: format!("system clock is earlier than the unix epoch: {error}"),
        })?;
    duration
        .as_millis()
        .try_into()
        .map_err(|error| ICloudError::InvalidResponse {
            message: format!("current unix timestamp does not fit in i64: {error}"),
        })
}

#[cfg(test)]
mod tests {
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
        ClientContext::production(
            "d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d",
        )
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
                auth_state: AuthState::AuthenticatedPreBootstrap(
                    icloud_types::EstablishedSession {
                        session_token: SecretString::from("session-token".to_owned()),
                        account_country_code: "JPN".to_owned(),
                        trust_token: Some(SecretString::from("trust-token".to_owned())),
                        challenge: None,
                    },
                ),
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
}
