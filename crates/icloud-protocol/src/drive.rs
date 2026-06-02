use crate::prelude::*;

use crate::json::Value;
use crate::{HeaderValue, Method, header::CONTENT_TYPE};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};
use url::Url;
#[cfg(feature = "std")]
use uuid::Uuid;

use icloud_types::{
    DriveContainer, DriveContainerContents, DriveContainerKind, DriveContainerUnavailable,
    DriveDocumentId, DriveDownload, DriveDownloadTarget, DriveDownloadToken, DriveFile, DriveNode,
    DriveNodeMetadata, DriveUnknownNode, DriveUploadFile, DriveUploadReceipt, DriveUploadTicket,
    DriveWsId, ICloudError,
};

use crate::{RequestBody, RequestSpec, ResponseParts};

#[derive(Debug, Clone)]
pub struct DriveProtocol<'a> {
    service_root: &'a Url,
    document_root: &'a Url,
    client_id: &'a str,
    dsid: &'a str,
}

impl<'a> DriveProtocol<'a> {
    #[must_use]
    pub fn new(
        service_root: &'a Url,
        document_root: &'a Url,
        client_id: &'a str,
        dsid: &'a str,
    ) -> Self {
        Self {
            service_root,
            document_root,
            client_id,
            dsid,
        }
    }

    pub fn retrieve_root(&self) -> Result<RequestSpec, ICloudError> {
        self.retrieve_item_details(&DriveWsId::from("FOLDER::com.apple.CloudDocs::root"))
    }

    pub fn retrieve_item_details(&self, drivewsid: &DriveWsId) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(
            Method::Post,
            self.service_url(
                self.service_root,
                "retrieveItemDetailsInFolders",
                core::iter::empty::<(&str, String)>(),
            )?,
        );
        request = request.with_json_body(format!(
            "[{{{},{}}}]",
            crate::json::string_member("drivewsid", drivewsid.as_str()),
            crate::json::bool_member("partialData", false)
        ))?;
        Ok(request)
    }

    pub fn retrieve_app_libraries(&self) -> Result<RequestSpec, ICloudError> {
        Ok(RequestSpec::new(
            Method::Get,
            self.service_url(
                self.service_root,
                "retrieveAppLibraries",
                core::iter::empty::<(&str, String)>(),
            )?,
        ))
    }

    pub fn download_by_id(
        &self,
        document_id: &DriveDocumentId,
        zone: &str,
    ) -> Result<RequestSpec, ICloudError> {
        Ok(RequestSpec::new(
            Method::Get,
            self.document_service_url(
                zone,
                "download/by_id",
                [("document_id", document_id.as_str().to_owned())],
            )?,
        ))
    }

    pub fn create_folders(
        &self,
        parent: &DriveWsId,
        name: &str,
    ) -> Result<RequestSpec, ICloudError> {
        let temp_client_id = temporary_folder_client_id();

        let mut request = RequestSpec::new(
            Method::Post,
            self.service_url(
                self.service_root,
                "createFolders",
                core::iter::empty::<(&str, String)>(),
            )?,
        );
        request.insert_header(CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        request = request.with_json_body(format!(
            "{{{},\"folders\":[{{{},{}}}]}}",
            crate::json::string_member("destinationDrivewsId", parent.as_str()),
            crate::json::string_member("clientId", temp_client_id.as_str()),
            crate::json::string_member("name", name)
        ))?;
        Ok(request)
    }

    pub fn rename_item(
        &self,
        drivewsid: &DriveWsId,
        etag: &str,
        name: &str,
    ) -> Result<RequestSpec, ICloudError> {
        let request = RequestSpec::new(
            Method::Post,
            self.service_url(
                self.service_root,
                "renameItems",
                core::iter::empty::<(&str, String)>(),
            )?,
        )
        .with_json_body(format!(
            "{{\"items\":[{{{},{},{}}}]}}",
            crate::json::string_member("drivewsid", drivewsid.as_str()),
            crate::json::string_member("etag", etag),
            crate::json::string_member("name", name)
        ))?;
        Ok(request)
    }

    pub fn move_item_to_trash(
        &self,
        drivewsid: &DriveWsId,
        etag: &str,
    ) -> Result<RequestSpec, ICloudError> {
        let request = RequestSpec::new(
            Method::Post,
            self.service_url(
                self.service_root,
                "moveItemsToTrash",
                core::iter::empty::<(&str, String)>(),
            )?,
        )
        .with_json_body(format!(
            "{{\"items\":[{{{},{},{}}}]}}",
            crate::json::string_member("drivewsid", drivewsid.as_str()),
            crate::json::string_member("etag", etag),
            crate::json::string_member("clientId", self.client_id)
        ))?;
        Ok(request)
    }

    pub fn begin_upload(
        &self,
        validate_token: &str,
        zone: &str,
        file: &DriveUploadFile,
    ) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(
            Method::Post,
            self.document_service_url(zone, "upload/web", [("token", validate_token.to_owned())])?,
        );
        request.insert_header(CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        request = request.with_json_body(format!(
            "{{{},{},{},{}}}",
            crate::json::string_member("filename", file.name.as_str()),
            crate::json::string_member("type", "FILE"),
            crate::json::string_member("content_type", file.content_type.as_deref().unwrap_or("")),
            crate::json::member("size", file.len().to_string())
        ))?;
        Ok(request)
    }

    pub fn upload_content(
        &self,
        content_url: &Url,
        file: &DriveUploadFile,
    ) -> Result<RequestSpec, ICloudError> {
        let boundary = format!("----icloud-rs-{}", unique_id());
        let mut request = RequestSpec::new(Method::Post, content_url.clone());
        request.insert_header(
            CONTENT_TYPE,
            HeaderValue::parse(format!("multipart/form-data; boundary={boundary}").as_str())
                .map_err(|error| ICloudError::InvalidHeaderValue {
                    name: "content-type",
                    message: error.to_string(),
                })?,
        );
        request.body = RequestBody::Raw(build_multipart_body(file, boundary.as_str())?);
        Ok(request)
    }

    pub fn commit_upload(
        &self,
        folder_document_id: &DriveDocumentId,
        zone: &str,
        ticket: &DriveUploadTicket,
        receipt: &DriveUploadReceipt,
        file: &DriveUploadFile,
        timestamp_ms: i64,
    ) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(
            Method::Post,
            self.document_service_url(
                zone,
                "update/documents",
                core::iter::empty::<(&str, String)>(),
            )?,
        );
        request.insert_header(CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        let mut data = vec![
            crate::json::string_member("signature", receipt.file_checksum.as_str()),
            crate::json::string_member("wrapping_key", receipt.wrapping_key.as_str()),
            crate::json::string_member("reference_signature", receipt.reference_checksum.as_str()),
            crate::json::member("size", receipt.size.to_string()),
        ];
        if let Some(value) = receipt.receipt.as_deref() {
            data.push(crate::json::string_member("receipt", value));
        }
        request = request.with_json_body(format!(
            "{{\"data\":{{{}}},{},{},{},\"path\":{{{},{}}},{},\"file_flags\":{{{},{},{}}},{},{}}}",
            data.join(","),
            crate::json::string_member("command", "add_file"),
            crate::json::bool_member("create_short_guid", true),
            crate::json::string_member("document_id", ticket.document_id.as_str()),
            crate::json::string_member("starting_document_id", folder_document_id.as_str()),
            crate::json::string_member("path", file.name.as_str()),
            crate::json::bool_member("allow_conflict", true),
            crate::json::bool_member("is_writable", true),
            crate::json::bool_member("is_executable", false),
            crate::json::bool_member("is_hidden", false),
            crate::json::member("mtime", timestamp_ms.to_string()),
            crate::json::member("btime", timestamp_ms.to_string())
        ))?;
        Ok(request)
    }

    pub fn download_content(&self, target: &DriveDownloadToken) -> RequestSpec {
        let mut url = target.url.clone();
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("clientId", self.client_id);
            query.append_pair("dsid", self.dsid);
        }
        RequestSpec::new(Method::Get, url)
    }

    pub fn extract_validate_token(cookie_value: &str) -> Result<String, ICloudError> {
        cookie_value
            .split(':')
            .find_map(|segment| segment.trim().strip_prefix("t=").map(str::to_owned))
            .ok_or_else(|| ICloudError::InvalidResponse {
                message: "X-APPLE-WEBAUTH-VALIDATE cookie does not contain a `t=` token".to_owned(),
            })
    }

    fn service_url(
        &self,
        base: &Url,
        path: &str,
        extra_query: impl IntoIterator<Item = (&'a str, String)>,
    ) -> Result<Url, ICloudError> {
        let mut url = base.join(path)?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("clientId", self.client_id);
            query.append_pair("dsid", self.dsid);
            for (key, value) in extra_query {
                query.append_pair(key, value.as_str());
            }
        }
        Ok(url)
    }

    fn document_service_url(
        &self,
        zone: &str,
        path: &str,
        extra_query: impl IntoIterator<Item = (&'a str, String)>,
    ) -> Result<Url, ICloudError> {
        self.service_url(
            self.document_root,
            format!("ws/{zone}/{path}").as_str(),
            extra_query,
        )
    }
}

pub fn parse_drive_container(response: &ResponseParts) -> Result<DriveContainer, ICloudError> {
    parse_drive_container_internal(response, None)
}

pub fn parse_drive_container_with_seed(
    response: &ResponseParts,
    seed: &DriveContainer,
) -> Result<DriveContainer, ICloudError> {
    parse_drive_container_internal(response, Some(seed))
}

pub fn parse_drive_app_libraries(
    response: &ResponseParts,
) -> Result<Vec<DriveContainer>, ICloudError> {
    let payload = crate::json::object(response)?;
    payload
        .get("items")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .cloned()
        .map(parse_drive_node_value)
        .map(|node| match node? {
            DriveNode::Container(container) => Ok(container),
            DriveNode::File(_) | DriveNode::Unknown(_) => Err(ICloudError::InvalidResponse {
                message: "retrieveAppLibraries returned a non-container node".to_owned(),
            }),
        })
        .collect()
}

pub fn parse_drive_download(response: &ResponseParts) -> Result<DriveDownload, ICloudError> {
    let payload = crate::json::object(response)?;
    let data_token = payload
        .get("data_token")
        .cloned()
        .map(parse_download_token)
        .transpose()?;
    let package_token = payload
        .get("package_token")
        .cloned()
        .map(parse_download_token)
        .transpose()?;
    let thumbnail_token = payload
        .get("thumbnail_token")
        .cloned()
        .map(parse_download_token)
        .transpose()?;
    let target = match (data_token, package_token) {
        (Some(token), _) => DriveDownloadTarget::Data(token),
        (None, Some(token)) => DriveDownloadTarget::Package(token),
        (None, None) => return Err(ICloudError::DriveDownloadTokenUnavailable),
    };
    Ok(DriveDownload {
        document_id: DriveDocumentId::from(
            crate::json::string_field(&payload, "document_id").unwrap_or_default(),
        ),
        target,
        thumbnail: thumbnail_token,
        double_etag: crate::json::string_field(&payload, "double_etag"),
    })
}

pub fn parse_drive_upload_ticket(
    response: &ResponseParts,
) -> Result<DriveUploadTicket, ICloudError> {
    let mut payload = crate::json::array(response)?;
    let Some(ticket) = payload.pop() else {
        return Err(ICloudError::InvalidResponse {
            message: "upload/web returned an empty ticket list".to_owned(),
        });
    };
    let ticket = crate::json::value_object(ticket, "upload ticket")?;
    Ok(DriveUploadTicket {
        document_id: DriveDocumentId::from(
            crate::json::string_field(&ticket, "document_id").unwrap_or_default(),
        ),
        content_url: Url::parse(
            crate::json::string_field(&ticket, "url")
                .ok_or_else(|| ICloudError::InvalidResponse {
                    message: "upload ticket is missing url".to_owned(),
                })?
                .as_str(),
        )?,
    })
}

pub fn parse_drive_upload_receipt(
    response: &ResponseParts,
) -> Result<DriveUploadReceipt, ICloudError> {
    let payload = crate::json::object(response)?;
    let single_file = payload
        .get("singleFile")
        .cloned()
        .and_then(|value| match value {
            Value::Object(value) => Some(value),
            _ => None,
        })
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: "upload receipt is missing singleFile".to_owned(),
        })?;
    Ok(DriveUploadReceipt {
        file_checksum: crate::json::string_field(&single_file, "fileChecksum").unwrap_or_default(),
        wrapping_key: crate::json::string_field(&single_file, "wrappingKey").unwrap_or_default(),
        reference_checksum: crate::json::string_field(&single_file, "referenceChecksum")
            .unwrap_or_default(),
        receipt: crate::json::string_field(&single_file, "receipt"),
        size: crate::json::u64_field(&single_file, "size").unwrap_or(0),
    })
}

fn parse_drive_container_internal(
    response: &ResponseParts,
    seed: Option<&DriveContainer>,
) -> Result<DriveContainer, ICloudError> {
    let mut payload = crate::json::array(response)?;
    let Some(value) = payload.pop() else {
        return Err(ICloudError::InvalidResponse {
            message: "drive response does not contain a root container".to_owned(),
        });
    };
    parse_drive_container_value(value, seed)
}

fn parse_drive_container_value(
    value: Value,
    seed: Option<&DriveContainer>,
) -> Result<DriveContainer, ICloudError> {
    let raw = crate::json::value_object(value, "drive container")?;
    let metadata = build_metadata(&raw, seed.map(|container| &container.metadata))?;
    let kind = match crate::json::string_field(&raw, "type")
        .as_deref()
        .or(seed.map(|container| match container.kind {
            DriveContainerKind::Folder => "FOLDER",
            DriveContainerKind::AppLibrary => "APP_LIBRARY",
        })) {
        Some("FOLDER") => DriveContainerKind::Folder,
        Some("APP_LIBRARY") => DriveContainerKind::AppLibrary,
        Some(other) => {
            return Err(ICloudError::InvalidResponse {
                message: format!("expected a drive container, received `{other}`"),
            });
        }
        None => {
            return Err(ICloudError::InvalidResponse {
                message: "drive container response is missing its `type`".to_owned(),
            });
        }
    };
    build_container(raw, metadata, kind)
}

/// Assembles a [`DriveContainer`] from the shared field set used by both the
/// dedicated container response and the `FOLDER`/`APP_LIBRARY` node variants.
fn build_container(
    raw: icloud_types::JsonObject,
    metadata: DriveNodeMetadata,
    kind: DriveContainerKind,
) -> Result<DriveContainer, ICloudError> {
    let contents = match raw.get("items").and_then(Value::as_array) {
        Some(items) => DriveContainerContents::Loaded(
            items
                .iter()
                .cloned()
                .map(parse_drive_node_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        None => match crate::json::string_field(&raw, "status") {
            Some(status) => {
                DriveContainerContents::Unavailable(DriveContainerUnavailable { status })
            }
            None => DriveContainerContents::Omitted,
        },
    };

    Ok(DriveContainer {
        metadata,
        kind,
        created_at: parse_optional_datetime(
            crate::json::string_field(&raw, "dateCreated").as_deref(),
        )?,
        asset_quota: crate::json::u64_field(&raw, "assetQuota"),
        file_count: crate::json::u64_field(&raw, "fileCount"),
        share_count: crate::json::u64_field(&raw, "shareCount"),
        share_alias_count: crate::json::u64_field(&raw, "shareAliasCount"),
        direct_children_count: crate::json::u64_field(&raw, "directChildrenCount"),
        number_of_items: crate::json::u64_field(&raw, "numberOfItems"),
        contents,
        extra: raw,
    })
}

fn parse_drive_node_value(value: Value) -> Result<DriveNode, ICloudError> {
    let raw = crate::json::value_object(value, "drive node")?;
    let metadata = build_metadata(&raw, None)?;
    match crate::json::string_field(&raw, "type").as_deref() {
        Some("FOLDER") => Ok(DriveNode::Container(build_container(
            raw,
            metadata,
            DriveContainerKind::Folder,
        )?)),
        Some("APP_LIBRARY") => Ok(DriveNode::Container(build_container(
            raw,
            metadata,
            DriveContainerKind::AppLibrary,
        )?)),
        Some("FILE") => Ok(DriveNode::File(DriveFile {
            metadata: DriveNodeMetadata {
                name: file_name(
                    crate::json::string_field(&raw, "name").as_deref(),
                    crate::json::string_field(&raw, "extension").as_deref(),
                )?,
                ..metadata
            },
            base_name: required_field(crate::json::string_field(&raw, "name"), "name")?,
            extension: crate::json::string_field(&raw, "extension"),
            hidden_extension: crate::json::bool_field(&raw, "hiddenExtension").unwrap_or(false),
            size: required_field(crate::json::u64_field(&raw, "size"), "size")?,
            date_changed: parse_optional_datetime(
                crate::json::string_field(&raw, "dateChanged").as_deref(),
            )?,
            date_modified: parse_optional_datetime(
                crate::json::string_field(&raw, "dateModified").as_deref(),
            )?,
            date_last_open: parse_optional_datetime(
                crate::json::string_field(&raw, "lastOpenTime").as_deref(),
            )?,
            extra: raw,
        })),
        Some(other) => Ok(DriveNode::Unknown(DriveUnknownNode {
            metadata,
            kind: other.to_owned(),
            extra: raw,
        })),
        None => Err(ICloudError::InvalidResponse {
            message: "drive node response is missing its `type`".to_owned(),
        }),
    }
}

fn build_metadata(
    raw: &icloud_types::JsonObject,
    seed: Option<&DriveNodeMetadata>,
) -> Result<DriveNodeMetadata, ICloudError> {
    Ok(DriveNodeMetadata {
        drivewsid: DriveWsId::from(required_field_with_seed(
            crate::json::string_field(raw, "drivewsid"),
            seed.map(|metadata| metadata.drivewsid.as_str()),
            "drivewsid",
        )?),
        docwsid: DriveDocumentId::from(required_field_with_seed(
            crate::json::string_field(raw, "docwsid"),
            seed.map(|metadata| metadata.docwsid.as_str()),
            "docwsid",
        )?),
        zone: required_field_with_seed(
            crate::json::string_field(raw, "zone"),
            seed.map(|metadata| metadata.zone.as_str()),
            "zone",
        )?,
        name: required_field_with_seed(
            crate::json::string_field(raw, "name"),
            seed.map(|metadata| metadata.name.as_str()),
            "name",
        )?,
        parent_id: crate::json::string_field(raw, "parentId")
            .map(DriveWsId::from)
            .or_else(|| seed.and_then(|metadata| metadata.parent_id.clone())),
        etag: crate::json::string_field(raw, "etag")
            .or_else(|| seed.and_then(|metadata| metadata.etag.clone())),
    })
}

fn parse_download_token(value: Value) -> Result<DriveDownloadToken, ICloudError> {
    let raw = crate::json::value_object(value, "download token")?;
    Ok(DriveDownloadToken {
        url: Url::parse(
            crate::json::string_field(&raw, "url")
                .ok_or_else(|| ICloudError::InvalidResponse {
                    message: "download token is missing url".to_owned(),
                })?
                .as_str(),
        )?,
        token: crate::json::string_field(&raw, "token").unwrap_or_default(),
        signature: crate::json::string_field(&raw, "signature").unwrap_or_default(),
        wrapping_key: crate::json::string_field(&raw, "wrapping_key")
            .or_else(|| crate::json::string_field(&raw, "wrappingKey"))
            .unwrap_or_default(),
        reference_signature: crate::json::string_field(&raw, "reference_signature")
            .or_else(|| crate::json::string_field(&raw, "referenceSignature"))
            .unwrap_or_default(),
    })
}

fn required_field<T>(value: Option<T>, name: &'static str) -> Result<T, ICloudError> {
    value.ok_or_else(|| ICloudError::InvalidResponse {
        message: format!("drive response is missing `{name}`"),
    })
}

fn required_field_with_seed(
    value: Option<String>,
    seed: Option<&str>,
    name: &'static str,
) -> Result<String, ICloudError> {
    value
        .or_else(|| seed.map(str::to_owned))
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: format!("drive response is missing `{name}`"),
        })
}

fn file_name(name: Option<&str>, extension: Option<&str>) -> Result<String, ICloudError> {
    let name = name.ok_or_else(|| ICloudError::InvalidResponse {
        message: "drive file response is missing `name`".to_owned(),
    })?;
    Ok(match extension {
        Some(extension)
            if !extension.is_empty() && !name.ends_with(format!(".{extension}").as_str()) =>
        {
            format!("{name}.{extension}")
        }
        Some(_) | None => name.to_owned(),
    })
}

fn parse_optional_datetime(value: Option<&str>) -> Result<Option<OffsetDateTime>, ICloudError> {
    value
        .map(|value| {
            OffsetDateTime::parse(value, &Rfc3339)
                .map(|datetime| datetime.to_offset(UtcOffset::UTC))
                .map_err(|error| ICloudError::InvalidResponse {
                    message: format!("invalid drive timestamp `{value}`: {error}"),
                })
        })
        .transpose()
}

fn build_multipart_body(file: &DriveUploadFile, boundary: &str) -> Result<Vec<u8>, ICloudError> {
    let escaped_name = sanitize_multipart_quoted_string(file.name.as_str(), "filename")?;
    let content_type = sanitize_multipart_header_value(
        file.content_type
            .as_deref()
            .unwrap_or("application/octet-stream"),
        "content-type",
    )?;
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{escaped_name}\"; filename=\"{escaped_name}\"\r\nContent-Type: {content_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(file.bytes.as_slice());
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Ok(body)
}

fn temporary_folder_client_id() -> String {
    format!("FOLDER::UNKNOWN_ZONE::TempId-{}", unique_id())
}

#[cfg(feature = "std")]
fn unique_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(not(feature = "std"))]
fn unique_id() -> String {
    use core::sync::atomic::{AtomicU32, Ordering};

    static NEXT_ID: AtomicU32 = AtomicU32::new(1);
    format!("no-std-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

fn sanitize_multipart_quoted_string(
    value: &str,
    name: &'static str,
) -> Result<String, ICloudError> {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii_control() {
            return Err(ICloudError::InvalidHeaderValue {
                name,
                message: "multipart quoted values cannot contain control characters".to_owned(),
            });
        }
        match character {
            '\\' | '"' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    Ok(escaped)
}

fn sanitize_multipart_header_value<'a>(
    value: &'a str,
    name: &'static str,
) -> Result<&'a str, ICloudError> {
    if value.chars().any(|character| character.is_ascii_control()) {
        return Err(ICloudError::InvalidHeaderValue {
            name,
            message: "multipart header values cannot contain control characters".to_owned(),
        });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use crate::{HeaderMap, StatusCode};
    use pretty_assertions::assert_eq;
    use time::macros::datetime;

    use super::*;

    fn parse_body(request: &RequestSpec) -> Value {
        ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            request
                .body
                .as_bytes()
                .expect("request body should be present")
                .to_vec(),
        )
        .json_value()
        .expect("request body should be valid json")
    }

    #[test]
    fn root_request_includes_authenticated_query_pairs() {
        let service_root = Url::parse("https://p43-drivews.icloud.com/").expect("valid url");
        let document_root = Url::parse("https://p43-docws.icloud.com/").expect("valid url");
        let protocol = DriveProtocol::new(&service_root, &document_root, "client-123", "42");

        let request = protocol.retrieve_root().expect("request should build");

        assert_eq!(
            request.url.as_str(),
            "https://p43-drivews.icloud.com/retrieveItemDetailsInFolders?clientId=client-123&dsid=42",
        );
    }

    #[test]
    fn detail_request_uses_existing_drivewsid() {
        let service_root = Url::parse("https://p43-drivews.icloud.com/").expect("valid url");
        let document_root = Url::parse("https://p43-docws.icloud.com/").expect("valid url");
        let protocol = DriveProtocol::new(&service_root, &document_root, "client-123", "42");

        let request = protocol
            .retrieve_item_details(&DriveWsId::from("FOLDER::com.apple.Keynote::documents"))
            .expect("request should build");
        let body = String::from_utf8(request.body.as_bytes().expect("json body").to_vec())
            .expect("utf8 request");

        assert_eq!(
            body,
            r#"[{"drivewsid":"FOLDER::com.apple.Keynote::documents","partialData":false}]"#,
        );
        assert_eq!(
            request
                .headers
                .get(CONTENT_TYPE)
                .expect("content type should be set"),
            "application/json",
        );
    }

    #[test]
    fn parser_extracts_root_children() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/root.json").to_vec(),
        );

        let root = parse_drive_container(&response).expect("response should parse");

        assert_eq!(root.name(), "");
        assert_eq!(root.kind, DriveContainerKind::Folder);
        let DriveContainerContents::Loaded(children) = &root.contents else {
            panic!("expected loaded children");
        };
        assert_eq!(
            children.iter().map(DriveNode::name).collect::<Vec<_>>(),
            vec!["Preview", "pyiCloud"],
        );
        assert_eq!(
            children[0]
                .as_container()
                .expect("preview should be a container")
                .kind,
            DriveContainerKind::AppLibrary,
        );
    }

    #[test]
    fn seeded_parser_preserves_app_library_metadata_on_invalid_detail_response() {
        let root = parse_drive_container(&ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/root.json").to_vec(),
        ))
        .expect("root parses");
        let preview = match &root.contents {
            DriveContainerContents::Loaded(children) => children[0]
                .as_container()
                .expect("preview should be a container")
                .clone(),
            DriveContainerContents::Unavailable(_) | DriveContainerContents::Omitted => {
                panic!("root should contain items")
            }
        };

        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/invalid_app_library.json").to_vec(),
        );

        let parsed =
            parse_drive_container_with_seed(&response, &preview).expect("seeded parse should work");

        assert_eq!(parsed.name(), "Preview");
        assert_eq!(parsed.kind, DriveContainerKind::AppLibrary);
        assert_eq!(
            parsed.contents,
            DriveContainerContents::Unavailable(DriveContainerUnavailable {
                status: "ID_INVALID".to_owned(),
            }),
        );
    }

    #[test]
    fn parser_extracts_file_metadata_in_utc() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/subfolder.json").to_vec(),
        );

        let folder = parse_drive_container(&response).expect("response should parse");
        let DriveContainerContents::Loaded(children) = folder.contents else {
            panic!("expected file children");
        };
        let file = children[1]
            .as_file()
            .expect("second child should be a file");

        assert_eq!(file.name(), "Scanned document 1.pdf");
        assert_eq!(file.size, 21_644_358);
        assert_eq!(file.date_changed, Some(datetime!(2020-05-03 0:16:17 UTC)));
        assert_eq!(file.date_modified, Some(datetime!(2020-05-03 0:15:17 UTC)));
        assert_eq!(file.date_last_open, Some(datetime!(2020-05-03 0:24:25 UTC)));
    }

    #[test]
    fn parser_does_not_duplicate_existing_extension_in_name() {
        let file = parse_drive_node_value(
            ResponseParts::new(
                StatusCode::OK,
                HeaderMap::new(),
                br#"{"drivewsid":"FILE::com.apple.CloudDocs::report","docwsid":"report-doc","zone":"com.apple.CloudDocs","name":"report.pdf","extension":"pdf","type":"FILE","size":42}"#.to_vec(),
            )
            .json_value()
            .expect("json should parse"),
        )
        .expect("file should parse");

        assert_eq!(file.name(), "report.pdf");
    }

    #[test]
    fn parser_prefers_data_download_tokens() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/download_by_id.json").to_vec(),
        );

        let download = parse_drive_download(&response).expect("response should parse");

        assert_eq!(
            download.target.token().url.as_str(),
            "https://cvws.icloud-content.com/B/signature1ref_signature1/Scanned+document+1.pdf?o=object1&v=1&x=3&a=token1&e=1588472097&k=wrapping_key1&fl=&r=request&ckc=com.apple.clouddocs&ckz=com.apple.CloudDocs&p=31&s=s1",
        );
    }

    #[test]
    fn content_download_request_includes_authenticated_query_pairs() {
        let service_root = Url::parse("https://p43-drivews.icloud.com/").expect("valid url");
        let document_root = Url::parse("https://p43-docws.icloud.com/").expect("valid url");
        let protocol = DriveProtocol::new(&service_root, &document_root, "client-123", "42");
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/drive/download_by_id.json").to_vec(),
        );
        let download = parse_drive_download(&response).expect("response should parse");

        let request = protocol.download_content(download.target.token());

        assert_eq!(
            request.url.as_str(),
            "https://cvws.icloud-content.com/B/signature1ref_signature1/Scanned+document+1.pdf?o=object1&v=1&x=3&a=token1&e=1588472097&k=wrapping_key1&fl=&r=request&ckc=com.apple.clouddocs&ckz=com.apple.CloudDocs&p=31&s=s1&clientId=client-123&dsid=42",
        );
    }

    #[test]
    fn document_requests_use_the_passed_zone() {
        let service_root = Url::parse("https://p43-drivews.icloud.com/").expect("valid url");
        let document_root = Url::parse("https://p43-docws.icloud.com/").expect("valid url");
        let protocol = DriveProtocol::new(&service_root, &document_root, "client-123", "42");
        let upload =
            DriveUploadFile::new("note.txt", b"hello".to_vec()).with_content_type("text/plain");

        let download_request = protocol
            .download_by_id(&DriveDocumentId::from("doc-123"), "com.apple.Preview")
            .expect("request should build");
        let upload_request = protocol
            .begin_upload("validate-token", "com.apple.Preview", &upload)
            .expect("request should build");
        let commit_request = protocol
            .commit_upload(
                &DriveDocumentId::from("folder-123"),
                "com.apple.Preview",
                &DriveUploadTicket {
                    document_id: DriveDocumentId::from("ticket-123"),
                    content_url: Url::parse("https://content.icloud.com/upload")
                        .expect("valid url"),
                },
                &DriveUploadReceipt {
                    file_checksum: "checksum".to_owned(),
                    wrapping_key: "wrapping".to_owned(),
                    reference_checksum: "reference".to_owned(),
                    receipt: None,
                    size: 5,
                },
                &upload,
                1_700_000_000_000,
            )
            .expect("request should build");

        assert_eq!(
            download_request.url.as_str(),
            "https://p43-docws.icloud.com/ws/com.apple.Preview/download/by_id?clientId=client-123&dsid=42&document_id=doc-123",
        );
        assert_eq!(
            upload_request.url.as_str(),
            "https://p43-docws.icloud.com/ws/com.apple.Preview/upload/web?clientId=client-123&dsid=42&token=validate-token",
        );
        assert_eq!(
            commit_request.url.as_str(),
            "https://p43-docws.icloud.com/ws/com.apple.Preview/update/documents?clientId=client-123&dsid=42",
        );
    }

    #[test]
    fn create_folder_request_uses_a_temporary_folder_client_id() {
        let service_root = Url::parse("https://p43-drivews.icloud.com/").expect("valid url");
        let document_root = Url::parse("https://p43-docws.icloud.com/").expect("valid url");
        let protocol = DriveProtocol::new(&service_root, &document_root, "client-123", "42");

        let request = protocol
            .create_folders(
                &DriveWsId::from("FOLDER::com.apple.CloudDocs::root"),
                "New Folder",
            )
            .expect("request should build");
        let body = parse_body(&request);
        let client_id = body["folders"][0]["clientId"]
            .as_str()
            .expect("folder clientId should be a string");

        assert!(client_id.starts_with("FOLDER::UNKNOWN_ZONE::TempId-"));
        assert_ne!(client_id, "client-123");
    }

    #[test]
    fn trash_request_uses_the_session_client_id() {
        let service_root = Url::parse("https://p43-drivews.icloud.com/").expect("valid url");
        let document_root = Url::parse("https://p43-docws.icloud.com/").expect("valid url");
        let protocol = DriveProtocol::new(&service_root, &document_root, "client-123", "42");

        let request = protocol
            .move_item_to_trash(
                &DriveWsId::from("FILE::com.apple.Preview::preview-doc"),
                "etag-1",
            )
            .expect("request should build");
        let body = parse_body(&request);

        assert_eq!(body["items"][0]["clientId"], "client-123",);
    }

    #[test]
    fn upload_content_request_builds_multipart_body() {
        let service_root = Url::parse("https://p43-drivews.icloud.com/").expect("valid url");
        let document_root = Url::parse("https://p43-docws.icloud.com/").expect("valid url");
        let protocol = DriveProtocol::new(&service_root, &document_root, "client-123", "42");
        let upload =
            DriveUploadFile::new("note.txt", b"hello".to_vec()).with_content_type("text/plain");
        let content_url = Url::parse("https://content.icloud.com/upload").expect("valid url");

        let request = protocol
            .upload_content(&content_url, &upload)
            .expect("request should build");

        let body = String::from_utf8(request.body.as_bytes().expect("raw body").to_vec())
            .expect("multipart should remain ascii");
        assert!(
            body.contains(
                "Content-Disposition: form-data; name=\"note.txt\"; filename=\"note.txt\""
            )
        );
        assert!(body.contains("Content-Type: text/plain"));
        assert!(body.contains("hello"));
    }

    #[test]
    fn upload_content_request_escapes_quoted_filename_characters() {
        let service_root = Url::parse("https://p43-drivews.icloud.com/").expect("valid url");
        let document_root = Url::parse("https://p43-docws.icloud.com/").expect("valid url");
        let protocol = DriveProtocol::new(&service_root, &document_root, "client-123", "42");
        let upload =
            DriveUploadFile::new("note\\\".txt", b"hello".to_vec()).with_content_type("text/plain");
        let content_url = Url::parse("https://content.icloud.com/upload").expect("valid url");

        let request = protocol
            .upload_content(&content_url, &upload)
            .expect("request should build");

        let body = String::from_utf8(request.body.as_bytes().expect("raw body").to_vec())
            .expect("multipart should remain ascii");
        assert!(body.contains(r#"name="note\\\".txt"; filename="note\\\".txt""#));
    }

    #[test]
    fn upload_content_rejects_control_characters_in_filename() {
        let service_root = Url::parse("https://p43-drivews.icloud.com/").expect("valid url");
        let document_root = Url::parse("https://p43-docws.icloud.com/").expect("valid url");
        let protocol = DriveProtocol::new(&service_root, &document_root, "client-123", "42");
        let upload =
            DriveUploadFile::new("note\r\n.txt", b"hello".to_vec()).with_content_type("text/plain");
        let content_url = Url::parse("https://content.icloud.com/upload").expect("valid url");

        let error = protocol
            .upload_content(&content_url, &upload)
            .expect_err("control characters should be rejected");

        assert_eq!(
            error.to_string(),
            "invalid header value for `filename`: multipart quoted values cannot contain control characters",
        );
    }

    #[test]
    fn upload_content_rejects_control_characters_in_content_type() {
        let service_root = Url::parse("https://p43-drivews.icloud.com/").expect("valid url");
        let document_root = Url::parse("https://p43-docws.icloud.com/").expect("valid url");
        let protocol = DriveProtocol::new(&service_root, &document_root, "client-123", "42");
        let upload = DriveUploadFile::new("note.txt", b"hello".to_vec())
            .with_content_type("text/plain\r\nX-Injected: 1");
        let content_url = Url::parse("https://content.icloud.com/upload").expect("valid url");

        let error = protocol
            .upload_content(&content_url, &upload)
            .expect_err("control characters should be rejected");

        assert_eq!(
            error.to_string(),
            "invalid header value for `content-type`: multipart header values cannot contain control characters",
        );
    }
}
