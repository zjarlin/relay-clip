import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from '@tauri-apps/plugin-autostart'
import type {
  AppStateSnapshot,
  BackgroundSyncState,
  ClipboardHistoryEntry,
  RuntimeCapabilities,
  RuntimePermissions,
  SettingsPatch,
  SyncStatus,
  TransferJob,
  TrustedDevice,
} from './types'

export function getAppState() {
  return invoke<AppStateSnapshot>('get_app_state')
}

export function getRuntimeCapabilities() {
  return invoke<RuntimeCapabilities>('get_runtime_capabilities')
}

export function requestRuntimePermissions() {
  return invoke<RuntimePermissions>('request_runtime_permissions')
}

export function getBackgroundSyncState() {
  return invoke<BackgroundSyncState>('get_background_sync_state')
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

export function listTransferJobs() {
  return invoke<TransferJob[]>('list_transfer_jobs')
}

export function listClipboardHistory() {
  return invoke<ClipboardHistoryEntry[]>('list_clipboard_history')
}

export function placeReceivedTransferOnClipboard(transferId: string) {
  return invoke<void>('place_received_transfer_on_clipboard', { transferId })
}

export function shareReceivedTransfer(transferId: string) {
  return invoke<void>('share_received_transfer', { transferId })
}

export function exportReceivedTransfer(transferId: string) {
  return invoke<void>('export_received_transfer', { transferId })
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
