use crate::models::{
    BackgroundSyncMode, BackgroundSyncState, RuntimeCapabilities, RuntimePermissions,
    RuntimePlatform,
};
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{
    plugin::{Builder as PluginBuilder, TauriPlugin},
    Manager, Runtime,
};

#[derive(Clone)]
pub struct RuntimeBridge {
    inner: Arc<RuntimeBridgeInner>,
}

struct RuntimeBridgeInner {
    platform: RuntimePlatform,
    capabilities: RuntimeCapabilities,
    permissions: Mutex<RuntimePermissions>,
}

impl RuntimeBridge {
    pub fn new() -> Self {
        let platform = RuntimePlatform::current();
        let capabilities = RuntimeCapabilities::for_platform(platform);
        let permissions = RuntimePermissions::for_platform(platform, &capabilities);

        Self {
            inner: Arc::new(RuntimeBridgeInner {
                platform,
                capabilities,
                permissions: Mutex::new(permissions),
            }),
        }
    }

    pub fn platform(&self) -> RuntimePlatform {
        self.inner.platform
    }

    pub fn capabilities(&self) -> RuntimeCapabilities {
        self.inner.capabilities.clone()
    }

    pub fn permissions(&self) -> RuntimePermissions {
        self.inner
            .permissions
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| {
                RuntimePermissions::for_platform(self.inner.platform, &self.inner.capabilities)
            })
    }

    pub fn request_permissions(&self) -> RuntimePermissions {
        let granted = RuntimePermissions::granted_for(&self.inner.capabilities);
        if let Ok(mut permissions) = self.inner.permissions.lock() {
            *permissions = granted.clone();
        }
        granted
    }

    pub fn background_sync_state(&self, enabled: bool) -> BackgroundSyncState {
        match self.inner.platform {
            RuntimePlatform::Android => BackgroundSyncState {
                supported: true,
                enabled,
                active: enabled,
                mode: BackgroundSyncMode::ForegroundService,
                message: Some("Android keeps sync alive through a foreground service when enabled.".to_string()),
            },
            RuntimePlatform::Ios => BackgroundSyncState {
                supported: true,
                enabled,
                active: enabled,
                mode: BackgroundSyncMode::AppRefresh,
                message: Some(
                    "iOS background sync is best-effort and resumes fully when the app returns to the foreground."
                        .to_string(),
                ),
            },
            RuntimePlatform::Windows | RuntimePlatform::Macos | RuntimePlatform::Linux => {
                BackgroundSyncState {
                    supported: false,
                    enabled,
                    active: true,
                    mode: BackgroundSyncMode::Desktop,
                    message: Some(
                        "Desktop builds stay active while the app is running and do not use a mobile background sync toggle."
                            .to_string(),
                    ),
                }
            }
            RuntimePlatform::Unknown => BackgroundSyncState {
                supported: false,
                enabled,
                active: false,
                mode: BackgroundSyncMode::Unsupported,
                message: Some("Background sync state is unavailable on this platform.".to_string()),
            },
        }
    }

    pub fn share_paths(&self, paths: &[PathBuf]) -> Result<()> {
        if paths.is_empty() {
            return Err(anyhow!("no transfer files are available"));
        }

        match self.inner.platform {
            RuntimePlatform::Android | RuntimePlatform::Ios => Err(anyhow!(
                "native share sheet integration is not available in this build"
            )),
            _ => Err(anyhow!("system sharing is only available on mobile builds")),
        }
    }

    pub fn export_paths(&self, paths: &[PathBuf]) -> Result<()> {
        if paths.is_empty() {
            return Err(anyhow!("no transfer files are available"));
        }

        match self.inner.platform {
            RuntimePlatform::Android | RuntimePlatform::Ios => Err(anyhow!(
                "native file export integration is not available in this build"
            )),
            _ => Err(anyhow!("file export is only available on mobile builds")),
        }
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    PluginBuilder::new("mobile-bridge")
        .setup(|app, _api| {
            app.manage(RuntimeBridge::new());
            Ok(())
        })
        .build()
}
