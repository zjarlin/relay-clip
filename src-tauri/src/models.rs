use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PROTOCOL_VERSION: &str = "relayclip/v1";
pub const CAPABILITIES: [&str; 3] = ["text_utf8", "image_png", "file_refs"];
pub const TRANSFER_RETENTION_HOURS: i64 = 24;
pub const CLIPBOARD_HISTORY_RETENTION_HOURS: i64 = 72;
pub const CLIPBOARD_HISTORY_LIMIT: usize = 60;

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

fn default_background_sync_enabled() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimePlatform {
    Windows,
    Macos,
    Linux,
    Android,
    Ios,
    #[default]
    Unknown,
}

impl RuntimePlatform {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "android") {
            Self::Android
        } else if cfg!(target_os = "ios") {
            Self::Ios
        } else {
            Self::Unknown
        }
    }

    pub fn is_mobile(self) -> bool {
        matches!(self, Self::Android | Self::Ios)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCapabilities {
    pub tray: bool,
    pub autostart: bool,
    pub clipboard_monitor: bool,
    pub clipboard_files: bool,
    pub open_cache_directory: bool,
    pub share_externally: bool,
    pub export_to_files: bool,
    pub background_sync: bool,
    pub native_discovery: bool,
}

impl RuntimeCapabilities {
    pub fn for_platform(platform: RuntimePlatform) -> Self {
        match platform {
            RuntimePlatform::Windows | RuntimePlatform::Macos | RuntimePlatform::Linux => Self {
                tray: true,
                autostart: true,
                clipboard_monitor: true,
                clipboard_files: true,
                open_cache_directory: true,
                share_externally: false,
                export_to_files: false,
                background_sync: false,
                native_discovery: true,
            },
            RuntimePlatform::Android | RuntimePlatform::Ios => Self {
                tray: false,
                autostart: false,
                clipboard_monitor: false,
                clipboard_files: false,
                open_cache_directory: false,
                share_externally: false,
                export_to_files: false,
                background_sync: true,
                native_discovery: false,
            },
            RuntimePlatform::Unknown => Self::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimePermissionState {
    Granted,
    Denied,
    Prompt,
    #[default]
    Unsupported,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePermissions {
    pub notifications: RuntimePermissionState,
    pub local_network: RuntimePermissionState,
    pub clipboard: RuntimePermissionState,
    pub background_sync: RuntimePermissionState,
    pub file_access: RuntimePermissionState,
}

impl RuntimePermissions {
    pub fn for_platform(platform: RuntimePlatform, capabilities: &RuntimeCapabilities) -> Self {
        let mobile_prompt = if platform.is_mobile() {
            RuntimePermissionState::Prompt
        } else {
            RuntimePermissionState::Granted
        };

        Self {
            notifications: if capabilities.share_externally || capabilities.background_sync {
                mobile_prompt
            } else {
                RuntimePermissionState::Unsupported
            },
            local_network: if capabilities.native_discovery || platform.is_mobile() {
                mobile_prompt
            } else {
                RuntimePermissionState::Unsupported
            },
            clipboard: if capabilities.clipboard_monitor || capabilities.clipboard_files {
                mobile_prompt
            } else {
                RuntimePermissionState::Unsupported
            },
            background_sync: if capabilities.background_sync {
                mobile_prompt
            } else {
                RuntimePermissionState::Unsupported
            },
            file_access: if capabilities.export_to_files || capabilities.open_cache_directory {
                mobile_prompt
            } else {
                RuntimePermissionState::Unsupported
            },
        }
    }

    pub fn granted_for(capabilities: &RuntimeCapabilities) -> Self {
        Self {
            notifications: if capabilities.share_externally || capabilities.background_sync {
                RuntimePermissionState::Granted
            } else {
                RuntimePermissionState::Unsupported
            },
            local_network: if capabilities.native_discovery || capabilities.background_sync {
                RuntimePermissionState::Granted
            } else {
                RuntimePermissionState::Unsupported
            },
            clipboard: if capabilities.clipboard_monitor || capabilities.clipboard_files {
                RuntimePermissionState::Granted
            } else {
                RuntimePermissionState::Unsupported
            },
            background_sync: if capabilities.background_sync {
                RuntimePermissionState::Granted
            } else {
                RuntimePermissionState::Unsupported
            },
            file_access: if capabilities.export_to_files || capabilities.open_cache_directory {
                RuntimePermissionState::Granted
            } else {
                RuntimePermissionState::Unsupported
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackgroundSyncMode {
    Desktop,
    ForegroundOnly,
    ForegroundService,
    AppRefresh,
    #[default]
    Unsupported,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundSyncState {
    pub supported: bool,
    pub enabled: bool,
    pub active: bool,
    pub mode: BackgroundSyncMode,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub device_name: String,
    pub launch_on_login: bool,
    #[serde(default = "default_background_sync_enabled")]
    pub background_sync_enabled: bool,
    pub discovery_enabled: bool,
    pub sync_enabled: bool,
    #[serde(default)]
    pub active_device_ids: Vec<String>,
    #[serde(default)]
    pub device_remarks: BTreeMap<String, String>,
    #[serde(default)]
    pub blocked_auto_pair_device_ids: Vec<String>,
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
    #[serde(default)]
    pub remark: Option<String>,
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
    pub runtime_platform: RuntimePlatform,
    pub capabilities: RuntimeCapabilities,
    pub permissions: RuntimePermissions,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub device_name: Option<String>,
    pub launch_on_login: Option<bool>,
    pub background_sync_enabled: Option<bool>,
    pub discovery_enabled: Option<bool>,
    pub sync_enabled: Option<bool>,
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
pub enum ClipboardHistoryKind {
    Text,
    Image,
    FileRefs,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardHistorySource {
    Local,
    Remote,
    Transfer,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardHistoryEntry {
    pub entry_id: String,
    pub kind: ClipboardHistoryKind,
    pub source: ClipboardHistorySource,
    #[serde(default)]
    pub origin_device_id: String,
    #[serde(default)]
    pub origin_device_name: String,
    pub display_name: String,
    pub preview_text: Option<String>,
    pub mime: Option<String>,
    pub hash: String,
    pub size: u64,
    pub file_count: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub payload_path: Option<String>,
    pub transfer_id: Option<String>,
    pub top_level_names: Vec<String>,
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
pub enum TransferAction {
    PlaceOnClipboard,
    ShareExternally,
    ExportToFiles,
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
    #[serde(default)]
    pub available_actions: Vec<TransferAction>,
}

pub fn current_platform() -> String {
    if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "ios") {
        "iOS".to_string()
    } else if cfg!(target_os = "android") {
        "Android".to_string()
    } else {
        std::env::consts::OS.to_string()
    }
}
