use crate::prelude::*;

use crate::JsonValue as Value;
use alloc::collections::BTreeMap as Map;
use time::OffsetDateTime;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotoLibraryState {
    Ready,
    Indexing,
    Unknown(String),
}

impl PhotoLibraryState {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ready => "FINISHED",
            Self::Indexing => "INDEXING",
            Self::Unknown(value) => value.as_str(),
        }
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

impl core::fmt::Display for PhotoLibraryState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for PhotoLibraryState {
    fn from(value: String) -> Self {
        match value.as_str() {
            "FINISHED" => Self::Ready,
            "INDEXING" => Self::Indexing,
            _ => Self::Unknown(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoAlbumDirection {
    Ascending,
    Descending,
}

impl PhotoAlbumDirection {
    #[must_use]
    pub fn as_api_value(&self) -> &'static str {
        match self {
            Self::Ascending => "ASCENDING",
            Self::Descending => "DESCENDING",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhotoSmartAlbumKind {
    AllPhotos,
    TimeLapse,
    Videos,
    SloMo,
    Bursts,
    Favorites,
    Panoramas,
    Screenshots,
    Live,
    RecentlyDeleted,
    Hidden,
}

impl PhotoSmartAlbumKind {
    #[must_use]
    pub fn all() -> &'static [Self] {
        const ALBUMS: [PhotoSmartAlbumKind; 11] = [
            PhotoSmartAlbumKind::AllPhotos,
            PhotoSmartAlbumKind::TimeLapse,
            PhotoSmartAlbumKind::Videos,
            PhotoSmartAlbumKind::SloMo,
            PhotoSmartAlbumKind::Bursts,
            PhotoSmartAlbumKind::Favorites,
            PhotoSmartAlbumKind::Panoramas,
            PhotoSmartAlbumKind::Screenshots,
            PhotoSmartAlbumKind::Live,
            PhotoSmartAlbumKind::RecentlyDeleted,
            PhotoSmartAlbumKind::Hidden,
        ];
        &ALBUMS
    }

    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::AllPhotos => "All Photos",
            Self::TimeLapse => "Time-lapse",
            Self::Videos => "Videos",
            Self::SloMo => "Slo-mo",
            Self::Bursts => "Bursts",
            Self::Favorites => "Favorites",
            Self::Panoramas => "Panoramas",
            Self::Screenshots => "Screenshots",
            Self::Live => "Live Photos",
            Self::RecentlyDeleted => "Recently Deleted",
            Self::Hidden => "Hidden",
        }
    }

    #[must_use]
    pub fn filter_code(&self) -> Option<&'static str> {
        match self {
            Self::TimeLapse => Some("TIMELAPSE"),
            Self::Videos => Some("VIDEO"),
            Self::SloMo => Some("SLOMO"),
            Self::Favorites => Some("FAVORITE"),
            Self::Panoramas => Some("PANORAMA"),
            Self::Screenshots => Some("SCREENSHOT"),
            Self::Live => Some("LIVE"),
            Self::AllPhotos | Self::Bursts | Self::RecentlyDeleted | Self::Hidden => None,
        }
    }

    #[must_use]
    pub fn query(&self) -> PhotoAlbumQuery {
        match self {
            Self::AllPhotos => PhotoAlbumQuery {
                list_type: "CPLAssetAndMasterByAddedDate".to_owned(),
                count_index_id: "CPLAssetByAddedDate".to_owned(),
                direction: PhotoAlbumDirection::Ascending,
                filters: Vec::new(),
            },
            Self::TimeLapse => PhotoAlbumQuery {
                list_type: "CPLAssetAndMasterInSmartAlbumByAssetDate".to_owned(),
                count_index_id: "CPLAssetInSmartAlbumByAssetDate:Timelapse".to_owned(),
                direction: PhotoAlbumDirection::Ascending,
                filters: vec![PhotoAlbumFilter::SmartAlbum(*self)],
            },
            Self::Videos => PhotoAlbumQuery {
                list_type: "CPLAssetAndMasterInSmartAlbumByAssetDate".to_owned(),
                count_index_id: "CPLAssetInSmartAlbumByAssetDate:Video".to_owned(),
                direction: PhotoAlbumDirection::Ascending,
                filters: vec![PhotoAlbumFilter::SmartAlbum(*self)],
            },
            Self::SloMo => PhotoAlbumQuery {
                list_type: "CPLAssetAndMasterInSmartAlbumByAssetDate".to_owned(),
                count_index_id: "CPLAssetInSmartAlbumByAssetDate:Slomo".to_owned(),
                direction: PhotoAlbumDirection::Ascending,
                filters: vec![PhotoAlbumFilter::SmartAlbum(*self)],
            },
            Self::Bursts => PhotoAlbumQuery {
                list_type: "CPLBurstStackAssetAndMasterByAssetDate".to_owned(),
                count_index_id: "CPLAssetBurstStackAssetByAssetDate".to_owned(),
                direction: PhotoAlbumDirection::Ascending,
                filters: Vec::new(),
            },
            Self::Favorites => PhotoAlbumQuery {
                list_type: "CPLAssetAndMasterInSmartAlbumByAssetDate".to_owned(),
                count_index_id: "CPLAssetInSmartAlbumByAssetDate:Favorite".to_owned(),
                direction: PhotoAlbumDirection::Ascending,
                filters: vec![PhotoAlbumFilter::SmartAlbum(*self)],
            },
            Self::Panoramas => PhotoAlbumQuery {
                list_type: "CPLAssetAndMasterInSmartAlbumByAssetDate".to_owned(),
                count_index_id: "CPLAssetInSmartAlbumByAssetDate:Panorama".to_owned(),
                direction: PhotoAlbumDirection::Ascending,
                filters: vec![PhotoAlbumFilter::SmartAlbum(*self)],
            },
            Self::Screenshots => PhotoAlbumQuery {
                list_type: "CPLAssetAndMasterInSmartAlbumByAssetDate".to_owned(),
                count_index_id: "CPLAssetInSmartAlbumByAssetDate:Screenshot".to_owned(),
                direction: PhotoAlbumDirection::Ascending,
                filters: vec![PhotoAlbumFilter::SmartAlbum(*self)],
            },
            Self::Live => PhotoAlbumQuery {
                list_type: "CPLAssetAndMasterInSmartAlbumByAssetDate".to_owned(),
                count_index_id: "CPLAssetInSmartAlbumByAssetDate:Live".to_owned(),
                direction: PhotoAlbumDirection::Ascending,
                filters: vec![PhotoAlbumFilter::SmartAlbum(*self)],
            },
            Self::RecentlyDeleted => PhotoAlbumQuery {
                list_type: "CPLAssetAndMasterDeletedByExpungedDate".to_owned(),
                count_index_id: "CPLAssetDeletedByExpungedDate".to_owned(),
                direction: PhotoAlbumDirection::Ascending,
                filters: Vec::new(),
            },
            Self::Hidden => PhotoAlbumQuery {
                list_type: "CPLAssetAndMasterHiddenByAssetDate".to_owned(),
                count_index_id: "CPLAssetHiddenByAssetDate".to_owned(),
                direction: PhotoAlbumDirection::Ascending,
                filters: Vec::new(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotoAlbumFilter {
    ParentId(String),
    SmartAlbum(PhotoSmartAlbumKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoAlbumQuery {
    pub list_type: String,
    pub count_index_id: String,
    pub direction: PhotoAlbumDirection,
    pub filters: Vec<PhotoAlbumFilter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotoAlbumIdentity {
    Smart(PhotoSmartAlbumKind),
    Custom { record_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoAlbumSummary {
    pub name: String,
    pub identity: PhotoAlbumIdentity,
    pub query: PhotoAlbumQuery,
}

impl PhotoAlbumSummary {
    #[must_use]
    pub fn smart(kind: PhotoSmartAlbumKind) -> Self {
        Self {
            name: kind.display_name().to_owned(),
            identity: PhotoAlbumIdentity::Smart(kind),
            query: kind.query(),
        }
    }

    #[must_use]
    pub fn custom(record_name: impl Into<String>, name: impl Into<String>) -> Self {
        Self::custom_with_direction(record_name, name, PhotoAlbumDirection::Ascending)
    }

    #[must_use]
    pub fn custom_with_direction(
        record_name: impl Into<String>,
        name: impl Into<String>,
        direction: PhotoAlbumDirection,
    ) -> Self {
        let record_name = record_name.into();
        Self {
            name: name.into(),
            identity: PhotoAlbumIdentity::Custom {
                record_name: record_name.clone(),
            },
            query: PhotoAlbumQuery {
                list_type: "CPLContainerRelationLiveByAssetDate".to_owned(),
                count_index_id: format!("CPLContainerRelationNotDeletedByAssetDate:{record_name}"),
                direction,
                filters: vec![PhotoAlbumFilter::ParentId(record_name)],
            },
        }
    }

    #[must_use]
    pub fn all_photos() -> Self {
        Self::smart(PhotoSmartAlbumKind::AllPhotos)
    }

    #[must_use]
    pub fn smart_albums() -> Vec<Self> {
        PhotoSmartAlbumKind::all()
            .iter()
            .copied()
            .map(Self::smart)
            .collect()
    }

    #[must_use]
    pub fn record_name(&self) -> Option<&str> {
        match &self.identity {
            PhotoAlbumIdentity::Smart(_) => None,
            PhotoAlbumIdentity::Custom { record_name } => Some(record_name.as_str()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoMediaKind {
    Image,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoAssetVersionKind {
    Original,
    Medium,
    Thumb,
}

impl PhotoAssetVersionKind {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Medium => "medium",
            Self::Thumb => "thumb",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoAssetVersion {
    pub url: Url,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub size: u64,
    pub file_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoAssetVersions {
    pub original: PhotoAssetVersion,
    pub medium: Option<PhotoAssetVersion>,
    pub thumb: Option<PhotoAssetVersion>,
}

impl PhotoAssetVersions {
    #[must_use]
    pub fn get(&self, kind: PhotoAssetVersionKind) -> Option<&PhotoAssetVersion> {
        match kind {
            PhotoAssetVersionKind::Original => Some(&self.original),
            PhotoAssetVersionKind::Medium => self.medium.as_ref(),
            PhotoAssetVersionKind::Thumb => self.thumb.as_ref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoDimensions {
    pub width: u64,
    pub height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoDeleteTarget {
    pub record_name: String,
    pub record_type: String,
    pub change_tag: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhotoAsset {
    pub id: String,
    pub filename: String,
    pub size: u64,
    pub created_at: Option<OffsetDateTime>,
    pub added_at: OffsetDateTime,
    pub dimensions: PhotoDimensions,
    pub media_kind: PhotoMediaKind,
    pub versions: PhotoAssetVersions,
    pub delete_target: PhotoDeleteTarget,
    pub master_fields: Map<String, Value>,
    pub asset_fields: Map<String, Value>,
}

impl PhotoAsset {
    #[must_use]
    pub fn version(&self, kind: PhotoAssetVersionKind) -> Option<&PhotoAssetVersion> {
        self.versions.get(kind)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhotoAlbumPage {
    pub assets: Vec<PhotoAsset>,
    pub next_offset: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::{PhotoAlbumSummary, PhotoSmartAlbumKind};

    #[test]
    fn all_photos_query_matches_pyicloud_added_date_album() {
        let album = PhotoAlbumSummary::smart(PhotoSmartAlbumKind::AllPhotos);

        assert_eq!(album.query.list_type, "CPLAssetAndMasterByAddedDate");
        assert_eq!(album.query.count_index_id, "CPLAssetByAddedDate");
        assert!(album.query.filters.is_empty());
    }
}
