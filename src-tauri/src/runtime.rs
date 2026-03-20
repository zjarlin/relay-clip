use crate::clipboard::{self, ClipboardPacket};
use crate::discovery::{self, DiscoveryHandle};
use crate::i18n;
use crate::mobile_bridge::RuntimeBridge;
use crate::models::{
    AppLanguage, AppSettings, AppStateSnapshot, BackgroundSyncState, ClipboardHistoryEntry,
    ClipboardHistoryKind, ClipboardHistorySource, ClipboardPayload, ClipboardPayloadKind,
    LocalDevice, PersistentState, ReadyActionState, RuntimeCapabilities, RuntimePermissions,
    RuntimePlatform, SettingsPatch, SyncState, SyncStatus, TransferAction, TransferDirection,
    TransferJob, TransferKind, TransferStage, TrustedDevice,
};
use crate::store;
use crate::transfers::{self, LocalFileClipboard};
use crate::transport::{self, ClipboardEnvelope, IncomingTransferOffer};
use crate::tray;
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
    bridge: RuntimeBridge,
    state_path: PathBuf,
    persistent: RwLock<PersistentState>,
    transfer_jobs: RwLock<Vec<TransferJob>>,
    clipboard_history: RwLock<Vec<ClipboardHistoryEntry>>,
    presence: RwLock<HashMap<String, DevicePresence>>,
    active_device_ids: RwLock<Vec<String>>,
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
    fingerprint: String,
    socket_addr: SocketAddr,
}

#[derive(Clone, Copy)]
enum PairingChangeSource {
    Manual,
    Remote,
    Auto,
}

impl PairingChangeSource {
    fn blocked_auto_pair_state(self, paired: bool) -> Option<bool> {
        match (self, paired) {
            (Self::Manual, false) | (Self::Remote, false) => Some(true),
            (Self::Manual, true) | (Self::Remote, true) => Some(false),
            (Self::Auto, _) => None,
        }
    }
}

impl RelayRuntime {
    pub fn new(app: AppHandle, bridge: RuntimeBridge) -> Result<Self> {
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
                bridge,
                state_path,
                persistent: RwLock::new(persistent),
                transfer_jobs: RwLock::new(transfer_jobs),
                clipboard_history: RwLock::new(std::mem::take(&mut clipboard_history)),
                presence: RwLock::new(HashMap::new()),
                active_device_ids: RwLock::new(Vec::new()),
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

    pub fn runtime_platform(&self) -> RuntimePlatform {
        self.inner.bridge.platform()
    }

    pub fn runtime_capabilities(&self) -> RuntimeCapabilities {
        self.inner.bridge.capabilities()
    }

    pub fn runtime_permissions(&self) -> RuntimePermissions {
        self.inner.bridge.permissions()
    }

    pub fn request_runtime_permissions(&self) -> RuntimePermissions {
        self.inner.bridge.request_permissions()
    }

    pub fn background_sync_state(&self) -> BackgroundSyncState {
        self.inner
            .bridge
            .background_sync_state(self.persistent_clone().settings.background_sync_enabled)
    }

    pub fn snapshot(&self) -> Result<AppStateSnapshot> {
        let persistent = self.persistent_clone();
        let mut settings = persistent.settings.clone();
        settings.active_device_ids = self.active_device_ids_clone();
        Ok(AppStateSnapshot {
            local_device: persistent.local_device.clone(),
            settings,
            devices: self.collect_devices(&persistent.local_device.device_id),
            sync_status: self.sync_status_clone(),
            runtime_platform: self.runtime_platform(),
            capabilities: self.runtime_capabilities(),
            permissions: self.runtime_permissions(),
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
        self.decorate_transfer_jobs(&mut jobs);
        jobs
    }

    pub fn list_clipboard_history(&self) -> Vec<ClipboardHistoryEntry> {
        self.inner
            .clipboard_history
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn set_device_remark(
        &self,
        device_id: String,
        remark: Option<String>,
    ) -> Result<AppStateSnapshot> {
        if device_id == self.local_device().device_id {
            bail!("cannot set a remark for this device");
        }

        let normalized = remark.and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        });

        {
            let mut persistent = self
                .inner
                .persistent
                .write()
                .map_err(|_| anyhow!("persistent state lock poisoned"))?;
            if let Some(remark) = normalized {
                persistent.settings.device_remarks.insert(device_id, remark);
            } else {
                persistent.settings.device_remarks.remove(&device_id);
            }
            self.persist_locked(&persistent)?;
        }

        self.emit_devices_updated();
        self.snapshot()
    }

    pub fn set_device_pairing(&self, device_id: String, paired: bool) -> Result<AppStateSnapshot> {
        self.set_device_pairing_internal(
            device_id,
            paired,
            PairingChangeSource::Manual,
            true,
        )?;
        self.snapshot()
    }

    fn set_device_pairing_internal(
        &self,
        device_id: String,
        paired: bool,
        source: PairingChangeSource,
        sync_peer: bool,
    ) -> Result<()> {
        let local_device_id = self.local_device().device_id;
        if device_id == local_device_id {
            bail!("cannot pair this device with itself");
        }

        let target = if sync_peer {
            match self.resolve_active_target(&device_id) {
                Ok(target) => Some(target),
                Err(error) if !paired => {
                    log::debug!("skipping pair sync because the peer is offline: {error}");
                    None
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };

        self.apply_pairing_state(&device_id, paired, source)?;

        self.refresh_sync_summary();
        self.emit_devices_updated();
        self.emit_sync_status_changed();

        if let Some(target) = target {
            self.sync_device_pairing_to_peer(target, device_id, paired);
        }

        Ok(())
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

            if let Some(background_sync_enabled) = patch.background_sync_enabled {
                persistent.settings.background_sync_enabled = background_sync_enabled;
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
        let local_device = self.local_device();
        let _ = self.record_packet_history(
            &packet,
            ClipboardHistorySource::Local,
            &local_device.device_id,
            &local_device.device_name,
        );

        let paired_device_ids = self.active_device_ids_clone();
        let targets = match self.active_targets() {
            Ok(targets) if targets.is_empty() => {
                self.set_sync_status(
                    SyncState::Discovering,
                    Some(if paired_device_ids.is_empty() {
                        i18n::no_paired_devices(self.language())
                    } else {
                        i18n::paired_devices_offline(self.language())
                    }),
                    Some(packet.meta),
                );
                return;
            }
            Ok(targets) => targets,
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
            Some(i18n::sending(language, &packet.meta.kind, targets.len())),
            Some(packet.meta.clone()),
        );

        for target in targets {
            match self
                .send_clipboard_packet_to_target(&local_device, &target, &packet)
                .await
            {
                Ok(_) => self.record_device_sync(
                    &target.device_id,
                    i18n::relayed(language, &packet.meta.kind),
                    Some(packet.meta.clone()),
                ),
                Err(error) => self.record_device_failure(
                    &target.device_id,
                    i18n::relay_failed(language, &error.to_string()),
                    Some(packet.meta.clone()),
                ),
            }
        }
    }

    pub async fn handle_local_file_list(&self, file_list: LocalFileClipboard) {
        if !self.is_sync_enabled() || self.is_recent_hash(&file_list.hash) {
            return;
        }

        self.remember_hash(file_list.hash.clone());
        let local_device = self.local_device();
        let _ = self.record_local_file_history(
            &file_list,
            &local_device.device_id,
            &local_device.device_name,
        );

        let targets = match self.active_targets() {
            Ok(targets) if targets.is_empty() => return,
            Ok(targets) => targets,
            Err(error) => {
                self.emit_clipboard_error(i18n::route_lookup_failed(
                    self.language(),
                    &error.to_string(),
                ));
                return;
            }
        };

        for target in targets {
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
                available_actions: Vec::new(),
            };

            let _ = self.upsert_transfer_job(job, true);
            self.register_transfer_control(transfer_id.clone(), cancel_flag.clone());

            let runtime = self.clone();
            let paths = file_list.paths.clone();
            tauri::async_runtime::spawn(async move {
                runtime
                    .process_outbound_transfer(transfer_id, target, paths, cancel_flag)
                    .await;
            });
        }
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
        if let Err(error) = self.auto_pair_device_if_only_online_peer(&envelope.origin_device_id) {
            log::warn!("failed to auto-pair the incoming clipboard source: {error}");
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
        let origin_device_name = self.device_name_for(&envelope.origin_device_id);
        let _ = self.record_packet_history(
            &packet,
            ClipboardHistorySource::Remote,
            &envelope.origin_device_id,
            &origin_device_name,
        );

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
        if !self.is_paired_device(&offer.origin_device_id) {
            if let Err(error) = self.auto_pair_device_if_only_online_peer(&offer.origin_device_id) {
                log::warn!("failed to auto-pair the incoming transfer source: {error}");
            }
        }
        if !self.is_paired_device(&offer.origin_device_id) {
            bail!("incoming file relay is only accepted from paired devices");
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
            available_actions: Vec::new(),
        };
        self.upsert_transfer_job(job, true)?;

        let staging_root =
            store::transfers_root_path(&self.inner.state_path).join(&offer.transfer_id);
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

        Ok(IncomingTransferPreparation {
            staging_root,
            lease,
        })
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

    pub fn share_received_transfer(&self, transfer_id: String) -> Result<()> {
        let job = self
            .find_transfer_job(&transfer_id)
            .ok_or_else(|| anyhow!("transfer job not found"))?;
        if job.direction != TransferDirection::Inbound || job.stage != TransferStage::Ready {
            bail!("transfer is not ready to share");
        }

        let paths = transfers::payload_paths_from_job(&job);
        self.inner.bridge.share_paths(&paths)?;
        let _ = self.mutate_transfer_job(&transfer_id, true, |job| {
            job.ready_to_paste = false;
            job.ready_action_state = ReadyActionState::Placed;
        })?;
        Ok(())
    }

    pub fn export_received_transfer(&self, transfer_id: String) -> Result<()> {
        let job = self
            .find_transfer_job(&transfer_id)
            .ok_or_else(|| anyhow!("transfer job not found"))?;
        if job.direction != TransferDirection::Inbound || job.stage != TransferStage::Ready {
            bail!("transfer is not ready to export");
        }

        let paths = transfers::payload_paths_from_job(&job);
        self.inner.bridge.export_paths(&paths)?;
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

        #[cfg(all(
            not(target_os = "windows"),
            not(target_os = "macos"),
            not(target_os = "android"),
            not(target_os = "ios")
        ))]
        {
            std::process::Command::new("xdg-open")
                .arg(&cache_root)
                .spawn()
                .context("failed to open cache directory")?;
        }

        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            bail!("opening the cache directory is not supported on mobile builds");
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
            service_map.insert(peer.service_fullname, peer.device_id.clone());
        }

        if self.auto_pair_device_if_only_online_peer(&peer.device_id)? {
            return Ok(());
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

        match self.auto_pair_single_online_peer() {
            Ok(true) => return,
            Ok(false) => {}
            Err(error) => {
                log::warn!("failed to auto-pair the remaining online peer: {error}");
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

    pub fn accept_remote_device_pairing_update(
        &self,
        update: &transport::DevicePairingUpdate,
        peer_addr: Option<SocketAddr>,
    ) -> Result<()> {
        if update.target_device_id != self.local_device().device_id {
            bail!("device pairing update is not addressed to this device");
        }
        if update.origin_device_id == self.local_device().device_id {
            bail!("cannot pair this device with itself");
        }

        self.apply_pairing_route_hint(update, peer_addr.map(|address| address.ip()))?;
        self.set_device_pairing_internal(
            update.origin_device_id.clone(),
            update.paired,
            PairingChangeSource::Remote,
            false,
        )
    }

    fn emit_transfer_ready(&self, job: TransferJob) {
        let mut job = job;
        self.decorate_transfer_job(&mut job);
        let _ = self.inner.app.emit("transfer_ready", job);
    }

    fn emit_transfer_failed(&self, job: TransferJob) {
        let mut job = job;
        self.decorate_transfer_job(&mut job);
        let _ = self.inner.app.emit("transfer_failed", job);
    }

    fn record_packet_history(
        &self,
        packet: &ClipboardPacket,
        source: ClipboardHistorySource,
        origin_device_id: &str,
        origin_device_name: &str,
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
            origin_device_id: origin_device_id.to_string(),
            origin_device_name: origin_device_name.to_string(),
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

    fn record_local_file_history(
        &self,
        file_list: &LocalFileClipboard,
        origin_device_id: &str,
        origin_device_name: &str,
    ) -> Result<()> {
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
            origin_device_id: origin_device_id.to_string(),
            origin_device_name: origin_device_name.to_string(),
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

        let origin_device_name = self.device_name_for(&job.peer_device_id);

        self.push_clipboard_history(ClipboardHistoryEntry {
            entry_id: Uuid::new_v4().to_string(),
            kind: ClipboardHistoryKind::FileRefs,
            source: ClipboardHistorySource::Transfer,
            origin_device_id: job.peer_device_id.clone(),
            origin_device_name,
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

    async fn send_clipboard_packet_to_target(
        &self,
        local_device: &LocalDevice,
        target: &ActiveTarget,
        packet: &ClipboardPacket,
    ) -> Result<()> {
        let sequence = self.inner.sequence.fetch_add(1, Ordering::SeqCst);
        let envelope = transport::envelope_from_packet(
            local_device.device_id.clone(),
            target.device_id.clone(),
            packet,
            sequence,
        );

        transport::send_envelope(target.socket_addr, &target.fingerprint, envelope).await
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
            entries.retain(|existing| {
                existing.hash != entry.hash
                    || existing.kind != entry.kind
                    || existing.origin_device_id != entry.origin_device_id
            });
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
        let prepared =
            match tauri::async_runtime::spawn_blocking(move || transfers::prepare_transfer(&paths))
                .await
            {
                Ok(Ok(prepared)) => prepared,
                Ok(Err(error)) => {
                    let _ = self.fail_transfer(&transfer_id, error.to_string());
                    return;
                }
                Err(error) => {
                    let _ = self.fail_transfer(
                        &transfer_id,
                        format!("prepare transfer join error: {error}"),
                    );
                    return;
                }
            };

        let _ = self.mutate_transfer_job(&transfer_id, true, |job| {
            job.display_name = prepared.display_name.clone();
            job.total_bytes = prepared.total_bytes;
            job.total_entries = prepared.total_entries();
            job.entries = prepared
                .entries
                .iter()
                .map(|entry| entry.entry.clone())
                .collect();
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
            entries: prepared
                .entries
                .iter()
                .map(|entry| entry.entry.clone())
                .collect(),
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
        let active_device_ids = self.active_device_ids_clone();

        let mut devices = presence
            .values()
            .filter(|device| device.device_id != local_device_id)
            .map(|device| TrustedDevice {
                device_id: device.device_id.clone(),
                name: device.name.clone(),
                remark: self.device_remark_for(&device.device_id),
                platform: device.platform.clone(),
                fingerprint: device.fingerprint.clone(),
                auto_trusted: false,
                capabilities: device.capabilities.clone(),
                last_seen: Some(device.last_seen),
                last_sync_at: device.last_sync_at,
                last_sync_status: device.last_sync_status.clone(),
                addresses: device
                    .addresses
                    .iter()
                    .map(|address| address.to_string())
                    .collect(),
                host_name: device.host_name.clone(),
                port: (device.port > 0).then_some(device.port),
                is_online: device.is_online,
                is_active: active_device_ids.iter().any(|id| id == &device.device_id),
            })
            .collect::<Vec<_>>();

        devices.sort_by(|left, right| {
            right
                .is_active
                .cmp(&left.is_active)
                .then(right.is_online.cmp(&left.is_online))
                .then(
                    self.display_device_name(left)
                        .cmp(&self.display_device_name(right)),
                )
        });
        devices
    }

    fn active_targets(&self) -> Result<Vec<ActiveTarget>> {
        let active_device_ids = self.active_device_ids_clone();
        let presence = self
            .inner
            .presence
            .read()
            .map_err(|_| anyhow!("presence lock poisoned"))?;
        let mut targets = Vec::new();

        for active_device_id in active_device_ids {
            let Some(online) = presence.get(&active_device_id) else {
                continue;
            };
            if !online.is_online {
                continue;
            }
            let Some(target) = active_target_from_presence(online) else {
                continue;
            };
            targets.push(target);
        }

        Ok(targets)
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
                    background_sync_enabled: false,
                    discovery_enabled: false,
                    sync_enabled: false,
                    active_device_ids: Vec::new(),
                    device_remarks: Default::default(),
                    blocked_auto_pair_device_ids: Vec::new(),
                    language: AppLanguage::detect_system(),
                },
                certificate_der_b64: String::new(),
                private_key_der_b64: String::new(),
            })
    }

    fn active_device_ids_clone(&self) -> Vec<String> {
        self.inner
            .active_device_ids
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    fn set_device_paired_state(&self, device_id: &str, paired: bool) {
        if let Ok(mut active_device_ids) = self.inner.active_device_ids.write() {
            if paired {
                if !active_device_ids.iter().any(|entry| entry == device_id) {
                    active_device_ids.push(device_id.to_string());
                }
            } else {
                active_device_ids.retain(|entry| entry != device_id);
            }
        }
    }

    fn is_paired_device(&self, device_id: &str) -> bool {
        self.active_device_ids_clone()
            .iter()
            .any(|active_device_id| active_device_id == device_id)
    }

    fn device_remark_for(&self, device_id: &str) -> Option<String> {
        self.persistent_clone()
            .settings
            .device_remarks
            .get(device_id)
            .cloned()
    }

    fn display_device_name(&self, device: &TrustedDevice) -> String {
        device
            .remark
            .as_deref()
            .unwrap_or(device.name.as_str())
            .to_string()
    }

    fn apply_pairing_state(
        &self,
        device_id: &str,
        paired: bool,
        source: PairingChangeSource,
    ) -> Result<()> {
        self.set_device_paired_state(device_id, paired);

        if let Some(blocked) = source.blocked_auto_pair_state(paired) {
            self.set_auto_pair_blocked_state(device_id, blocked)?;
        }

        Ok(())
    }

    fn set_auto_pair_blocked_state(&self, device_id: &str, blocked: bool) -> Result<()> {
        let mut persistent = self
            .inner
            .persistent
            .write()
            .map_err(|_| anyhow!("persistent state lock poisoned"))?;
        let blocked_ids = &mut persistent.settings.blocked_auto_pair_device_ids;

        if blocked {
            if !blocked_ids.iter().any(|entry| entry == device_id) {
                blocked_ids.push(device_id.to_string());
            }
        } else {
            blocked_ids.retain(|entry| entry != device_id);
        }

        self.persist_locked(&persistent)
    }

    fn is_auto_pair_blocked(&self, device_id: &str) -> bool {
        self.persistent_clone()
            .settings
            .blocked_auto_pair_device_ids
            .iter()
            .any(|blocked_id| blocked_id == device_id)
    }

    fn resolve_active_target(&self, device_id: &str) -> Result<ActiveTarget> {
        let presence = self
            .inner
            .presence
            .read()
            .map_err(|_| anyhow!("presence lock poisoned"))?;
        let Some(device) = presence.get(device_id) else {
            bail!("unknown device");
        };
        if !device.is_online {
            bail!("device is offline");
        }
        active_target_from_presence(device)
            .ok_or_else(|| anyhow!("the paired device has no routable address"))
    }

    fn sync_device_pairing_to_peer(&self, target: ActiveTarget, device_id: String, paired: bool) {
        let runtime = self.clone();
        let update = self.build_device_pairing_update(device_id, paired);
        tauri::async_runtime::spawn(async move {
            if let Err(error) = transport::send_device_pairing_update(
                target.socket_addr,
                &target.fingerprint,
                update,
            )
            .await
            {
                log::warn!("failed to sync device pairing to peer: {error}");
                runtime.refresh_sync_summary();
                runtime.emit_devices_updated();
                runtime.emit_sync_status_changed();
            }
        });
    }

    fn build_device_pairing_update(
        &self,
        target_device_id: String,
        paired: bool,
    ) -> transport::DevicePairingUpdate {
        let local_device = self.local_device();
        transport::DevicePairingUpdate {
            origin_device_id: local_device.device_id,
            target_device_id,
            paired,
            origin_device_name: local_device.device_name,
            origin_platform: local_device.platform,
            origin_capabilities: local_device.capabilities,
            origin_fingerprint: local_device.fingerprint,
            origin_listen_port: (self.listen_port() > 0).then_some(self.listen_port()),
        }
    }

    fn auto_pair_device_if_only_online_peer(&self, device_id: &str) -> Result<bool> {
        if self.single_auto_pair_candidate()?.as_deref() != Some(device_id) {
            return Ok(false);
        }

        self.set_device_pairing_internal(device_id.to_string(), true, PairingChangeSource::Auto, true)?;
        Ok(true)
    }

    fn auto_pair_single_online_peer(&self) -> Result<bool> {
        let Some(device_id) = self.single_auto_pair_candidate()? else {
            return Ok(false);
        };

        self.set_device_pairing_internal(device_id, true, PairingChangeSource::Auto, true)?;
        Ok(true)
    }

    fn single_auto_pair_candidate(&self) -> Result<Option<String>> {
        let local_device_id = self.local_device().device_id;
        let active_device_ids = self.active_device_ids_clone();
        let presence = self
            .inner
            .presence
            .read()
            .map_err(|_| anyhow!("presence lock poisoned"))?;
        let online_peers = presence
            .values()
            .filter(|device| device.is_online && device.device_id != local_device_id)
            .collect::<Vec<_>>();
        if online_peers.len() != 1 {
            return Ok(None);
        }

        if active_device_ids.iter().any(|device_id| {
            presence
                .get(device_id)
                .is_some_and(|device| device.is_online)
        }) {
            return Ok(None);
        }

        let candidate = online_peers[0];
        if active_device_ids
            .iter()
            .any(|active_device_id| active_device_id == &candidate.device_id)
        {
            return Ok(None);
        }
        if self.is_auto_pair_blocked(&candidate.device_id) {
            return Ok(None);
        }
        if active_target_from_presence(candidate).is_none() {
            return Ok(None);
        }

        Ok(Some(candidate.device_id.clone()))
    }

    fn apply_pairing_route_hint(
        &self,
        update: &transport::DevicePairingUpdate,
        peer_ip: Option<IpAddr>,
    ) -> Result<()> {
        if !update.paired {
            return Ok(());
        }

        let mut presence = self
            .inner
            .presence
            .write()
            .map_err(|_| anyhow!("presence lock poisoned"))?;
        let previous = presence.get(&update.origin_device_id).cloned();
        let mut addresses = peer_ip.into_iter().collect::<Vec<_>>();
        if addresses.is_empty() {
            addresses = previous
                .as_ref()
                .map(|device| device.addresses.clone())
                .unwrap_or_default();
        }

        let port = update
            .origin_listen_port
            .filter(|port| *port > 0)
            .or_else(|| {
                previous
                    .as_ref()
                    .map(|device| device.port)
                    .filter(|port| *port > 0)
            })
            .unwrap_or_default();
        let name = if update.origin_device_name.trim().is_empty() {
            previous
                .as_ref()
                .map(|device| device.name.clone())
                .unwrap_or_else(|| update.origin_device_id.clone())
        } else {
            update.origin_device_name.clone()
        };
        let platform = if update.origin_platform.trim().is_empty() {
            previous
                .as_ref()
                .map(|device| device.platform.clone())
                .unwrap_or_else(|| "Unknown".to_string())
        } else {
            update.origin_platform.clone()
        };
        let fingerprint = if update.origin_fingerprint.trim().is_empty() {
            previous
                .as_ref()
                .map(|device| device.fingerprint.clone())
                .unwrap_or_default()
        } else {
            update.origin_fingerprint.clone()
        };
        let capabilities = if update.origin_capabilities.is_empty() {
            previous
                .as_ref()
                .map(|device| device.capabilities.clone())
                .unwrap_or_default()
        } else {
            update.origin_capabilities.clone()
        };

        presence.insert(
            update.origin_device_id.clone(),
            DevicePresence {
                device_id: update.origin_device_id.clone(),
                name,
                platform,
                fingerprint,
                capabilities,
                host_name: previous
                    .as_ref()
                    .and_then(|device| device.host_name.clone()),
                addresses,
                port,
                is_online: true,
                last_seen: Utc::now(),
                last_sync_at: previous.as_ref().and_then(|device| device.last_sync_at),
                last_sync_status: previous.and_then(|device| device.last_sync_status),
            },
        );
        Ok(())
    }

    fn device_name_for(&self, device_id: &str) -> String {
        if device_id == self.local_device().device_id {
            return self.local_device().device_name;
        }

        if let Some(remark) = self.device_remark_for(device_id) {
            return remark;
        }

        self.inner
            .presence
            .read()
            .ok()
            .and_then(|presence| presence.get(device_id).map(|device| device.name.clone()))
            .unwrap_or_else(|| device_id.to_string())
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

        let paired_online_count = self
            .active_device_ids_clone()
            .into_iter()
            .filter(|device_id| presence.get(device_id).is_some_and(|entry| entry.is_online))
            .count();

        if paired_online_count > 0 {
            self.set_sync_status(
                SyncState::Connected,
                Some(i18n::paired_devices_ready(
                    persistent.settings.language,
                    paired_online_count,
                )),
                payload,
            );
            return;
        }

        if !self.active_device_ids_clone().is_empty() {
            self.set_sync_status(
                SyncState::Idle,
                Some(i18n::paired_devices_offline(persistent.settings.language)),
                payload,
            );
            return;
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
        self.inner.transfer_jobs.read().ok().and_then(|jobs| {
            jobs.iter()
                .find(|job| job.transfer_id == transfer_id)
                .cloned()
        })
    }

    fn available_actions_for_job(&self, job: &TransferJob) -> Vec<TransferAction> {
        if job.direction != TransferDirection::Inbound || job.stage != TransferStage::Ready {
            return Vec::new();
        }

        let capabilities = self.runtime_capabilities();
        let mut actions = Vec::new();
        if capabilities.clipboard_files {
            actions.push(TransferAction::PlaceOnClipboard);
        }
        if capabilities.share_externally {
            actions.push(TransferAction::ShareExternally);
        }
        if capabilities.export_to_files {
            actions.push(TransferAction::ExportToFiles);
        }
        actions
    }

    fn decorate_transfer_job(&self, job: &mut TransferJob) {
        job.available_actions = self.available_actions_for_job(job);
    }

    fn decorate_transfer_jobs(&self, jobs: &mut [TransferJob]) {
        for job in jobs {
            self.decorate_transfer_job(job);
        }
    }

    fn upsert_transfer_job(&self, job: TransferJob, persist: bool) -> Result<()> {
        let snapshot = {
            let mut jobs = self
                .inner
                .transfer_jobs
                .write()
                .map_err(|_| anyhow!("transfer jobs lock poisoned"))?;
            if let Some(existing) = jobs
                .iter_mut()
                .find(|existing| existing.transfer_id == job.transfer_id)
            {
                *existing = job;
            } else {
                jobs.push(job);
            }
            if persist {
                Some(jobs.clone())
            } else {
                None
            }
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
            let updated = jobs
                .iter_mut()
                .find(|job| job.transfer_id == transfer_id)
                .map(|job| {
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
            if matches!(
                stage,
                TransferStage::Failed | TransferStage::Canceled | TransferStage::Ready
            ) {
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
        let mut jobs = jobs;
        self.decorate_transfer_jobs(&mut jobs);
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
    if port == 0 {
        return None;
    }

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

fn active_target_from_presence(device: &DevicePresence) -> Option<ActiveTarget> {
    Some(ActiveTarget {
        device_id: device.device_id.clone(),
        fingerprint: device.fingerprint.clone(),
        socket_addr: preferred_socket_addr(&device.addresses, device.port)?,
    })
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
