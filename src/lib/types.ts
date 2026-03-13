export type PayloadKind = 'text' | 'image'
export type AppLanguage = 'en' | 'zh-CN'

export type SyncState =
  | 'idle'
  | 'discovering'
  | 'connected'
  | 'syncing'
  | 'paused'
  | 'error'

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
