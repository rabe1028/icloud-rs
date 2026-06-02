use crate::prelude::*;

use crate::Method;
use crate::json::{bool_field, object, string_field, u64_field};
use url::Url;

use icloud_types::{
    AccountDevice, AccountQuotaStatus, AccountStorage, AccountStorageUsage,
    AccountStorageUsageForMedia, FamilyMember, ICloudError,
};

use crate::{RequestSpec, ResponseParts};

#[derive(Debug, Clone)]
pub struct AccountProtocol<'a> {
    service_root: &'a Url,
    client_id: &'a str,
    dsid: &'a str,
}

impl<'a> AccountProtocol<'a> {
    #[must_use]
    pub fn new(service_root: &'a Url, client_id: &'a str, dsid: &'a str) -> Self {
        Self {
            service_root,
            client_id,
            dsid,
        }
    }

    pub fn devices(&self) -> Result<RequestSpec, ICloudError> {
        self.get(
            "setup/web/device/getDevices",
            core::iter::empty::<(&str, &str)>(),
        )
    }

    pub fn family(&self) -> Result<RequestSpec, ICloudError> {
        self.get(
            "setup/web/family/getFamilyDetails",
            core::iter::empty::<(&str, &str)>(),
        )
    }

    pub fn family_member_photo(&self, member_id: &str) -> Result<RequestSpec, ICloudError> {
        self.get("setup/web/family/getMemberPhoto", [("memberId", member_id)])
    }

    pub fn storage(&self) -> Result<RequestSpec, ICloudError> {
        let mut url = self.service_root.join("setup/ws/1/storageUsageInfo")?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("clientId", self.client_id);
            query.append_pair("dsid", self.dsid);
        }
        Ok(RequestSpec::new(Method::Post, url))
    }

    fn get<I>(&self, path: &str, extra: I) -> Result<RequestSpec, ICloudError>
    where
        I: IntoIterator<Item = (&'static str, &'a str)>,
    {
        let mut url = self.service_root.join(path)?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("clientId", self.client_id);
            query.append_pair("dsid", self.dsid);
            for (key, value) in extra {
                query.append_pair(key, value);
            }
        }
        Ok(RequestSpec::new(Method::Get, url))
    }
}

pub fn parse_account_devices(response: &ResponseParts) -> Result<Vec<AccountDevice>, ICloudError> {
    let mut payload = object(response)?;
    payload
        .remove("devices")
        .and_then(|value| match value {
            icloud_types::JsonValue::Array(values) => Some(values),
            _ => None,
        })
        .unwrap_or_default()
        .into_iter()
        .map(parse_account_device)
        .collect::<Result<Vec<_>, _>>()
}

pub fn parse_family_members(response: &ResponseParts) -> Result<Vec<FamilyMember>, ICloudError> {
    let mut payload = object(response)?;
    payload
        .remove("familyMembers")
        .and_then(|value| match value {
            icloud_types::JsonValue::Array(values) => Some(values),
            _ => None,
        })
        .unwrap_or_default()
        .into_iter()
        .map(parse_family_member)
        .collect::<Result<Vec<_>, _>>()
}

pub fn parse_account_storage(response: &ResponseParts) -> Result<AccountStorage, ICloudError> {
    let mut payload = object(response)?;
    let storage_usage_info = payload
        .remove("storageUsageInfo")
        .and_then(|value| match value {
            icloud_types::JsonValue::Object(value) => Some(value),
            _ => None,
        })
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: "missing storageUsageInfo".to_owned(),
        })?;
    let quota_status = payload
        .remove("quotaStatus")
        .and_then(|value| match value {
            icloud_types::JsonValue::Object(value) => Some(parse_quota_status(value)),
            _ => None,
        })
        .unwrap_or_default();
    let storage_usage_by_media = payload
        .remove("storageUsageByMedia")
        .and_then(|value| match value {
            icloud_types::JsonValue::Array(values) => Some(values),
            _ => None,
        })
        .unwrap_or_default()
        .into_iter()
        .map(parse_usage_for_media)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AccountStorage {
        usage: AccountStorageUsage {
            comp_storage_in_bytes: u64_field(&storage_usage_info, "compStorageInBytes")
                .unwrap_or(0),
            used_storage_in_bytes: u64_field(&storage_usage_info, "usedStorageInBytes")
                .unwrap_or(0),
            total_storage_in_bytes: u64_field(&storage_usage_info, "totalStorageInBytes")
                .unwrap_or(0),
            commerce_storage_in_bytes: u64_field(&storage_usage_info, "commerceStorageInBytes")
                .unwrap_or(0),
            quota_status,
            extra: storage_usage_info,
        },
        usages_by_media: storage_usage_by_media,
        extra: payload,
    })
}

fn parse_account_device(value: icloud_types::JsonValue) -> Result<AccountDevice, ICloudError> {
    let raw = crate::json::value_object(value, "account device")?;
    Ok(AccountDevice {
        name: string_field(&raw, "name"),
        model_display_name: string_field(&raw, "modelDisplayName"),
        device_class: string_field(&raw, "deviceClass"),
        serial_number: string_field(&raw, "serialNumber"),
        extra: raw,
    })
}

fn parse_family_member(value: icloud_types::JsonValue) -> Result<FamilyMember, ICloudError> {
    let raw = crate::json::value_object(value, "family member")?;
    Ok(FamilyMember {
        dsid: string_field(&raw, "dsid"),
        original_invitation_email: string_field(&raw, "originalInvitationEmail"),
        full_name: string_field(&raw, "fullName"),
        age_classification: string_field(&raw, "ageClassification"),
        apple_id_for_purchases: string_field(&raw, "appleIdForPurchases"),
        apple_id: string_field(&raw, "appleId"),
        family_id: string_field(&raw, "familyId"),
        first_name: string_field(&raw, "firstName"),
        last_name: string_field(&raw, "lastName"),
        has_parental_privileges: bool_field(&raw, "hasParentalPrivileges"),
        has_screen_time_enabled: bool_field(&raw, "hasScreenTimeEnabled"),
        has_ask_to_buy_enabled: bool_field(&raw, "hasAskToBuyEnabled"),
        has_share_purchases_enabled: bool_field(&raw, "hasSharePurchasesEnabled"),
        share_my_location_enabled_family_members: raw
            .get("shareMyLocationEnabledFamilyMembers")
            .and_then(|value| value.as_array())
            .unwrap_or_default()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        has_share_my_location_enabled: bool_field(&raw, "hasShareMyLocationEnabled"),
        dsid_for_purchases: string_field(&raw, "dsidForPurchases"),
        extra: raw,
    })
}

fn parse_quota_status(raw: icloud_types::JsonObject) -> AccountQuotaStatus {
    AccountQuotaStatus {
        over_quota: bool_field(&raw, "overQuota").unwrap_or(false),
        have_max_quota_tier: bool_field(&raw, "haveMaxQuotaTier").unwrap_or(false),
        almost_full: bool_field(&raw, "almost-full").unwrap_or(false),
        paid_quota: bool_field(&raw, "paidQuota").unwrap_or(false),
        extra: raw,
    }
}

fn parse_usage_for_media(
    value: icloud_types::JsonValue,
) -> Result<AccountStorageUsageForMedia, ICloudError> {
    let raw = crate::json::value_object(value, "storage media usage")?;
    Ok(AccountStorageUsageForMedia {
        media_key: string_field(&raw, "mediaKey").unwrap_or_default(),
        display_label: string_field(&raw, "displayLabel"),
        display_color: string_field(&raw, "displayColor"),
        usage_in_bytes: u64_field(&raw, "usageInBytes").unwrap_or(0),
        extra: raw,
    })
}

#[cfg(test)]
mod tests {
    use crate::{HeaderMap, Method, StatusCode};
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn devices_request_uses_account_service_root() {
        let root = Url::parse("https://p43-setup.icloud.com").expect("valid url");
        let protocol = AccountProtocol::new(&root, "client-123", "42");

        let request = protocol.devices().expect("request should build");

        assert_eq!(request.method, Method::Get);
        assert_eq!(
            request.url.as_str(),
            "https://p43-setup.icloud.com/setup/web/device/getDevices?clientId=client-123&dsid=42",
        );
    }

    #[test]
    fn family_photo_request_includes_member_identifier() {
        let root = Url::parse("https://p43-setup.icloud.com").expect("valid url");
        let protocol = AccountProtocol::new(&root, "client-123", "42");

        let request = protocol
            .family_member_photo("member-1")
            .expect("request should build");

        assert_eq!(
            request.url.as_str(),
            "https://p43-setup.icloud.com/setup/web/family/getMemberPhoto?clientId=client-123&dsid=42&memberId=member-1",
        );
    }

    #[test]
    fn storage_request_uses_post_and_sharded_setup_root() {
        let root = Url::parse("https://p43-setup.icloud.com").expect("valid url");
        let protocol = AccountProtocol::new(&root, "client-123", "42");

        let request = protocol.storage().expect("request should build");

        assert_eq!(request.method, Method::Post);
        assert_eq!(
            request.url.as_str(),
            "https://p43-setup.icloud.com/setup/ws/1/storageUsageInfo?clientId=client-123&dsid=42",
        );
    }

    #[test]
    fn parser_extracts_account_devices_family_and_storage() {
        let devices = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/account/devices.json").to_vec(),
        );
        let family = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/account/family.json").to_vec(),
        );
        let storage = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/account/storage.json").to_vec(),
        );

        let devices = parse_account_devices(&devices).expect("devices should parse");
        let family = parse_family_members(&family).expect("family should parse");
        let storage = parse_account_storage(&storage).expect("storage should parse");

        assert_eq!(devices.len(), 2);
        assert_eq!(
            devices[0].model_display_name.as_deref(),
            Some("MacBook Pro")
        );
        assert_eq!(family[0].full_name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(storage.usage.available_storage_in_bytes(), 16_106_127_360);
        assert_eq!(storage.usages_by_media[0].media_key, "drive");
        assert!(storage.usage.quota_status.paid_quota);
    }
}
