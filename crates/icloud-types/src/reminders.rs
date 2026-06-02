use crate::prelude::*;

use crate::JsonValue as Value;
use alloc::collections::BTreeMap as Map;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct ReminderItem {
    pub guid: String,
    pub title: Option<String>,
    pub notes: Option<String>,
    pub due_at: Option<OffsetDateTime>,
    pub completed: bool,
    pub priority: Option<i64>,
    pub list_guid: Option<String>,
    pub raw: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReminderList {
    pub guid: String,
    pub title: Option<String>,
    pub color: Option<String>,
    pub reminders: Vec<ReminderItem>,
    pub raw: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemindersResponse {
    pub lists: Vec<ReminderList>,
    pub extra: Map<String, Value>,
}
