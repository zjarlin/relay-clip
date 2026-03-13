import type {
  AppLanguage,
  ReadyActionState,
  SyncState,
  TransferDirection,
  TransferStage,
} from './types'

export interface Messages {
  title: string
  nearby: string
  currentPair: string
  waitingDevices: string
  waitingDevicesHint: string
  noActivePair: string
  noActivePairHint: string
  makeActive: string
  activeNow: string
  online: string
  offline: string
  transfers: string
  noTransfers: string
  readyToPaste: string
  placeOnClipboard: string
  dismiss: string
  cancel: string
  launchOnLogin: string
  syncEnabled: string
  deviceName: string
  saving: string
  syncState(state: SyncState): string
  transferState(stage: TransferStage, direction: TransferDirection): string
  transferSummary(done: number, total: number): string
  notificationTitle: string
  notificationBody(name: string): string
  replaceWarning: string
  hiddenReadyState(state: ReadyActionState): string
}

const english: Messages = {
  title: 'RelayClip',
  nearby: 'Nearby devices',
  currentPair: 'Current pairing',
  waitingDevices: 'No other online devices found',
  waitingDevicesHint: 'Devices on the same LAN will appear here automatically.',
  noActivePair: 'No device paired yet',
  noActivePairHint: 'Choose one online device below when it appears.',
  makeActive: 'Pair',
  activeNow: 'Connected',
  online: 'Online',
  offline: 'Offline',
  transfers: 'Transfers',
  noTransfers: 'No file relays yet.',
  readyToPaste: 'Ready to place on clipboard',
  placeOnClipboard: 'Place on clipboard',
  dismiss: 'Dismiss',
  cancel: 'Cancel',
  launchOnLogin: 'Launch on login',
  syncEnabled: 'Clipboard sync',
  deviceName: 'Device name',
  saving: 'Saving...',
  syncState: (state) =>
    ({
      idle: 'Idle',
      discovering: 'Discovering',
      connected: 'Connected',
      syncing: 'Syncing',
      paused: 'Paused',
      error: 'Error',
    })[state],
  transferState: (stage, direction) => {
    if (stage === 'ready') {
      return direction === 'inbound' ? 'Downloaded' : 'Sent'
    }
    return (
      {
        preparing: 'Preparing',
        queued: 'Queued',
        downloading: 'Downloading',
        verifying: 'Verifying',
        ready: 'Ready',
        failed: 'Failed',
        canceled: 'Canceled',
      } as const
    )[stage]
  },
  transferSummary: (done, total) => `${done}/${total} items`,
  notificationTitle: 'File relay complete',
  notificationBody: (name) => `${name} is ready to place on the clipboard.`,
  replaceWarning: 'This replaces the current clipboard content.',
  hiddenReadyState: (state) =>
    ({
      pendingPrompt: 'Pending',
      dismissed: 'Dismissed',
      placed: 'Placed',
    })[state],
}

const chinese: Messages = {
  title: 'RelayClip',
  nearby: '附近设备',
  currentPair: '当前配对',
  waitingDevices: '还没有发现其他在线设备',
  waitingDevicesHint: '同一局域网内的设备会自动出现在这里。',
  noActivePair: '还没有配对设备',
  noActivePairHint: '下方出现在线设备后可直接配对。',
  makeActive: '配对',
  activeNow: '已连接',
  online: '在线',
  offline: '离线',
  transfers: '文件接力',
  noTransfers: '还没有文件接力任务。',
  readyToPaste: '已下载完成，可放入剪贴板',
  placeOnClipboard: '放入剪贴板',
  dismiss: '忽略',
  cancel: '取消',
  launchOnLogin: '开机启动',
  syncEnabled: '剪贴板同步',
  deviceName: '设备名称',
  saving: '保存中...',
  syncState: (state) =>
    ({
      idle: '空闲',
      discovering: '搜索中',
      connected: '已连接',
      syncing: '同步中',
      paused: '已暂停',
      error: '出错',
    })[state],
  transferState: (stage, direction) => {
    if (stage === 'ready') {
      return direction === 'inbound' ? '已下载' : '已发送'
    }
    return (
      {
        preparing: '准备中',
        queued: '排队中',
        downloading: '下载中',
        verifying: '校验中',
        ready: '已完成',
        failed: '失败',
        canceled: '已取消',
      } as const
    )[stage]
  },
  transferSummary: (done, total) => `${done}/${total} 项`,
  notificationTitle: '文件已接力完成',
  notificationBody: (name) => `${name} 已可放入剪贴板。`,
  replaceWarning: '这会替换当前剪贴板内容。',
  hiddenReadyState: (state) =>
    ({
      pendingPrompt: '待处理',
      dismissed: '已忽略',
      placed: '已放入',
    })[state],
}

export function normalizeLanguage(input?: string | null): AppLanguage {
  if (input?.toLowerCase().startsWith('zh')) {
    return 'zh-CN'
  }

  return 'en'
}

export function getMessages(language: AppLanguage): Messages {
  return language === 'zh-CN' ? chinese : english
}
