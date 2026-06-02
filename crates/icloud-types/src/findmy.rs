use crate::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindMySelectedDevice {
    All,
    Device(String),
}

impl FindMySelectedDevice {
    #[must_use]
    pub fn as_api_value(&self) -> &str {
        match self {
            Self::All => "all",
            Self::Device(device_id) => device_id.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindMyRefreshOptions {
    pub with_family: bool,
    pub should_locate: bool,
    pub selected_device: FindMySelectedDevice,
    pub device_list_version: u8,
}

impl Default for FindMyRefreshOptions {
    fn default() -> Self {
        Self {
            with_family: true,
            should_locate: true,
            selected_device: FindMySelectedDevice::All,
            device_list_version: 1,
        }
    }
}

impl FindMyRefreshOptions {
    #[must_use]
    pub fn for_device(device_id: impl Into<String>) -> Self {
        Self {
            selected_device: FindMySelectedDevice::Device(device_id.into()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindMyBatteryStatus {
    Charging,
    NotCharging,
    Unknown(String),
}

impl FindMyBatteryStatus {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Charging => "Charging",
            Self::NotCharging => "NotCharging",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl core::fmt::Display for FindMyBatteryStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for FindMyBatteryStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Charging" => Self::Charging,
            "NotCharging" => Self::NotCharging,
            _ => Self::Unknown(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FindMyLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub horizontal_accuracy: Option<f64>,
    pub location_type: Option<String>,
    pub position_type: Option<String>,
    pub time_stamp: Option<i64>,
    pub is_old: Option<bool>,
    pub extra: crate::JsonObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FindMyDevice {
    pub id: String,
    pub name: String,
    pub device_display_name: Option<String>,
    pub model_display_name: Option<String>,
    pub raw_device_model: Option<String>,
    pub device_status: Option<i64>,
    pub battery_level: Option<f64>,
    pub battery_status: Option<FindMyBatteryStatus>,
    pub location: Option<FindMyLocation>,
    pub lost_mode_enabled: bool,
    pub location_capable: bool,
    pub location_enabled: bool,
    pub this_device: bool,
    pub is_mac: bool,
    pub extra: crate::JsonObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FindMyDevicesResponse {
    pub devices: Vec<FindMyDevice>,
}

impl FindMyDevicesResponse {
    #[must_use]
    pub fn device(&self, id: &str) -> Option<&FindMyDevice> {
        self.devices.iter().find(|device| device.id == id)
    }

    #[must_use]
    pub fn into_device(self, id: &str) -> Option<FindMyDevice> {
        self.devices.into_iter().find(|device| device.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindMySoundCommand {
    pub device_id: String,
    pub subject: String,
    pub with_family: bool,
}

impl FindMySoundCommand {
    #[must_use]
    pub fn new(device_id: impl Into<String>, subject: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
            subject: subject.into(),
            with_family: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindMyMessageCommand {
    pub device_id: String,
    pub subject: String,
    pub text: String,
    pub play_sound: bool,
}

impl FindMyMessageCommand {
    #[must_use]
    pub fn new(
        device_id: impl Into<String>,
        subject: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            subject: subject.into(),
            text: text.into(),
            play_sound: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindMyLostModeCommand {
    pub device_id: String,
    pub owner_number: String,
    pub text: String,
    pub passcode: String,
}

impl FindMyLostModeCommand {
    #[must_use]
    pub fn new(
        device_id: impl Into<String>,
        owner_number: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            owner_number: owner_number.into(),
            text: text.into(),
            passcode: String::new(),
        }
    }
}
