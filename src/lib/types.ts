export type PayloadKind = 'text' | 'image'
export type AppLanguage = 'en' | 'zh-CN'
export type ClipboardHistoryKind = 'text' | 'image' | 'fileRefs'
export type ClipboardHistorySource = 'local' | 'remote' | 'transfer'

export type SyncState =
  | 'idle'
  | 'discovering'
  | 'connected'
  | 'syncing'
  | 'paused'
  | 'error'

export type TransferDirection = 'outbound' | 'inbound'
export type TransferKind = 'fileRefs'
export type TransferStage =
  | 'preparing'
  | 'queued'
  | 'downloading'
  | 'verifying'
  | 'ready'
  | 'failed'
  | 'canceled'
export type ReadyActionState = 'pendingPrompt' | 'dismissed' | 'placed'
export type TransferEntryKind = 'file' | 'directory'

export interface AppSettings {
  deviceName: string
  launchOnLogin: boolean
  discoveryEnabled: boolean
  syncEnabled: boolean
  activeDeviceId: string | null
  language: AppLanguage
}

export interface LocalDevice {
  deviceId: string
  deviceName: string
  platform: string
  protocolVersion: string
  capabilities: string[]
  fingerprint: string
}

export interface ClipboardPayload {
  kind: PayloadKind
  mime: string
  size: number
  hash: string
}

export interface ClipboardHistoryEntry {
  entryId: string
  kind: ClipboardHistoryKind
  source: ClipboardHistorySource
  displayName: string
  previewText: string | null
  mime: string | null
  hash: string
  size: number
  fileCount: number | null
  createdAt: string
  payloadPath: string | null
  transferId: string | null
  topLevelNames: string[]
}

export interface TrustedDevice {
  deviceId: string
  name: string
  platform: string
  fingerprint: string
  autoTrusted: boolean
  capabilities: string[]
  lastSeen: string | null
  lastSyncAt: string | null
  lastSyncStatus: string | null
  addresses: string[]
  hostName: string | null
  port: number | null
  isOnline: boolean
  isActive: boolean
}

export interface TransferEntry {
  relativePath: string
  entryKind: TransferEntryKind
  size: number
  modifiedAt: string | null
}

export interface TransferJob {
  transferId: string
  peerDeviceId: string
  direction: TransferDirection
  kind: TransferKind
  displayName: string
  totalBytes: number
  completedBytes: number
  totalEntries: number
  completedEntries: number
  stage: TransferStage
  startedAt: string
  finishedAt: string | null
  errorMessage: string | null
  warningMessage: string | null
  readyToPaste: boolean
  readyActionState: ReadyActionState
  stagingPath: string | null
  entries: TransferEntry[]
  topLevelNames: string[]
}

export interface SyncStatus {
  state: SyncState
  message: string | null
  updatedAt: string
  lastPayload: ClipboardPayload | null
}

export interface AppStateSnapshot {
  localDevice: LocalDevice
  settings: AppSettings
  devices: TrustedDevice[]
  syncStatus: SyncStatus
}

export interface SettingsPatch {
  deviceName?: string
  launchOnLogin?: boolean
  discoveryEnabled?: boolean
  syncEnabled?: boolean
  activeDeviceId?: string | null
  language?: AppLanguage
}
