use crate::i18n;
use crate::runtime::RelayRuntime;
use anyhow::{anyhow, Result};
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{image::Image, AppHandle, Manager};

const TRAY_ID: &str = "relayclip-tray";

pub fn setup(app: &AppHandle, relay: RelayRuntime) -> Result<()> {
    let menu = build_menu(app, &relay)?;
    let tray_icon = Image::from_bytes(include_bytes!("../icons/tray-icon.png"))?;
    let relay_for_menu = relay.clone();
    let relay_for_click = relay.clone();

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref().to_string();
            match id.as_str() {
                "open-settings" => {
                    let _ = show_main_window(app);
                }
                "toggle-sync" => {
                    let enabled = !relay_for_menu
                        .snapshot()
                        .map(|snapshot| snapshot.settings.sync_enabled)
                        .unwrap_or(true);
                    let _ = relay_for_menu.toggle_sync(enabled);
                    let _ = refresh(app, &relay_for_menu);
                }
                "quit-app" => {
                    app.exit(0);
                }
                _ if id.starts_with("device:") => {
                    let device_id = id.trim_start_matches("device:").to_string();
                    let _ = relay_for_menu.set_active_device(device_id);
                    let _ = refresh(app, &relay_for_menu);
                    let _ = show_main_window(app);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = show_main_window(tray.app_handle());
                let _ = refresh(tray.app_handle(), &relay_for_click);
            }
        })
        .build(app)?;

    Ok(())
}

pub fn refresh(app: &AppHandle, relay: &RelayRuntime) -> Result<()> {
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| anyhow!("tray icon was not initialized"))?;
    let menu = build_menu(app, relay)?;
    tray.set_menu(Some(menu))?;
    Ok(())
}

fn build_menu(app: &AppHandle, relay: &RelayRuntime) -> Result<tauri::menu::Menu<tauri::Wry>> {
    let snapshot = relay.snapshot()?;
    let language = snapshot.settings.language;
    let visible_devices = snapshot
        .devices
        .iter()
        .filter(|device| device.is_online)
        .cloned()
        .collect::<Vec<_>>();
    let active_name = snapshot
        .devices
        .iter()
        .find(|device| device.is_active)
        .map(|device| device.name.clone())
        .unwrap_or_else(|| i18n::no_active_device_label(language).to_string());
    let sync_label = if snapshot.settings.sync_enabled {
        i18n::tray_pause_sync(language)
    } else {
        i18n::tray_resume_sync(language)
    };

    let mut builder = MenuBuilder::new(app);
    builder = builder.text("active-label", i18n::active_label(language, &active_name));
    builder = builder.separator();

    let mut device_submenu = SubmenuBuilder::new(app, i18n::tray_devices(language));
    if visible_devices.is_empty() {
        device_submenu =
            device_submenu.text("device:none", i18n::tray_waiting_for_devices(language));
    } else {
        for device in visible_devices {
            let label = if device.is_active {
                format!("* {}", device.name)
            } else {
                device.name
            };
            let item = MenuItemBuilder::with_id(format!("device:{}", device.device_id), label)
                .build(app)?;
            device_submenu = device_submenu.item(&item);
        }
    }

    builder = builder.item(&device_submenu.build()?);
    builder = builder.separator();
    let toggle_item = MenuItemBuilder::with_id("toggle-sync", sync_label).build(app)?;
    let open_item =
        MenuItemBuilder::with_id("open-settings", i18n::tray_open_settings(language)).build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit-app", i18n::tray_quit(language)).build(app)?;
    builder = builder.item(&toggle_item);
    builder = builder.item(&open_item);
    builder = builder.item(&PredefinedMenuItem::separator(app)?);
    builder = builder.item(&quit_item);

    Ok(builder.build()?)
}

fn show_main_window(app: &AppHandle) -> Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
    }
    Ok(())
}
