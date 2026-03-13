import type { AppLanguage, PayloadKind, SyncState } from './types'

export interface Messages {
  relayHub(platform: string): string
  overview: string
  trustedDeviceCount(count: number): string
  activePairing: string
  switchTarget: string
  chooseFirstDevice: string
  waitingNearbyDevices: string
  waitingNearbyDevicesDesc: string
  current: string
  onlineNow: string
  offline: string
  autoTrusted: string
  knownPeer: string
  status: string
  headline: string
  subheadline: string
  waitingForActivity: string
  clipboardEvent: string
  localDevice: string
  fingerprint: string
  lastPayload: string
  noRelayYet: string
  updated: string
  preferences: string
  systemBehavior: string
  language: string
  languageDescription: string
  deviceName: string
  deviceNameDescription: string
  launchOnLogin: string
  launchOnLoginDescription: string
  lanDiscovery: string
  lanDiscoveryDescription: string
  clipboardSync: string
  clipboardSyncDescription: string
  trustedPeers: string
  availableDevices: string
  onlineCount(count: number): string
  noAddressYet: string
  reachable: string
  selected: string
  setActive: string
  deviceDetail: string
  currentTarget: string
  platform: string
  capabilities: string
  host: string
  unknown: string
  port: string
  pending: string
  lastSeen: string
  lastSync: string
  noSyncHistoryYet: string
  routing: string
  pairThisDevice: string
  routingDescription: string
  alreadyActive: string
  makeActiveDevice: string
  backToOverview: string
  loadingTitle: string
  loadingSubtitle: string
  never: string
  payload(kind: PayloadKind, sizeKb: number): string
  statusState(state: SyncState): string
  kind(kind: PayloadKind): string
}

function english(): Messages {
  return {
    relayHub: (platform) => `${platform} relay hub`,
    overview: 'Overview',
    trustedDeviceCount: (count) => `${count} trusted device${count === 1 ? '' : 's'}`,
    activePairing: 'Active Pairing',
    switchTarget: 'Switch target',
    chooseFirstDevice: 'Choose your first device',
    waitingNearbyDevices: 'Waiting for nearby devices',
    waitingNearbyDevicesDesc:
      'RelayClip is advertising on your local network and will auto-trust private LAN peers.',
    current: 'Current',
    onlineNow: 'Online now',
    offline: 'Offline',
    autoTrusted: 'Auto trusted',
    knownPeer: 'Known peer',
    status: 'Status',
    headline: 'Clipboard relay across macOS and Windows',
    subheadline: 'Text and image payloads sync over local TLS with LAN auto discovery.',
    waitingForActivity: 'Waiting for activity',
    clipboardEvent: 'Clipboard event',
    localDevice: 'Local device',
    fingerprint: 'Fingerprint',
    lastPayload: 'Last payload',
    noRelayYet: 'No relay yet',
    updated: 'Updated',
    preferences: 'Preferences',
    systemBehavior: 'System behavior',
    language: 'Language',
    languageDescription: 'Choose the display language for the window and tray.',
    deviceName: 'Device name',
    deviceNameDescription: 'The name broadcast to nearby RelayClip peers.',
    launchOnLogin: 'Launch on login',
    launchOnLoginDescription: 'Register RelayClip with the OS startup hooks.',
    lanDiscovery: 'LAN discovery',
    lanDiscoveryDescription: 'Advertise and browse for `_relayclip._tcp.local` peers.',
    clipboardSync: 'Clipboard sync',
    clipboardSyncDescription: 'Pause or resume all outbound and inbound relay traffic.',
    trustedPeers: 'Trusted peers',
    availableDevices: 'Available devices',
    onlineCount: (count) => `${count} online`,
    noAddressYet: 'No address yet',
    reachable: 'Reachable',
    selected: 'Selected',
    setActive: 'Set active',
    deviceDetail: 'Device detail',
    currentTarget: 'Current target',
    platform: 'Platform',
    capabilities: 'Capabilities',
    host: 'Host',
    unknown: 'Unknown',
    port: 'Port',
    pending: 'Pending',
    lastSeen: 'Last seen',
    lastSync: 'Last sync',
    noSyncHistoryYet: 'No sync history yet',
    routing: 'Routing',
    pairThisDevice: 'Pair this device',
    routingDescription:
      'RelayClip only pushes new clipboard changes to one active peer at a time. Inbound sync remains bidirectional so this device can still send updates back.',
    alreadyActive: 'Already active',
    makeActiveDevice: 'Make active device',
    backToOverview: 'Back to overview',
    loadingTitle: 'RelayClip is starting',
    loadingSubtitle: 'Spinning up the local transport, tray menu, and LAN discovery services.',
    never: 'Never',
    payload: (kind, sizeKb) => `${kind === 'text' ? 'text' : 'image'} · ${sizeKb} KB`,
    statusState: (state) =>
      ({
        idle: 'Idle',
        discovering: 'Discovering',
        connected: 'Connected',
        syncing: 'Syncing',
        paused: 'Paused',
        error: 'Error',
      })[state],
    kind: (kind) => (kind === 'text' ? 'text' : 'image'),
  }
}

function chinese(): Messages {
  return {
    relayHub: (platform) => `${platform} 接力中心`,
    overview: '总览',
    trustedDeviceCount: (count) => `${count} 台可信设备`,
    activePairing: '当前配对',
    switchTarget: '切换接力目标',
    chooseFirstDevice: '先选择第一台设备',
    waitingNearbyDevices: '正在等待附近设备',
    waitingNearbyDevicesDesc: 'RelayClip 正在局域网内广播，并会自动信任私有网络中的设备。',
    current: '当前',
    onlineNow: '在线',
    offline: '离线',
    autoTrusted: '自动信任',
    knownPeer: '已知设备',
    status: '状态',
    headline: '在 macOS 和 Windows 之间接力剪贴板',
    subheadline: '文本和图片内容会通过本地 TLS 与局域网自动发现进行同步。',
    waitingForActivity: '等待新的同步活动',
    clipboardEvent: '剪贴板事件',
    localDevice: '本机设备',
    fingerprint: '指纹',
    lastPayload: '最近内容',
    noRelayYet: '还没有接力记录',
    updated: '更新时间',
    preferences: '偏好设置',
    systemBehavior: '系统行为',
    language: '语言',
    languageDescription: '设置窗口和托盘菜单的显示语言。',
    deviceName: '设备名称',
    deviceNameDescription: '向附近 RelayClip 设备广播的名字。',
    launchOnLogin: '开机启动',
    launchOnLoginDescription: '把 RelayClip 注册到系统启动项。',
    lanDiscovery: '局域网发现',
    lanDiscoveryDescription: '广播并发现 `_relayclip._tcp.local` 设备。',
    clipboardSync: '剪贴板同步',
    clipboardSyncDescription: '暂停或恢复所有入站与出站接力流量。',
    trustedPeers: '可信设备',
    availableDevices: '可用设备',
    onlineCount: (count) => `${count} 台在线`,
    noAddressYet: '还没有地址',
    reachable: '可连接',
    selected: '已选择',
    setActive: '设为当前',
    deviceDetail: '设备详情',
    currentTarget: '当前目标',
    platform: '平台',
    capabilities: '能力',
    host: '主机',
    unknown: '未知',
    port: '端口',
    pending: '待定',
    lastSeen: '最近在线',
    lastSync: '最近同步',
    noSyncHistoryYet: '还没有同步历史',
    routing: '路由',
    pairThisDevice: '配对这台设备',
    routingDescription:
      'RelayClip 一次只会把新的剪贴板变化推送给一台活动设备，但入站同步仍保持双向，因此这台设备仍可以把内容回传给你。',
    alreadyActive: '已是当前设备',
    makeActiveDevice: '设为当前设备',
    backToOverview: '返回总览',
    loadingTitle: 'RelayClip 正在启动',
    loadingSubtitle: '正在初始化本地传输、托盘菜单与局域网发现服务。',
    never: '从未',
    payload: (kind, sizeKb) => `${kind === 'text' ? '文本' : '图片'} · ${sizeKb} KB`,
    statusState: (state) =>
      ({
        idle: '空闲',
        discovering: '发现中',
        connected: '已连接',
        syncing: '同步中',
        paused: '已暂停',
        error: '错误',
      })[state],
    kind: (kind) => (kind === 'text' ? '文本' : '图片'),
  }
}

export function normalizeLanguage(input?: string | null): AppLanguage {
  if (input?.toLowerCase().startsWith('zh')) {
    return 'zh-CN'
  }

  return 'en'
}

export function languageLabel(language: AppLanguage) {
  return language === 'zh-CN' ? '中文' : 'English'
}

export function getMessages(language: AppLanguage): Messages {
  return language === 'zh-CN' ? chinese() : english()
}
