use crate::prelude::*;

use alloc::collections::BTreeMap;
use nojson::{JsonParseError, JsonValueKind, RawJsonOwned, RawJsonValue};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub type JsonObject = BTreeMap<String, JsonValue>;
pub type RawJson = RawJsonOwned;
static NULL: JsonValue = JsonValue::Null;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(JsonNumber),
    String(String),
    Array(Vec<JsonValue>),
    Object(JsonObject),
}

/// A JSON number kept in its original textual form.
///
/// The value is stored as the raw source string so that no precision is lost
/// up front. The `as_*` accessors parse that string on every call, so callers
/// that read the same number repeatedly should cache the result.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct JsonNumber(String);

impl JsonNumber {
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        self.0.parse().ok()
    }

    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        self.0.parse().ok()
    }

    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        self.0.parse().ok()
    }
}

impl core::fmt::Display for JsonNumber {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl JsonValue {
    #[must_use]
    pub fn as_object(&self) -> Option<&JsonObject> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            Self::Array(value) => Some(value.as_slice()),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }
}

/// Renders a short, human-readable summary of the value.
///
/// This is **not** a JSON serializer: strings are emitted without quotes and
/// arrays/objects collapse to the placeholders `[array]` / `{object}`. It is
/// intended for diagnostics and error messages, not for round-tripping.
impl core::fmt::Display for JsonValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Number(value) => write!(f, "{value}"),
            Self::String(value) => write!(f, "{value}"),
            Self::Array(_) => f.write_str("[array]"),
            Self::Object(_) => f.write_str("{object}"),
        }
    }
}

impl core::ops::Index<&str> for JsonValue {
    type Output = JsonValue;

    fn index(&self, index: &str) -> &Self::Output {
        self.as_object()
            .and_then(|object| object.get(index))
            .unwrap_or(&NULL)
    }
}

impl core::ops::Index<usize> for JsonValue {
    type Output = JsonValue;

    fn index(&self, index: usize) -> &Self::Output {
        self.as_array()
            .and_then(|array| array.get(index))
            .unwrap_or(&NULL)
    }
}

impl PartialEq<&str> for JsonValue {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == Some(*other)
    }
}

impl PartialEq<JsonValue> for &str {
    fn eq(&self, other: &JsonValue) -> bool {
        other == self
    }
}

impl PartialEq<i32> for JsonValue {
    fn eq(&self, other: &i32) -> bool {
        match self {
            Self::Number(value) => value.as_i64() == Some(i64::from(*other)),
            _ => false,
        }
    }
}

impl PartialEq<JsonValue> for i32 {
    fn eq(&self, other: &JsonValue) -> bool {
        other == self
    }
}

impl TryFrom<RawJsonValue<'_, '_>> for JsonValue {
    type Error = JsonParseError;

    fn try_from(value: RawJsonValue<'_, '_>) -> Result<Self, Self::Error> {
        match value.kind() {
            JsonValueKind::Null => Ok(Self::Null),
            JsonValueKind::Boolean => Ok(Self::Bool(bool::try_from(value)?)),
            JsonValueKind::Integer | JsonValueKind::Float => {
                Ok(Self::Number(JsonNumber(value.as_number_str()?.to_owned())))
            }
            JsonValueKind::String => Ok(Self::String(String::try_from(value)?)),
            JsonValueKind::Array => value
                .to_array()?
                .map(JsonValue::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map(Self::Array),
            JsonValueKind::Object => value
                .to_object()?
                .map(|(name, value)| Ok((String::try_from(name)?, JsonValue::try_from(value)?)))
                .collect::<Result<JsonObject, JsonParseError>>()
                .map(Self::Object),
        }
    }
}
