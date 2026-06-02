use crate::prelude::*;

use crate::Method;
use crate::json::{Map, Value};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;

use icloud_types::{
    CalendarAttendee, CalendarEvent, CalendarEventDetail, CalendarEventsResponse, ICloudError,
};

use crate::{RequestSpec, ResponseParts};

const DEFAULT_USER_TIMEZONE: &str = "UTC";

#[derive(Debug, Clone)]
pub struct CalendarProtocol<'a> {
    service_root: &'a Url,
    client_id: &'a str,
    dsid: &'a str,
}

impl<'a> CalendarProtocol<'a> {
    #[must_use]
    pub fn new(service_root: &'a Url, client_id: &'a str, dsid: &'a str) -> Self {
        Self {
            service_root,
            client_id,
            dsid,
        }
    }

    pub fn events(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> Result<RequestSpec, ICloudError> {
        let mut url = self.service_root.join("ca/events")?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("clientId", self.client_id);
            query.append_pair("dsid", self.dsid);
            query.append_pair("lang", "en-us");
            query.append_pair("usertz", DEFAULT_USER_TIMEZONE);
            query.append_pair("startDate", date_parameter(start).as_str());
            query.append_pair("endDate", date_parameter(end).as_str());
        }
        Ok(RequestSpec::new(Method::Get, url))
    }

    pub fn event_detail(
        &self,
        calendar_guid: &str,
        event_guid: &str,
    ) -> Result<RequestSpec, ICloudError> {
        let mut url = self
            .service_root
            .join(format!("ca/eventdetail/{calendar_guid}/{event_guid}").as_str())?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("clientId", self.client_id);
            query.append_pair("dsid", self.dsid);
            query.append_pair("lang", "en-us");
            query.append_pair("usertz", DEFAULT_USER_TIMEZONE);
        }
        Ok(RequestSpec::new(Method::Get, url))
    }
}

pub fn parse_calendar_events(
    response: &ResponseParts,
) -> Result<CalendarEventsResponse, ICloudError> {
    let mut payload = response_object(response)?;
    let events = extract_event_maps(&mut payload)?
        .into_iter()
        .map(parse_calendar_event)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CalendarEventsResponse {
        events,
        extra: payload,
    })
}

pub fn parse_calendar_event_detail(
    response: &ResponseParts,
) -> Result<CalendarEventDetail, ICloudError> {
    let mut payload = response_object(response)?;
    let event = parse_calendar_event(extract_event_map(&mut payload)?)?;
    let attendees = extract_attendee_maps(&mut payload)?
        .into_iter()
        .map(parse_calendar_attendee)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CalendarEventDetail {
        event,
        attendees,
        extra: payload,
    })
}

fn response_object(response: &ResponseParts) -> Result<Map<String, Value>, ICloudError> {
    let payload: Value = response.json_value_checked()?;
    payload
        .as_object()
        .cloned()
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: "calendar response payload is not an object".to_owned(),
        })
}

fn extract_event_maps(
    payload: &mut Map<String, Value>,
) -> Result<Vec<Map<String, Value>>, ICloudError> {
    let value = remove_any(payload, &["events", "Events", "event", "Event"])
        .unwrap_or(Value::Array(Vec::new()));
    value_to_objects(value, "calendar events")
}

fn extract_event_map(payload: &mut Map<String, Value>) -> Result<Map<String, Value>, ICloudError> {
    if let Some(value) = remove_any(payload, &["event", "Event", "eventDetail", "eventDetails"]) {
        let mut events = value_to_objects(value, "calendar event detail")?;
        return events.pop().ok_or_else(|| ICloudError::InvalidResponse {
            message: "calendar event detail response did not include an event".to_owned(),
        });
    }

    if looks_like_event(payload) {
        return Ok(core::mem::take(payload));
    }

    Err(ICloudError::InvalidResponse {
        message: "calendar event detail response did not include an event".to_owned(),
    })
}

fn extract_attendee_maps(
    payload: &mut Map<String, Value>,
) -> Result<Vec<Map<String, Value>>, ICloudError> {
    let Some(value) = remove_any(payload, &["attendees", "Attendees", "invitees", "Invitees"])
    else {
        return Ok(Vec::new());
    };
    value_to_objects(value, "calendar attendees")
}

fn value_to_objects(value: Value, context: &str) -> Result<Vec<Map<String, Value>>, ICloudError> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Array(values) => values
            .into_iter()
            .map(|value| {
                value
                    .as_object()
                    .cloned()
                    .ok_or_else(|| ICloudError::InvalidResponse {
                        message: format!("{context} entry is not an object"),
                    })
            })
            .collect(),
        Value::Object(object) => Ok(vec![object]),
        other => Err(ICloudError::InvalidResponse {
            message: format!("{context} payload is not an array or object: {other}"),
        }),
    }
}

fn parse_calendar_event(raw: Map<String, Value>) -> Result<CalendarEvent, ICloudError> {
    let guid = string_field_any(&raw, &["eventGuid", "guid", "identifier"]).ok_or_else(|| {
        ICloudError::InvalidResponse {
            message: "calendar event is missing an identifier".to_owned(),
        }
    })?;

    Ok(CalendarEvent {
        guid,
        title: string_field_any(&raw, &["title", "summary"]),
        start_at: required_timestamp_field(&raw, &["startDate", "startTime"], "calendar event")?,
        end_at: required_timestamp_field(&raw, &["endDate", "endTime"], "calendar event")?,
        all_day: bool_field_any(&raw, &["allDay"]).unwrap_or(false),
        location: string_field_any(&raw, &["location"]),
        notes: string_field_any(&raw, &["notes", "description"]),
        calendar_name: string_field_any(&raw, &["calendarName", "calendarTitle"]),
        calendar_guid: string_field_any(&raw, &["calendarGuid", "calendarId"]),
        recurrence: string_field_any(&raw, &["recurrence", "rrule"]),
        raw,
    })
}

fn parse_calendar_attendee(raw: Map<String, Value>) -> Result<CalendarAttendee, ICloudError> {
    Ok(CalendarAttendee {
        name: string_field_any(&raw, &["name", "commonName"]),
        email: string_field_any(&raw, &["email", "emailAddress"]),
        status: string_field_any(&raw, &["status", "participationStatus"]),
        extra: raw,
    })
}

fn required_timestamp_field(
    raw: &Map<String, Value>,
    keys: &[&str],
    context: &str,
) -> Result<OffsetDateTime, ICloudError> {
    let value = value_field_any(raw, keys).ok_or_else(|| ICloudError::InvalidResponse {
        message: format!("{context} is missing `{}`", keys[0]),
    })?;
    parse_timestamp_value(value, context)
}

fn parse_timestamp_value(value: &Value, context: &str) -> Result<OffsetDateTime, ICloudError> {
    match value {
        Value::Number(number) => crate::timeutil::datetime_from_number(number, context),
        Value::String(value) => {
            if let Ok(millis) = value.parse::<i64>() {
                crate::timeutil::datetime_from_millis(millis, context)
            } else {
                OffsetDateTime::parse(value, &Rfc3339).map_err(|error| {
                    ICloudError::InvalidResponse {
                        message: format!("invalid {context} timestamp: {error}"),
                    }
                })
            }
        }
        other => Err(ICloudError::InvalidResponse {
            message: format!("{context} timestamp is not a number or string: {other}"),
        }),
    }
}

fn string_field_any(raw: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| match raw.get(*key) {
        Some(Value::String(value)) => Some(value.clone()),
        _ => None,
    })
}

fn bool_field_any(raw: &Map<String, Value>, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| match raw.get(*key) {
        Some(Value::Bool(value)) => Some(*value),
        _ => None,
    })
}

fn value_field_any<'a>(raw: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| raw.get(*key))
}

fn remove_any(raw: &mut Map<String, Value>, keys: &[&str]) -> Option<Value> {
    keys.iter().find_map(|key| raw.remove(*key))
}

fn looks_like_event(raw: &Map<String, Value>) -> bool {
    (raw.contains_key("eventGuid") || raw.contains_key("guid"))
        && (raw.contains_key("startDate") || raw.contains_key("startTime"))
}

fn date_parameter(timestamp: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        timestamp.year(),
        u8::from(timestamp.month()),
        timestamp.day()
    )
}

#[cfg(test)]
mod tests {
    use crate::{HeaderMap, StatusCode};
    use pretty_assertions::assert_eq;
    use time::macros::datetime;

    use super::*;

    #[test]
    fn events_request_includes_calendar_window_params() {
        let root = Url::parse("https://p43-calendarws.icloud.com").expect("valid url");
        let protocol = CalendarProtocol::new(&root, "client-123", "42");

        let request = protocol
            .events(
                datetime!(2024-06-01 0:00 UTC),
                datetime!(2024-07-01 0:00 UTC),
            )
            .expect("request should build");

        assert_eq!(
            request.url.as_str(),
            "https://p43-calendarws.icloud.com/ca/events?clientId=client-123&dsid=42&lang=en-us&usertz=UTC&startDate=2024-06-01&endDate=2024-07-01",
        );
    }

    #[test]
    fn event_detail_request_includes_calendar_and_event_guid() {
        let root = Url::parse("https://p43-calendarws.icloud.com").expect("valid url");
        let protocol = CalendarProtocol::new(&root, "client-123", "42");

        let request = protocol
            .event_detail("calendar-1", "event-1")
            .expect("request should build");

        assert_eq!(
            request.url.as_str(),
            "https://p43-calendarws.icloud.com/ca/eventdetail/calendar-1/event-1?clientId=client-123&dsid=42&lang=en-us&usertz=UTC",
        );
    }

    #[test]
    fn parser_extracts_calendar_events() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/calendar/events.json").to_vec(),
        );

        let events = parse_calendar_events(&response).expect("events should parse");

        assert_eq!(events.events.len(), 2);
        assert_eq!(events.events[0].guid, "event-1");
        assert_eq!(events.events[0].location.as_deref(), Some("Room A"));
        assert_eq!(events.events[1].guid, "event-2");
        assert!(events.events[1].all_day);
    }

    #[test]
    fn parser_extracts_calendar_event_detail() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/calendar/event_detail.json").to_vec(),
        );

        let detail = parse_calendar_event_detail(&response).expect("detail should parse");

        assert_eq!(detail.event.guid, "event-1");
        assert_eq!(detail.event.notes.as_deref(), Some("Bring status updates"));
        assert_eq!(detail.attendees.len(), 2);
        assert_eq!(
            detail.attendees[1].email.as_deref(),
            Some("grace@example.com")
        );
    }
}
