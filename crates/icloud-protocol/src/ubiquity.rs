use crate::prelude::*;

use crate::Method;
use crate::json::Value;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};
use url::Url;

use icloud_types::{
    ICloudError, UbiquityFile, UbiquityFolder, UbiquityNode, UbiquityNodeId, UbiquityNodeMetadata,
    UbiquityUnknownNode,
};

use crate::{RequestSpec, ResponseParts};

#[derive(Debug, Clone)]
pub struct UbiquityProtocol<'a> {
    service_root: &'a Url,
    dsid: &'a str,
}

impl<'a> UbiquityProtocol<'a> {
    #[must_use]
    pub fn new(service_root: &'a Url, dsid: &'a str) -> Self {
        Self { service_root, dsid }
    }

    pub fn retrieve_root(&self) -> Result<RequestSpec, ICloudError> {
        self.retrieve_node(&UbiquityNodeId::root())
    }

    pub fn retrieve_node(&self, node_id: &UbiquityNodeId) -> Result<RequestSpec, ICloudError> {
        Ok(RequestSpec::new(
            Method::Get,
            self.node_url(node_id, "item")?,
        ))
    }

    pub fn retrieve_children(&self, node_id: &UbiquityNodeId) -> Result<RequestSpec, ICloudError> {
        Ok(RequestSpec::new(
            Method::Get,
            self.node_url(node_id, "parent")?,
        ))
    }

    pub fn download_file(&self, node_id: &UbiquityNodeId) -> Result<RequestSpec, ICloudError> {
        Ok(RequestSpec::new(
            Method::Get,
            self.node_url(node_id, "file")?,
        ))
    }

    fn node_url(&self, node_id: &UbiquityNodeId, variant: &str) -> Result<Url, ICloudError> {
        Ok(self
            .service_root
            .join(format!("ws/{}/{variant}/{}", self.dsid, node_id.as_str()).as_str())?)
    }
}

pub fn parse_ubiquity_folder(response: &ResponseParts) -> Result<UbiquityFolder, ICloudError> {
    let node = parse_ubiquity_node(response)?;
    match node {
        UbiquityNode::Folder(folder) => Ok(folder),
        UbiquityNode::File(_) | UbiquityNode::Unknown(_) => Err(ICloudError::InvalidResponse {
            message: "expected a ubiquity folder node".to_owned(),
        }),
    }
}

pub fn parse_ubiquity_node(response: &ResponseParts) -> Result<UbiquityNode, ICloudError> {
    parse_ubiquity_node_value(response.json_value_checked()?)
}

pub fn parse_ubiquity_children(response: &ResponseParts) -> Result<Vec<UbiquityNode>, ICloudError> {
    let payload = crate::json::object(response)?;
    let items = payload
        .get("item_list")
        .and_then(Value::as_array)
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: "ubiquity children response is missing `item_list`".to_owned(),
        })?;
    items
        .iter()
        .cloned()
        .map(parse_ubiquity_node_value)
        .collect()
}

fn parse_ubiquity_node_value(value: Value) -> Result<UbiquityNode, ICloudError> {
    let raw = crate::json::value_object(value, "ubiquity node")?;
    let metadata = build_metadata(&raw)?;
    let size = parse_optional_size(raw.get("size"))?;
    let kind = required_field(crate::json::string_field(&raw, "type"), "type")?;
    match kind.to_ascii_lowercase().as_str() {
        "folder" => Ok(UbiquityNode::Folder(UbiquityFolder {
            metadata,
            size,
            extra: raw,
        })),
        "file" => Ok(UbiquityNode::File(UbiquityFile {
            metadata,
            size,
            extra: raw,
        })),
        _ => Ok(UbiquityNode::Unknown(UbiquityUnknownNode {
            metadata,
            kind,
            size,
            extra: raw,
        })),
    }
}

fn build_metadata(raw: &icloud_types::JsonObject) -> Result<UbiquityNodeMetadata, ICloudError> {
    Ok(UbiquityNodeMetadata {
        item_id: parse_node_id(raw.get("item_id"))?,
        name: required_field(crate::json::string_field(raw, "name"), "name")?,
        modified_at: parse_optional_modified(
            crate::json::string_field(raw, "modified").as_deref(),
        )?,
    })
}

fn parse_node_id(value: Option<&Value>) -> Result<UbiquityNodeId, ICloudError> {
    let value = value.ok_or_else(|| ICloudError::InvalidResponse {
        message: "ubiquity response is missing `item_id`".to_owned(),
    })?;
    match value {
        Value::String(value) => Ok(UbiquityNodeId::from(value.clone())),
        Value::Number(value) => {
            value
                .as_u64()
                .map(UbiquityNodeId::from)
                .ok_or_else(|| ICloudError::InvalidResponse {
                    message: format!("invalid ubiquity item_id `{value}`"),
                })
        }
        other => Err(ICloudError::InvalidResponse {
            message: format!("invalid ubiquity item_id `{other}`"),
        }),
    }
}

fn parse_optional_size(value: Option<&Value>) -> Result<Option<u64>, ICloudError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => {
            value
                .as_u64()
                .map(Some)
                .ok_or_else(|| ICloudError::InvalidResponse {
                    message: format!("invalid ubiquity size `{value}`"),
                })
        }
        Some(Value::String(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(trimmed.parse::<u64>().ok())
            }
        }
        Some(other) => Err(ICloudError::InvalidResponse {
            message: format!("invalid ubiquity size `{other}`"),
        }),
    }
}

fn parse_optional_modified(value: Option<&str>) -> Result<Option<OffsetDateTime>, ICloudError> {
    value
        .map(|value| {
            OffsetDateTime::parse(value, &Rfc3339)
                .map(|datetime| datetime.to_offset(UtcOffset::UTC))
                .map_err(|error| ICloudError::InvalidResponse {
                    message: format!("invalid ubiquity timestamp `{value}`: {error}"),
                })
        })
        .transpose()
}

fn required_field<T>(value: Option<T>, name: &'static str) -> Result<T, ICloudError> {
    value.ok_or_else(|| ICloudError::InvalidResponse {
        message: format!("ubiquity response is missing `{name}`"),
    })
}

#[cfg(test)]
mod tests {
    use crate::{HeaderMap, StatusCode};
    use pretty_assertions::assert_eq;
    use time::macros::datetime;

    use super::*;

    #[test]
    fn root_request_uses_dsid_path() {
        let service_root = Url::parse("https://p43-ubiquityws.icloud.com/").expect("valid url");
        let protocol = UbiquityProtocol::new(&service_root, "42");

        let request = protocol.retrieve_root().expect("request should build");

        assert_eq!(
            request.url.as_str(),
            "https://p43-ubiquityws.icloud.com/ws/42/item/0",
        );
    }

    #[test]
    fn child_and_file_requests_use_expected_variants() {
        let service_root = Url::parse("https://p43-ubiquityws.icloud.com/").expect("valid url");
        let protocol = UbiquityProtocol::new(&service_root, "42");
        let node_id = UbiquityNodeId::from("folder-1");

        let children = protocol
            .retrieve_children(&node_id)
            .expect("request should build");
        let file = protocol
            .download_file(&node_id)
            .expect("request should build");

        assert_eq!(
            children.url.as_str(),
            "https://p43-ubiquityws.icloud.com/ws/42/parent/folder-1",
        );
        assert_eq!(
            file.url.as_str(),
            "https://p43-ubiquityws.icloud.com/ws/42/file/folder-1",
        );
    }

    #[test]
    fn parser_extracts_root_folder() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/ubiquity/root.json").to_vec(),
        );

        let root = parse_ubiquity_folder(&response).expect("response should parse");

        assert_eq!(root.item_id().as_str(), "0");
        assert_eq!(root.name(), "");
        assert_eq!(root.modified_at(), Some(datetime!(2020-05-03 00:00:00 UTC)));
    }

    #[test]
    fn parser_unwraps_child_items_and_tolerates_invalid_sizes() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/ubiquity/children.json").to_vec(),
        );

        let children = parse_ubiquity_children(&response).expect("response should parse");

        assert_eq!(
            children.iter().map(UbiquityNode::name).collect::<Vec<_>>(),
            vec!["Documents", "notes.txt", "corrupt.bin"],
        );
        assert_eq!(children[1].size(), Some(12));
        assert_eq!(children[2].size(), None);
        assert_eq!(
            children[1].modified_at(),
            Some(datetime!(2020-05-03 00:12:00 UTC))
        );
    }

    #[test]
    fn parser_extracts_single_file_node() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/ubiquity/file.json").to_vec(),
        );

        let node = parse_ubiquity_node(&response).expect("response should parse");

        assert_eq!(node.item_id().as_str(), "notes-file");
        assert!(node.as_file().is_some());
    }
}
