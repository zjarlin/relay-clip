use crate::models::{
    current_platform, default_app_language, AppSettings, ClipboardHistoryEntry,
    CLIPBOARD_HISTORY_LIMIT, CLIPBOARD_HISTORY_RETENTION_HOURS, LocalDevice, PersistentState,
    TransferJob, TransferStage, TRANSFER_RETENTION_HOURS, CAPABILITIES, PROTOCOL_VERSION,
};
use crate::transport;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{Duration, Utc};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub fn load_or_create() -> Result<(PathBuf, PersistentState)> {
    let path = state_file_path()?;

    if path.exists() {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read state file {}", path.display()))?;
        let mut state: PersistentState = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse state file {}", path.display()))?;
        normalize_state(&mut state)?;
        save(&path, &state)?;
        return Ok((path, state));
    }

    let state = default_state()?;
    save(&path, &state)?;
    Ok((path, state))
}

pub fn save(path: &PathBuf, state: &PersistentState) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create state directory {}", parent.display()))?;
    }

    let body = serde_json::to_string_pretty(state)?;
    fs::write(path, body).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn decode_material(data: &str) -> Result<Vec<u8>> {
    STANDARD
        .decode(data)
        .context("failed to decode persisted identity material")
}

pub fn transfer_state_path(state_path: &Path) -> PathBuf {
    state_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("transfers.json")
}

pub fn transfers_root_path(state_path: &Path) -> PathBuf {
    state_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("transfers")
        .join("inbox")
}

pub fn cache_root_path(state_path: &Path) -> PathBuf {
    state_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("cache")
}

pub fn clipboard_history_path(state_path: &Path) -> PathBuf {
    state_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("clipboard-history.json")
}

pub fn clipboard_history_root_path(state_path: &Path) -> PathBuf {
    cache_root_path(state_path).join("clipboard-history")
}

pub fn clipboard_history_entry_dir(state_path: &Path, entry_id: &str) -> PathBuf {
    clipboard_history_root_path(state_path).join(entry_id)
}

pub fn load_transfer_jobs(state_path: &Path) -> Result<Vec<TransferJob>> {
    let transfer_path = transfer_state_path(state_path);
    if !transfer_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&transfer_path)
        .with_context(|| format!("failed to read {}", transfer_path.display()))?;
    let mut jobs: Vec<TransferJob> = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", transfer_path.display()))?;
    normalize_transfer_jobs(&mut jobs, state_path)?;
    save_transfer_jobs(state_path, &jobs)?;
    Ok(jobs)
}

pub fn save_transfer_jobs(state_path: &Path, jobs: &[TransferJob]) -> Result<()> {
    let transfer_path = transfer_state_path(state_path);
    if let Some(parent) = transfer_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let body = serde_json::to_string_pretty(jobs)?;
    fs::write(&transfer_path, body)
        .with_context(|| format!("failed to write {}", transfer_path.display()))?;
    Ok(())
}

pub fn load_clipboard_history(state_path: &Path) -> Result<Vec<ClipboardHistoryEntry>> {
    let history_path = clipboard_history_path(state_path);
    if !history_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&history_path)
        .with_context(|| format!("failed to read {}", history_path.display()))?;
    let mut entries: Vec<ClipboardHistoryEntry> = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", history_path.display()))?;
    normalize_clipboard_history(&mut entries, state_path)?;
    save_clipboard_history(state_path, &entries)?;
    Ok(entries)
}

pub fn save_clipboard_history(state_path: &Path, entries: &[ClipboardHistoryEntry]) -> Result<()> {
    let history_path = clipboard_history_path(state_path);
    if let Some(parent) = history_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let body = serde_json::to_string_pretty(entries)?;
    fs::write(&history_path, body)
        .with_context(|| format!("failed to write {}", history_path.display()))?;
    Ok(())
}

pub fn cleanup_transfer_artifacts(state_path: &Path, jobs: &mut Vec<TransferJob>) -> Result<()> {
    normalize_transfer_jobs(jobs, state_path)?;
    prune_transfer_directories(state_path, jobs)?;
    Ok(())
}

fn state_file_path() -> Result<PathBuf> {
    if let Ok(override_dir) = std::env::var("RELAYCLIP_DATA_DIR") {
        let override_dir = PathBuf::from(override_dir);
        if !override_dir.as_os_str().is_empty() {
            return Ok(override_dir.join("state.json"));
        }
    }

    let base = dirs::data_local_dir().context("local data directory is unavailable")?;
    Ok(base.join("RelayClip").join("state.json"))
}

fn default_state() -> Result<PersistentState> {
    let device_name = default_device_name();
    let device_id = Uuid::new_v4().to_string();
    let (cert_der, key_der, fingerprint) =
        transport::generate_self_signed_identity(&device_name, &device_id)?;

    Ok(PersistentState {
        local_device: LocalDevice {
            device_id: device_id.clone(),
            device_name: device_name.clone(),
            platform: current_platform(),
            protocol_version: PROTOCOL_VERSION.to_string(),
            capabilities: CAPABILITIES.iter().map(|entry| entry.to_string()).collect(),
            fingerprint,
        },
        trusted_devices: BTreeMap::new(),
        settings: AppSettings {
            device_name,
            launch_on_login: true,
            background_sync_enabled: true,
            discovery_enabled: true,
            sync_enabled: true,
            active_device_id: None,
            language: default_app_language(),
        },
        certificate_der_b64: STANDARD.encode(cert_der),
        private_key_der_b64: STANDARD.encode(key_der),
    })
}

fn normalize_state(state: &mut PersistentState) -> Result<()> {
    if state.local_device.device_id.trim().is_empty() {
        state.local_device.device_id = Uuid::new_v4().to_string();
    }

    if state.local_device.device_name.trim().is_empty() {
        state.local_device.device_name = default_device_name();
    }

    if state.settings.device_name.trim().is_empty() {
        state.settings.device_name = state.local_device.device_name.clone();
    }

    state.settings.active_device_id = None;
    state.trusted_devices.clear();

    state.local_device.device_name = state.settings.device_name.clone();
    state.local_device.platform = current_platform();
    state.local_device.protocol_version = PROTOCOL_VERSION.to_string();
    state.local_device.capabilities = CAPABILITIES.iter().map(|entry| entry.to_string()).collect();

    if state.certificate_der_b64.trim().is_empty() || state.private_key_der_b64.trim().is_empty() {
        let (cert_der, key_der, fingerprint) = transport::generate_self_signed_identity(
            &state.local_device.device_name,
            &state.local_device.device_id,
        )?;
        state.certificate_der_b64 = STANDARD.encode(cert_der);
        state.private_key_der_b64 = STANDARD.encode(key_der);
        state.local_device.fingerprint = fingerprint;
    }

    if state.local_device.fingerprint.trim().is_empty() {
        let cert_der = decode_material(&state.certificate_der_b64)?;
        state.local_device.fingerprint = transport::fingerprint_from_bytes(&cert_der);
    }

    Ok(())
}

fn normalize_transfer_jobs(jobs: &mut Vec<TransferJob>, state_path: &Path) -> Result<()> {
    let now = Utc::now();
    let cutoff = now - Duration::hours(TRANSFER_RETENTION_HOURS);
    let transfers_root = transfers_root_path(state_path);

    for job in jobs.iter_mut() {
        if !job.stage.is_terminal() {
            job.stage = TransferStage::Failed;
            job.error_message = Some("Transfer interrupted before completion".to_string());
            job.finished_at = Some(now);
            job.ready_to_paste = false;
        }
    }

    jobs.retain(|job| {
        let keep = job
            .finished_at
            .or(Some(job.started_at))
            .is_some_and(|stamp| stamp >= cutoff);

        if !keep {
            if let Some(staging_path) = job.staging_path.as_deref() {
                let _ = fs::remove_dir_all(staging_path);
            }
        }

        keep
    });

    fs::create_dir_all(&transfers_root)
        .with_context(|| format!("failed to create {}", transfers_root.display()))?;
    Ok(())
}

fn normalize_clipboard_history(
    entries: &mut Vec<ClipboardHistoryEntry>,
    state_path: &Path,
) -> Result<()> {
    let now = Utc::now();
    let cutoff = now - Duration::hours(CLIPBOARD_HISTORY_RETENTION_HOURS);
    let history_root = clipboard_history_root_path(state_path);

    entries.retain(|entry| {
        if entry.created_at < cutoff {
            if let Some(payload_path) = entry.payload_path.as_deref() {
                let payload_path = PathBuf::from(payload_path);
                if payload_path.starts_with(&history_root) {
                    let _ = fs::remove_dir_all(
                        payload_path
                            .parent()
                            .unwrap_or_else(|| Path::new(&history_root)),
                    );
                }
            }
            return false;
        }

        entry
            .payload_path
            .as_deref()
            .is_none_or(|payload_path| Path::new(payload_path).exists())
    });

    if entries.len() > CLIPBOARD_HISTORY_LIMIT {
        for entry in entries.iter().skip(CLIPBOARD_HISTORY_LIMIT) {
            if let Some(payload_path) = entry.payload_path.as_deref() {
                let payload_path = PathBuf::from(payload_path);
                if payload_path.starts_with(&history_root) {
                    let _ = fs::remove_dir_all(
                        payload_path
                            .parent()
                            .unwrap_or_else(|| Path::new(&history_root)),
                    );
                }
            }
        }
        entries.truncate(CLIPBOARD_HISTORY_LIMIT);
    }

    fs::create_dir_all(&history_root)
        .with_context(|| format!("failed to create {}", history_root.display()))?;
    prune_clipboard_history_directories(state_path, entries)?;
    Ok(())
}

fn prune_transfer_directories(state_path: &Path, jobs: &[TransferJob]) -> Result<()> {
    let root = transfers_root_path(state_path);
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;

    let retained = jobs
        .iter()
        .filter_map(|job| job.staging_path.as_deref())
        .map(PathBuf::from)
        .collect::<Vec<_>>();

    for entry in fs::read_dir(&root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !retained.iter().any(|retained_path| retained_path == &path) {
            let _ = fs::remove_dir_all(path);
        }
    }

    Ok(())
}

fn prune_clipboard_history_directories(
    state_path: &Path,
    entries: &[ClipboardHistoryEntry],
) -> Result<()> {
    let root = clipboard_history_root_path(state_path);
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;

    let retained = entries
        .iter()
        .filter_map(|entry| entry.payload_path.as_deref())
        .map(PathBuf::from)
        .filter(|path| path.starts_with(&root))
        .filter_map(|path| path.parent().map(PathBuf::from))
        .collect::<Vec<_>>();

    for entry in fs::read_dir(&root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !retained.iter().any(|retained_path| retained_path == &path) {
            let _ = fs::remove_dir_all(path);
        }
    }

    Ok(())
}

fn default_device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "RelayClip Device".to_string())
}
