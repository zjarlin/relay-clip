use crate::clipboard::{self, ClipboardPacket};
use crate::discovery::{self, DiscoveryHandle};
use crate::i18n;
use crate::models::{
    AppLanguage, AppSettings, AppStateSnapshot, ClipboardPayload, LocalDevice, PersistentState,
    SettingsPatch, StoredTrustedDevice, SyncState, SyncStatus, TrustedDevice,
};
use crate::store;
use crate::transport::{self, ClipboardEnvelope};
use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Emitter};

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

struct RuntimeInner {
    app: AppHandle,
    state_path: PathBuf,
    persistent: RwLock<PersistentState>,
    presence: RwLock<HashMap<String, DevicePresence>>,
    service_map: Mutex<HashMap<String, String>>,
    recent_hashes: Mutex<VecDeque<String>>,
    sync_status: RwLock<SyncStatus>,
    discovery: Mutex<Option<DiscoveryHandle>>,
    listen_port: AtomicU16,
    sequence: AtomicU64,
}

#[derive(Clone, Debug)]
struct DevicePresence {
    host_name: Option<String>,
    addresses: Vec<IpAddr>,
    port: u16,
    is_online: bool,
    last_seen: DateTime<Utc>,
}

impl RelayRuntime {
    pub fn new(app: AppHandle) -> Result<Self> {
        let (state_path, persistent) = store::load_or_create()?;
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
                presence: RwLock::new(HashMap::new()),
                service_map: Mutex::new(HashMap::new()),
                recent_hashes: Mutex::new(VecDeque::new()),
                sync_status: RwLock::new(sync_status),
                discovery: Mutex::new(None),
                listen_port: AtomicU16::new(0),
                sequence: AtomicU64::new(1),
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
        Ok(AppStateSnapshot {
            local_device: persistent.local_device.clone(),
            settings: persistent.settings.clone(),
            devices: self.collect_devices(&persistent),
            sync_status: self.sync_status_clone(),
        })
    }

    pub fn list_devices(&self) -> Vec<TrustedDevice> {
        self.snapshot()
            .map(|snapshot| snapshot.devices)
            .unwrap_or_default()
    }

    pub fn set_active_device(&self, device_id: String) -> Result<AppStateSnapshot> {
        {
            let mut persistent = self
                .inner
                .persistent
                .write()
                .map_err(|_| anyhow!("persistent state lock poisoned"))?;
            if !persistent.trusted_devices.contains_key(&device_id) {
                bail!(match persistent.settings.language {
                    AppLanguage::ZhCn => "未知设备",
                    AppLanguage::En => "unknown device",
                });
            }
            persistent.settings.active_device_id = Some(device_id);
            self.persist_locked(&persistent)?;
        }

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

            if let Some(active_device_id) = patch.active_device_id {
                persistent.settings.active_device_id = active_device_id;
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
                let language = self.language();
                self.emit_clipboard_error(i18n::route_lookup_failed(language, &error.to_string()));
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
            Ok(_) => {
                self.record_device_sync(
                    &target.device_id,
                    i18n::relayed(language, &packet.meta.kind),
                    Some(packet.meta),
                );
            }
            Err(error) => {
                self.record_device_failure(
                    &target.device_id,
                    i18n::relay_failed(language, &error.to_string()),
                    Some(packet.meta),
                );
            }
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

        self.record_device_sync(
            &envelope.origin_device_id,
            i18n::received(self.language(), &packet.meta.kind),
            Some(packet.meta),
        );
        Ok(())
    }

    pub fn upsert_discovered_device(&self, peer: DiscoveredPeer) -> Result<()> {
        if peer.protocol_version != crate::models::PROTOCOL_VERSION {
            return Ok(());
        }

        let can_auto_trust = peer
            .host_name
            .as_deref()
            .is_some_and(|host| host.ends_with(".local."))
            && peer.addresses.iter().any(is_private_ip);

        {
            let mut persistent = self
                .inner
                .persistent
                .write()
                .map_err(|_| anyhow!("persistent state lock poisoned"))?;

            match persistent.trusted_devices.get_mut(&peer.device_id) {
                Some(device) => {
                    if !device.fingerprint.eq(&peer.fingerprint) {
                        bail!("stored fingerprint does not match the discovered device");
                    }
                    device.name = peer.name.clone();
                    device.platform = peer.platform.clone();
                    device.capabilities = peer.capabilities.clone();
                    device.host_name = peer.host_name.clone();
                    device.last_seen = Some(Utc::now());
                }
                None if can_auto_trust => {
                    persistent.trusted_devices.insert(
                        peer.device_id.clone(),
                        StoredTrustedDevice {
                            device_id: peer.device_id.clone(),
                            name: peer.name.clone(),
                            platform: peer.platform.clone(),
                            fingerprint: peer.fingerprint.clone(),
                            auto_trusted: true,
                            capabilities: peer.capabilities.clone(),
                            last_seen: Some(Utc::now()),
                            last_sync_at: None,
                            last_sync_status: None,
                            host_name: peer.host_name.clone(),
                        },
                    );

                    if persistent.settings.active_device_id.is_none() {
                        persistent.settings.active_device_id = Some(peer.device_id.clone());
                    }
                }
                None => return Ok(()),
            }

            self.persist_locked(&persistent)?;
        }

        {
            let mut presence = self
                .inner
                .presence
                .write()
                .map_err(|_| anyhow!("presence lock poisoned"))?;
            presence.insert(
                peer.device_id.clone(),
                DevicePresence {
                    host_name: peer.host_name.clone(),
                    addresses: peer.addresses.clone(),
                    port: peer.port,
                    is_online: true,
                    last_seen: Utc::now(),
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
                    if let Some(entry) = presence.get_mut(&device_id) {
                        entry.is_online = false;
                    }
                }
            }
        }

        self.refresh_sync_summary();
        self.emit_devices_updated();
    }

    pub fn emit_devices_updated(&self) {
        let _ = self.inner.app.emit("devices_updated", self.list_devices());
    }

    pub fn emit_sync_status_changed(&self) {
        let _ = self
            .inner
            .app
            .emit("sync_status_changed", self.sync_status_clone());
    }

    pub fn emit_clipboard_error(&self, message: impl Into<String>) {
        let message = message.into();
        self.set_sync_status(SyncState::Error, Some(message.clone()), None);
        let _ = self.inner.app.emit("clipboard_error", message);
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

    fn collect_devices(&self, persistent: &PersistentState) -> Vec<TrustedDevice> {
        let presence = self
            .inner
            .presence
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();

        let mut devices = persistent
            .trusted_devices
            .values()
            .map(|stored| {
                let online = presence.get(&stored.device_id);
                TrustedDevice {
                    device_id: stored.device_id.clone(),
                    name: stored.name.clone(),
                    platform: stored.platform.clone(),
                    fingerprint: stored.fingerprint.clone(),
                    auto_trusted: stored.auto_trusted,
                    capabilities: stored.capabilities.clone(),
                    last_seen: online.map(|entry| entry.last_seen).or(stored.last_seen),
                    last_sync_at: stored.last_sync_at,
                    last_sync_status: stored.last_sync_status.clone(),
                    addresses: online
                        .map(|entry| {
                            entry
                                .addresses
                                .iter()
                                .map(|address| address.to_string())
                                .collect()
                        })
                        .unwrap_or_default(),
                    host_name: online
                        .and_then(|entry| entry.host_name.clone())
                        .or_else(|| stored.host_name.clone()),
                    port: online.map(|entry| entry.port),
                    is_online: online.map(|entry| entry.is_online).unwrap_or(false),
                    is_active: persistent.settings.active_device_id.as_deref()
                        == Some(stored.device_id.as_str()),
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
        let persistent = self.persistent_clone();
        let Some(active_device_id) = persistent.settings.active_device_id else {
            return Ok(None);
        };
        let Some(stored_device) = persistent.trusted_devices.get(&active_device_id) else {
            return Ok(None);
        };

        let presence = self
            .inner
            .presence
            .read()
            .map_err(|_| anyhow!("presence lock poisoned"))?;
        let Some(online) = presence.get(&active_device_id) else {
            bail!(match self.language() {
                AppLanguage::ZhCn => "当前活动设备不可达",
                AppLanguage::En => "the active device is not reachable",
            });
        };

        let socket_addr =
            preferred_socket_addr(&online.addresses, online.port).ok_or_else(|| {
                anyhow!(match self.language() {
                    AppLanguage::ZhCn => "当前活动设备没有可用的路由地址",
                    AppLanguage::En => "the active device has no routable address",
                })
            })?;

        Ok(Some(ActiveTarget {
            device_id: active_device_id,
            name: stored_device.name.clone(),
            fingerprint: stored_device.fingerprint.clone(),
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
        if let Ok(mut persistent) = self.inner.persistent.write() {
            if let Some(device) = persistent.trusted_devices.get_mut(device_id) {
                device.last_sync_at = Some(Utc::now());
                device.last_sync_status = Some(status_message.clone());
                let _ = self.persist_locked(&persistent);
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
        if let Ok(mut persistent) = self.inner.persistent.write() {
            if let Some(device) = persistent.trusted_devices.get_mut(device_id) {
                device.last_sync_at = Some(Utc::now());
                device.last_sync_status = Some(status_message.clone());
                let _ = self.persist_locked(&persistent);
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

        if let Some(active_device_id) = persistent.settings.active_device_id.clone() {
            let presence = self
                .inner
                .presence
                .read()
                .map(|guard| guard.clone())
                .unwrap_or_default();
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
}

#[derive(Clone)]
struct ActiveTarget {
    device_id: String,
    name: String,
    fingerprint: String,
    socket_addr: SocketAddr,
}

fn is_private_ip(address: &IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => ip.is_private() || ip.is_link_local() || *ip == Ipv4Addr::LOCALHOST,
        IpAddr::V6(ip) => {
            ip.is_unique_local() || ip.is_unicast_link_local() || *ip == Ipv6Addr::LOCALHOST
        }
    }
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
