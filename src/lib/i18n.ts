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
  clipboardHistory: string
  noClipboardHistory: string
  restoreHistory: string
  openCacheDirectory: string
  readyToPaste: string
  placeOnClipboard: string
  dismiss: string
  cancel: string
  syncEnabled: string
  saving: string
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
  clipboardHistory: 'Clipboard history',
  noClipboardHistory: 'No clipboard history yet.',
  restoreHistory: 'Copy again',
  openCacheDirectory: 'Open cache',
  readyToPaste: 'Ready to place on clipboard',
  placeOnClipboard: 'Place on clipboard',
  dismiss: 'Dismiss',
  cancel: 'Cancel',
  syncEnabled: 'Clipboard sync',
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
  clipboardHistory: '剪贴板历史',
  noClipboardHistory: '还没有剪贴板历史。',
  restoreHistory: '再次复制',
  openCacheDirectory: '打开缓存目录',
  readyToPaste: '已下载完成，可放入剪贴板',
  placeOnClipboard: '放入剪贴板',
  dismiss: '忽略',
  cancel: '取消',
  syncEnabled: '剪贴板同步',
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
