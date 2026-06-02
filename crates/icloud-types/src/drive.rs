use crate::prelude::*;

use crate::JsonValue as Value;
use alloc::collections::BTreeMap as Map;
use time::OffsetDateTime;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DriveDocumentId(String);

impl DriveDocumentId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for DriveDocumentId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for DriveDocumentId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DriveWsId(String);

impl DriveWsId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for DriveWsId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for DriveWsId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveNodeMetadata {
    pub drivewsid: DriveWsId,
    pub docwsid: DriveDocumentId,
    pub zone: String,
    pub name: String,
    pub parent_id: Option<DriveWsId>,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveContainerKind {
    Folder,
    AppLibrary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveContainerUnavailable {
    pub status: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DriveContainerContents {
    Loaded(Vec<DriveNode>),
    Unavailable(DriveContainerUnavailable),
    Omitted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DriveContainer {
    pub metadata: DriveNodeMetadata,
    pub kind: DriveContainerKind,
    pub created_at: Option<OffsetDateTime>,
    pub asset_quota: Option<u64>,
    pub file_count: Option<u64>,
    pub share_count: Option<u64>,
    pub share_alias_count: Option<u64>,
    pub direct_children_count: Option<u64>,
    pub number_of_items: Option<u64>,
    pub contents: DriveContainerContents,
    pub extra: Map<String, Value>,
}

impl DriveContainer {
    #[must_use]
    pub fn name(&self) -> &str {
        self.metadata.name.as_str()
    }

    #[must_use]
    pub fn document_id(&self) -> &DriveDocumentId {
        &self.metadata.docwsid
    }

    #[must_use]
    pub fn drivewsid(&self) -> &DriveWsId {
        &self.metadata.drivewsid
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DriveFile {
    pub metadata: DriveNodeMetadata,
    pub base_name: String,
    pub extension: Option<String>,
    pub hidden_extension: bool,
    pub size: u64,
    pub date_changed: Option<OffsetDateTime>,
    pub date_modified: Option<OffsetDateTime>,
    pub date_last_open: Option<OffsetDateTime>,
    pub extra: Map<String, Value>,
}

impl DriveFile {
    #[must_use]
    pub fn name(&self) -> &str {
        self.metadata.name.as_str()
    }

    #[must_use]
    pub fn document_id(&self) -> &DriveDocumentId {
        &self.metadata.docwsid
    }

    #[must_use]
    pub fn drivewsid(&self) -> &DriveWsId {
        &self.metadata.drivewsid
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DriveUnknownNode {
    pub metadata: DriveNodeMetadata,
    pub kind: String,
    pub extra: Map<String, Value>,
}

impl DriveUnknownNode {
    #[must_use]
    pub fn name(&self) -> &str {
        self.metadata.name.as_str()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DriveNode {
    Container(DriveContainer),
    File(DriveFile),
    Unknown(DriveUnknownNode),
}

impl DriveNode {
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Container(node) => node.name(),
            Self::File(node) => node.name(),
            Self::Unknown(node) => node.name(),
        }
    }

    #[must_use]
    pub fn as_container(&self) -> Option<&DriveContainer> {
        match self {
            Self::Container(node) => Some(node),
            Self::File(_) | Self::Unknown(_) => None,
        }
    }

    #[must_use]
    pub fn as_file(&self) -> Option<&DriveFile> {
        match self {
            Self::File(node) => Some(node),
            Self::Container(_) | Self::Unknown(_) => None,
        }
    }

    #[must_use]
    pub fn metadata(&self) -> &DriveNodeMetadata {
        match self {
            Self::Container(node) => &node.metadata,
            Self::File(node) => &node.metadata,
            Self::Unknown(node) => &node.metadata,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveDownloadToken {
    pub url: Url,
    pub token: String,
    pub signature: String,
    pub wrapping_key: String,
    pub reference_signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveDownloadTarget {
    Data(DriveDownloadToken),
    Package(DriveDownloadToken),
}

impl DriveDownloadTarget {
    #[must_use]
    pub fn token(&self) -> &DriveDownloadToken {
        match self {
            Self::Data(token) | Self::Package(token) => token,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveDownload {
    pub document_id: DriveDocumentId,
    pub target: DriveDownloadTarget,
    pub thumbnail: Option<DriveDownloadToken>,
    pub double_etag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveUploadFile {
    pub name: String,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

impl DriveUploadFile {
    #[must_use]
    pub fn new(name: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            content_type: None,
            bytes: bytes.into(),
        }
    }

    #[must_use]
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveUploadTicket {
    pub document_id: DriveDocumentId,
    pub content_url: Url,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveUploadReceipt {
    pub file_checksum: String,
    pub wrapping_key: String,
    pub reference_checksum: String,
    pub receipt: Option<String>,
    pub size: u64,
}
