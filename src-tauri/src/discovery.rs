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
                    if let Some(peer) =
                        resolved_to_peer(*info, &runtime_for_browse.local_device().device_id)
                    {
                        if let Err(error) = runtime_for_browse.upsert_discovered_device(peer) {
                            runtime_for_browse.emit_clipboard_error(format!(
                                "Discovered peer could not be registered: {error}"
                            ));
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

fn resolved_to_peer(
    info: mdns_sd::ResolvedService,
    local_device_id: &str,
) -> Option<DiscoveredPeer> {
    let device_id = info.get_property_val_str("device_id")?.to_string();
    if device_id == local_device_id {
        return None;
    }

    Some(DiscoveredPeer {
        service_fullname: info.get_fullname().to_string(),
        device_id,
        name: info
            .get_property_val_str("device_name")
            .unwrap_or("RelayClip Peer")
            .to_string(),
        platform: info
            .get_property_val_str("platform")
            .unwrap_or("Unknown")
            .to_string(),
        protocol_version: info
            .get_property_val_str("protocol_version")
            .unwrap_or(PROTOCOL_VERSION)
            .to_string(),
        capabilities: info
            .get_property_val_str("capabilities")
            .unwrap_or_default()
            .split(',')
            .filter(|entry| !entry.trim().is_empty())
            .map(|entry| entry.trim().to_string())
            .collect(),
        fingerprint: info
            .get_property_val_str("pubkey_fingerprint")
            .unwrap_or_default()
            .to_string(),
        host_name: Some(info.get_hostname().to_string()),
        addresses: info
            .get_addresses()
            .iter()
            .map(|address| address.to_ip_addr())
            .collect(),
        port: info.get_port(),
    })
}
