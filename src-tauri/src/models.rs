use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PROTOCOL_VERSION: &str = "relayclip/v1";
pub const CAPABILITIES: [&str; 3] = ["text_utf8", "image_png", "file_refs"];
pub const TRANSFER_RETENTION_HOURS: i64 = 24;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppLanguage {
    #[default]
    #[serde(rename = "en")]
    En,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

impl AppLanguage {
    pub fn detect_system() -> Self {
        sys_locale::get_locale()
            .as_deref()
            .map(Self::from_locale_tag)
            .unwrap_or(Self::En)
    }

    pub fn from_locale_tag(locale: &str) -> Self {
        if locale.to_ascii_lowercase().starts_with("zh") {
            Self::ZhCn
        } else {
            Self::En
        }
    }
}

pub fn default_app_language() -> AppLanguage {
    AppLanguage::detect_system()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub device_name: String,
    pub launch_on_login: bool,
    pub discovery_enabled: bool,
    pub sync_enabled: bool,
    pub active_device_id: Option<String>,
    #[serde(default = "default_app_language")]
    pub language: AppLanguage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDevice {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub protocol_version: String,
    pub capabilities: Vec<String>,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardPayload {
    pub kind: ClipboardPayloadKind,
    pub mime: String,
    pub size: usize,
    pub hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardPayloadKind {
    Text,
    Image,
}

impl ClipboardPayloadKind {
    pub fn as_transport_kind(&self) -> i32 {
        match self {
            Self::Text => 1,
            Self::Image => 2,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedDevice {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub fingerprint: String,
    pub auto_trusted: bool,
    pub capabilities: Vec<String>,
    pub last_seen: Option<DateTime<Utc>>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_sync_status: Option<String>,
    pub addresses: Vec<String>,
    pub host_name: Option<String>,
    pub port: Option<u16>,
    pub is_online: bool,
    pub is_active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub state: SyncState,
    pub message: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub last_payload: Option<ClipboardPayload>,
}

impl SyncStatus {
    pub fn new(state: SyncState, message: impl Into<Option<String>>) -> Self {
        Self {
            state,
            message: message.into(),
            updated_at: Utc::now(),
            last_payload: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncState {
    Idle,
    Discovering,
    Connected,
    Syncing,
    Paused,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateSnapshot {
    pub local_device: LocalDevice,
    pub settings: AppSettings,
    pub devices: Vec<TrustedDevice>,
    pub sync_status: SyncStatus,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub device_name: Option<String>,
    pub launch_on_login: Option<bool>,
    pub discovery_enabled: Option<bool>,
    pub sync_enabled: Option<bool>,
    pub active_device_id: Option<Option<String>>,
    pub language: Option<AppLanguage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredTrustedDevice {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub fingerprint: String,
    pub auto_trusted: bool,
    pub capabilities: Vec<String>,
    pub last_seen: Option<DateTime<Utc>>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_sync_status: Option<String>,
    pub host_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistentState {
    pub local_device: LocalDevice,
    pub trusted_devices: BTreeMap<String, StoredTrustedDevice>,
    pub settings: AppSettings,
    pub certificate_der_b64: String,
    pub private_key_der_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TransferDirection {
    Outbound,
    Inbound,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TransferKind {
    FileRefs,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum TransferStage {
    Preparing,
    Queued,
    Downloading,
    Verifying,
    Ready,
    Failed,
    Canceled,
}

impl TransferStage {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Ready | Self::Failed | Self::Canceled)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReadyActionState {
    PendingPrompt,
    Dismissed,
    Placed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TransferEntryKind {
    File,
    Directory,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferEntry {
    pub relative_path: String,
    pub entry_kind: TransferEntryKind,
    pub size: u64,
    pub modified_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferJob {
    pub transfer_id: String,
    pub peer_device_id: String,
    pub direction: TransferDirection,
    pub kind: TransferKind,
    pub display_name: String,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub total_entries: u32,
    pub completed_entries: u32,
    pub stage: TransferStage,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub warning_message: Option<String>,
    pub ready_to_paste: bool,
    pub ready_action_state: ReadyActionState,
    pub staging_path: Option<String>,
    pub entries: Vec<TransferEntry>,
    pub top_level_names: Vec<String>,
}

pub fn current_platform() -> String {
    if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else {
        std::env::consts::OS.to_string()
    }
}
