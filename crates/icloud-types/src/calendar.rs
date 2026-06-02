use crate::prelude::*;

use crate::JsonValue as Value;
use alloc::collections::BTreeMap as Map;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarEvent {
    pub guid: String,
    pub title: Option<String>,
    pub start_at: OffsetDateTime,
    pub end_at: OffsetDateTime,
    pub all_day: bool,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub calendar_name: Option<String>,
    pub calendar_guid: Option<String>,
    pub recurrence: Option<String>,
    pub raw: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarAttendee {
    pub name: Option<String>,
    pub email: Option<String>,
    pub status: Option<String>,
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarEventsResponse {
    pub events: Vec<CalendarEvent>,
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarEventDetail {
    pub event: CalendarEvent,
    pub attendees: Vec<CalendarAttendee>,
    pub extra: Map<String, Value>,
}
