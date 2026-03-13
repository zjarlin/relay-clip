use crate::models::{
    current_platform, default_app_language, AppSettings, LocalDevice, PersistentState,
    CAPABILITIES, PROTOCOL_VERSION,
};
use crate::transport;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
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
            launch_on_login: false,
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

fn default_device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "RelayClip Device".to_string())
}
