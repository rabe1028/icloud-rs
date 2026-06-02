use crate::prelude::*;

use icloud_types::{ICloudError, JsonNumber};
use time::OffsetDateTime;

/// Converts a Unix timestamp expressed in milliseconds into an [`OffsetDateTime`].
pub(crate) fn datetime_from_millis(
    millis: i64,
    context: &str,
) -> Result<OffsetDateTime, ICloudError> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000).map_err(|error| {
        ICloudError::InvalidResponse {
            message: format!("invalid {context} timestamp: {error}"),
        }
    })
}

/// Extracts an integer millisecond timestamp from a JSON number, coercing
/// across the `i64`/`u64` representations that iCloud responses use.
pub(crate) fn millis_from_number(number: &JsonNumber, context: &str) -> Result<i64, ICloudError> {
    if let Some(millis) = number.as_i64() {
        Ok(millis)
    } else if let Some(millis) = number.as_u64() {
        i64::try_from(millis).map_err(|error| ICloudError::InvalidResponse {
            message: format!("{context} timestamp does not fit in i64: {error}"),
        })
    } else {
        Err(ICloudError::InvalidResponse {
            message: format!("{context} timestamp is not an integer: {number}"),
        })
    }
}

/// Parses a JSON number as a millisecond Unix timestamp.
pub(crate) fn datetime_from_number(
    number: &JsonNumber,
    context: &str,
) -> Result<OffsetDateTime, ICloudError> {
    datetime_from_millis(millis_from_number(number, context)?, context)
}
