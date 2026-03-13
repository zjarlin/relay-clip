mod clipboard;
mod discovery;
mod i18n;
mod models;
mod runtime;
mod store;
mod transport;
mod transfers;
mod tray;

use crate::models::{AppStateSnapshot, ClipboardHistoryEntry, SettingsPatch, TransferJob, TrustedDevice};
use crate::runtime::RelayRuntime;
use tauri::{Manager, State, WindowEvent};

fn to_error_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[tauri::command]
fn get_app_state(runtime: State<'_, RelayRuntime>) -> Result<AppStateSnapshot, String> {
    runtime.snapshot().map_err(to_error_string)
}

#[tauri::command]
fn list_devices(runtime: State<'_, RelayRuntime>) -> Result<Vec<TrustedDevice>, String> {
    Ok(runtime.list_devices())
}

#[tauri::command]
fn set_active_device(
    runtime: State<'_, RelayRuntime>,
    device_id: String,
) -> Result<AppStateSnapshot, String> {
    let snapshot = runtime
        .set_active_device(device_id)
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
fn dismiss_transfer_job(
    runtime: State<'_, RelayRuntime>,
    transfer_id: String,
) -> Result<(), String> {
    runtime.dismiss_transfer_job(transfer_id).map_err(to_error_string)
}

#[tauri::command]
fn cancel_transfer_job(
    runtime: State<'_, RelayRuntime>,
    transfer_id: String,
) -> Result<(), String> {
    runtime.cancel_transfer_job(transfer_id).map_err(to_error_string)
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

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let relay = RelayRuntime::new(app.handle().clone())?;
            relay.initialize()?;
            app.manage(relay.clone());
            tray::setup(app.handle(), relay)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            list_devices,
            set_active_device,
            toggle_sync,
            update_settings,
            list_transfer_jobs,
            list_clipboard_history,
            place_received_transfer_on_clipboard,
            dismiss_transfer_job,
            cancel_transfer_job,
            restore_clipboard_history_entry,
            open_cache_directory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
