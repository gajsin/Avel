use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrightnessBackend {
    Wmi,
    DdcHigh,
    DdcVcp,
}

impl BrightnessBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wmi => "wmi",
            Self::DdcHigh => "ddc_high",
            Self::DdcVcp => "ddc_vcp",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "wmi" => Some(Self::Wmi),
            "ddc_high" => Some(Self::DdcHigh),
            "ddc_vcp" => Some(Self::DdcVcp),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeState {
    NotChecked,
    Checking,
    ReadOk,
    NoResponse,
    Unavailable,
    Ambiguous,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationState {
    NotTested,
    Testing,
    DeviceConfirmed,
    UserConfirmed,
    Failed,
    RecoveryPending,
    Stale,
}

impl VerificationState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotTested => "not_tested",
            Self::Testing => "testing",
            Self::DeviceConfirmed => "device_confirmed",
            Self::UserConfirmed => "user_confirmed",
            Self::Failed => "failed",
            Self::RecoveryPending => "recovery_pending",
            Self::Stale => "stale",
        }
    }

    pub fn from_str_safe(s: &str) -> Self {
        match s {
            "not_tested" => Self::NotTested,
            "testing" => Self::Testing,
            "device_confirmed" => Self::DeviceConfirmed,
            "user_confirmed" => Self::UserConfirmed,
            "failed" => Self::Failed,
            "recovery_pending" => Self::RecoveryPending,
            "stale" => Self::Stale,
            _ => Self::NotTested,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrightnessError {
    pub code: String,
    pub native_code: Option<u32>,
    pub operation: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrightnessDisplay {
    pub id: String,
    pub generation: u64,
    pub name: String,
    pub connection: Option<String>,
    pub is_primary: bool,
    pub backend: Option<BrightnessBackend>,
    pub probe_state: ProbeState,
    pub verification: VerificationState,
    pub current_percent: Option<u32>,
    pub target_percent: Option<u32>,
    pub raw_min: Option<u32>,
    pub raw_max: Option<u32>,
    pub available_levels: Option<Vec<u32>>,
    pub busy: bool,
    pub error: Option<BrightnessError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrightnessPreferences {
    pub sync_group: bool,
    pub step_percent: u32,
    pub group_display_ids: Vec<String>,
    pub hotkey_brightness_target: String, // "cursor" | "group"
    pub hotkey_brightness_up: Option<String>,
    pub hotkey_brightness_down: Option<String>,
    pub hotkey_brightness_screen: Option<String>,
}

impl Default for BrightnessPreferences {
    fn default() -> Self {
        Self {
            sync_group: false,
            step_percent: 5,
            group_display_ids: Vec::new(),
            hotkey_brightness_target: "cursor".to_string(),
            hotkey_brightness_up: None,
            hotkey_brightness_down: None,
            hotkey_brightness_screen: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrightnessMonitorPreference {
    pub display_key: String,
    pub preferred_backend: Option<BrightnessBackend>,
    pub last_verification_state: VerificationState,
    pub last_verification_connection: Option<String>,
    pub custom_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrightnessTestRecoveryRecord {
    #[serde(default)]
    pub test_id: String,
    pub display_key: String,
    pub connection_key: String,
    pub backend: String,
    pub original_raw: u32,
    pub test_raw: u32,
    pub stage: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrightnessStateSnapshot {
    pub revision: u64,
    pub displays: Vec<BrightnessDisplay>,
    pub preferences: BrightnessPreferences,
    pub active_test_display_id: Option<String>,
}
