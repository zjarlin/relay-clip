import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from '@tauri-apps/plugin-autostart'
import type {
  AppStateSnapshot,
  SettingsPatch,
  SyncStatus,
  TrustedDevice,
} from './types'

export function getAppState() {
  return invoke<AppStateSnapshot>('get_app_state')
}

export function listDevices() {
  return invoke<TrustedDevice[]>('list_devices')
}

export function setActiveDevice(deviceId: string) {
  return invoke<AppStateSnapshot>('set_active_device', { deviceId })
}

export function toggleSync(enabled: boolean) {
  return invoke<AppStateSnapshot>('toggle_sync', { enabled })
}

export function updateSettings(patch: SettingsPatch) {
  return invoke<AppStateSnapshot>('update_settings', { patch })
}

export function onDevicesUpdated(handler: (devices: TrustedDevice[]) => void) {
  return listen<TrustedDevice[]>('devices_updated', (event) => handler(event.payload))
}

export function onSyncStatusChanged(handler: (status: SyncStatus) => void) {
  return listen<SyncStatus>('sync_status_changed', (event) => handler(event.payload))
}

export function onClipboardError(handler: (message: string) => void) {
  return listen<string>('clipboard_error', (event) => handler(event.payload))
}

export async function syncAutostart(enabled: boolean) {
  const active = await isAutostartEnabled()
  if (enabled && !active) {
    await enableAutostart()
    return
  }

  if (!enabled && active) {
    await disableAutostart()
  }
}

export { isAutostartEnabled }
