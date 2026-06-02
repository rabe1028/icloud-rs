//! Transport-agnostic request builders and response parsers for the iCloud
//! web services.
//!
//! Each service exposes a `*Protocol` type whose methods build a
//! [`RequestSpec`] (URL, method, headers, body) without performing any I/O, and
//! a set of `parse_*` free functions that turn a [`ResponseParts`] into the
//! strongly typed models from `icloud-types`. Driving the actual HTTP exchange
//! is left to a separate transport layer (see `icloud-client`), which keeps
//! this crate `no_std`-compatible and free of any async runtime.
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
mod drive;
mod findmy;
mod json;
mod photos;
mod reminders;
mod request;
mod timeutil;
mod ubiquity;

pub use account::{
    AccountProtocol, parse_account_devices, parse_account_storage, parse_family_members,
};
pub use auth::{
    AuthProtocol, SignInOutcome, parse_account_bootstrap, parse_challenge_headers,
    parse_session_seed, parse_sign_in_response,
};
pub use calendar::{CalendarProtocol, parse_calendar_event_detail, parse_calendar_events};
pub use contacts::{ContactsProtocol, parse_contacts_page, parse_contacts_startup};
pub use drive::{
    DriveProtocol, parse_drive_app_libraries, parse_drive_container,
    parse_drive_container_with_seed, parse_drive_download, parse_drive_upload_receipt,
    parse_drive_upload_ticket,
};
pub use findmy::{FindMyProtocol, parse_findmy_devices};
pub use photos::{
    PhotosProtocol, parse_photo_album_count, parse_photo_album_folders, parse_photo_album_page,
    parse_photo_albums, parse_photo_library_state,
};
pub use reminders::{RemindersProtocol, parse_reminders};
pub use request::{
    HeaderMap, HeaderName, HeaderValue, Method, RequestBody, RequestSpec, ResponseParts,
    StatusCode, header,
};
pub use ubiquity::{
    UbiquityProtocol, parse_ubiquity_children, parse_ubiquity_folder, parse_ubiquity_node,
};
