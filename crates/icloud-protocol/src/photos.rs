use crate::prelude::*;

use alloc::collections::BTreeMap;
use core::num::NonZeroU32;

use crate::json::{Map, Value};
use crate::{
    HeaderMap, HeaderValue, Method,
    header::{ACCEPT, CONTENT_TYPE},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use time::OffsetDateTime;
use url::Url;

use icloud_types::{
    ICloudError, PhotoAlbumDirection, PhotoAlbumFilter, PhotoAlbumPage, PhotoAlbumSummary,
    PhotoAsset, PhotoAssetVersion, PhotoAssetVersions, PhotoDeleteTarget, PhotoDimensions,
    PhotoLibraryState, PhotoMediaKind,
};

use crate::{RequestSpec, ResponseParts};

const ZONE_NAME: &str = "PrimarySync";
const ROOT_FOLDER_RECORD_NAME: &str = "----Root-Folder----";
const PROJECT_ROOT_FOLDER_RECORD_NAME: &str = "----Project-Root-Folder----";
const ALBUM_TYPE_FOLDER: u64 = 3;
const PHOTO_PAGE_KEYS: [&str; 27] = [
    "resJPEGThumbRes",
    "filenameEnc",
    "resJPEGThumbWidth",
    "resJPEGThumbHeight",
    "resTruncatedRes",
    "resTruncatedWidth",
    "resTruncatedHeight",
    "resJPEGMedRes",
    "resJPEGMedWidth",
    "resJPEGMedHeight",
    "resVidSmallRes",
    "resVidSmallWidth",
    "resVidSmallHeight",
    "resVidMedRes",
    "resVidMedWidth",
    "resVidMedHeight",
    "resOriginalRes",
    "resOriginalWidth",
    "resOriginalHeight",
    "itemType",
    "resOriginalFileType",
    "resJPEGMedFileType",
    "resVidSmallFileType",
    "resVidMedFileType",
    "masterRef",
    "assetDate",
    "addedDate",
];

#[derive(Debug, Clone)]
pub struct PhotosProtocol<'a> {
    service_root: &'a Url,
    client_id: &'a str,
    dsid: &'a str,
}

impl<'a> PhotosProtocol<'a> {
    #[must_use]
    pub fn new(service_root: &'a Url, client_id: &'a str, dsid: &'a str) -> Self {
        Self {
            service_root,
            client_id,
            dsid,
        }
    }

    pub fn indexing_state(&self) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(Method::Post, self.endpoint("records/query")?);
        request.headers = cloudkit_headers();
        request = request.with_json_body(format!(
            "{{\"query\":{{{}}},\"zoneID\":{{{}}}}}",
            crate::json::string_member("recordType", "CheckIndexingState"),
            crate::json::string_member("zoneName", ZONE_NAME)
        ))?;
        Ok(request)
    }

    pub fn query_albums(&self, parent_id: Option<&str>) -> Result<RequestSpec, ICloudError> {
        let filter_by = parent_id
            .map(|parent_id| {
                format!(
                    ",\"filterBy\":[{}]",
                    cloudkit_filter_json("parentId", "STRING", parent_id)
                )
            })
            .unwrap_or_default();
        let mut request = RequestSpec::new(Method::Post, self.endpoint("records/query")?);
        request.headers = cloudkit_headers();
        request = request.with_json_body(format!(
            "{{\"query\":{{{}{} }},\"desiredKeys\":[{}],\"zoneID\":{{{}}}}}",
            crate::json::string_member("recordType", "CPLAlbumByPositionLive"),
            filter_by,
            string_array(["albumNameEnc", "albumType", "sortAscending", "isDeleted"]),
            crate::json::string_member("zoneName", ZONE_NAME)
        ))?;
        Ok(request)
    }

    pub fn album_count(&self, album: &PhotoAlbumSummary) -> Result<RequestSpec, ICloudError> {
        let mut request =
            RequestSpec::new(Method::Post, self.endpoint("internal/records/query/batch")?);
        request.headers = cloudkit_headers();
        request = request.with_json_body(format!(
            "{{\"batch\":[{{\"resultsLimit\":1,\"query\":{{\"filterBy\":[{{\"fieldName\":\"indexCountID\",\"fieldValue\":{{\"type\":\"STRING_LIST\",\"value\":[{}]}},\"comparator\":\"IN\"}}],{}}},\"zoneWide\":true,\"zoneID\":{{{}}}}}]}}",
            crate::json::quote(album.query.count_index_id.as_str()),
            crate::json::string_member("recordType", "HyperionIndexCountLookup"),
            crate::json::string_member("zoneName", ZONE_NAME)
        ))?;
        Ok(request)
    }

    pub fn album_page(
        &self,
        album: &PhotoAlbumSummary,
        start_rank: u64,
        page_size: NonZeroU32,
    ) -> Result<RequestSpec, ICloudError> {
        let results_limit =
            page_size
                .get()
                .checked_mul(2)
                .ok_or_else(|| ICloudError::InvalidResponse {
                    message: "photo page size overflowed CloudKit resultsLimit".to_owned(),
                })?;

        let mut filter_by = vec![
            format!(
                "{{\"fieldName\":\"startRank\",\"fieldValue\":{{\"type\":\"INT64\",\"value\":{start_rank}}},\"comparator\":\"EQUALS\"}}"
            ),
            cloudkit_filter_json("direction", "STRING", album.query.direction.as_api_value()),
        ];
        for filter in &album.query.filters {
            filter_by.push(cloudkit_filter_value(filter)?);
        }

        let mut request = RequestSpec::new(Method::Post, self.endpoint("records/query")?);
        request.headers = cloudkit_headers();
        request = request.with_json_body(format!(
            "{{\"query\":{{\"filterBy\":[{}],{}}},\"resultsLimit\":{},\"desiredKeys\":[{}],\"zoneID\":{{{}}}}}",
            filter_by.join(","),
            crate::json::string_member("recordType", album.query.list_type.as_str()),
            results_limit,
            string_array(PHOTO_PAGE_KEYS),
            crate::json::string_member("zoneName", ZONE_NAME)
        ))?;
        Ok(request)
    }

    pub fn download_version(
        &self,
        version: &PhotoAssetVersion,
    ) -> Result<RequestSpec, ICloudError> {
        Ok(RequestSpec::new(Method::Get, version.url.clone()))
    }

    pub fn delete_asset(&self, asset: &PhotoAsset) -> Result<RequestSpec, ICloudError> {
        let mut request = RequestSpec::new(Method::Post, self.endpoint("records/modify")?);
        request.headers = cloudkit_headers();
        request = request.with_json_body(format!(
            "{{\"operations\":[{{\"operationType\":\"update\",\"record\":{{{},{},{},\"fields\":{{\"isDeleted\":{{\"value\":1}}}}}}}}],\"zoneID\":{{{}}},\"atomic\":true}}",
            crate::json::string_member("recordName", asset.delete_target.record_name.as_str()),
            crate::json::string_member("recordType", asset.delete_target.record_type.as_str()),
            crate::json::string_member("recordChangeTag", asset.delete_target.change_tag.as_str()),
            crate::json::string_member("zoneName", ZONE_NAME)
        ))?;
        Ok(request)
    }

    fn endpoint(&self, path: &str) -> Result<Url, ICloudError> {
        let mut url = self.service_root.join(
            format!("database/1/com.apple.photos.cloud/production/private/{path}").as_str(),
        )?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("clientId", self.client_id);
            query.append_pair("dsid", self.dsid);
            query.append_pair("remapEnums", "true");
            query.append_pair("getCurrentSyncToken", "true");
        }
        Ok(url)
    }
}

pub fn parse_photo_library_state(
    response: &ResponseParts,
) -> Result<PhotoLibraryState, ICloudError> {
    let payload = records_response(response)?;
    let record =
        payload
            .records
            .into_iter()
            .next()
            .ok_or_else(|| ICloudError::InvalidResponse {
                message: "photos indexing response is missing `records[0]`".to_owned(),
            })?;
    Ok(PhotoLibraryState::from(required_string_field(
        &record,
        "state",
        "photos indexing response",
    )?))
}

pub fn parse_photo_albums(response: &ResponseParts) -> Result<Vec<PhotoAlbumSummary>, ICloudError> {
    Ok(parse_photo_album_entries(response)?
        .into_iter()
        .filter(|entry| !entry.is_folder)
        .map(|entry| {
            PhotoAlbumSummary::custom_with_direction(entry.record_name, entry.name, entry.direction)
        })
        .collect())
}

pub fn parse_photo_album_folders(response: &ResponseParts) -> Result<Vec<String>, ICloudError> {
    Ok(parse_photo_album_entries(response)?
        .into_iter()
        .filter(|entry| entry.is_folder)
        .map(|entry| entry.record_name)
        .collect())
}

pub fn parse_photo_album_count(response: &ResponseParts) -> Result<u64, ICloudError> {
    let payload = batch_response(response)?;
    let record = payload
        .batch
        .into_iter()
        .next()
        .and_then(|records| records.records.into_iter().next())
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: "photo count response is missing `batch[0].records[0]`".to_owned(),
        })?;
    required_u64_field(&record, "itemCount", "photo count response")
}

pub fn parse_photo_album_page(
    response: &ResponseParts,
    album: &PhotoAlbumSummary,
    start_rank: u64,
    _page_size: NonZeroU32,
) -> Result<PhotoAlbumPage, ICloudError> {
    let payload = records_response(response)?;
    let mut assets_by_master = BTreeMap::new();
    let mut masters = Vec::new();

    for record in payload.records {
        match record.record_type.as_str() {
            "CPLAsset" => {
                let master_ref = required_record_reference(&record, "masterRef")?;
                assets_by_master.insert(master_ref, record);
            }
            "CPLMaster" => masters.push(record),
            _ => {}
        }
    }

    let master_records_len = masters.len() as u64;
    let mut assets = Vec::new();
    for master in masters {
        let Some(asset) = assets_by_master.remove(master.record_name.as_str()) else {
            continue;
        };
        assets.push(parse_photo_asset(master, asset)?);
    }

    let next_offset = if master_records_len == 0 {
        None
    } else {
        next_offset(start_rank, master_records_len, album.query.direction)
    };

    Ok(PhotoAlbumPage {
        assets,
        next_offset,
    })
}

fn parse_photo_asset(master: RecordWire, asset: RecordWire) -> Result<PhotoAsset, ICloudError> {
    let media_kind = if field_value(&master, "resVidSmallRes").is_some() {
        PhotoMediaKind::Video
    } else {
        PhotoMediaKind::Image
    };
    let filename = decode_base64_string(
        required_string_field(&master, "filenameEnc", "photo master record")?.as_str(),
        "filenameEnc",
    )?;
    let original =
        parse_version(&master, "resOriginal")?.ok_or_else(|| ICloudError::InvalidResponse {
            message: "photo master record is missing `resOriginalRes`".to_owned(),
        })?;
    let (medium_prefix, thumb_prefix) = match media_kind {
        PhotoMediaKind::Image => ("resJPEGMed", "resJPEGThumb"),
        PhotoMediaKind::Video => ("resVidMed", "resVidSmall"),
    };
    let versions = PhotoAssetVersions {
        original: original.clone(),
        medium: parse_version(&master, medium_prefix)?,
        thumb: parse_version(&master, thumb_prefix)?,
    };
    let dimensions = PhotoDimensions {
        width: required_u64_field(&master, "resOriginalWidth", "photo master record")?,
        height: required_u64_field(&master, "resOriginalHeight", "photo master record")?,
    };
    let delete_target = PhotoDeleteTarget {
        record_name: asset.record_name.clone(),
        record_type: asset.record_type.clone(),
        change_tag: asset.record_change_tag.clone().ok_or_else(|| {
            ICloudError::InvalidResponse {
                message: "photo asset record is missing `recordChangeTag`".to_owned(),
            }
        })?,
    };
    let created_at = optional_timestamp_field(&asset, "assetDate")?;
    let added_at = required_timestamp_field(&asset, "addedDate", "photo asset record")?;

    Ok(PhotoAsset {
        id: master.record_name,
        filename,
        size: original.size,
        created_at,
        added_at,
        dimensions,
        media_kind,
        versions,
        delete_target,
        master_fields: master.fields,
        asset_fields: asset.fields,
    })
}

fn parse_photo_album_entries(
    response: &ResponseParts,
) -> Result<Vec<PhotoAlbumEntry>, ICloudError> {
    let payload = records_response(response)?;
    let mut entries = Vec::new();
    for record in payload.records {
        if matches!(
            record.record_name.as_str(),
            ROOT_FOLDER_RECORD_NAME | PROJECT_ROOT_FOLDER_RECORD_NAME
        ) || field_bool(&record, "isDeleted")?
        {
            continue;
        }

        let is_folder = optional_u64_field(&record, "albumType")? == Some(ALBUM_TYPE_FOLDER);
        let name = optional_string_field(&record, "albumNameEnc")?
            .filter(|encoded_name| !encoded_name.is_empty())
            .map(|encoded_name| decode_base64_string(encoded_name.as_str(), "albumNameEnc"))
            .transpose()?;

        if is_folder {
            entries.push(PhotoAlbumEntry {
                record_name: record.record_name,
                name: name.unwrap_or_default(),
                is_folder: true,
                direction: PhotoAlbumDirection::Ascending,
            });
            continue;
        }

        let Some(name) = name else {
            continue;
        };
        let direction = if optional_bool_field(&record, "sortAscending")?.unwrap_or(true) {
            PhotoAlbumDirection::Ascending
        } else {
            PhotoAlbumDirection::Descending
        };
        entries.push(PhotoAlbumEntry {
            record_name: record.record_name,
            name,
            is_folder: false,
            direction,
        });
    }
    Ok(entries)
}

fn parse_version(
    record: &RecordWire,
    prefix: &str,
) -> Result<Option<PhotoAssetVersion>, ICloudError> {
    let Some(resource) = field_value(record, format!("{prefix}Res").as_str()) else {
        return Ok(None);
    };
    let resource = resource
        .as_object()
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: format!("photo resource `{prefix}Res` is not an object"),
        })?;

    Ok(Some(PhotoAssetVersion {
        url: Url::parse(
            required_string_value(
                resource.get("downloadURL"),
                format!("photo resource `{prefix}Res.downloadURL`").as_str(),
            )?
            .as_str(),
        )?,
        width: optional_u64_field(record, format!("{prefix}Width").as_str())?,
        height: optional_u64_field(record, format!("{prefix}Height").as_str())?,
        size: required_u64_value(
            resource.get("size"),
            format!("photo resource `{prefix}Res.size`").as_str(),
        )?,
        file_type: optional_string_field(record, format!("{prefix}FileType").as_str())?,
    }))
}

fn cloudkit_filter_value(filter: &PhotoAlbumFilter) -> Result<String, ICloudError> {
    match filter {
        PhotoAlbumFilter::ParentId(parent_id) => {
            Ok(cloudkit_filter_json("parentId", "STRING", parent_id))
        }
        PhotoAlbumFilter::SmartAlbum(kind) => {
            let value = kind
                .filter_code()
                .ok_or_else(|| ICloudError::InvalidResponse {
                    message: format!(
                        "photo smart album `{}` does not have a filter code",
                        kind.display_name()
                    ),
                })?;
            Ok(cloudkit_filter_json("smartAlbum", "STRING", value))
        }
    }
}

fn cloudkit_filter_json(field_name: &str, value_type: &str, value: &str) -> String {
    format!(
        "{{{},{},\"comparator\":\"EQUALS\"}}",
        crate::json::string_member("fieldName", field_name),
        crate::json::member(
            "fieldValue",
            format!(
                "{{{},{}}}",
                crate::json::string_member("type", value_type),
                crate::json::string_member("value", value)
            )
        )
    )
}

fn string_array<const N: usize>(values: [&str; N]) -> String {
    values
        .into_iter()
        .map(crate::json::quote)
        .collect::<Vec<_>>()
        .join(",")
}

fn field_value<'a>(record: &'a RecordWire, name: &str) -> Option<&'a Value> {
    record.fields.get(name)?.as_object()?.get("value")
}

fn optional_string_field(record: &RecordWire, name: &str) -> Result<Option<String>, ICloudError> {
    field_value(record, name)
        .map(|value| required_string_value(Some(value), format!("photo field `{name}`").as_str()))
        .transpose()
}

fn optional_bool_field(record: &RecordWire, name: &str) -> Result<Option<bool>, ICloudError> {
    field_value(record, name)
        .map(|value| parse_bool_value(value, format!("photo field `{name}`").as_str()))
        .transpose()
}

fn required_string_field(
    record: &RecordWire,
    name: &str,
    context: &str,
) -> Result<String, ICloudError> {
    required_string_value(
        field_value(record, name),
        format!("{context} field `{name}`").as_str(),
    )
}

fn required_string_value(value: Option<&Value>, context: &str) -> Result<String, ICloudError> {
    match value {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(other) => Err(ICloudError::InvalidResponse {
            message: format!("{context} is not a string: {other}"),
        }),
        None => Err(ICloudError::InvalidResponse {
            message: format!("missing {context}"),
        }),
    }
}

fn optional_u64_field(record: &RecordWire, name: &str) -> Result<Option<u64>, ICloudError> {
    field_value(record, name)
        .map(|value| required_u64_value(Some(value), format!("photo field `{name}`").as_str()))
        .transpose()
}

fn required_u64_field(record: &RecordWire, name: &str, context: &str) -> Result<u64, ICloudError> {
    required_u64_value(
        field_value(record, name),
        format!("{context} field `{name}`").as_str(),
    )
}

fn required_u64_value(value: Option<&Value>, context: &str) -> Result<u64, ICloudError> {
    match value {
        Some(Value::Number(value)) => value.as_u64().ok_or_else(|| ICloudError::InvalidResponse {
            message: format!("{context} is not an unsigned integer: {value}"),
        }),
        Some(other) => Err(ICloudError::InvalidResponse {
            message: format!("{context} is not an unsigned integer: {other}"),
        }),
        None => Err(ICloudError::InvalidResponse {
            message: format!("missing {context}"),
        }),
    }
}

fn field_bool(record: &RecordWire, name: &str) -> Result<bool, ICloudError> {
    Ok(optional_bool_field(record, name)?.unwrap_or(false))
}

fn parse_bool_value(value: &Value, context: &str) -> Result<bool, ICloudError> {
    match value {
        Value::Bool(value) => Ok(*value),
        Value::Number(value) => Ok(value.as_i64().is_some_and(|value| value != 0)),
        other => Err(ICloudError::InvalidResponse {
            message: format!("{context} is not a boolean: {other}"),
        }),
    }
}

fn optional_timestamp_field(
    record: &RecordWire,
    name: &str,
) -> Result<Option<OffsetDateTime>, ICloudError> {
    field_value(record, name)
        .map(|value| parse_timestamp_value(value, format!("photo field `{name}`").as_str()))
        .transpose()
}

fn required_timestamp_field(
    record: &RecordWire,
    name: &str,
    context: &str,
) -> Result<OffsetDateTime, ICloudError> {
    let value = field_value(record, name).ok_or_else(|| ICloudError::InvalidResponse {
        message: format!("missing {context} field `{name}`"),
    })?;
    parse_timestamp_value(value, format!("{context} field `{name}`").as_str())
}

fn parse_timestamp_value(value: &Value, context: &str) -> Result<OffsetDateTime, ICloudError> {
    match value {
        Value::Number(number) => crate::timeutil::datetime_from_number(number, context),
        other => Err(ICloudError::InvalidResponse {
            message: format!("{context} is not an integer: {other}"),
        }),
    }
}

fn required_record_reference(record: &RecordWire, name: &str) -> Result<String, ICloudError> {
    let value = field_value(record, name).ok_or_else(|| ICloudError::InvalidResponse {
        message: format!("photo asset record is missing `{name}`"),
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: format!("photo asset field `{name}` is not an object"),
        })?;
    required_string_value(
        object.get("recordName"),
        format!("photo asset field `{name}.recordName`").as_str(),
    )
}

fn decode_base64_string(value: &str, field: &str) -> Result<String, ICloudError> {
    let decoded = STANDARD
        .decode(value)
        .map_err(|error| ICloudError::InvalidResponse {
            message: format!("failed to decode photo field `{field}` from base64: {error}"),
        })?;
    String::from_utf8(decoded).map_err(|error| ICloudError::InvalidResponse {
        message: format!("photo field `{field}` is not valid utf-8: {error}"),
    })
}

fn next_offset(start_rank: u64, count: u64, direction: PhotoAlbumDirection) -> Option<u64> {
    match direction {
        PhotoAlbumDirection::Ascending => start_rank.checked_add(count),
        PhotoAlbumDirection::Descending => start_rank.checked_sub(count),
    }
}

fn cloudkit_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/plain"));
    headers
}

struct RecordsResponse {
    records: Vec<RecordWire>,
}

struct BatchResponse {
    batch: Vec<RecordsResponse>,
}

struct RecordWire {
    record_name: String,
    record_type: String,
    record_change_tag: Option<String>,
    fields: Map<String, Value>,
}

fn records_response(response: &ResponseParts) -> Result<RecordsResponse, ICloudError> {
    let payload = crate::json::object(response)?;
    Ok(RecordsResponse {
        records: payload
            .get("records")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .cloned()
            .map(record_wire)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn batch_response(response: &ResponseParts) -> Result<BatchResponse, ICloudError> {
    let payload = crate::json::object(response)?;
    Ok(BatchResponse {
        batch: payload
            .get("batch")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .cloned()
            .map(|value| {
                let object = crate::json::value_object(value, "photo batch entry")?;
                Ok(RecordsResponse {
                    records: object
                        .get("records")
                        .and_then(Value::as_array)
                        .unwrap_or_default()
                        .iter()
                        .cloned()
                        .map(record_wire)
                        .collect::<Result<Vec<_>, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, ICloudError>>()?,
    })
}

fn record_wire(value: Value) -> Result<RecordWire, ICloudError> {
    let object = crate::json::value_object(value, "photo record")?;
    let fields = object
        .get("fields")
        .cloned()
        .and_then(|value| match value {
            Value::Object(value) => Some(value),
            _ => None,
        })
        .unwrap_or_default();
    Ok(RecordWire {
        record_name: crate::json::string_field(&object, "recordName").unwrap_or_default(),
        record_type: crate::json::string_field(&object, "recordType").unwrap_or_default(),
        record_change_tag: crate::json::string_field(&object, "recordChangeTag"),
        fields,
    })
}

#[derive(Debug)]
struct PhotoAlbumEntry {
    record_name: String,
    name: String,
    is_folder: bool,
    direction: PhotoAlbumDirection,
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU32;

    use crate::{HeaderMap, StatusCode};
    use pretty_assertions::assert_eq;
    use time::macros::datetime;

    use super::*;
    use icloud_types::{PhotoAssetVersionKind, PhotoSmartAlbumKind};

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
    fn indexing_state_request_uses_text_plain_cloudkit_endpoint() {
        let root = Url::parse("https://p43-ckdatabasews.icloud.com/").expect("valid url");
        let protocol = PhotosProtocol::new(&root, "client-123", "42");

        let request = protocol.indexing_state().expect("request should build");

        assert_eq!(
            request.url.as_str(),
            "https://p43-ckdatabasews.icloud.com/database/1/com.apple.photos.cloud/production/private/records/query?clientId=client-123&dsid=42&remapEnums=true&getCurrentSyncToken=true",
        );
        assert_eq!(
            request.headers.get(CONTENT_TYPE).expect("content type"),
            "text/plain",
        );
    }

    #[test]
    fn album_query_can_target_folder_children() {
        let root = Url::parse("https://p43-ckdatabasews.icloud.com/").expect("valid url");
        let protocol = PhotosProtocol::new(&root, "client-123", "42");

        let request = protocol
            .query_albums(Some("folder-container"))
            .expect("request should build");
        let body = parse_body(&request);

        assert_eq!(body["query"]["recordType"], "CPLAlbumByPositionLive");
        assert_eq!(body["query"]["filterBy"][0]["fieldName"], "parentId");
        assert_eq!(
            body["query"]["filterBy"][0]["fieldValue"]["value"],
            "folder-container",
        );
    }

    #[test]
    fn album_page_request_matches_upstream_paging_shape() {
        let root = Url::parse("https://p43-ckdatabasews.icloud.com/").expect("valid url");
        let protocol = PhotosProtocol::new(&root, "client-123", "42");

        let request = protocol
            .album_page(
                &PhotoAlbumSummary::smart(PhotoSmartAlbumKind::Videos),
                10,
                NonZeroU32::new(2).expect("non-zero"),
            )
            .expect("request should build");
        let body = parse_body(&request);

        assert_eq!(body["resultsLimit"], 4);
        assert_eq!(
            body["query"]["recordType"],
            "CPLAssetAndMasterInSmartAlbumByAssetDate"
        );
        assert_eq!(body["query"]["filterBy"][0]["fieldName"], "startRank");
        assert_eq!(body["query"]["filterBy"][0]["fieldValue"]["value"], 10);
        assert_eq!(body["query"]["filterBy"][1]["fieldName"], "direction");
        assert_eq!(
            body["query"]["filterBy"][1]["fieldValue"]["value"],
            "ASCENDING"
        );
        assert_eq!(body["query"]["filterBy"][2]["fieldName"], "smartAlbum");
        assert_eq!(body["query"]["filterBy"][2]["fieldValue"]["value"], "VIDEO");
    }

    #[test]
    fn slomo_count_request_uses_the_correct_index_casing() {
        let root = Url::parse("https://p43-ckdatabasews.icloud.com/").expect("valid url");
        let protocol = PhotosProtocol::new(&root, "client-123", "42");

        let request = protocol
            .album_count(&PhotoAlbumSummary::smart(PhotoSmartAlbumKind::SloMo))
            .expect("request should build");
        let body = parse_body(&request);

        assert_eq!(body["batch"][0]["resultsLimit"], 1);
        assert_eq!(
            body["batch"][0]["query"]["recordType"],
            "HyperionIndexCountLookup",
        );
        assert_eq!(body["batch"][0]["query"]["filterBy"][0]["comparator"], "IN");
        assert_eq!(
            body["batch"][0]["query"]["filterBy"][0]["fieldValue"]["value"][0],
            "CPLAssetInSmartAlbumByAssetDate:Slomo",
        );
    }

    #[test]
    fn parser_extracts_ready_and_indexing_states() {
        let ready = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/photos/indexing_finished.json").to_vec(),
        );
        let indexing = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/photos/indexing_in_progress.json").to_vec(),
        );

        assert_eq!(
            parse_photo_library_state(&ready).expect("state should parse"),
            PhotoLibraryState::Ready,
        );
        assert_eq!(
            parse_photo_library_state(&indexing).expect("state should parse"),
            PhotoLibraryState::Indexing,
        );
    }

    #[test]
    fn parser_extracts_custom_albums_and_skips_root_deleted_entries() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/photos/custom_albums.json").to_vec(),
        );

        let albums = parse_photo_albums(&response).expect("albums should parse");

        assert_eq!(
            albums,
            vec![PhotoAlbumSummary::custom_with_direction(
                "folder-1",
                "Vacation",
                PhotoAlbumDirection::Descending,
            )],
        );
    }

    #[test]
    fn parser_extracts_photo_album_folders() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/photos/custom_albums.json").to_vec(),
        );

        let folders = parse_photo_album_folders(&response).expect("folders should parse");

        assert_eq!(folders, vec!["folder-container".to_owned()]);
    }

    #[test]
    fn parser_extracts_photo_assets_and_skips_orphan_masters() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/photos/album_page.json").to_vec(),
        );
        let album = PhotoAlbumSummary::all_photos();

        let page =
            parse_photo_album_page(&response, &album, 0, NonZeroU32::new(2).expect("non-zero"))
                .expect("page should parse");

        assert_eq!(page.assets.len(), 2);
        assert_eq!(page.next_offset, Some(3));
        assert_eq!(page.assets[0].filename, "IMG_0001.JPG");
        assert_eq!(
            page.assets[0]
                .version(PhotoAssetVersionKind::Thumb)
                .expect("thumb version")
                .url
                .as_str(),
            "https://cvws.icloud.com/thumb/1",
        );
        assert_eq!(
            page.assets[0].created_at,
            Some(datetime!(2024-04-10 06:25:01 UTC))
        );
        assert_eq!(page.assets[1].filename, "movie.mov");
        assert_eq!(page.assets[1].media_kind, PhotoMediaKind::Video);
        assert_eq!(page.assets[1].created_at, None);
    }

    #[test]
    fn delete_request_uses_asset_change_tag() {
        let root = Url::parse("https://p43-ckdatabasews.icloud.com/").expect("valid url");
        let protocol = PhotosProtocol::new(&root, "client-123", "42");
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/photos/album_page.json").to_vec(),
        );
        let asset = parse_photo_album_page(
            &response,
            &PhotoAlbumSummary::all_photos(),
            0,
            NonZeroU32::new(2).expect("non-zero"),
        )
        .expect("page should parse")
        .assets
        .into_iter()
        .next()
        .expect("sample asset");

        let request = protocol.delete_asset(&asset).expect("request should build");
        let body = parse_body(&request);

        assert_eq!(
            body["operations"][0]["record"]["recordName"],
            "asset-photo-1"
        );
        assert_eq!(
            body["operations"][0]["record"]["recordChangeTag"],
            "asset-tag-1",
        );
    }
}
