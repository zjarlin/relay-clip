use crate::clipboard::{self, ClipboardPacket};
use crate::discovery::{self, DiscoveryHandle};
use crate::i18n;
use crate::models::{
    AppLanguage, AppSettings, AppStateSnapshot, ClipboardHistoryEntry, ClipboardHistoryKind,
    ClipboardHistorySource, ClipboardPayload, ClipboardPayloadKind, LocalDevice, PersistentState,
    ReadyActionState, SettingsPatch, SyncState, SyncStatus, TransferDirection, TransferJob,
    TransferKind, TransferStage, TrustedDevice,
};
use crate::store;
use crate::transport::{self, ClipboardEnvelope, IncomingTransferOffer};
use crate::tray;
use crate::transfers::{self, LocalFileClipboard};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Emitter};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

pub const SERVICE_TYPE: &str = "_relayclip._tcp.local.";
const RECENT_HASH_LIMIT: usize = 32;

#[derive(Clone, Debug)]
pub struct DiscoveredPeer {
    pub service_fullname: String,
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub protocol_version: String,
    pub capabilities: Vec<String>,
    pub fingerprint: String,
    pub host_name: Option<String>,
    pub addresses: Vec<IpAddr>,
    pub port: u16,
}

#[derive(Clone)]
pub struct RelayRuntime {
    inner: Arc<RuntimeInner>,
}

pub(crate) struct TransferLease {
    _permit: OwnedSemaphorePermit,
}

pub(crate) struct IncomingTransferPreparation {
    pub staging_root: PathBuf,
    pub lease: TransferLease,
}

struct RuntimeInner {
    app: AppHandle,
    state_path: PathBuf,
    persistent: RwLock<PersistentState>,
    transfer_jobs: RwLock<Vec<TransferJob>>,
    clipboard_history: RwLock<Vec<ClipboardHistoryEntry>>,
    presence: RwLock<HashMap<String, DevicePresence>>,
    active_device_id: RwLock<Option<String>>,
    service_map: Mutex<HashMap<String, String>>,
    recent_hashes: Mutex<VecDeque<String>>,
    sync_status: RwLock<SyncStatus>,
    discovery: Mutex<Option<DiscoveryHandle>>,
    listen_port: AtomicU16,
    sequence: AtomicU64,
    transfer_gate: Arc<Semaphore>,
    transfer_controls: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

#[derive(Clone, Debug)]
struct DevicePresence {
    device_id: String,
    name: String,
    platform: String,
    fingerprint: String,
    capabilities: Vec<String>,
    host_name: Option<String>,
    addresses: Vec<IpAddr>,
    port: u16,
    is_online: bool,
    last_seen: DateTime<Utc>,
    last_sync_at: Option<DateTime<Utc>>,
    last_sync_status: Option<String>,
}

#[derive(Clone)]
struct ActiveTarget {
    device_id: String,
    name: String,
    fingerprint: String,
    socket_addr: SocketAddr,
}

impl RelayRuntime {
    pub fn new(app: AppHandle) -> Result<Self> {
        let (state_path, persistent) = store::load_or_create()?;
        let mut transfer_jobs = store::load_transfer_jobs(&state_path)?;
        let mut clipboard_history = store::load_clipboard_history(&state_path)?;
        store::cleanup_transfer_artifacts(&state_path, &mut transfer_jobs)?;
        store::save_transfer_jobs(&state_path, &transfer_jobs)?;
        store::save_clipboard_history(&state_path, &clipboard_history)?;

        let language = persistent.settings.language;
        let sync_status = if persistent.settings.sync_enabled {
            SyncStatus::new(SyncState::Discovering, Some(i18n::advertising(language)))
        } else {
            SyncStatus::new(SyncState::Paused, Some(i18n::paused(language)))
        };

        Ok(Self {
            inner: Arc::new(RuntimeInner {
                app,
                state_path,
                persistent: RwLock::new(persistent),
                transfer_jobs: RwLock::new(transfer_jobs),
                clipboard_history: RwLock::new(std::mem::take(&mut clipboard_history)),
                presence: RwLock::new(HashMap::new()),
                active_device_id: RwLock::new(None),
                service_map: Mutex::new(HashMap::new()),
                recent_hashes: Mutex::new(VecDeque::new()),
                sync_status: RwLock::new(sync_status),
                discovery: Mutex::new(None),
                listen_port: AtomicU16::new(0),
                sequence: AtomicU64::new(1),
                transfer_gate: Arc::new(Semaphore::new(transfers::MAX_ACTIVE_TRANSFER_JOBS)),
                transfer_controls: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub fn initialize(&self) -> Result<()> {
        let persistent = self.persistent_clone();
        let cert_der = store::decode_material(&persistent.certificate_der_b64)?;
        let key_der = store::decode_material(&persistent.private_key_der_b64)?;
        let port = tauri::async_runtime::block_on(transport::start_server(
            self.clone(),
            cert_der,
            key_der,
        ))?;
        self.inner.listen_port.store(port, Ordering::SeqCst);

        if persistent.settings.discovery_enabled {
            self.start_discovery()?;
        }

        clipboard::start_monitor(self.clone());
        self.emit_devices_updated();
        self.emit_sync_status_changed();
        self.emit_transfer_jobs_updated();
        self.emit_clipboard_history_updated();
        Ok(())
    }

    pub fn app_handle(&self) -> &AppHandle {
        &self.inner.app
    }

    pub fn local_device(&self) -> LocalDevice {
        self.persistent_clone().local_device
    }

    pub fn language(&self) -> AppLanguage {
        self.persistent_clone().settings.language
    }

    pub fn snapshot(&self) -> Result<AppStateSnapshot> {
        let persistent = self.persistent_clone();
        let mut settings = persistent.settings.clone();
        settings.active_device_id = self.active_device_id_clone();
        Ok(AppStateSnapshot {
            local_device: persistent.local_device.clone(),
            settings,
            devices: self.collect_devices(&persistent.local_device.device_id),
            sync_status: self.sync_status_clone(),
        })
    }

    pub fn list_devices(&self) -> Vec<TrustedDevice> {
        self.snapshot()
            .map(|snapshot| snapshot.devices)
            .unwrap_or_default()
    }

    pub fn list_transfer_jobs(&self) -> Vec<TransferJob> {
        let mut jobs = self
            .inner
            .transfer_jobs
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        jobs.sort_by(|left, right| {
            transfer_sort_rank(left)
                .cmp(&transfer_sort_rank(right))
                .then(right.started_at.cmp(&left.started_at))
        });
        jobs
    }

    pub fn list_clipboard_history(&self) -> Vec<ClipboardHistoryEntry> {
        self.inner
            .clipboard_history
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn set_active_device(&self, device_id: String) -> Result<AppStateSnapshot> {
        let local_device_id = self.local_device().device_id;
        if device_id == local_device_id {
            bail!("cannot pair this device with itself");
        }
        let presence = self
            .inner
            .presence
            .read()
            .map_err(|_| anyhow!("presence lock poisoned"))?;
        let Some(device) = presence.get(&device_id) else {
            bail!("unknown device");
        };
        if !device.is_online {
            bail!("device is offline");
        }
        drop(presence);
        self.set_active_device_id(Some(device_id));

        self.refresh_sync_summary();
        self.emit_devices_updated();
        self.emit_sync_status_changed();
        self.snapshot()
    }

    pub fn toggle_sync(&self, enabled: bool) -> Result<AppStateSnapshot> {
        {
            let mut persistent = self
                .inner
                .persistent
                .write()
                .map_err(|_| anyhow!("persistent state lock poisoned"))?;
            persistent.settings.sync_enabled = enabled;
            self.persist_locked(&persistent)?;
        }

        self.refresh_sync_summary();
        self.emit_sync_status_changed();
        self.snapshot()
    }

    pub fn update_settings(&self, patch: SettingsPatch) -> Result<AppStateSnapshot> {
        let mut restart_discovery = false;
        let mut enabled_after_restart = None;

        {
            let mut persistent = self
                .inner
                .persistent
                .write()
                .map_err(|_| anyhow!("persistent state lock poisoned"))?;

            if let Some(device_name) = patch.device_name {
                let device_name = device_name.trim().to_string();
                if !device_name.is_empty() && device_name != persistent.settings.device_name {
                    persistent.settings.device_name = device_name.clone();
                    persistent.local_device.device_name = device_name;
                    restart_discovery = true;
                }
            }

            if let Some(launch_on_login) = patch.launch_on_login {
                persistent.settings.launch_on_login = launch_on_login;
            }

            if let Some(discovery_enabled) = patch.discovery_enabled {
                if discovery_enabled != persistent.settings.discovery_enabled {
                    persistent.settings.discovery_enabled = discovery_enabled;
                    restart_discovery = true;
                    enabled_after_restart = Some(discovery_enabled);
                }
            }

            if let Some(sync_enabled) = patch.sync_enabled {
                persistent.settings.sync_enabled = sync_enabled;
            }

            if let Some(language) = patch.language {
                persistent.settings.language = language;
            }

            self.persist_locked(&persistent)?;
        }

        if restart_discovery {
            let enabled = enabled_after_restart
                .unwrap_or_else(|| self.persistent_clone().settings.discovery_enabled);
            self.restart_discovery(enabled)?;
        }

        if let Some(active_device_id) = patch.active_device_id {
            self.set_active_device_id(active_device_id);
        }

        self.refresh_sync_summary();
        self.emit_devices_updated();
        self.emit_sync_status_changed();
        self.snapshot()
    }

    pub async fn handle_local_clipboard(&self, packet: ClipboardPacket) {
        if !self.is_sync_enabled() || self.is_recent_hash(&packet.meta.hash) {
            return;
        }

        self.remember_hash(packet.meta.hash.clone());
        let _ = self.record_packet_history(&packet, ClipboardHistorySource::Local);

        let target = match self.active_target() {
            Ok(Some(target)) => target,
            Ok(None) => {
                self.set_sync_status(
                    SyncState::Discovering,
                    Some(i18n::no_active_device(self.language())),
                    Some(packet.meta),
                );
                return;
            }
            Err(error) => {
                self.emit_clipboard_error(i18n::route_lookup_failed(
                    self.language(),
                    &error.to_string(),
                ));
                return;
            }
        };

        let language = self.language();
        self.set_sync_status(
            SyncState::Syncing,
            Some(i18n::sending(language, &packet.meta.kind, &target.name)),
            Some(packet.meta.clone()),
        );

        let sequence = self.inner.sequence.fetch_add(1, Ordering::SeqCst);
        let envelope = transport::envelope_from_packet(
            self.local_device().device_id,
            target.device_id.clone(),
            &packet,
            sequence,
        );

        match transport::send_envelope(target.socket_addr, &target.fingerprint, envelope).await {
            Ok(_) => self.record_device_sync(
                &target.device_id,
                i18n::relayed(language, &packet.meta.kind),
                Some(packet.meta),
            ),
            Err(error) => self.record_device_failure(
                &target.device_id,
                i18n::relay_failed(language, &error.to_string()),
                Some(packet.meta),
            ),
        }
    }

    pub async fn handle_local_file_list(&self, file_list: LocalFileClipboard) {
        if !self.is_sync_enabled() || self.is_recent_hash(&file_list.hash) {
            return;
        }

        self.remember_hash(file_list.hash.clone());
        let _ = self.record_local_file_history(&file_list);

        let target = match self.active_target() {
            Ok(Some(target)) => target,
            Ok(None) => return,
            Err(error) => {
                self.emit_clipboard_error(i18n::route_lookup_failed(
                    self.language(),
                    &error.to_string(),
                ));
                return;
            }
        };

        let transfer_id = Uuid::new_v4().to_string();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let job = TransferJob {
            transfer_id: transfer_id.clone(),
            peer_device_id: target.device_id.clone(),
            direction: TransferDirection::Outbound,
            kind: TransferKind::FileRefs,
            display_name: file_list.display_name.clone(),
            total_bytes: 0,
            completed_bytes: 0,
            total_entries: 0,
            completed_entries: 0,
            stage: TransferStage::Preparing,
            started_at: Utc::now(),
            finished_at: None,
            error_message: None,
            warning_message: None,
            ready_to_paste: false,
            ready_action_state: ReadyActionState::Placed,
            staging_path: None,
            entries: Vec::new(),
            top_level_names: Vec::new(),
        };

        let _ = self.upsert_transfer_job(job, true);
        self.register_transfer_control(transfer_id.clone(), cancel_flag.clone());

        let runtime = self.clone();
        tauri::async_runtime::spawn(async move {
            runtime
                .process_outbound_transfer(transfer_id, target, file_list.paths, cancel_flag)
                .await;
        });
    }

    pub async fn handle_incoming_envelope(&self, envelope: ClipboardEnvelope) -> Result<()> {
        if !self.is_sync_enabled() {
            return Ok(());
        }

        if envelope.target_device_id != self.local_device().device_id {
            return Ok(());
        }

        if envelope.origin_device_id == self.local_device().device_id
            || self.is_recent_hash(&envelope.content_hash)
        {
            return Ok(());
        }

        let kind = transport::kind_from_i32(envelope.payload_kind)?;
        let packet = clipboard::packet_from_remote(
            kind,
            envelope.mime,
            envelope.content_hash,
            envelope.payload_bytes,
        )?;

        self.remember_hash(packet.meta.hash.clone());
        tauri::async_runtime::spawn_blocking({
            let packet = packet.clone();
            move || clipboard::write_remote(&packet)
        })
        .await
        .map_err(|error| anyhow!("clipboard write join error: {error}"))??;
        let _ = self.record_packet_history(&packet, ClipboardHistorySource::Remote);

        self.record_device_sync(
            &envelope.origin_device_id,
            i18n::received(self.language(), &packet.meta.kind),
            Some(packet.meta),
        );
        Ok(())
    }

    pub async fn prepare_incoming_transfer(
        &self,
        offer: &IncomingTransferOffer,
    ) -> Result<IncomingTransferPreparation> {
        if !self.is_sync_enabled() {
            bail!("file relay is paused");
        }
        if offer.target_device_id != self.local_device().device_id {
            bail!("incoming transfer is not addressed to this device");
        }
        if self.active_device_id_clone().as_deref() != Some(offer.origin_device_id.as_str()) {
            bail!("incoming file relay is only accepted from the active device");
        }

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.register_transfer_control(offer.transfer_id.clone(), cancel_flag);

        let job = TransferJob {
            transfer_id: offer.transfer_id.clone(),
            peer_device_id: offer.origin_device_id.clone(),
            direction: TransferDirection::Inbound,
            kind: TransferKind::FileRefs,
            display_name: offer.display_name.clone(),
            total_bytes: offer.total_bytes,
            completed_bytes: 0,
            total_entries: offer.total_entries,
            completed_entries: 0,
            stage: TransferStage::Preparing,
            started_at: Utc::now(),
            finished_at: None,
            error_message: None,
            warning_message: offer.warning_message.clone(),
            ready_to_paste: false,
            ready_action_state: ReadyActionState::PendingPrompt,
            staging_path: None,
            entries: offer.entries.clone(),
            top_level_names: offer.top_level_names.clone(),
        };
        self.upsert_transfer_job(job, true)?;

        let staging_root = store::transfers_root_path(&self.inner.state_path).join(&offer.transfer_id);
        if staging_root.exists() {
            let _ = std::fs::remove_dir_all(&staging_root);
        }
        std::fs::create_dir_all(transfers::transfer_payload_dir(&staging_root))
            .with_context(|| format!("failed to create {}", staging_root.display()))?;
        transfers::ensure_space_for(Path::new(&staging_root), offer.total_bytes)?;

        let lease = self.acquire_transfer_lease(&offer.transfer_id).await?;
        self.set_transfer_stage(
            &offer.transfer_id,
            TransferStage::Downloading,
            Some(staging_root.to_string_lossy().into_owned()),
            true,
        )?;

        let payload_root = transfers::transfer_payload_dir(&staging_root);
        for entry in &offer.entries {
            if entry.entry_kind == crate::models::TransferEntryKind::Directory {
                let dir_path = payload_root.join(relative_path_to_native(&entry.relative_path));
                std::fs::create_dir_all(&dir_path)
                    .with_context(|| format!("failed to create {}", dir_path.display()))?;
            }
        }

        Ok(IncomingTransferPreparation { staging_root, lease })
    }

    pub fn complete_incoming_transfer(&self, transfer_id: &str) -> Result<()> {
        if let Some(job) = self.finish_transfer(transfer_id, TransferStage::Ready, None, |job| {
            job.ready_to_paste = true;
            job.ready_action_state = ReadyActionState::PendingPrompt;
        })? {
            let _ = self.record_received_transfer_history(&job);
            self.emit_transfer_ready(job);
        }
        Ok(())
    }

    pub fn complete_outbound_transfer(&self, transfer_id: &str) -> Result<()> {
        let _ = self.finish_transfer(transfer_id, TransferStage::Ready, None, |job| {
            job.ready_to_paste = false;
            job.ready_action_state = ReadyActionState::Placed;
        })?;
        Ok(())
    }

    pub fn fail_transfer(&self, transfer_id: &str, message: impl Into<String>) -> Result<()> {
        if let Some(job) = self.finish_transfer(
            transfer_id,
            TransferStage::Failed,
            Some(message.into()),
            |job| job.ready_to_paste = false,
        )? {
            self.emit_transfer_failed(job);
        }
        Ok(())
    }

    pub fn cancel_transfer(&self, transfer_id: &str) -> Result<()> {
        if let Ok(mut controls) = self.inner.transfer_controls.lock() {
            if let Some(cancel) = controls.remove(transfer_id) {
                cancel.store(true, Ordering::SeqCst);
            }
        }

        let _ = self.finish_transfer(transfer_id, TransferStage::Canceled, None, |job| {
            job.ready_to_paste = false;
        })?;
        Ok(())
    }

    pub fn increment_transfer_progress(
        &self,
        transfer_id: &str,
        bytes_delta: u64,
        entries_delta: u32,
    ) -> Result<()> {
        let snapshot = {
            let mut jobs = self
                .inner
                .transfer_jobs
                .write()
                .map_err(|_| anyhow!("transfer jobs lock poisoned"))?;
            if let Some(job) = jobs.iter_mut().find(|job| job.transfer_id == transfer_id) {
                job.completed_bytes = job.completed_bytes.saturating_add(bytes_delta);
                job.completed_entries = job.completed_entries.saturating_add(entries_delta);
            }
            jobs.clone()
        };
        self.emit_transfer_jobs(snapshot);
        Ok(())
    }

    pub fn place_received_transfer_on_clipboard(&self, transfer_id: String) -> Result<()> {
        let job = self
            .find_transfer_job(&transfer_id)
            .ok_or_else(|| anyhow!("transfer job not found"))?;
        if job.direction != TransferDirection::Inbound || job.stage != TransferStage::Ready {
            bail!("transfer is not ready to place on the clipboard");
        }

        let paths = transfers::payload_paths_from_job(&job);
        if paths.is_empty() {
            bail!("the received files are no longer available");
        }

        self.remember_hash(transfers::hash_paths(&paths)?);
        tauri::async_runtime::block_on(tauri::async_runtime::spawn_blocking(move || {
            clipboard::write_file_list(&paths)
        }))
        .map_err(|error| anyhow!("clipboard file list join error: {error}"))??;

        let _ = self.mutate_transfer_job(&transfer_id, true, |job| {
            job.ready_to_paste = false;
            job.ready_action_state = ReadyActionState::Placed;
        })?;
        Ok(())
    }

    pub fn dismiss_transfer_job(&self, transfer_id: String) -> Result<()> {
        let _ = self.mutate_transfer_job(&transfer_id, true, |job| {
            job.ready_to_paste = false;
            job.ready_action_state = ReadyActionState::Dismissed;
        })?;
        Ok(())
    }

    pub fn cancel_transfer_job(&self, transfer_id: String) -> Result<()> {
        self.cancel_transfer(&transfer_id)
    }

    pub fn restore_clipboard_history_entry(&self, entry_id: String) -> Result<()> {
        let entry = self
            .list_clipboard_history()
            .into_iter()
            .find(|entry| entry.entry_id == entry_id)
            .ok_or_else(|| anyhow!("clipboard history entry not found"))?;

        match entry.kind {
            ClipboardHistoryKind::Text | ClipboardHistoryKind::Image => {
                let payload_path = entry
                    .payload_path
                    .as_deref()
                    .ok_or_else(|| anyhow!("clipboard payload is missing"))?;
                let bytes = std::fs::read(payload_path)
                    .with_context(|| format!("failed to read {}", payload_path))?;
                let kind = match entry.kind {
                    ClipboardHistoryKind::Text => ClipboardPayloadKind::Text,
                    ClipboardHistoryKind::Image => ClipboardPayloadKind::Image,
                    ClipboardHistoryKind::FileRefs => unreachable!(),
                };
                let packet = clipboard::packet_from_remote(
                    kind,
                    entry
                        .mime
                        .clone()
                        .unwrap_or_else(|| "application/octet-stream".to_string()),
                    entry.hash.clone(),
                    bytes,
                )?;
                self.remember_hash(entry.hash.clone());
                clipboard::write_remote(&packet)?;
            }
            ClipboardHistoryKind::FileRefs => {
                let paths = self.restore_file_history_paths(&entry)?;
                if paths.is_empty() {
                    bail!("no files are available to restore");
                }
                self.remember_hash(entry.hash.clone());
                clipboard::write_file_list(&paths)?;
            }
        }

        Ok(())
    }

    pub fn open_cache_directory(&self) -> Result<()> {
        let cache_root = store::cache_root_path(&self.inner.state_path);
        std::fs::create_dir_all(&cache_root)
            .with_context(|| format!("failed to create {}", cache_root.display()))?;

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(&cache_root)
                .spawn()
                .context("failed to open cache directory")?;
        }

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(&cache_root)
                .spawn()
                .context("failed to open cache directory")?;
        }

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            std::process::Command::new("xdg-open")
                .arg(&cache_root)
                .spawn()
                .context("failed to open cache directory")?;
        }

        Ok(())
    }

    pub fn upsert_discovered_device(&self, peer: DiscoveredPeer) -> Result<()> {
        if peer.protocol_version != crate::models::PROTOCOL_VERSION {
            return Ok(());
        }

        {
            let mut presence = self
                .inner
                .presence
                .write()
                .map_err(|_| anyhow!("presence lock poisoned"))?;
            let previous = presence.get(&peer.device_id).cloned();
            presence.insert(
                peer.device_id.clone(),
                DevicePresence {
                    device_id: peer.device_id.clone(),
                    name: peer.name.clone(),
                    platform: peer.platform.clone(),
                    fingerprint: peer.fingerprint.clone(),
                    capabilities: peer.capabilities.clone(),
                    host_name: peer.host_name.clone(),
                    addresses: peer.addresses.clone(),
                    port: peer.port,
                    is_online: true,
                    last_seen: Utc::now(),
                    last_sync_at: previous.as_ref().and_then(|device| device.last_sync_at),
                    last_sync_status: previous.and_then(|device| device.last_sync_status),
                },
            );
        }

        {
            let mut service_map = self
                .inner
                .service_map
                .lock()
                .map_err(|_| anyhow!("service map lock poisoned"))?;
            service_map.insert(peer.service_fullname, peer.device_id);
        }

        self.refresh_sync_summary();
        self.emit_devices_updated();
        Ok(())
    }

    pub fn mark_device_offline(&self, fullname: &str) {
        if let Ok(mut service_map) = self.inner.service_map.lock() {
            if let Some(device_id) = service_map.remove(fullname) {
                if let Ok(mut presence) = self.inner.presence.write() {
                    presence.remove(&device_id);
                }
            }
        }

        self.refresh_sync_summary();
        self.emit_devices_updated();
    }

    pub fn emit_devices_updated(&self) {
        let _ = self.inner.app.emit("devices_updated", self.list_devices());
        let _ = tray::refresh(&self.inner.app, self);
    }

    pub fn emit_sync_status_changed(&self) {
        let _ = self
            .inner
            .app
            .emit("sync_status_changed", self.sync_status_clone());
        let _ = tray::refresh(&self.inner.app, self);
    }

    pub fn emit_transfer_jobs_updated(&self) {
        let _ = self
            .inner
            .app
            .emit("transfer_jobs_updated", self.list_transfer_jobs());
    }

    pub fn emit_clipboard_history_updated(&self) {
        let _ = self
            .inner
            .app
            .emit("clipboard_history_updated", self.list_clipboard_history());
    }

    pub fn emit_clipboard_error(&self, message: impl Into<String>) {
        let message = message.into();
        self.set_sync_status(SyncState::Error, Some(message.clone()), None);
        let _ = self.inner.app.emit("clipboard_error", message);
    }

    fn emit_transfer_ready(&self, job: TransferJob) {
        let _ = self.inner.app.emit("transfer_ready", job);
    }

    fn emit_transfer_failed(&self, job: TransferJob) {
        let _ = self.inner.app.emit("transfer_failed", job);
    }

    fn record_packet_history(
        &self,
        packet: &ClipboardPacket,
        source: ClipboardHistorySource,
    ) -> Result<()> {
        let entry_id = Uuid::new_v4().to_string();
        let entry_dir = store::clipboard_history_entry_dir(&self.inner.state_path, &entry_id);
        std::fs::create_dir_all(&entry_dir)
            .with_context(|| format!("failed to create {}", entry_dir.display()))?;

        let (filename, display_name, preview_text) = match packet.meta.kind {
            ClipboardPayloadKind::Text => (
                "payload.txt",
                "Text clip".to_string(),
                String::from_utf8(packet.bytes.clone())
                    .ok()
                    .map(|text| summarize_text(&text)),
            ),
            ClipboardPayloadKind::Image => ("payload.png", "Image clip".to_string(), None),
        };
        let payload_path = entry_dir.join(filename);
        std::fs::write(&payload_path, &packet.bytes)
            .with_context(|| format!("failed to write {}", payload_path.display()))?;

        self.push_clipboard_history(ClipboardHistoryEntry {
            entry_id,
            kind: match packet.meta.kind {
                ClipboardPayloadKind::Text => ClipboardHistoryKind::Text,
                ClipboardPayloadKind::Image => ClipboardHistoryKind::Image,
            },
            source,
            display_name,
            preview_text,
            mime: Some(packet.meta.mime.clone()),
            hash: packet.meta.hash.clone(),
            size: packet.meta.size as u64,
            file_count: None,
            created_at: Utc::now(),
            payload_path: Some(payload_path.to_string_lossy().into_owned()),
            transfer_id: None,
            top_level_names: Vec::new(),
        })
    }

    fn record_local_file_history(&self, file_list: &LocalFileClipboard) -> Result<()> {
        let entry_id = Uuid::new_v4().to_string();
        let entry_dir = store::clipboard_history_entry_dir(&self.inner.state_path, &entry_id);
        std::fs::create_dir_all(&entry_dir)
            .with_context(|| format!("failed to create {}", entry_dir.display()))?;
        let payload_path = entry_dir.join("paths.json");
        let serialized_paths = file_list
            .paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        std::fs::write(&payload_path, serde_json::to_vec_pretty(&serialized_paths)?)
            .with_context(|| format!("failed to write {}", payload_path.display()))?;

        self.push_clipboard_history(ClipboardHistoryEntry {
            entry_id,
            kind: ClipboardHistoryKind::FileRefs,
            source: ClipboardHistorySource::Local,
            display_name: file_list.display_name.clone(),
            preview_text: None,
            mime: None,
            hash: file_list.hash.clone(),
            size: 0,
            file_count: Some(file_list.paths.len() as u32),
            created_at: Utc::now(),
            payload_path: Some(payload_path.to_string_lossy().into_owned()),
            transfer_id: None,
            top_level_names: file_list
                .paths
                .iter()
                .map(|path| {
                    path.file_name()
                        .map(|value| value.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.to_string_lossy().into_owned())
                })
                .collect(),
        })
    }

    fn record_received_transfer_history(&self, job: &TransferJob) -> Result<()> {
        if job.direction != TransferDirection::Inbound {
            return Ok(());
        }

        self.push_clipboard_history(ClipboardHistoryEntry {
            entry_id: Uuid::new_v4().to_string(),
            kind: ClipboardHistoryKind::FileRefs,
            source: ClipboardHistorySource::Transfer,
            display_name: job.display_name.clone(),
            preview_text: None,
            mime: None,
            hash: format!("transfer:{}", job.transfer_id),
            size: job.total_bytes,
            file_count: Some(job.total_entries),
            created_at: Utc::now(),
            payload_path: job.staging_path.clone(),
            transfer_id: Some(job.transfer_id.clone()),
            top_level_names: job.top_level_names.clone(),
        })
    }

    fn restore_file_history_paths(&self, entry: &ClipboardHistoryEntry) -> Result<Vec<PathBuf>> {
        match entry.source {
            ClipboardHistorySource::Transfer => {
                let Some(staging_path) = entry.payload_path.as_deref() else {
                    return Ok(Vec::new());
                };
                let payload_root = transfers::transfer_payload_dir(Path::new(staging_path));
                Ok(entry
                    .top_level_names
                    .iter()
                    .map(|name| payload_root.join(name))
                    .filter(|path| path.exists())
                    .collect())
            }
            ClipboardHistorySource::Local | ClipboardHistorySource::Remote => {
                let payload_path = entry
                    .payload_path
                    .as_deref()
                    .ok_or_else(|| anyhow!("clipboard file list is missing"))?;
                let content = std::fs::read_to_string(payload_path)
                    .with_context(|| format!("failed to read {}", payload_path))?;
                let stored_paths: Vec<String> = serde_json::from_str(&content)
                    .with_context(|| format!("failed to parse {}", payload_path))?;
                Ok(stored_paths
                    .into_iter()
                    .map(PathBuf::from)
                    .filter(|path| path.exists())
                    .collect())
            }
        }
    }

    fn push_clipboard_history(&self, entry: ClipboardHistoryEntry) -> Result<()> {
        let snapshot = {
            let mut entries = self
                .inner
                .clipboard_history
                .write()
                .map_err(|_| anyhow!("clipboard history lock poisoned"))?;
            entries.retain(|existing| existing.hash != entry.hash || existing.kind != entry.kind);
            entries.insert(0, entry);
            store::save_clipboard_history(&self.inner.state_path, &entries)?;
            entries.clone()
        };
        self.emit_clipboard_history(snapshot);
        Ok(())
    }

    async fn process_outbound_transfer(
        &self,
        transfer_id: String,
        target: ActiveTarget,
        paths: Vec<PathBuf>,
        cancel_flag: Arc<AtomicBool>,
    ) {
        let prepared = match tauri::async_runtime::spawn_blocking(move || transfers::prepare_transfer(&paths))
            .await
        {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(error)) => {
                let _ = self.fail_transfer(&transfer_id, error.to_string());
                return;
            }
            Err(error) => {
                let _ = self.fail_transfer(&transfer_id, format!("prepare transfer join error: {error}"));
                return;
            }
        };

        let _ = self.mutate_transfer_job(&transfer_id, true, |job| {
            job.display_name = prepared.display_name.clone();
            job.total_bytes = prepared.total_bytes;
            job.total_entries = prepared.total_entries();
            job.entries = prepared.entries.iter().map(|entry| entry.entry.clone()).collect();
            job.top_level_names = prepared.top_level_names.clone();
            job.warning_message = prepared.warning_message.clone();
        });

        if cancel_flag.load(Ordering::SeqCst) {
            let _ = self.cancel_transfer(&transfer_id);
            return;
        }

        let lease = match self.acquire_transfer_lease(&transfer_id).await {
            Ok(lease) => lease,
            Err(error) => {
                let _ = self.fail_transfer(&transfer_id, error.to_string());
                return;
            }
        };

        let _ = self.set_transfer_stage(&transfer_id, TransferStage::Downloading, None, true);
        let offer = IncomingTransferOffer {
            transfer_id: transfer_id.clone(),
            origin_device_id: self.local_device().device_id,
            target_device_id: target.device_id.clone(),
            display_name: prepared.display_name.clone(),
            total_bytes: prepared.total_bytes,
            total_entries: prepared.total_entries(),
            entries: prepared.entries.iter().map(|entry| entry.entry.clone()).collect(),
            top_level_names: prepared.top_level_names.clone(),
            warning_message: prepared.warning_message.clone(),
        };

        let result = transport::send_transfer(
            self.clone(),
            target.socket_addr,
            &target.fingerprint,
            offer,
            prepared,
            cancel_flag.clone(),
        )
        .await;

        drop(lease);
        if cancel_flag.load(Ordering::SeqCst) {
            let _ = self.cancel_transfer(&transfer_id);
            return;
        }

        match result {
            Ok(_) => {
                let _ = self.complete_outbound_transfer(&transfer_id);
            }
            Err(error) => {
                let _ = self.fail_transfer(&transfer_id, error.to_string());
            }
        }
    }

    fn restart_discovery(&self, enabled: bool) -> Result<()> {
        if let Some(handle) = self
            .inner
            .discovery
            .lock()
            .map_err(|_| anyhow!("discovery handle lock poisoned"))?
            .take()
        {
            handle.stop();
        }

        if enabled {
            self.start_discovery()?;
        } else {
            self.set_sync_status(
                SyncState::Paused,
                Some(i18n::discovery_disabled(self.language())),
                None,
            );
        }

        Ok(())
    }

    fn start_discovery(&self) -> Result<()> {
        let handle = discovery::start(self.clone(), self.listen_port())?;
        *self
            .inner
            .discovery
            .lock()
            .map_err(|_| anyhow!("discovery handle lock poisoned"))? = Some(handle);
        self.refresh_sync_summary();
        Ok(())
    }

    async fn acquire_transfer_lease(&self, transfer_id: &str) -> Result<TransferLease> {
        if self.inner.transfer_gate.available_permits() == 0 {
            let _ = self.set_transfer_stage(transfer_id, TransferStage::Queued, None, true);
        }

        let permit = self
            .inner
            .transfer_gate
            .clone()
            .acquire_owned()
            .await
            .context("failed to acquire a transfer slot")?;
        Ok(TransferLease { _permit: permit })
    }

    fn collect_devices(&self, local_device_id: &str) -> Vec<TrustedDevice> {
        let presence = self
            .inner
            .presence
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let active_device_id = self.active_device_id_clone();

        let mut devices = presence
            .values()
            .filter(|device| device.device_id != local_device_id)
            .map(|device| {
                TrustedDevice {
                    device_id: device.device_id.clone(),
                    name: device.name.clone(),
                    platform: device.platform.clone(),
                    fingerprint: device.fingerprint.clone(),
                    auto_trusted: false,
                    capabilities: device.capabilities.clone(),
                    last_seen: Some(device.last_seen),
                    last_sync_at: device.last_sync_at,
                    last_sync_status: device.last_sync_status.clone(),
                    addresses: device.addresses.iter().map(|address| address.to_string()).collect(),
                    host_name: device.host_name.clone(),
                    port: Some(device.port),
                    is_online: device.is_online,
                    is_active: active_device_id.as_deref() == Some(device.device_id.as_str()),
                }
            })
            .collect::<Vec<_>>();

        devices.sort_by(|left, right| {
            right
                .is_active
                .cmp(&left.is_active)
                .then(right.is_online.cmp(&left.is_online))
                .then(left.name.cmp(&right.name))
        });
        devices
    }

    fn active_target(&self) -> Result<Option<ActiveTarget>> {
        let Some(active_device_id) = self.active_device_id_clone() else {
            return Ok(None);
        };

        let presence = self
            .inner
            .presence
            .read()
            .map_err(|_| anyhow!("presence lock poisoned"))?;
        let Some(online) = presence.get(&active_device_id) else {
            bail!("the active device is not reachable");
        };
        if !online.is_online {
            bail!("the active device is not reachable");
        }

        let socket_addr = preferred_socket_addr(&online.addresses, online.port)
            .ok_or_else(|| anyhow!("the active device has no routable address"))?;

        Ok(Some(ActiveTarget {
            device_id: active_device_id,
            name: online.name.clone(),
            fingerprint: online.fingerprint.clone(),
            socket_addr,
        }))
    }

    fn persistent_clone(&self) -> PersistentState {
        self.inner
            .persistent
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| PersistentState {
                local_device: LocalDevice {
                    device_id: String::new(),
                    device_name: String::new(),
                    platform: String::new(),
                    protocol_version: String::new(),
                    capabilities: Vec::new(),
                    fingerprint: String::new(),
                },
                trusted_devices: Default::default(),
                settings: AppSettings {
                    device_name: String::new(),
                    launch_on_login: false,
                    discovery_enabled: false,
                    sync_enabled: false,
                    active_device_id: None,
                    language: AppLanguage::detect_system(),
                },
                certificate_der_b64: String::new(),
                private_key_der_b64: String::new(),
            })
    }

    fn active_device_id_clone(&self) -> Option<String> {
        self.inner
            .active_device_id
            .read()
            .map(|guard| guard.clone())
            .unwrap_or(None)
    }

    fn set_active_device_id(&self, device_id: Option<String>) {
        if let Ok(mut active_device_id) = self.inner.active_device_id.write() {
            *active_device_id = device_id;
        }
    }

    fn sync_status_clone(&self) -> SyncStatus {
        self.inner
            .sync_status
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| {
                let language = AppLanguage::detect_system();
                SyncStatus::new(
                    SyncState::Error,
                    Some(i18n::sync_status_unavailable(language)),
                )
            })
    }

    fn is_sync_enabled(&self) -> bool {
        self.persistent_clone().settings.sync_enabled
    }

    fn listen_port(&self) -> u16 {
        self.inner.listen_port.load(Ordering::SeqCst)
    }

    fn persist_locked(&self, persistent: &PersistentState) -> Result<()> {
        store::save(&self.inner.state_path, persistent)
    }

    fn set_sync_status(
        &self,
        state: SyncState,
        message: Option<String>,
        last_payload: Option<ClipboardPayload>,
    ) {
        if let Ok(mut status) = self.inner.sync_status.write() {
            status.state = state;
            status.message = message;
            status.updated_at = Utc::now();
            if last_payload.is_some() {
                status.last_payload = last_payload;
            }
        }
        self.emit_sync_status_changed();
    }

    fn record_device_sync(
        &self,
        device_id: &str,
        status_message: String,
        payload: Option<ClipboardPayload>,
    ) {
        if let Ok(mut presence) = self.inner.presence.write() {
            if let Some(device) = presence.get_mut(device_id) {
                device.last_sync_at = Some(Utc::now());
                device.last_sync_status = Some(status_message.clone());
            }
        }

        self.set_sync_status(SyncState::Connected, Some(status_message), payload);
        self.emit_devices_updated();
    }

    fn record_device_failure(
        &self,
        device_id: &str,
        status_message: String,
        payload: Option<ClipboardPayload>,
    ) {
        if let Ok(mut presence) = self.inner.presence.write() {
            if let Some(device) = presence.get_mut(device_id) {
                device.last_sync_at = Some(Utc::now());
                device.last_sync_status = Some(status_message.clone());
            }
        }

        self.set_sync_status(SyncState::Error, Some(status_message), payload);
        self.emit_devices_updated();
    }

    fn refresh_sync_summary(&self) {
        let payload = self.sync_status_clone().last_payload;
        let persistent = self.persistent_clone();
        if !persistent.settings.sync_enabled {
            self.set_sync_status(
                SyncState::Paused,
                Some(i18n::paused(persistent.settings.language)),
                payload,
            );
            return;
        }

        let presence = self
            .inner
            .presence
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();

        if let Some(active_device_id) = self.active_device_id_clone() {
            if let Some(entry) = presence.get(&active_device_id) {
                if entry.is_online {
                    self.set_sync_status(
                        SyncState::Connected,
                        Some(i18n::active_device_ready(persistent.settings.language)),
                        payload,
                    );
                    return;
                }
            }
        }

        let online_peer_count = presence
            .iter()
            .filter(|(device_id, entry)| {
                entry.is_online && device_id.as_str() != persistent.local_device.device_id
            })
            .count();

        if online_peer_count > 0 {
            self.set_sync_status(
                SyncState::Idle,
                Some(i18n::available_peers(
                    persistent.settings.language,
                    online_peer_count,
                )),
                payload,
            );
            return;
        }

        self.set_sync_status(
            SyncState::Discovering,
            Some(i18n::looking_for_peers(persistent.settings.language)),
            payload,
        );
    }

    fn remember_hash(&self, hash: String) {
        if let Ok(mut recent_hashes) = self.inner.recent_hashes.lock() {
            if recent_hashes.contains(&hash) {
                return;
            }
            recent_hashes.push_front(hash);
            while recent_hashes.len() > RECENT_HASH_LIMIT {
                recent_hashes.pop_back();
            }
        }
    }

    fn is_recent_hash(&self, hash: &str) -> bool {
        self.inner
            .recent_hashes
            .lock()
            .map(|recent_hashes| recent_hashes.iter().any(|entry| entry == hash))
            .unwrap_or(false)
    }

    fn find_transfer_job(&self, transfer_id: &str) -> Option<TransferJob> {
        self.inner
            .transfer_jobs
            .read()
            .ok()
            .and_then(|jobs| jobs.iter().find(|job| job.transfer_id == transfer_id).cloned())
    }

    fn upsert_transfer_job(&self, job: TransferJob, persist: bool) -> Result<()> {
        let snapshot = {
            let mut jobs = self
                .inner
                .transfer_jobs
                .write()
                .map_err(|_| anyhow!("transfer jobs lock poisoned"))?;
            if let Some(existing) = jobs.iter_mut().find(|existing| existing.transfer_id == job.transfer_id)
            {
                *existing = job;
            } else {
                jobs.push(job);
            }
            if persist { Some(jobs.clone()) } else { None }
        };

        if let Some(snapshot) = snapshot {
            store::save_transfer_jobs(&self.inner.state_path, &snapshot)?;
            self.emit_transfer_jobs(snapshot);
        } else {
            self.emit_transfer_jobs_updated();
        }
        Ok(())
    }

    fn mutate_transfer_job<F>(
        &self,
        transfer_id: &str,
        persist: bool,
        apply: F,
    ) -> Result<Option<TransferJob>>
    where
        F: FnOnce(&mut TransferJob),
    {
        let (updated, snapshot) = {
            let mut jobs = self
                .inner
                .transfer_jobs
                .write()
                .map_err(|_| anyhow!("transfer jobs lock poisoned"))?;
            let updated = jobs.iter_mut().find(|job| job.transfer_id == transfer_id).map(|job| {
                apply(job);
                job.clone()
            });
            let snapshot = if persist { Some(jobs.clone()) } else { None };
            (updated, snapshot)
        };

        if let Some(snapshot) = snapshot {
            store::save_transfer_jobs(&self.inner.state_path, &snapshot)?;
            self.emit_transfer_jobs(snapshot);
        } else {
            self.emit_transfer_jobs_updated();
        }
        Ok(updated)
    }

    fn set_transfer_stage(
        &self,
        transfer_id: &str,
        stage: TransferStage,
        staging_path: Option<String>,
        persist: bool,
    ) -> Result<Option<TransferJob>> {
        self.mutate_transfer_job(transfer_id, persist, move |job| {
            job.stage = stage.clone();
            if let Some(staging_path) = staging_path.clone() {
                job.staging_path = Some(staging_path);
            }
            if matches!(stage, TransferStage::Failed | TransferStage::Canceled | TransferStage::Ready) {
                job.finished_at = Some(Utc::now());
            }
        })
    }

    fn finish_transfer<F>(
        &self,
        transfer_id: &str,
        stage: TransferStage,
        error_message: Option<String>,
        apply: F,
    ) -> Result<Option<TransferJob>>
    where
        F: FnOnce(&mut TransferJob),
    {
        let updated = self.mutate_transfer_job(transfer_id, true, move |job| {
            if job.stage == TransferStage::Canceled && stage != TransferStage::Canceled {
                return;
            }
            job.stage = stage.clone();
            job.finished_at = Some(Utc::now());
            job.error_message = error_message.clone();
            apply(job);
        })?;

        if let Ok(mut controls) = self.inner.transfer_controls.lock() {
            controls.remove(transfer_id);
        }

        Ok(updated)
    }

    fn register_transfer_control(&self, transfer_id: String, cancel: Arc<AtomicBool>) {
        if let Ok(mut controls) = self.inner.transfer_controls.lock() {
            controls.insert(transfer_id, cancel);
        }
    }

    fn emit_transfer_jobs(&self, jobs: Vec<TransferJob>) {
        let _ = self.inner.app.emit("transfer_jobs_updated", jobs);
    }

    fn emit_clipboard_history(&self, entries: Vec<ClipboardHistoryEntry>) {
        let _ = self.inner.app.emit("clipboard_history_updated", entries);
    }
}

fn transfer_sort_rank(job: &TransferJob) -> (u8, u8) {
    let primary = match (&job.stage, &job.direction, &job.ready_action_state) {
        (TransferStage::Ready, TransferDirection::Inbound, ReadyActionState::PendingPrompt) => 0,
        (TransferStage::Downloading, _, _) => 1,
        (TransferStage::Verifying, _, _) => 2,
        (TransferStage::Preparing, _, _) => 3,
        (TransferStage::Queued, _, _) => 4,
        (TransferStage::Failed, _, _) => 5,
        (TransferStage::Canceled, _, _) => 6,
        (TransferStage::Ready, _, _) => 7,
    };

    let secondary = match job.direction {
        TransferDirection::Inbound => 0,
        TransferDirection::Outbound => 1,
    };

    (primary, secondary)
}

fn preferred_socket_addr(addresses: &[IpAddr], port: u16) -> Option<SocketAddr> {
    addresses
        .iter()
        .find(|address| matches!(address, IpAddr::V4(ip) if ip.is_private()))
        .or_else(|| {
            addresses
                .iter()
                .find(|address| matches!(address, IpAddr::V4(ip) if ip.is_link_local() || *ip == Ipv4Addr::LOCALHOST))
        })
        .or_else(|| {
            addresses
                .iter()
                .find(|address| matches!(address, IpAddr::V6(ip) if ip.is_unique_local()))
        })
        .or_else(|| addresses.iter().find(|address| matches!(address, IpAddr::V4(_))))
        .or_else(|| {
            addresses.iter().find(|address| {
                matches!(address, IpAddr::V6(ip) if !ip.is_unicast_link_local() || *ip == Ipv6Addr::LOCALHOST)
            })
        })
        .map(|address| SocketAddr::new(*address, port))
}

fn relative_path_to_native(relative_path: &str) -> PathBuf {
    relative_path
        .split('/')
        .fold(PathBuf::new(), |path, segment| path.join(segment))
}

fn summarize_text(text: &str) -> String {
    let condensed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = condensed.trim();
    if trimmed.is_empty() {
        return "Empty text".to_string();
    }

    let mut summary = trimmed.chars().take(80).collect::<String>();
    if trimmed.chars().count() > 80 {
        summary.push_str("...");
    }
    summary
}
