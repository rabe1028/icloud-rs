use crate::prelude::*;

use crate::JsonValue as Value;
use alloc::collections::BTreeMap as Map;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UbiquityNodeId(String);

impl UbiquityNodeId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn root() -> Self {
        Self::from("0")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for UbiquityNodeId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for UbiquityNodeId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<u64> for UbiquityNodeId {
    fn from(value: u64) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UbiquityNodeMetadata {
    pub item_id: UbiquityNodeId,
    pub name: String,
    pub modified_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UbiquityFolder {
    pub metadata: UbiquityNodeMetadata,
    pub size: Option<u64>,
    pub extra: Map<String, Value>,
}

impl UbiquityFolder {
    #[must_use]
    pub fn name(&self) -> &str {
        self.metadata.name.as_str()
    }

    #[must_use]
    pub fn item_id(&self) -> &UbiquityNodeId {
        &self.metadata.item_id
    }

    #[must_use]
    pub fn modified_at(&self) -> Option<OffsetDateTime> {
        self.metadata.modified_at
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UbiquityFile {
    pub metadata: UbiquityNodeMetadata,
    pub size: Option<u64>,
    pub extra: Map<String, Value>,
}

impl UbiquityFile {
    #[must_use]
    pub fn name(&self) -> &str {
        self.metadata.name.as_str()
    }

    #[must_use]
    pub fn item_id(&self) -> &UbiquityNodeId {
        &self.metadata.item_id
    }

    #[must_use]
    pub fn modified_at(&self) -> Option<OffsetDateTime> {
        self.metadata.modified_at
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UbiquityUnknownNode {
    pub metadata: UbiquityNodeMetadata,
    pub kind: String,
    pub size: Option<u64>,
    pub extra: Map<String, Value>,
}

impl UbiquityUnknownNode {
    #[must_use]
    pub fn name(&self) -> &str {
        self.metadata.name.as_str()
    }

    #[must_use]
    pub fn item_id(&self) -> &UbiquityNodeId {
        &self.metadata.item_id
    }

    #[must_use]
    pub fn modified_at(&self) -> Option<OffsetDateTime> {
        self.metadata.modified_at
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UbiquityNode {
    Folder(UbiquityFolder),
    File(UbiquityFile),
    Unknown(UbiquityUnknownNode),
}

impl UbiquityNode {
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Folder(node) => node.name(),
            Self::File(node) => node.name(),
            Self::Unknown(node) => node.name(),
        }
    }

    #[must_use]
    pub fn item_id(&self) -> &UbiquityNodeId {
        match self {
            Self::Folder(node) => node.item_id(),
            Self::File(node) => node.item_id(),
            Self::Unknown(node) => node.item_id(),
        }
    }

    #[must_use]
    pub fn modified_at(&self) -> Option<OffsetDateTime> {
        match self {
            Self::Folder(node) => node.modified_at(),
            Self::File(node) => node.modified_at(),
            Self::Unknown(node) => node.modified_at(),
        }
    }

    #[must_use]
    pub fn size(&self) -> Option<u64> {
        match self {
            Self::Folder(node) => node.size,
            Self::File(node) => node.size,
            Self::Unknown(node) => node.size,
        }
    }

    #[must_use]
    pub fn as_folder(&self) -> Option<&UbiquityFolder> {
        match self {
            Self::Folder(node) => Some(node),
            Self::File(_) | Self::Unknown(_) => None,
        }
    }

    #[must_use]
    pub fn as_file(&self) -> Option<&UbiquityFile> {
        match self {
            Self::File(node) => Some(node),
            Self::Folder(_) | Self::Unknown(_) => None,
        }
    }

    #[must_use]
    pub fn metadata(&self) -> &UbiquityNodeMetadata {
        match self {
            Self::Folder(node) => &node.metadata,
            Self::File(node) => &node.metadata,
            Self::Unknown(node) => &node.metadata,
        }
    }
}
