import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from '@tauri-apps/plugin-autostart'
import type {
  AppStateSnapshot,
  ClipboardHistoryEntry,
  SettingsPatch,
  SyncStatus,
  TransferJob,
  TrustedDevice,
} from './types'

export function getAppState() {
  return invoke<AppStateSnapshot>('get_app_state')
}

export function listDevices() {
  return invoke<TrustedDevice[]>('list_devices')
}

export function setDevicePairing(deviceId: string, paired: boolean) {
  return invoke<AppStateSnapshot>('set_device_pairing', { deviceId, paired })
}

export function toggleSync(enabled: boolean) {
  return invoke<AppStateSnapshot>('toggle_sync', { enabled })
}

export function updateSettings(patch: SettingsPatch) {
  return invoke<AppStateSnapshot>('update_settings', { patch })
}

export function listTransferJobs() {
  return invoke<TransferJob[]>('list_transfer_jobs')
}

export function listClipboardHistory() {
  return invoke<ClipboardHistoryEntry[]>('list_clipboard_history')
}

export function placeReceivedTransferOnClipboard(transferId: string) {
  return invoke<void>('place_received_transfer_on_clipboard', { transferId })
}

export function dismissTransferJob(transferId: string) {
  return invoke<void>('dismiss_transfer_job', { transferId })
}

export function cancelTransferJob(transferId: string) {
  return invoke<void>('cancel_transfer_job', { transferId })
}

export function restoreClipboardHistoryEntry(entryId: string) {
  return invoke<void>('restore_clipboard_history_entry', { entryId })
}

export function openCacheDirectory() {
  return invoke<void>('open_cache_directory')
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

export function onTransferJobsUpdated(handler: (jobs: TransferJob[]) => void) {
  return listen<TransferJob[]>('transfer_jobs_updated', (event) => handler(event.payload))
}

export function onClipboardHistoryUpdated(handler: (entries: ClipboardHistoryEntry[]) => void) {
  return listen<ClipboardHistoryEntry[]>('clipboard_history_updated', (event) =>
    handler(event.payload),
  )
}

export function onTransferReady(handler: (job: TransferJob) => void) {
  return listen<TransferJob>('transfer_ready', (event) => handler(event.payload))
}

export function onTransferFailed(handler: (job: TransferJob) => void) {
  return listen<TransferJob>('transfer_failed', (event) => handler(event.payload))
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
