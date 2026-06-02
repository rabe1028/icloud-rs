use crate::prelude::*;

use crate::JsonValue as Value;
use alloc::collections::BTreeMap as Map;

#[derive(Debug, Clone, PartialEq)]
pub struct ContactsStartup {
    pub pref_token: String,
    pub sync_token: String,
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContactEmail {
    pub label: Option<String>,
    pub address: String,
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContactPhone {
    pub label: Option<String>,
    pub number: String,
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContactCard {
    pub id: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub display_name: Option<String>,
    pub middle_name: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub nickname: Option<String>,
    pub company_name: Option<String>,
    pub emails: Vec<ContactEmail>,
    pub phones: Vec<ContactPhone>,
    pub raw: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContactsPage {
    pub contacts: Vec<ContactCard>,
    pub sync_token: Option<String>,
    pub extra: Map<String, Value>,
}
