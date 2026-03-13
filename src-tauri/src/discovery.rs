use crate::runtime::RelayRuntime;
use anyhow::Result;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod platform {
    use crate::models::{CAPABILITIES, PROTOCOL_VERSION};
    use crate::runtime::{DiscoveredPeer, RelayRuntime, SERVICE_TYPE};
    use anyhow::{Context, Result};
    use mdns_sd::{DaemonEvent, ServiceDaemon, ServiceEvent, ServiceInfo};

    pub struct DiscoveryHandle {
        mdns: ServiceDaemon,
    }

    impl DiscoveryHandle {
        pub fn stop(self) {
            let _ = self.mdns.shutdown();
        }
    }

    pub fn start(runtime: RelayRuntime, port: u16) -> Result<DiscoveryHandle> {
        let mdns = ServiceDaemon::new().context("failed to create the mdns daemon")?;
        let local = runtime.local_device();
        let instance_name = format!("{}-{}", local.device_name, &local.device_id[..8]);
        let host_name = format!("relayclip-{}.local.", &local.device_id[..8]);
        let capabilities = CAPABILITIES.join(",");
        let port_string = port.to_string();
        let properties = [
            ("device_id", local.device_id.as_str()),
            ("device_name", local.device_name.as_str()),
            ("platform", local.platform.as_str()),
            ("protocol_version", PROTOCOL_VERSION),
            ("capabilities", capabilities.as_str()),
            ("pubkey_fingerprint", local.fingerprint.as_str()),
            ("listen_port", port_string.as_str()),
        ];

        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &host_name,
            "",
            port,
            &properties[..],
        )?
        .enable_addr_auto();

        let monitor = mdns.monitor().context("failed to monitor mdns")?;
        let browse = mdns
            .browse(SERVICE_TYPE)
            .context("failed to start mdns browse")?;
        mdns.register(service_info)
            .context("failed to register relayclip service")?;

        let runtime_for_monitor = runtime.clone();
        std::thread::spawn(move || {
            while let Ok(event) = monitor.recv() {
                if let DaemonEvent::Error(error) = event {
                    runtime_for_monitor.emit_clipboard_error(format!("Discovery error: {error}"));
                }
            }
        });

        let runtime_for_browse = runtime.clone();
        std::thread::spawn(move || {
            while let Ok(event) = browse.recv() {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        if let Some(peer) = peer_from_info(info) {
                            if let Err(error) = runtime_for_browse.upsert_discovered_device(peer) {
                                runtime_for_browse
                                    .emit_clipboard_error(format!("Discovery update failed: {error}"));
                            }
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        runtime_for_browse.mark_device_offline(&fullname);
                    }
                    _ => {}
                }
            }
        });

        Ok(DiscoveryHandle { mdns })
    }

    fn peer_from_info(info: ServiceInfo) -> Option<DiscoveredPeer> {
        let properties = info.get_properties();
        Some(DiscoveredPeer {
            service_fullname: info.get_fullname().to_string(),
            device_id: properties.get("device_id")?.val_str().ok()?.to_string(),
            name: properties.get("device_name")?.val_str().ok()?.to_string(),
            platform: properties.get("platform")?.val_str().ok()?.to_string(),
            protocol_version: properties
                .get("protocol_version")?
                .val_str()
                .ok()?
                .to_string(),
            capabilities: properties
                .get("capabilities")
                .and_then(|value| value.val_str().ok())
                .map(|value| {
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|entry| !entry.is_empty())
                        .map(|entry| entry.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            fingerprint: properties
                .get("pubkey_fingerprint")?
                .val_str()
                .ok()?
                .to_string(),
            host_name: Some(info.get_hostname().to_string()),
            addresses: info.get_addresses().iter().copied().collect(),
            port: info.get_port(),
        })
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
mod platform {
    use super::RelayRuntime;
    use anyhow::Result;

    pub struct DiscoveryHandle;

    impl DiscoveryHandle {
        pub fn stop(self) {}
    }

    pub fn start(_runtime: RelayRuntime, _port: u16) -> Result<DiscoveryHandle> {
        Ok(DiscoveryHandle)
    }
}

pub use platform::{start, DiscoveryHandle};
