use crate::prelude::*;

use alloc::collections::BTreeMap;
use icloud_types::{ICloudError, JsonObject, JsonValue};

pub(crate) type Map<K, V> = BTreeMap<K, V>;
pub(crate) type Value = JsonValue;

pub(crate) fn object(response: &crate::ResponseParts) -> Result<JsonObject, ICloudError> {
    match response.json_value_checked()? {
        JsonValue::Object(value) => Ok(value),
        _ => invalid("expected JSON object"),
    }
}

pub(crate) fn array(response: &crate::ResponseParts) -> Result<Vec<JsonValue>, ICloudError> {
    match response.json_value_checked()? {
        JsonValue::Array(value) => Ok(value),
        _ => invalid("expected JSON array"),
    }
}

pub(crate) fn string_field(raw: &JsonObject, name: &str) -> Option<String> {
    match raw.get(name) {
        Some(JsonValue::String(value)) => Some(value.clone()),
        _ => None,
    }
}

pub(crate) fn bool_field(raw: &JsonObject, name: &str) -> Option<bool> {
    raw.get(name).and_then(JsonValue::as_bool)
}

pub(crate) fn u64_field(raw: &JsonObject, name: &str) -> Option<u64> {
    match raw.get(name) {
        Some(JsonValue::Number(value)) => value.as_u64(),
        Some(JsonValue::String(value)) => value.parse().ok(),
        _ => None,
    }
}

pub(crate) fn i64_field(raw: &JsonObject, name: &str) -> Option<i64> {
    match raw.get(name) {
        Some(JsonValue::Number(value)) => value.as_i64(),
        Some(JsonValue::String(value)) => value.parse().ok(),
        _ => None,
    }
}

pub(crate) fn f64_field(raw: &JsonObject, name: &str) -> Option<f64> {
    match raw.get(name) {
        Some(JsonValue::Number(value)) => value.as_f64(),
        Some(JsonValue::String(value)) => value.parse().ok(),
        _ => None,
    }
}

pub(crate) fn value_object(value: JsonValue, context: &str) -> Result<JsonObject, ICloudError> {
    match value {
        JsonValue::Object(value) => Ok(value),
        _ => invalid(format!("{context} was not an object")),
    }
}

pub(crate) fn invalid<T>(message: impl Into<String>) -> Result<T, ICloudError> {
    Err(ICloudError::InvalidResponse {
        message: message.into(),
    })
}

pub(crate) fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch < ' ' => {
                use core::fmt::Write;
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

pub(crate) fn member(name: &str, value: impl AsRef<str>) -> String {
    format!("{}:{}", quote(name), value.as_ref())
}

pub(crate) fn string_member(name: &str, value: &str) -> String {
    member(name, quote(value))
}

pub(crate) fn bool_member(name: &str, value: bool) -> String {
    member(name, if value { "true" } else { "false" })
}
