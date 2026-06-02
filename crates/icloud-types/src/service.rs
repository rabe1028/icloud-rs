use alloc::{collections::BTreeMap, string::String};
use core::fmt;

use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ServiceId {
    Account,
    Calendar,
    Contacts,
    CkDatabase,
    Docs,
    Drive,
    FindMy,
    Photos,
    Reminders,
    Ubiquity,
    Custom(String),
}

impl ServiceId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Account => "account",
            Self::Calendar => "calendar",
            Self::Contacts => "contacts",
            Self::CkDatabase => "ckdatabasews",
            Self::Docs => "docws",
            Self::Drive => "drivews",
            Self::FindMy => "findme",
            Self::Photos => "photosupload",
            Self::Reminders => "reminders",
            Self::Ubiquity => "ubiquity",
            Self::Custom(name) => name.as_str(),
        }
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for ServiceId {
    fn from(value: String) -> Self {
        match value.as_str() {
            "account" => Self::Account,
            "calendar" => Self::Calendar,
            "contacts" => Self::Contacts,
            "ckdatabasews" => Self::CkDatabase,
            "docws" => Self::Docs,
            "drivews" => Self::Drive,
            "findme" => Self::FindMy,
            "photosupload" | "photos" => Self::Photos,
            "reminders" => Self::Reminders,
            "ubiquity" => Self::Ubiquity,
            _ => Self::Custom(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceStatus {
    Active,
    Inactive,
    Unknown(String),
}

impl ServiceStatus {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for ServiceStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "active" => Self::Active,
            "inactive" => Self::Inactive,
            _ => Self::Unknown(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebService {
    pub url: Url,
    pub status: ServiceStatus,
}

pub type WebServiceMap = BTreeMap<ServiceId, WebService>;
