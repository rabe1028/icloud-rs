use crate::prelude::*;

use crate::{
    HeaderMap, HeaderValue, Method,
    header::{ACCEPT, CONTENT_TYPE},
};
use url::Url;

use icloud_types::{
    FindMyBatteryStatus, FindMyDevice, FindMyDevicesResponse, FindMyLocation,
    FindMyLostModeCommand, FindMyMessageCommand, FindMyRefreshOptions, FindMySoundCommand,
    ICloudError, JsonObject, JsonValue,
};

use crate::{RequestSpec, ResponseParts};

#[derive(Debug, Clone)]
pub struct FindMyProtocol<'a> {
    service_root: &'a Url,
    client_id: &'a str,
    dsid: &'a str,
}

impl<'a> FindMyProtocol<'a> {
    #[must_use]
    pub fn new(service_root: &'a Url, client_id: &'a str, dsid: &'a str) -> Self {
        Self {
            service_root,
            client_id,
            dsid,
        }
    }

    pub fn refresh_client(
        &self,
        options: &FindMyRefreshOptions,
    ) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(Method::Post, self.endpoint("refreshClient")?);
        request.headers = json_headers();
        request = request.with_json_body(format!(
            "{{\"clientContext\":{{{},{},{},{}}}}}",
            crate::json::bool_member("fmly", options.with_family),
            crate::json::bool_member("shouldLocate", options.should_locate),
            crate::json::string_member("selectedDevice", options.selected_device.as_api_value()),
            crate::json::member("deviceListVersion", options.device_list_version.to_string())
        ))?;
        Ok(request)
    }

    pub fn play_sound(&self, command: &FindMySoundCommand) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(Method::Post, self.endpoint("playSound")?);
        request.headers = json_headers();
        request = request.with_json_body(format!(
            "{{{},{},\"clientContext\":{{{}}}}}",
            crate::json::string_member("device", command.device_id.as_str()),
            crate::json::string_member("subject", command.subject.as_str()),
            crate::json::bool_member("fmly", command.with_family)
        ))?;
        Ok(request)
    }

    pub fn send_message(&self, command: &FindMyMessageCommand) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(Method::Post, self.endpoint("sendMessage")?);
        request.headers = json_headers();
        request = request.with_json_body(format!(
            "{{{},{},{},{},{}}}",
            crate::json::string_member("device", command.device_id.as_str()),
            crate::json::string_member("subject", command.subject.as_str()),
            crate::json::bool_member("sound", command.play_sound),
            crate::json::bool_member("userText", true),
            crate::json::string_member("text", command.text.as_str())
        ))?;
        Ok(request)
    }

    pub fn lost_device(&self, command: &FindMyLostModeCommand) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(Method::Post, self.endpoint("lostDevice")?);
        request.headers = json_headers();
        request = request.with_json_body(format!(
            "{{{},{},{},{},{},{},{}}}",
            crate::json::string_member("text", command.text.as_str()),
            crate::json::bool_member("userText", true),
            crate::json::string_member("ownerNbr", command.owner_number.as_str()),
            crate::json::bool_member("lostModeEnabled", true),
            crate::json::bool_member("trackingEnabled", true),
            crate::json::string_member("device", command.device_id.as_str()),
            crate::json::string_member("passcode", command.passcode.as_str())
        ))?;
        Ok(request)
    }

    fn endpoint(&self, action: &str) -> Result<Url, ICloudError> {
        let mut url = self
            .service_root
            .join(format!("fmipservice/client/web/{action}").as_str())?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("clientId", self.client_id);
            query.append_pair("dsid", self.dsid);
        }
        Ok(url)
    }
}

pub fn parse_findmy_devices(
    response: &ResponseParts,
) -> Result<FindMyDevicesResponse, ICloudError> {
    let envelope = crate::json::object(response)?;
    let devices = envelope
        .get("content")
        .and_then(JsonValue::as_array)
        .unwrap_or_default()
        .iter()
        .cloned()
        .map(parse_device)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FindMyDevicesResponse { devices })
}

fn parse_device(value: JsonValue) -> Result<FindMyDevice, ICloudError> {
    let raw = crate::json::value_object(value, "find my device")?;
    Ok(FindMyDevice {
        id: crate::json::string_field(&raw, "id").unwrap_or_default(),
        name: crate::json::string_field(&raw, "name").unwrap_or_default(),
        device_display_name: crate::json::string_field(&raw, "deviceDisplayName"),
        model_display_name: crate::json::string_field(&raw, "modelDisplayName"),
        raw_device_model: crate::json::string_field(&raw, "rawDeviceModel"),
        device_status: crate::json::i64_field(&raw, "deviceStatus"),
        battery_level: crate::json::f64_field(&raw, "batteryLevel"),
        battery_status: crate::json::string_field(&raw, "batteryStatus")
            .map(FindMyBatteryStatus::from),
        location: raw
            .get("location")
            .cloned()
            .and_then(|value| match value {
                JsonValue::Object(value) => Some(parse_location(value)),
                JsonValue::Null => None,
                _ => None,
            })
            .transpose()?,
        lost_mode_enabled: crate::json::bool_field(&raw, "lostModeEnabled").unwrap_or(false),
        location_capable: crate::json::bool_field(&raw, "locationCapable").unwrap_or(false),
        location_enabled: crate::json::bool_field(&raw, "locationEnabled").unwrap_or(false),
        this_device: crate::json::bool_field(&raw, "thisDevice").unwrap_or(false),
        is_mac: crate::json::bool_field(&raw, "isMac").unwrap_or(false),
        extra: raw,
    })
}

fn parse_location(raw: JsonObject) -> Result<FindMyLocation, ICloudError> {
    Ok(FindMyLocation {
        latitude: crate::json::f64_field(&raw, "latitude").unwrap_or_default(),
        longitude: crate::json::f64_field(&raw, "longitude").unwrap_or_default(),
        horizontal_accuracy: crate::json::f64_field(&raw, "horizontalAccuracy"),
        location_type: crate::json::string_field(&raw, "locationType"),
        position_type: crate::json::string_field(&raw, "positionType"),
        time_stamp: crate::json::i64_field(&raw, "timeStamp"),
        is_old: crate::json::bool_field(&raw, "isOld"),
        extra: raw,
    })
}

fn json_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers
}

#[cfg(test)]
mod tests {
    use crate::{HeaderMap, StatusCode};
    use pretty_assertions::assert_eq;
    use url::Url;

    use super::*;

    #[test]
    fn refresh_request_matches_pyicloud_shape() {
        let root = Url::parse("https://p43-fmip.icloud.com/").expect("valid url");
        let protocol = FindMyProtocol::new(&root, "client-123", "42");

        let request = protocol
            .refresh_client(&FindMyRefreshOptions::default())
            .expect("request should build");

        assert_eq!(
            request.url.as_str(),
            "https://p43-fmip.icloud.com/fmipservice/client/web/refreshClient?clientId=client-123&dsid=42",
        );
        let body = String::from_utf8(request.body.as_bytes().expect("json body").to_vec())
            .expect("utf8 request");
        assert_eq!(
            body,
            r#"{"clientContext":{"fmly":true,"shouldLocate":true,"selectedDevice":"all","deviceListVersion":1}}"#,
        );
    }

    #[test]
    fn refresh_request_can_target_a_single_device() {
        let root = Url::parse("https://p43-fmip.icloud.com/").expect("valid url");
        let protocol = FindMyProtocol::new(&root, "client-123", "42");

        let request = protocol
            .refresh_client(&FindMyRefreshOptions::for_device("device-1"))
            .expect("request should build");
        let body = String::from_utf8(request.body.as_bytes().expect("json body").to_vec())
            .expect("utf8 request");

        assert_eq!(
            body,
            r#"{"clientContext":{"fmly":true,"shouldLocate":true,"selectedDevice":"device-1","deviceListVersion":1}}"#,
        );
    }

    #[test]
    fn parser_extracts_findmy_devices() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/findmy/refresh_client.json").to_vec(),
        );

        let parsed = parse_findmy_devices(&response).expect("response should parse");

        assert_eq!(parsed.devices.len(), 1);
        assert_eq!(parsed.devices[0].id, "device-1");
        assert_eq!(parsed.devices[0].battery_level, Some(0.76));
        assert_eq!(
            parsed.devices[0]
                .location
                .as_ref()
                .map(|location| (location.latitude, location.longitude)),
            Some((35.681236, 139.767125)),
        );
    }

    #[test]
    fn parser_defaults_missing_content_to_empty_devices() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            br#"{"serverContext":{"minCallbackIntervalInMS":5000}}"#.to_vec(),
        );

        let parsed = parse_findmy_devices(&response).expect("empty response should parse");

        assert!(parsed.devices.is_empty());
    }
}
