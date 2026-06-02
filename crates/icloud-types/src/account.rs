use crate::prelude::*;

use crate::JsonValue as Value;
use alloc::collections::BTreeMap as Map;

#[derive(Debug, Clone, PartialEq)]
pub struct AccountDevice {
    pub name: Option<String>,
    pub model_display_name: Option<String>,
    pub device_class: Option<String>,
    pub serial_number: Option<String>,
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FamilyMember {
    pub dsid: Option<String>,
    pub original_invitation_email: Option<String>,
    pub full_name: Option<String>,
    pub age_classification: Option<String>,
    pub apple_id_for_purchases: Option<String>,
    pub apple_id: Option<String>,
    pub family_id: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub has_parental_privileges: Option<bool>,
    pub has_screen_time_enabled: Option<bool>,
    pub has_ask_to_buy_enabled: Option<bool>,
    pub has_share_purchases_enabled: Option<bool>,
    pub share_my_location_enabled_family_members: Vec<String>,
    pub has_share_my_location_enabled: Option<bool>,
    pub dsid_for_purchases: Option<String>,
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountMemberPhoto {
    pub member_id: String,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountStorageUsageForMedia {
    pub media_key: String,
    pub display_label: Option<String>,
    pub display_color: Option<String>,
    pub usage_in_bytes: u64,
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccountQuotaStatus {
    pub over_quota: bool,
    pub have_max_quota_tier: bool,
    pub almost_full: bool,
    pub paid_quota: bool,
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountStorageUsage {
    pub comp_storage_in_bytes: u64,
    pub used_storage_in_bytes: u64,
    pub total_storage_in_bytes: u64,
    pub commerce_storage_in_bytes: u64,
    pub quota_status: AccountQuotaStatus,
    pub extra: Map<String, Value>,
}

impl AccountStorageUsage {
    #[must_use]
    pub fn available_storage_in_bytes(&self) -> u64 {
        self.total_storage_in_bytes
            .saturating_sub(self.used_storage_in_bytes)
    }

    #[must_use]
    pub fn used_storage_in_percent(&self) -> f64 {
        percentage(self.used_storage_in_bytes, self.total_storage_in_bytes)
    }

    #[must_use]
    pub fn available_storage_in_percent(&self) -> f64 {
        percentage(
            self.available_storage_in_bytes(),
            self.total_storage_in_bytes,
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountStorage {
    pub usage: AccountStorageUsage,
    pub usages_by_media: Vec<AccountStorageUsageForMedia>,
    pub extra: Map<String, Value>,
}

fn percentage(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        return 0.0;
    }
    let scaled = ((u128::from(part) * 10_000) + (u128::from(whole) / 2)) / u128::from(whole);
    scaled as f64 / 100.0
}
