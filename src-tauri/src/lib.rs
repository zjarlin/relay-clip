mod clipboard;
mod discovery;
mod i18n;
mod mobile_bridge;
mod models;
mod runtime;
mod store;
mod transfers;
mod transport;
mod tray;

use crate::models::{
    AppStateSnapshot, BackgroundSyncState, ClipboardHistoryEntry, RuntimeCapabilities,
    RuntimePermissions, SettingsPatch, TransferJob, TrustedDevice,
};
use crate::runtime::RelayRuntime;
use std::time::Duration;
use tauri::{Manager, State};

fn to_error_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(target_os = "macos")]
fn configure_macos_status_bar_app(app: &tauri::AppHandle) -> tauri::Result<()> {
    if cfg!(debug_assertions) {
        return Ok(());
    }

    app.set_activation_policy(tauri::ActivationPolicy::Accessory)?;
    app.set_dock_visibility(false)?;
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn focus_main_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    app.show()?;

    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
    }

    Ok(())
}

#[tauri::command]
fn get_app_state(runtime: State<'_, RelayRuntime>) -> Result<AppStateSnapshot, String> {
    runtime.snapshot().map_err(to_error_string)
}

#[tauri::command]
fn get_runtime_capabilities(
    runtime: State<'_, RelayRuntime>,
) -> Result<RuntimeCapabilities, String> {
    Ok(runtime.runtime_capabilities())
}

#[tauri::command]
fn request_runtime_permissions(
    runtime: State<'_, RelayRuntime>,
) -> Result<RuntimePermissions, String> {
    Ok(runtime.request_runtime_permissions())
}

#[tauri::command]
fn get_background_sync_state(
    runtime: State<'_, RelayRuntime>,
) -> Result<BackgroundSyncState, String> {
    Ok(runtime.background_sync_state())
}

#[tauri::command]
fn list_devices(runtime: State<'_, RelayRuntime>) -> Result<Vec<TrustedDevice>, String> {
    Ok(runtime.list_devices())
}

#[tauri::command]
fn set_device_pairing(
    runtime: State<'_, RelayRuntime>,
    device_id: String,
    paired: bool,
) -> Result<AppStateSnapshot, String> {
    let snapshot = runtime
        .set_device_pairing(device_id, paired)
        .map_err(to_error_string)?;
    let _ = tray::refresh(runtime.app_handle(), &runtime);
    Ok(snapshot)
}

#[tauri::command]
fn set_device_remark(
    runtime: State<'_, RelayRuntime>,
    device_id: String,
    remark: Option<String>,
) -> Result<AppStateSnapshot, String> {
    let snapshot = runtime
        .set_device_remark(device_id, remark)
        .map_err(to_error_string)?;
    let _ = tray::refresh(runtime.app_handle(), &runtime);
    Ok(snapshot)
}

#[tauri::command]
fn toggle_sync(
    runtime: State<'_, RelayRuntime>,
    enabled: bool,
) -> Result<AppStateSnapshot, String> {
    let snapshot = runtime.toggle_sync(enabled).map_err(to_error_string)?;
    let _ = tray::refresh(runtime.app_handle(), &runtime);
    Ok(snapshot)
}

#[tauri::command]
fn update_settings(
    runtime: State<'_, RelayRuntime>,
    patch: SettingsPatch,
) -> Result<AppStateSnapshot, String> {
    let snapshot = runtime.update_settings(patch).map_err(to_error_string)?;
    let _ = tray::refresh(runtime.app_handle(), &runtime);
    Ok(snapshot)
}

#[tauri::command]
fn list_transfer_jobs(runtime: State<'_, RelayRuntime>) -> Result<Vec<TransferJob>, String> {
    Ok(runtime.list_transfer_jobs())
}

#[tauri::command]
fn list_clipboard_history(
    runtime: State<'_, RelayRuntime>,
) -> Result<Vec<ClipboardHistoryEntry>, String> {
    Ok(runtime.list_clipboard_history())
}

#[tauri::command]
fn place_received_transfer_on_clipboard(
    runtime: State<'_, RelayRuntime>,
    transfer_id: String,
) -> Result<(), String> {
    runtime
        .place_received_transfer_on_clipboard(transfer_id)
        .map_err(to_error_string)
}

#[tauri::command]
fn share_received_transfer(
    runtime: State<'_, RelayRuntime>,
    transfer_id: String,
) -> Result<(), String> {
    runtime
        .share_received_transfer(transfer_id)
        .map_err(to_error_string)
}

#[tauri::command]
fn export_received_transfer(
    runtime: State<'_, RelayRuntime>,
    transfer_id: String,
) -> Result<(), String> {
    runtime
        .export_received_transfer(transfer_id)
        .map_err(to_error_string)
}

#[tauri::command]
fn dismiss_transfer_job(
    runtime: State<'_, RelayRuntime>,
    transfer_id: String,
) -> Result<(), String> {
    runtime
        .dismiss_transfer_job(transfer_id)
        .map_err(to_error_string)
}

#[tauri::command]
fn cancel_transfer_job(
    runtime: State<'_, RelayRuntime>,
    transfer_id: String,
) -> Result<(), String> {
    runtime
        .cancel_transfer_job(transfer_id)
        .map_err(to_error_string)
}

#[tauri::command]
fn restore_clipboard_history_entry(
    runtime: State<'_, RelayRuntime>,
    entry_id: String,
) -> Result<(), String> {
    runtime
        .restore_clipboard_history_entry(entry_id)
        .map_err(to_error_string)
}

#[tauri::command]
fn open_cache_directory(runtime: State<'_, RelayRuntime>) -> Result<(), String> {
    runtime.open_cache_directory().map_err(to_error_string)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let mut builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(mobile_bridge::init())
        .plugin(tauri_plugin_notification::init());

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.plugin(tauri_plugin_autostart::Builder::new().build());
    }

    builder
        .setup(|app| {
            #[cfg(target_os = "macos")]
            configure_macos_status_bar_app(app.handle())?;

            let bridge = app.state::<mobile_bridge::RuntimeBridge>().inner().clone();
            let relay = RelayRuntime::new(app.handle().clone(), bridge)?;
            relay.initialize()?;
            app.manage(relay.clone());
            tray::setup(app.handle(), relay)?;

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    let _ = focus_main_window(&app_handle);
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            get_runtime_capabilities,
            request_runtime_permissions,
            get_background_sync_state,
            list_devices,
            set_device_pairing,
            set_device_remark,
            toggle_sync,
            update_settings,
            list_transfer_jobs,
            list_clipboard_history,
            place_received_transfer_on_clipboard,
            share_received_transfer,
            export_received_transfer,
            dismiss_transfer_job,
            cancel_transfer_job,
            restore_clipboard_history_entry,
            open_cache_directory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
