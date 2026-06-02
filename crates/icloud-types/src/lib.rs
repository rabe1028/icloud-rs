#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod prelude {
    pub(crate) use alloc::{
        borrow::ToOwned,
        format,
        string::{String, ToString},
        vec,
        vec::Vec,
    };
}

mod account;
mod auth;
mod calendar;
mod contacts;
mod context;
mod cookie;
mod drive;
mod error;
mod findmy;
mod json;
mod photos;
mod reminders;
mod service;
mod ubiquity;

pub use account::{
    AccountDevice, AccountMemberPhoto, AccountQuotaStatus, AccountStorage, AccountStorageUsage,
    AccountStorageUsageForMedia, FamilyMember,
};
pub use auth::{
    AccountBootstrap, AuthState, AuthenticatedSession, ChallengeHeaders, ClientStateSnapshot,
    Credentials, EstablishedSession, PendingTwoFactorAuth, SessionSeed,
};
pub use calendar::{CalendarAttendee, CalendarEvent, CalendarEventDetail, CalendarEventsResponse};
pub use contacts::{ContactCard, ContactEmail, ContactPhone, ContactsPage, ContactsStartup};
pub use context::{ClientContext, ClientIdentity, ClientProfile, EndpointSet};
pub use cookie::{CookieIdentity, CookieJar, CookiePair, StoredCookie};
pub use drive::{
    DriveContainer, DriveContainerContents, DriveContainerKind, DriveContainerUnavailable,
    DriveDocumentId, DriveDownload, DriveDownloadTarget, DriveDownloadToken, DriveFile, DriveNode,
    DriveNodeMetadata, DriveUnknownNode, DriveUploadFile, DriveUploadReceipt, DriveUploadTicket,
    DriveWsId,
};
pub use error::{ICloudApiError, ICloudApiErrorCode, ICloudError, JsonError};
pub use findmy::{
    FindMyBatteryStatus, FindMyDevice, FindMyDevicesResponse, FindMyLocation,
    FindMyLostModeCommand, FindMyMessageCommand, FindMyRefreshOptions, FindMySelectedDevice,
    FindMySoundCommand,
};
pub use json::{JsonNumber, JsonObject, JsonValue, RawJson};
pub use photos::{
    PhotoAlbumDirection, PhotoAlbumFilter, PhotoAlbumIdentity, PhotoAlbumPage, PhotoAlbumQuery,
    PhotoAlbumSummary, PhotoAsset, PhotoAssetVersion, PhotoAssetVersionKind, PhotoAssetVersions,
    PhotoDeleteTarget, PhotoDimensions, PhotoLibraryState, PhotoMediaKind, PhotoSmartAlbumKind,
};
pub use reminders::{ReminderItem, ReminderList, RemindersResponse};
pub use service::{ServiceId, ServiceStatus, WebService, WebServiceMap};
pub use ubiquity::{
    UbiquityFile, UbiquityFolder, UbiquityNode, UbiquityNodeId, UbiquityNodeMetadata,
    UbiquityUnknownNode,
};
