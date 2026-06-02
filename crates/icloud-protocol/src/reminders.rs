use crate::prelude::*;

use alloc::collections::BTreeMap;

use crate::Method;
use crate::json::{Map, Value};
use time::{
    Date, Month, OffsetDateTime, PrimitiveDateTime, Time, format_description::well_known::Rfc3339,
};
use url::Url;

use icloud_types::{ICloudError, ReminderItem, ReminderList, RemindersResponse};

use crate::{RequestSpec, ResponseParts};

type ReminderItemsByList = BTreeMap<Option<String>, Vec<Map<String, Value>>>;
const DEFAULT_USER_TIMEZONE: &str = "UTC";

#[derive(Debug, Clone)]
pub struct RemindersProtocol<'a> {
    service_root: &'a Url,
    client_id: &'a str,
    dsid: &'a str,
}

impl<'a> RemindersProtocol<'a> {
    #[must_use]
    pub fn new(service_root: &'a Url, client_id: &'a str, dsid: &'a str) -> Self {
        Self {
            service_root,
            client_id,
            dsid,
        }
    }

    pub fn refresh(&self) -> Result<RequestSpec, ICloudError> {
        let mut url = self.service_root.join("rd/startup")?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("clientId", self.client_id);
            query.append_pair("dsid", self.dsid);
            query.append_pair("clientVersion", "4.0");
            query.append_pair("lang", "en-us");
            query.append_pair("usertz", DEFAULT_USER_TIMEZONE);
        }
        Ok(RequestSpec::new(Method::Get, url))
    }
}

pub fn parse_reminders(response: &ResponseParts) -> Result<RemindersResponse, ICloudError> {
    let mut payload = response_object(response)?;
    let top_level_items =
        extract_items(&mut payload, &["reminders", "Reminders", "items", "tasks"])?;
    let grouped_items = group_items_by_list(top_level_items)?;

    let lists = extract_lists(&mut payload)?
        .into_iter()
        .map(|mut list| parse_list(&mut list, &grouped_items))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(RemindersResponse {
        lists,
        extra: payload,
    })
}

fn response_object(response: &ResponseParts) -> Result<Map<String, Value>, ICloudError> {
    let payload: Value = response.json_value_checked()?;
    payload
        .as_object()
        .cloned()
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: "reminders response payload is not an object".to_owned(),
        })
}

fn extract_lists(payload: &mut Map<String, Value>) -> Result<Vec<Map<String, Value>>, ICloudError> {
    let value =
        remove_any(payload, &["lists", "Lists", "Collections"]).unwrap_or(Value::Array(Vec::new()));
    value_to_objects(value, "reminder lists")
}

fn extract_items(
    payload: &mut Map<String, Value>,
    keys: &[&str],
) -> Result<Vec<Map<String, Value>>, ICloudError> {
    let Some(value) = remove_any(payload, keys) else {
        return Ok(Vec::new());
    };
    value_to_objects(value, "reminder items")
}

fn parse_list(
    raw: &mut Map<String, Value>,
    grouped_items: &ReminderItemsByList,
) -> Result<ReminderList, ICloudError> {
    let guid = string_field_any(raw, &["guid", "listGuid", "identifier"]).ok_or_else(|| {
        ICloudError::InvalidResponse {
            message: "reminder list is missing an identifier".to_owned(),
        }
    })?;

    let mut reminders = extract_items(raw, &["reminders", "Reminders", "items", "tasks"])?;
    if let Some(grouped) = grouped_items.get(&Some(guid.clone())) {
        reminders.extend(grouped.iter().cloned());
    }

    Ok(ReminderList {
        guid,
        title: string_field_any(raw, &["title", "name"]),
        color: string_field_any(raw, &["color", "hexColor"]),
        reminders: reminders
            .into_iter()
            .map(parse_item)
            .collect::<Result<Vec<_>, _>>()?,
        raw: raw.clone(),
    })
}

fn group_items_by_list(items: Vec<Map<String, Value>>) -> Result<ReminderItemsByList, ICloudError> {
    let mut grouped = ReminderItemsByList::new();
    for item in items {
        let key = string_field_any(
            &item,
            &["pGuid", "listGuid", "parentGuid", "collectionGuid"],
        );
        grouped.entry(key).or_default().push(item);
    }
    Ok(grouped)
}

fn parse_item(raw: Map<String, Value>) -> Result<ReminderItem, ICloudError> {
    let guid = string_field_any(&raw, &["guid", "reminderId", "identifier"]).ok_or_else(|| {
        ICloudError::InvalidResponse {
            message: "reminder item is missing an identifier".to_owned(),
        }
    })?;

    Ok(ReminderItem {
        guid,
        title: string_field_any(&raw, &["title", "summary"]),
        notes: string_field_any(&raw, &["notes", "description"]),
        due_at: optional_timestamp_field(&raw, &["dueDate", "due"])?,
        completed: bool_field_any(&raw, &["completed", "isCompleted"]).unwrap_or(false),
        priority: integer_field_any(&raw, &["priority"])?,
        list_guid: string_field_any(&raw, &["pGuid", "listGuid", "parentGuid", "collectionGuid"]),
        raw,
    })
}

fn optional_timestamp_field(
    raw: &Map<String, Value>,
    keys: &[&str],
) -> Result<Option<OffsetDateTime>, ICloudError> {
    value_field_any(raw, keys)
        .map(|value| parse_timestamp_value(value, "reminder"))
        .transpose()
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
        Value::Array(values) => offset_datetime_from_components(values.as_slice(), context),
        other => Err(ICloudError::InvalidResponse {
            message: format!("{context} timestamp is not a number, string, or array: {other}"),
        }),
    }
}

fn offset_datetime_from_components(
    values: &[Value],
    context: &str,
) -> Result<OffsetDateTime, ICloudError> {
    let year = component_i32(values, 1, context)?;
    let month = Month::try_from(component_u8(values, 2, context)?).map_err(|error| {
        ICloudError::InvalidResponse {
            message: format!("invalid {context} month: {error}"),
        }
    })?;
    let day = component_u8(values, 3, context)?;
    let hour = component_u8_or_default(values, 4, 0, context)?;
    let minute = component_u8_or_default(values, 5, 0, context)?;
    let date = Date::from_calendar_date(year, month, day).map_err(|error| {
        ICloudError::InvalidResponse {
            message: format!("invalid {context} date: {error}"),
        }
    })?;
    let time = Time::from_hms(hour, minute, 0).map_err(|error| ICloudError::InvalidResponse {
        message: format!("invalid {context} time: {error}"),
    })?;
    Ok(PrimitiveDateTime::new(date, time).assume_utc())
}

fn component_i32(values: &[Value], index: usize, context: &str) -> Result<i32, ICloudError> {
    let raw = component_i64(values, index, context)?;
    i32::try_from(raw).map_err(|error| ICloudError::InvalidResponse {
        message: format!(
            "{context} timestamp component at index {index} does not fit in i32: {error}"
        ),
    })
}

fn component_u8(values: &[Value], index: usize, context: &str) -> Result<u8, ICloudError> {
    let raw = component_i64(values, index, context)?;
    u8::try_from(raw).map_err(|error| ICloudError::InvalidResponse {
        message: format!(
            "{context} timestamp component at index {index} does not fit in u8: {error}"
        ),
    })
}

fn component_u8_or_default(
    values: &[Value],
    index: usize,
    default: u8,
    context: &str,
) -> Result<u8, ICloudError> {
    if index >= values.len() {
        return Ok(default);
    }
    component_u8(values, index, context)
}

fn component_i64(values: &[Value], index: usize, context: &str) -> Result<i64, ICloudError> {
    let Some(value) = values.get(index) else {
        return Err(ICloudError::InvalidResponse {
            message: format!("{context} timestamp array is missing index {index}"),
        });
    };
    match value {
        Value::Number(number) => number.as_i64().ok_or_else(|| ICloudError::InvalidResponse {
            message: format!(
                "{context} timestamp component at index {index} is not an integer: {number}"
            ),
        }),
        other => Err(ICloudError::InvalidResponse {
            message: format!(
                "{context} timestamp component at index {index} is not numeric: {other}"
            ),
        }),
    }
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

fn integer_field_any(raw: &Map<String, Value>, keys: &[&str]) -> Result<Option<i64>, ICloudError> {
    let Some(value) = value_field_any(raw, keys) else {
        return Ok(None);
    };
    match value {
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                Ok(Some(value))
            } else if let Some(value) = number.as_u64() {
                i64::try_from(value)
                    .map(Some)
                    .map_err(|error| ICloudError::InvalidResponse {
                        message: format!("reminder integer field does not fit in i64: {error}"),
                    })
            } else {
                Err(ICloudError::InvalidResponse {
                    message: format!("reminder integer field is not an integer: {number}"),
                })
            }
        }
        other => Err(ICloudError::InvalidResponse {
            message: format!("reminder integer field is not a number: {other}"),
        }),
    }
}

fn value_field_any<'a>(raw: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| raw.get(*key))
}

fn remove_any(raw: &mut Map<String, Value>, keys: &[&str]) -> Option<Value> {
    keys.iter().find_map(|key| raw.remove(*key))
}

#[cfg(test)]
mod tests {
    use crate::{HeaderMap, StatusCode};
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn refresh_request_matches_pyicloud_shape() {
        let root = Url::parse("https://p43-remindersws.icloud.com").expect("valid url");
        let protocol = RemindersProtocol::new(&root, "client-123", "42");

        let request = protocol.refresh().expect("request should build");

        assert_eq!(
            request.url.as_str(),
            "https://p43-remindersws.icloud.com/rd/startup?clientId=client-123&dsid=42&clientVersion=4.0&lang=en-us&usertz=UTC",
        );
    }

    #[test]
    fn parser_extracts_lists_and_items() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/reminders/reminders.json").to_vec(),
        );

        let reminders = parse_reminders(&response).expect("reminders should parse");

        assert_eq!(reminders.lists.len(), 2);
        assert_eq!(reminders.lists[0].guid, "list-1");
        assert_eq!(reminders.lists[0].reminders.len(), 1);
        assert_eq!(reminders.lists[1].reminders[0].guid, "task-2");
        assert!(reminders.lists[1].reminders[0].completed);
        assert_eq!(
            reminders.lists[0].reminders[0]
                .due_at
                .expect("fixture due date should parse")
                .unix_timestamp(),
            1_717_243_200
        );
    }
}
