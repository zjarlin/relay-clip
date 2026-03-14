import type {
  AppLanguage,
  ClipboardHistoryKind,
  ClipboardHistorySource,
  ReadyActionState,
  SyncState,
  TransferDirection,
  TransferStage,
} from './types'

export interface Messages {
  title: string
  nearby: string
  pairedDevices: string
  pairedDevicesCount(count: number): string
  noPairedDevices: string
  noPairedDevicesHint: string
  noNearbyDevices: string
  nearbyDevicesHint: string
  pair: string
  paired: string
  unpair: string
  activeNow: string
  online: string
  offline: string
  transfers: string
  noTransfers: string
  copyCenter: string
  noClipboardHistory: string
  restoreHistory: string
  openCacheDirectory: string
  readyToPaste: string
  placeOnClipboard: string
  dismiss: string
  cancel: string
  syncEnabled: string
  saving: string
  unknownDevice: string
  thisDevice: string
  clips(count: number): string
  syncState(state: SyncState): string
  transferState(stage: TransferStage, direction: TransferDirection): string
  transferSummary(done: number, total: number): string
  notificationTitle: string
  notificationBody(name: string): string
  replaceWarning: string
  hiddenReadyState(state: ReadyActionState): string
  historyKind(kind: ClipboardHistoryKind, fileCount: number | null): string
  historySource(source: ClipboardHistorySource): string
}

const english: Messages = {
  title: 'RelayClip',
  nearby: 'Nearby devices',
  pairedDevices: 'Paired devices',
  pairedDevicesCount: (count) => `${count} paired`,
  noPairedDevices: 'No paired devices yet',
  noPairedDevicesHint: 'Copies will fan out to every paired device.',
  noNearbyDevices: 'No other online devices found',
  nearbyDevicesHint: 'Devices on the same LAN appear here automatically.',
  pair: 'Pair',
  paired: 'Paired',
  unpair: 'Unpair',
  activeNow: 'Connected',
  online: 'Online',
  offline: 'Offline',
  transfers: 'Transfers',
  noTransfers: 'No file relays yet.',
  copyCenter: 'Copy center',
  noClipboardHistory: 'No clipboard history yet.',
  restoreHistory: 'Copy',
  openCacheDirectory: 'Open cache',
  readyToPaste: 'Ready to place on clipboard',
  placeOnClipboard: 'Place on clipboard',
  dismiss: 'Dismiss',
  cancel: 'Cancel',
  syncEnabled: 'Clipboard sync',
  saving: 'Saving...',
  unknownDevice: 'Unknown device',
  thisDevice: 'This device',
  clips: (count) => `${count} clip${count === 1 ? '' : 's'}`,
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
  historyKind: (kind, fileCount) =>
    ({
      text: 'Text',
      image: 'Image',
      fileRefs: fileCount && fileCount > 1 ? `${fileCount} files` : 'File',
    })[kind],
  historySource: (source) =>
    ({
      local: 'Local copy',
      remote: 'Received clip',
      transfer: 'Received files',
    })[source],
}

const chinese: Messages = {
  title: 'RelayClip',
  nearby: '附近设备',
  pairedDevices: '已配对设备',
  pairedDevicesCount: (count) => `已配对 ${count} 台`,
  noPairedDevices: '还没有已配对设备',
  noPairedDevicesHint: '复制内容会同步到所有已配对设备。',
  noNearbyDevices: '还没有发现其他在线设备',
  nearbyDevicesHint: '同一局域网中的设备会自动出现在这里。',
  pair: '配对',
  paired: '已配对',
  unpair: '取消配对',
  activeNow: '已连接',
  online: '在线',
  offline: '离线',
  transfers: '文件接力',
  noTransfers: '还没有文件接力任务。',
  copyCenter: '复制中心',
  noClipboardHistory: '还没有剪贴板历史。',
  restoreHistory: '复制',
  openCacheDirectory: '打开缓存',
  readyToPaste: '已下载完成，可放入剪贴板',
  placeOnClipboard: '放入剪贴板',
  dismiss: '忽略',
  cancel: '取消',
  syncEnabled: '剪贴板同步',
  saving: '保存中...',
  unknownDevice: '未知设备',
  thisDevice: '本机',
  clips: (count) => `${count} 条`,
  syncState: (state) =>
    ({
      idle: '空闲',
      discovering: '搜索中',
      connected: '已连接',
      syncing: '同步中',
      paused: '已暂停',
      error: '错误',
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
  historyKind: (kind, fileCount) =>
    ({
      text: '文本',
      image: '图片',
      fileRefs: fileCount && fileCount > 1 ? `${fileCount} 个文件` : '文件',
    })[kind],
  historySource: (source) =>
    ({
      local: '本机复制',
      remote: '远端接力',
      transfer: '接收文件',
    })[source],
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
