import type {
  AppLanguage,
  BackgroundSyncMode,
  ClipboardHistoryKind,
  ClipboardHistorySource,
  ReadyActionState,
  RuntimePermissionState,
  RuntimePlatform,
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
  shareExternally: string
  exportToFiles: string
  dismiss: string
  cancel: string
  syncEnabled: string
  launchOnLogin: string
  launchOnLoginHint: string
  backgroundSync: string
  backgroundSyncHint: string
  environment: string
  permissions: string
  requestPermissions: string
  permissionsHint: string
  notificationsPermission: string
  localNetworkPermission: string
  clipboardPermission: string
  fileAccessPermission: string
  backgroundSyncPermission: string
  actionsUnavailable: string
  mobileLimitedHint: string
  saving: string
  syncState(state: SyncState): string
  runtimePlatform(platform: RuntimePlatform): string
  permissionState(state: RuntimePermissionState): string
  backgroundMode(mode: BackgroundSyncMode, active: boolean): string
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
  restoreHistory: 'Copy',
  openCacheDirectory: 'Open cache',
  readyToPaste: 'Ready to place on clipboard',
  placeOnClipboard: 'Place on clipboard',
  shareExternally: 'Share',
  exportToFiles: 'Export',
  dismiss: 'Dismiss',
  cancel: 'Cancel',
  syncEnabled: 'Clipboard sync',
  launchOnLogin: 'Launch on login',
  launchOnLoginHint: 'Desktop only. Keeps RelayClip ready after sign-in.',
  backgroundSync: 'Background sync',
  backgroundSyncHint: 'Mobile only. The actual runtime behavior depends on platform policy.',
  environment: 'Runtime',
  permissions: 'Permissions',
  requestPermissions: 'Grant access',
  permissionsHint: 'Review platform permissions before expecting background discovery or notifications.',
  notificationsPermission: 'Notifications',
  localNetworkPermission: 'Local network',
  clipboardPermission: 'Clipboard',
  fileAccessPermission: 'Files',
  backgroundSyncPermission: 'Background sync',
  actionsUnavailable: 'No supported action is available for this platform yet.',
  mobileLimitedHint: 'Mobile builds show only the actions that are currently available on the running platform.',
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
  runtimePlatform: (platform) =>
    ({
      windows: 'Windows',
      macos: 'macOS',
      linux: 'Linux',
      android: 'Android',
      ios: 'iOS',
      unknown: 'Unknown',
    })[platform],
  permissionState: (state) =>
    ({
      granted: 'Granted',
      denied: 'Denied',
      prompt: 'Needs prompt',
      unsupported: 'Unsupported',
    })[state],
  backgroundMode: (mode, active) => {
    const label =
      ({
        desktop: 'Desktop runtime',
        foregroundOnly: 'Foreground only',
        foregroundService: 'Foreground service',
        appRefresh: 'App refresh',
        unsupported: 'Unsupported',
      })[mode]
    return active ? `${label} active` : label
  },
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
  notificationBody: (name) => `${name} is ready for the next action.`,
  replaceWarning: 'This replaces the current clipboard content.',
  hiddenReadyState: (state) =>
    ({
      pendingPrompt: 'Pending',
      dismissed: 'Dismissed',
      placed: 'Handled',
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
  noActivePairHint: '下方出现在线设备后可以直接配对。',
  makeActive: '配对',
  activeNow: '已连接',
  online: '在线',
  offline: '离线',
  transfers: '文件接力',
  noTransfers: '还没有文件接力任务。',
  clipboardHistory: '剪贴板历史',
  noClipboardHistory: '还没有剪贴板历史。',
  restoreHistory: '复制',
  openCacheDirectory: '打开缓存',
  readyToPaste: '可以放入剪贴板',
  placeOnClipboard: '放入剪贴板',
  shareExternally: '系统分享',
  exportToFiles: '导出文件',
  dismiss: '忽略',
  cancel: '取消',
  syncEnabled: '剪贴板同步',
  launchOnLogin: '开机启动',
  launchOnLoginHint: '仅桌面端可用，登录后自动准备好 RelayClip。',
  backgroundSync: '后台同步',
  backgroundSyncHint: '仅移动端可用，具体表现会受系统策略限制。',
  environment: '运行环境',
  permissions: '权限',
  requestPermissions: '申请权限',
  permissionsHint: '移动端的发现、通知和后台同步都依赖系统授权。',
  notificationsPermission: '通知',
  localNetworkPermission: '本地网络',
  clipboardPermission: '剪贴板',
  fileAccessPermission: '文件访问',
  backgroundSyncPermission: '后台同步',
  actionsUnavailable: '当前平台还没有可执行的后续动作。',
  mobileLimitedHint: '移动端会只显示当前平台真正可用的动作。',
  saving: '保存中...',
  syncState: (state) =>
    ({
      idle: '空闲',
      discovering: '发现中',
      connected: '已连接',
      syncing: '同步中',
      paused: '已暂停',
      error: '异常',
    })[state],
  runtimePlatform: (platform) =>
    ({
      windows: 'Windows',
      macos: 'macOS',
      linux: 'Linux',
      android: 'Android',
      ios: 'iOS',
      unknown: '未知平台',
    })[platform],
  permissionState: (state) =>
    ({
      granted: '已授权',
      denied: '已拒绝',
      prompt: '待授权',
      unsupported: '不支持',
    })[state],
  backgroundMode: (mode, active) => {
    const label =
      ({
        desktop: '桌面常驻',
        foregroundOnly: '仅前台',
        foregroundService: '前台服务',
        appRefresh: '应用刷新',
        unsupported: '不支持',
      })[mode]
    return active ? `${label}（活跃）` : label
  },
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
  notificationTitle: '文件接力完成',
  notificationBody: (name) => `${name} 已经可以继续处理。`,
  replaceWarning: '这会替换当前剪贴板内容。',
  hiddenReadyState: (state) =>
    ({
      pendingPrompt: '待处理',
      dismissed: '已忽略',
      placed: '已处理',
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
