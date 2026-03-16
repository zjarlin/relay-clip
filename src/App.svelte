<script lang="ts">
  import { onMount } from 'svelte'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
  } from '$lib/components/ui/card'
  import { Input } from '$lib/components/ui/input'
  import { Progress } from '$lib/components/ui/progress'
  import { Switch } from '$lib/components/ui/switch'
  import { cn } from '$lib/utils'
  import {
    cancelTransferJob,
    dismissTransferJob,
    exportReceivedTransfer,
    getAppState,
    getBackgroundSyncState,
    isAutostartEnabled,
    listClipboardHistory,
    listTransferJobs,
    onClipboardError,
    onClipboardHistoryUpdated,
    onDevicesUpdated,
    onSyncStatusChanged,
    onTransferJobsUpdated,
    onTransferReady,
    openCacheDirectory,
    placeReceivedTransferOnClipboard,
    requestRuntimePermissions,
    restoreClipboardHistoryEntry,
    setDevicePairing,
    shareReceivedTransfer,
    syncAutostart,
    toggleSync,
    updateSettings,
  } from './lib/api'
  import { getMessages, normalizeLanguage } from './lib/i18n'
  import type {
    AppStateSnapshot,
    BackgroundSyncState,
    ClipboardHistoryEntry,
    RuntimeCapabilities,
    RuntimePermissions,
    RuntimePermissionState,
    TransferAction,
    TransferJob,
  } from './lib/types'
  import {
    AlertCircle,
    Check,
    Download,
    Laptop2,
    Link2,
    MessageCircle,
    RotateCcw,
    Settings,
    Share2,
    Smartphone,
    Wifi,
    WifiOff,
    X,
  } from '@lucide/svelte'

  interface HistoryGroup {
    deviceId: string
    deviceName: string
    isLocal: boolean
    entries: ClipboardHistoryEntry[]
  }

  const fallbackCapabilities: RuntimeCapabilities = {
    tray: false,
    autostart: false,
    clipboardMonitor: false,
    clipboardFiles: false,
    openCacheDirectory: false,
    shareExternally: false,
    exportToFiles: false,
    backgroundSync: false,
    nativeDiscovery: false,
  }

  const fallbackPermissions: RuntimePermissions = {
    notifications: 'unsupported',
    localNetwork: 'unsupported',
    clipboard: 'unsupported',
    backgroundSync: 'unsupported',
    fileAccess: 'unsupported',
  }

  let snapshot: AppStateSnapshot | null = null
  let transferJobs: TransferJob[] = []
  let clipboardHistory: ClipboardHistoryEntry[] = []
  let backgroundSyncState: BackgroundSyncState | null = null
  let busyTransferId: string | null = null
  let busyHistoryId: string | null = null
  let busyPairingId: string | null = null
  let settingsBusy = false
  let permissionsBusy = false
  let errorBanner: string | null = null
  let activeTab: 'devices' | 'history' | 'settings' = 'devices'
  const notifiedReady = new Set<string>()

  $: currentLanguage =
    snapshot?.settings.language ??
    normalizeLanguage(typeof navigator === 'undefined' ? undefined : navigator.language)
  $: copy = getMessages(currentLanguage)
  $: devices = snapshot?.devices ?? []
  $: localDevice = snapshot?.localDevice ?? null
  $: pairedDevices = devices.filter((device) => device.isActive)
  $: nearbyDevices = devices.filter(
    (device) =>
      !device.isActive &&
      device.isOnline &&
      device.deviceId !== (localDevice?.deviceId ?? ''),
  )
  $: visibleTransferJobs = transferJobs
  $: syncStatus = snapshot?.syncStatus
  $: historyGroups = groupHistory(
    clipboardHistory,
    localDevice?.deviceId ?? '',
    localDevice?.deviceName ?? copy.thisDevice,
  )
  $: runtimePlatform = snapshot?.runtimePlatform ?? 'unknown'
  $: runtimeCapabilities = snapshot?.capabilities ?? fallbackCapabilities
  $: runtimePermissions = snapshot?.permissions ?? fallbackPermissions
  $: isMobile = runtimePlatform === 'android' || runtimePlatform === 'ios'
  $: onlinePeerCount = devices.filter((device) => device.isOnline).length
  $: pairedOnlineCount = pairedDevices.filter((device) => device.isOnline).length
  $: permissionsNeedAttention = [
    runtimePermissions.notifications,
    runtimePermissions.localNetwork,
    runtimePermissions.clipboard,
    runtimePermissions.fileAccess,
    runtimePermissions.backgroundSync,
  ].some((state) => ['prompt', 'denied'].includes(state))
  $: permissionRows = [
    { label: copy.notificationsPermission, state: runtimePermissions.notifications },
    { label: copy.localNetworkPermission, state: runtimePermissions.localNetwork },
    { label: copy.clipboardPermission, state: runtimePermissions.clipboard },
    { label: copy.fileAccessPermission, state: runtimePermissions.fileAccess },
    { label: copy.backgroundSyncPermission, state: runtimePermissions.backgroundSync },
  ]
  $: ui =
    currentLanguage === 'zh-CN'
      ? {
          currentDevice: '当前设备',
          currentDeviceHint: '查看本机身份、连接状态和在线设备概况。',
          availableHint: '单个在线设备会自动配对；有多台设备时仍可手动选择。',
          transferHint: '文件接力完成后可以继续复制、分享或导出。',
          historyHint: '按来源设备分组，方便继续接力最近的内容。',
          settingsHint: '桌面与移动端现在共用同一套界面风格和交互。',
          runtimeHint: '查看当前平台能力、后台同步模式和缓存入口。',
          onlinePeers: '在线对端',
          readyPeers: '已连接',
          protocol: '协议',
          deviceName: '设备名称',
          settingsTab: '设置',
          dismissError: '关闭提示',
        }
      : {
          currentDevice: 'Current device',
          currentDeviceHint: 'See your local identity, sync state, and live peers at a glance.',
          availableHint:
            'A lone online peer now auto-pairs. Manual pairing still applies when multiple peers are available.',
          transferHint: 'Finished relays can be copied, shared, or exported from the same surface.',
          historyHint: 'Recent clipboard items stay grouped by source device for quick handoff.',
          settingsHint: 'Desktop and mobile now share the same surface and interaction style.',
          runtimeHint: 'Inspect platform capabilities, background mode, and cache access here.',
          onlinePeers: 'Online peers',
          readyPeers: 'Ready links',
          protocol: 'Protocol',
          deviceName: 'Device name',
          settingsTab: 'Settings',
          dismissError: 'Dismiss',
        }

  async function refresh() {
    const [state, jobs, history, backgroundState] = await Promise.all([
      getAppState(),
      listTransferJobs(),
      listClipboardHistory(),
      getBackgroundSyncState().catch(() => null),
    ])

    snapshot = state
    transferJobs = jobs
    clipboardHistory = history
    backgroundSyncState = backgroundState

    try {
      if (state.capabilities.autostart) {
        let shouldPersistAutostart = state.settings.launchOnLogin !== true
        const autostartEnabled = await isAutostartEnabled().catch(
          () => state.settings.launchOnLogin,
        )
        if (!autostartEnabled) {
          await syncAutostart(true)
          shouldPersistAutostart = true
        }
        if (shouldPersistAutostart) {
          snapshot = await updateSettings({ launchOnLogin: true })
        }
      }
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    }
  }

  async function patchSettings(patch: Parameters<typeof updateSettings>[0]) {
    if (!snapshot) return
    settingsBusy = true
    errorBanner = null

    try {
      snapshot = await updateSettings(patch)
      backgroundSyncState = await getBackgroundSyncState().catch(() => backgroundSyncState)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      settingsBusy = false
    }
  }

  async function updatePairing(deviceId: string, paired: boolean) {
    busyPairingId = deviceId
    errorBanner = null
    try {
      snapshot = await setDevicePairing(deviceId, paired)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      busyPairingId = null
    }
  }

  async function refreshPermissions() {
    permissionsBusy = true
    errorBanner = null

    try {
      const permissions = await requestRuntimePermissions()
      if (snapshot) {
        snapshot = { ...snapshot, permissions }
      }
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      permissionsBusy = false
    }
  }

  async function toggleClipboardSync(enabled: boolean) {
    errorBanner = null
    try {
      snapshot = await toggleSync(enabled)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    }
  }

  async function placeTransfer(job: TransferJob) {
    busyTransferId = job.transferId
    errorBanner = null
    try {
      await placeReceivedTransferOnClipboard(job.transferId)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      busyTransferId = null
    }
  }

  async function shareTransfer(job: TransferJob) {
    busyTransferId = job.transferId
    errorBanner = null
    try {
      await shareReceivedTransfer(job.transferId)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      busyTransferId = null
    }
  }

  async function exportTransfer(job: TransferJob) {
    busyTransferId = job.transferId
    errorBanner = null
    try {
      await exportReceivedTransfer(job.transferId)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      busyTransferId = null
    }
  }

  async function dismissTransfer(job: TransferJob) {
    busyTransferId = job.transferId
    errorBanner = null
    try {
      await dismissTransferJob(job.transferId)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      busyTransferId = null
    }
  }

  async function cancelTransfer(job: TransferJob) {
    busyTransferId = job.transferId
    errorBanner = null
    try {
      await cancelTransferJob(job.transferId)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      busyTransferId = null
    }
  }

  async function restoreHistory(entry: ClipboardHistoryEntry) {
    busyHistoryId = entry.entryId
    errorBanner = null
    try {
      await restoreClipboardHistoryEntry(entry.entryId)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      busyHistoryId = null
    }
  }

  async function openCache() {
    errorBanner = null
    try {
      await openCacheDirectory()
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    }
  }

  async function notifyTransferReady(job: TransferJob) {
    if (notifiedReady.has(job.transferId)) return
    notifiedReady.add(job.transferId)

    // 移动端暂不支持通知，跳过
    if (isMobile) return

    try {
      const { isPermissionGranted, requestPermission, sendNotification } = await import('@tauri-apps/plugin-notification')
      let permissionGranted = await isPermissionGranted()
      if (!permissionGranted) {
        const permission = await requestPermission()
        permissionGranted = permission === 'granted'
      }

      if (permissionGranted) {
        await sendNotification({
          title: copy.notificationTitle,
          body: copy.notificationBody(job.displayName),
        })
      }
    } catch {
      // 通知插件不可用，静默失败
    }
  }

  function groupHistory(
    entries: ClipboardHistoryEntry[],
    localDeviceId: string,
    localDeviceName: string,
  ): HistoryGroup[] {
    const groups = new Map<string, HistoryGroup>()

    for (const entry of entries) {
      const isLocal = entry.originDeviceId === localDeviceId || entry.source === 'local'
      const deviceId =
        entry.originDeviceId || (isLocal ? localDeviceId : `unknown:${entry.entryId}`)
      const deviceName =
        entry.originDeviceName || (isLocal ? localDeviceName : copy.unknownDevice)

      if (!groups.has(deviceId)) {
        groups.set(deviceId, {
          deviceId,
          deviceName,
          isLocal,
          entries: [],
        })
      }

      groups.get(deviceId)?.entries.push(entry)
    }

    return Array.from(groups.values()).sort((left, right) => {
      if (left.isLocal !== right.isLocal) {
        return left.isLocal ? -1 : 1
      }

      const leftTime = left.entries[0]?.createdAt ?? ''
      const rightTime = right.entries[0]?.createdAt ?? ''
      return rightTime.localeCompare(leftTime)
    })
  }

  function isMobilePlatform(platform: string) {
    const normalized = platform.toLowerCase()
    return (
      normalized.includes('ios') ||
      normalized.includes('iphone') ||
      normalized.includes('ipad') ||
      normalized.includes('android') ||
      normalized.includes('mobile')
    )
  }

  function platformLabel(platform: string) {
    if (!platform) return copy.unknownDevice
    return platform
  }

  function formatBytes(value: number) {
    if (value <= 0) return '0 B'
    const units = ['B', 'KB', 'MB', 'GB', 'TB']
    let size = value
    let unitIndex = 0
    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024
      unitIndex += 1
    }
    return `${size.toFixed(size >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`
  }

  function progress(job: TransferJob) {
    if (job.totalBytes > 0) {
      return Math.min(100, Math.round((job.completedBytes / job.totalBytes) * 100))
    }
    if (job.totalEntries > 0) {
      return Math.min(100, Math.round((job.completedEntries / job.totalEntries) * 100))
    }
    return job.stage === 'ready' ? 100 : 0
  }

  function formatTime(value: string) {
    const date = new Date(value)
    const now = new Date()
    const diff = now.getTime() - date.getTime()
    const days = Math.floor(diff / (1000 * 60 * 60 * 24))

    if (days === 0) {
      return date.toLocaleTimeString(currentLanguage === 'zh-CN' ? 'zh-CN' : 'en-US', {
        hour: '2-digit',
        minute: '2-digit',
      })
    } else if (days === 1) {
      return '昨天'
    } else if (days < 7) {
      return ['周日', '周一', '周二', '周三', '周四', '周五', '周六'][date.getDay()]
    } else {
      return date.toLocaleDateString(currentLanguage === 'zh-CN' ? 'zh-CN' : 'en-US', {
        month: 'numeric',
        day: 'numeric',
      })
    }
  }

  function historyPreview(entry: ClipboardHistoryEntry) {
    if (entry.previewText) return entry.previewText
    if (entry.topLevelNames.length > 0) return entry.topLevelNames.join(', ')
    return formatBytes(entry.size)
  }

  function hasAction(job: TransferJob, action: TransferAction) {
    return job.availableActions.includes(action)
  }

  function syncBadgeClass(state: string | undefined) {
    if (state === 'connected') {
      return 'border-emerald-200 bg-emerald-50 text-emerald-700'
    }
    if (state === 'syncing') {
      return 'border-sky-200 bg-sky-50 text-sky-700'
    }
    if (state === 'error') {
      return 'border-destructive/30 bg-destructive/10 text-destructive'
    }
    if (state === 'paused') {
      return 'border-amber-200 bg-amber-50 text-amber-700'
    }
    return 'border-border bg-muted text-muted-foreground'
  }

  function permissionBadgeClass(state: RuntimePermissionState) {
    if (state === 'granted') {
      return 'border-emerald-200 bg-emerald-50 text-emerald-700'
    }
    if (state === 'denied') {
      return 'border-destructive/30 bg-destructive/10 text-destructive'
    }
    if (state === 'prompt') {
      return 'border-amber-200 bg-amber-50 text-amber-700'
    }
    return 'border-border bg-muted text-muted-foreground'
  }

  function presenceBadgeClass(isOnline: boolean) {
    return isOnline
      ? 'border-emerald-200 bg-emerald-50 text-emerald-700'
      : 'border-border bg-muted text-muted-foreground'
  }

  function platformIconComponent(platform: string) {
    return isMobilePlatform(platform) ? Smartphone : Laptop2
  }

  function inputValueFromEvent(event: Event) {
    return (event.currentTarget as HTMLInputElement | null)?.value ?? ''
  }

  function checkedFromEvent(event: Event) {
    return (event.currentTarget as HTMLInputElement | null)?.checked ?? false
  }

  onMount(() => {
    let disposed = false
    void refresh()

    const unlisten = Promise.all([
      onDevicesUpdated((devices) => {
        if (!snapshot || disposed) return
        snapshot = { ...snapshot, devices }
      }),
      onSyncStatusChanged((status) => {
        if (!snapshot || disposed) return
        snapshot = { ...snapshot, syncStatus: status }
      }),
      onClipboardError((message) => {
        if (!disposed) errorBanner = message
      }),
      onClipboardHistoryUpdated((entries) => {
        if (!disposed) clipboardHistory = entries
      }),
      onTransferJobsUpdated((jobs) => {
        if (!disposed) transferJobs = jobs
      }),
      onTransferReady((job) => {
        if (!disposed) void notifyTransferReady(job)
      }),
    ])

    return () => {
      disposed = true
      void unlisten.then((callbacks) => callbacks.forEach((dispose) => dispose()))
    }
  })
</script>

{#if snapshot}
  <div class="min-h-[100dvh] bg-[radial-gradient(circle_at_top,rgba(15,23,42,0.05),transparent_40%),linear-gradient(180deg,rgba(248,250,252,0.96),rgba(241,245,249,0.88))]">
    <div class="mx-auto flex min-h-[100dvh] w-full max-w-6xl flex-col px-4 pb-[calc(1.5rem+env(safe-area-inset-bottom))] pt-4 md:px-6 lg:px-8">
      <section class="sticky top-0 z-20 -mx-4 border-b border-border/60 bg-background/92 px-4 pb-4 pt-[max(env(safe-area-inset-top),1rem)] backdrop-blur md:mx-0 md:rounded-[28px] md:border md:px-6 md:shadow-sm lg:static lg:mb-6">
        <div class="flex flex-col gap-5 lg:flex-row lg:items-center lg:justify-between">
          <div class="space-y-3">
            <div class="flex items-center gap-4">
              <div class="flex h-12 w-12 items-center justify-center rounded-2xl bg-primary text-primary-foreground shadow-sm">
                <Link2 size={22} />
              </div>
              <div class="space-y-1">
                <p class="text-sm font-medium text-muted-foreground">{copy.title}</p>
                <h1 class="text-2xl font-semibold tracking-tight">{snapshot.settings.deviceName}</h1>
              </div>
            </div>
            <p class="max-w-2xl text-sm leading-6 text-muted-foreground">
              {syncStatus?.message ?? ui.currentDeviceHint}
            </p>
          </div>

          <div class="grid gap-3 sm:grid-cols-2 lg:min-w-[420px]">
            <div class="rounded-2xl border border-border/70 bg-card px-4 py-3 shadow-sm">
              <div class="flex items-center justify-between gap-3">
                <div class="space-y-1">
                  <p class="text-xs uppercase tracking-[0.24em] text-muted-foreground">
                    {copy.syncState(syncStatus?.state ?? 'discovering')}
                  </p>
                  <p class="text-sm font-medium text-foreground">
                    {syncStatus?.lastPayload
                      ? `${syncStatus.lastPayload.kind} · ${formatBytes(syncStatus.lastPayload.size)}`
                      : ui.currentDeviceHint}
                  </p>
                </div>
                <Badge
                  variant="outline"
                  class={cn(
                    'rounded-full px-3 py-1 text-xs font-medium',
                    syncBadgeClass(syncStatus?.state),
                  )}
                >
                  {syncStatus?.state === 'syncing' ? copy.syncState('syncing') : copy.syncState(syncStatus?.state ?? 'idle')}
                </Badge>
              </div>
            </div>

            <div class="flex items-center justify-between gap-4 rounded-2xl border border-border/70 bg-card px-4 py-3 shadow-sm">
              <div class="space-y-1">
                <p class="text-sm font-medium text-foreground">{copy.syncEnabled}</p>
                <p class="text-xs leading-5 text-muted-foreground">{copy.noPairedDevicesHint}</p>
              </div>
              <Switch
                checked={snapshot.settings.syncEnabled}
                disabled={settingsBusy}
                on:change={(event) => toggleClipboardSync(checkedFromEvent(event))}
              />
            </div>
          </div>
        </div>

        <div class="mt-5 grid grid-cols-3 gap-2 rounded-2xl border border-border/60 bg-muted/50 p-1">
          <Button
            variant={activeTab === 'devices' ? 'secondary' : 'ghost'}
            class="h-11 justify-center rounded-xl text-sm"
            on:click={() => (activeTab = 'devices')}
          >
            <Link2 size={16} />
            <span class="hidden sm:inline">{copy.nearby}</span>
            <span class="sm:hidden">{currentLanguage === 'zh-CN' ? '设备' : 'Peers'}</span>
          </Button>
          <Button
            variant={activeTab === 'history' ? 'secondary' : 'ghost'}
            class="h-11 justify-center rounded-xl text-sm"
            on:click={() => (activeTab = 'history')}
          >
            <MessageCircle size={16} />
            <span>{copy.copyCenter}</span>
          </Button>
          <Button
            variant={activeTab === 'settings' ? 'secondary' : 'ghost'}
            class="h-11 justify-center rounded-xl text-sm"
            on:click={() => (activeTab = 'settings')}
          >
            <Settings size={16} />
            <span>{ui.settingsTab}</span>
          </Button>
        </div>
      </section>

      {#if errorBanner}
        <Card class="mt-4 border-destructive/30 bg-destructive/5 shadow-sm">
          <CardContent class="flex items-start justify-between gap-3 p-4">
            <div class="flex items-start gap-3 text-destructive">
              <AlertCircle class="mt-0.5 shrink-0" size={18} />
              <p class="text-sm leading-6">{errorBanner}</p>
            </div>
            <Button
              variant="ghost"
              size="icon"
              class="h-8 w-8 shrink-0 text-destructive hover:bg-destructive/10 hover:text-destructive"
              on:click={() => (errorBanner = null)}
              aria-label={ui.dismissError}
            >
              <X size={16} />
            </Button>
          </CardContent>
        </Card>
      {/if}

      {#if activeTab === 'devices'}
        <div class="mt-4 grid gap-4 xl:grid-cols-[1.15fr,0.85fr]">
          <div class="space-y-4">
            <Card class="border-border/70 shadow-sm">
              <CardHeader class="pb-2">
                <div class="flex items-start justify-between gap-3">
                  <div>
                    <CardTitle>{ui.currentDevice}</CardTitle>
                    <CardDescription>{ui.currentDeviceHint}</CardDescription>
                  </div>
                  <Badge variant="secondary" class="rounded-full px-3 py-1 text-xs">
                    {copy.thisDevice}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent class="space-y-4">
                <div class="flex items-center gap-4 rounded-2xl border border-border/70 bg-muted/40 p-4">
                  <div class="flex h-12 w-12 items-center justify-center rounded-2xl bg-primary/10 text-primary">
                    {#if isMobilePlatform(localDevice?.platform ?? '')}
                      <Smartphone size={22} />
                    {:else}
                      <Laptop2 size={22} />
                    {/if}
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="truncate text-base font-semibold">{snapshot.settings.deviceName}</div>
                    <div class="mt-1 text-sm text-muted-foreground">
                      {platformLabel(localDevice?.platform ?? '')}
                    </div>
                  </div>
                  <Badge
                    variant="outline"
                    class="rounded-full border-emerald-200 bg-emerald-50 px-3 py-1 text-xs font-medium text-emerald-700"
                  >
                    {copy.online}
                  </Badge>
                </div>

                <div class="grid gap-3 sm:grid-cols-3">
                  <div class="rounded-2xl bg-muted/60 p-3">
                    <div class="text-xs uppercase tracking-wide text-muted-foreground">{ui.onlinePeers}</div>
                    <div class="mt-2 text-lg font-semibold">{onlinePeerCount}</div>
                  </div>
                  <div class="rounded-2xl bg-muted/60 p-3">
                    <div class="text-xs uppercase tracking-wide text-muted-foreground">{ui.readyPeers}</div>
                    <div class="mt-2 text-lg font-semibold">{pairedOnlineCount}</div>
                  </div>
                  <div class="rounded-2xl bg-muted/60 p-3">
                    <div class="text-xs uppercase tracking-wide text-muted-foreground">{ui.protocol}</div>
                    <div class="mt-2 text-sm font-semibold">{localDevice?.protocolVersion}</div>
                  </div>
                </div>
              </CardContent>
            </Card>

            <Card class="border-border/70 shadow-sm">
              <CardHeader class="pb-2">
                <div class="flex items-start justify-between gap-3">
                  <div>
                    <CardTitle>{copy.pairedDevices}</CardTitle>
                    <CardDescription>{copy.noPairedDevicesHint}</CardDescription>
                  </div>
                  <Badge variant="outline" class="rounded-full px-3 py-1 text-xs">
                    {copy.pairedDevicesCount(pairedDevices.length)}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent class="space-y-3">
                {#if pairedDevices.length === 0}
                  <div class="rounded-2xl border border-dashed border-border bg-muted/30 px-4 py-10 text-center">
                    <WifiOff class="mx-auto mb-3 text-muted-foreground" size={28} />
                    <p class="text-sm font-medium">{copy.noPairedDevices}</p>
                    <p class="mt-2 text-sm text-muted-foreground">{copy.noPairedDevicesHint}</p>
                  </div>
                {:else}
                  {#each pairedDevices as device}
                    <article class="flex flex-col gap-4 rounded-2xl border border-border/70 bg-muted/30 p-4 sm:flex-row sm:items-center sm:justify-between">
                      <div class="flex min-w-0 items-start gap-4">
                        <div
                          class={cn(
                            'flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl border',
                            device.isOnline
                              ? 'border-primary/15 bg-primary/10 text-primary'
                              : 'border-border bg-muted text-muted-foreground',
                          )}
                        >
                          <svelte:component this={platformIconComponent(device.platform)} size={20} />
                        </div>
                        <div class="min-w-0 space-y-1">
                          <div class="truncate text-sm font-semibold">{device.name}</div>
                          <div class="text-sm text-muted-foreground">
                            {platformLabel(device.platform)}
                          </div>
                        </div>
                      </div>

                      <div class="flex flex-wrap items-center gap-2">
                        <Badge variant="secondary" class="rounded-full px-3 py-1 text-xs">
                          {copy.paired}
                        </Badge>
                        <Badge
                          variant="outline"
                          class={cn(
                            'rounded-full px-3 py-1 text-xs font-medium',
                            presenceBadgeClass(device.isOnline),
                          )}
                        >
                          {device.isOnline ? copy.online : copy.offline}
                        </Badge>
                        <Button
                          variant="outline"
                          size="sm"
                          disabled={busyPairingId === device.deviceId}
                          on:click={() => updatePairing(device.deviceId, false)}
                        >
                          {busyPairingId === device.deviceId ? copy.saving : copy.unpair}
                        </Button>
                      </div>
                    </article>
                  {/each}
                {/if}
              </CardContent>
            </Card>

            <Card class="border-border/70 shadow-sm">
              <CardHeader class="pb-2">
                <CardTitle>{copy.nearby}</CardTitle>
                <CardDescription>{ui.availableHint}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-3">
                {#if nearbyDevices.length === 0}
                  <div class="rounded-2xl border border-dashed border-border bg-muted/30 px-4 py-10 text-center">
                    <Wifi class="mx-auto mb-3 text-muted-foreground" size={28} />
                    <p class="text-sm font-medium">{copy.noNearbyDevices}</p>
                    <p class="mt-2 text-sm text-muted-foreground">{copy.nearbyDevicesHint}</p>
                  </div>
                {:else}
                  {#each nearbyDevices as device}
                    <article class="flex flex-col gap-4 rounded-2xl border border-border/70 bg-muted/30 p-4 sm:flex-row sm:items-center sm:justify-between">
                      <div class="flex min-w-0 items-start gap-4">
                        <div class="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl border border-border bg-card text-muted-foreground">
                          <svelte:component this={platformIconComponent(device.platform)} size={20} />
                        </div>
                        <div class="min-w-0 space-y-1">
                          <div class="truncate text-sm font-semibold">{device.name}</div>
                          <div class="text-sm text-muted-foreground">
                            {platformLabel(device.platform)}
                          </div>
                        </div>
                      </div>

                      <div class="flex flex-wrap items-center gap-2">
                        <Badge
                          variant="outline"
                          class={cn(
                            'rounded-full px-3 py-1 text-xs font-medium',
                            presenceBadgeClass(device.isOnline),
                          )}
                        >
                          {device.isOnline ? copy.online : copy.offline}
                        </Badge>
                        <Button
                          size="sm"
                          disabled={busyPairingId === device.deviceId}
                          on:click={() => updatePairing(device.deviceId, true)}
                        >
                          {busyPairingId === device.deviceId ? copy.saving : copy.pair}
                        </Button>
                      </div>
                    </article>
                  {/each}
                {/if}
              </CardContent>
            </Card>
          </div>

          <div class="space-y-4">
            <Card class="border-border/70 shadow-sm">
              <CardHeader class="pb-2">
                <CardTitle>{copy.transfers}</CardTitle>
                <CardDescription>{ui.transferHint}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-3">
                {#if visibleTransferJobs.length === 0}
                  <div class="rounded-2xl border border-dashed border-border bg-muted/30 px-4 py-10 text-center">
                    <Download class="mx-auto mb-3 text-muted-foreground" size={28} />
                    <p class="text-sm font-medium">{copy.noTransfers}</p>
                    <p class="mt-2 text-sm text-muted-foreground">{ui.transferHint}</p>
                  </div>
                {:else}
                  {#each visibleTransferJobs as job}
                    <article class="rounded-2xl border border-border/70 bg-muted/30 p-4">
                      <div class="flex items-start justify-between gap-3">
                        <div class="min-w-0">
                          <div class="truncate text-sm font-semibold">{job.displayName}</div>
                          <div class="mt-1 text-sm text-muted-foreground">
                            {formatTime(job.startedAt)} · {copy.transferState(job.stage, job.direction)}
                          </div>
                        </div>
                        <Badge
                          variant="outline"
                          class={cn(
                            'rounded-full px-3 py-1 text-xs font-medium',
                            job.stage === 'ready'
                              ? 'border-emerald-200 bg-emerald-50 text-emerald-700'
                              : job.errorMessage
                                ? 'border-destructive/30 bg-destructive/10 text-destructive'
                                : 'border-sky-200 bg-sky-50 text-sky-700',
                          )}
                        >
                          {progress(job)}%
                        </Badge>
                      </div>

                      <div class="mt-4 space-y-3">
                        <Progress value={progress(job)} class="h-2.5" />
                        <div class="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
                          <span>{formatBytes(job.completedBytes)} / {formatBytes(job.totalBytes)}</span>
                          <span>{copy.transferSummary(job.completedEntries, job.totalEntries)}</span>
                        </div>

                        {#if job.errorMessage}
                          <p class="text-sm text-destructive">{job.errorMessage}</p>
                        {/if}

                        {#if job.stage === 'ready' && job.direction === 'inbound'}
                          <div class="flex flex-wrap gap-2">
                            {#if hasAction(job, 'placeOnClipboard')}
                              <Button
                                size="sm"
                                disabled={busyTransferId === job.transferId}
                                on:click={() => placeTransfer(job)}
                              >
                                {busyTransferId === job.transferId ? copy.saving : copy.placeOnClipboard}
                              </Button>
                            {/if}
                            {#if hasAction(job, 'shareExternally')}
                              <Button
                                variant="outline"
                                size="sm"
                                disabled={busyTransferId === job.transferId}
                                on:click={() => shareTransfer(job)}
                              >
                                <Share2 size={14} />
                                <span>{busyTransferId === job.transferId ? copy.saving : copy.shareExternally}</span>
                              </Button>
                            {/if}
                            {#if hasAction(job, 'exportToFiles')}
                              <Button
                                variant="outline"
                                size="sm"
                                disabled={busyTransferId === job.transferId}
                                on:click={() => exportTransfer(job)}
                              >
                                <Download size={14} />
                                <span>{busyTransferId === job.transferId ? copy.saving : copy.exportToFiles}</span>
                              </Button>
                            {/if}
                            <Button
                              variant="ghost"
                              size="sm"
                              disabled={busyTransferId === job.transferId}
                              on:click={() => dismissTransfer(job)}
                            >
                              {copy.dismiss}
                            </Button>
                          </div>
                          {#if job.availableActions.length === 0}
                            <p class="text-sm text-muted-foreground">{copy.actionsUnavailable}</p>
                          {/if}
                        {:else if ['preparing', 'queued', 'downloading', 'verifying'].includes(job.stage)}
                          <div class="flex flex-wrap gap-2">
                            <Button
                              variant="ghost"
                              size="sm"
                              disabled={busyTransferId === job.transferId}
                              on:click={() => cancelTransfer(job)}
                            >
                              {copy.cancel}
                            </Button>
                          </div>
                        {/if}
                      </div>
                    </article>
                  {/each}
                {/if}
              </CardContent>
            </Card>

            <Card class="border-border/70 shadow-sm">
              <CardHeader class="pb-2">
                <CardTitle>{copy.environment}</CardTitle>
                <CardDescription>{ui.runtimeHint}</CardDescription>
              </CardHeader>
              <CardContent class="space-y-3">
                <div class="grid gap-3 sm:grid-cols-2">
                  <div class="rounded-2xl bg-muted/60 p-3">
                    <div class="text-xs uppercase tracking-wide text-muted-foreground">
                      {copy.environment}
                    </div>
                    <div class="mt-2 text-sm font-semibold">
                      {copy.runtimePlatform(runtimePlatform)}
                    </div>
                  </div>
                  <div class="rounded-2xl bg-muted/60 p-3">
                    <div class="text-xs uppercase tracking-wide text-muted-foreground">
                      {copy.backgroundSync}
                    </div>
                    <div class="mt-2 text-sm font-semibold">
                      {copy.backgroundMode(
                        backgroundSyncState?.mode ?? 'unsupported',
                        backgroundSyncState?.active ?? false,
                      )}
                    </div>
                  </div>
                </div>

                {#if isMobile}
                  <p class="text-sm text-muted-foreground">{copy.mobileLimitedHint}</p>
                {/if}

                {#if runtimeCapabilities.openCacheDirectory}
                  <Button variant="outline" class="w-full sm:w-auto" on:click={openCache}>
                    {copy.openCacheDirectory}
                  </Button>
                {/if}
              </CardContent>
            </Card>
          </div>
        </div>
      {/if}

      {#if activeTab === 'history'}
        <div class="mt-4 grid gap-4 xl:grid-cols-2">
          {#if historyGroups.length === 0}
            <Card class="border-border/70 shadow-sm xl:col-span-2">
              <CardContent class="px-4 py-16 text-center">
                <MessageCircle class="mx-auto mb-3 text-muted-foreground" size={30} />
                <p class="text-sm font-medium">{copy.noClipboardHistory}</p>
                <p class="mt-2 text-sm text-muted-foreground">{ui.historyHint}</p>
              </CardContent>
            </Card>
          {:else}
            {#each historyGroups as group}
              <Card class="border-border/70 shadow-sm">
                <CardHeader class="pb-2">
                  <div class="flex items-start justify-between gap-3">
                    <div>
                      <CardTitle>{group.deviceName}</CardTitle>
                      <CardDescription>{copy.clips(group.entries.length)}</CardDescription>
                    </div>
                    {#if group.isLocal}
                      <Badge variant="secondary" class="rounded-full px-3 py-1 text-xs">
                        {copy.thisDevice}
                      </Badge>
                    {/if}
                  </div>
                </CardHeader>
                <CardContent class="space-y-3">
                  {#each group.entries as entry}
                    <article class="flex items-start justify-between gap-3 rounded-2xl border border-border/70 bg-muted/30 p-4">
                      <div class="min-w-0 flex-1">
                        <div class="truncate text-sm font-semibold">{entry.displayName}</div>
                        <p
                          class="mt-2 overflow-hidden text-sm leading-6 text-muted-foreground"
                          style="-webkit-line-clamp: 2; -webkit-box-orient: vertical; display: -webkit-box;"
                        >
                          {historyPreview(entry)}
                        </p>
                        <div class="mt-3 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                          <span>{formatTime(entry.createdAt)}</span>
                          <span>•</span>
                          <span>{copy.historyKind(entry.kind, entry.fileCount)}</span>
                          <span>•</span>
                          <span>{copy.historySource(entry.source)}</span>
                          <span>•</span>
                          <span>{formatBytes(entry.size)}</span>
                        </div>
                      </div>
                      <Button
                        variant="outline"
                        size="sm"
                        class="shrink-0"
                        disabled={busyHistoryId === entry.entryId}
                        on:click={() => restoreHistory(entry)}
                      >
                        <RotateCcw size={14} />
                        <span>{busyHistoryId === entry.entryId ? copy.saving : copy.restoreHistory}</span>
                      </Button>
                    </article>
                  {/each}
                </CardContent>
              </Card>
            {/each}
          {/if}
        </div>
      {/if}

      {#if activeTab === 'settings'}
        <div class="mt-4 grid gap-4 xl:grid-cols-[1.05fr,0.95fr]">
          <Card class="border-border/70 shadow-sm">
            <CardHeader class="pb-2">
              <CardTitle>{copy.syncEnabled}</CardTitle>
              <CardDescription>{ui.settingsHint}</CardDescription>
            </CardHeader>
            <CardContent class="space-y-4">
              <div class="space-y-2">
                <label class="text-sm font-medium" for="device-name">{ui.deviceName}</label>
                <Input
                  id="device-name"
                  value={snapshot.settings.deviceName}
                  disabled={settingsBusy}
                  class="h-11 rounded-xl"
                  on:change={(event) => patchSettings({ deviceName: inputValueFromEvent(event) })}
                />
              </div>

              <div class="flex items-start justify-between gap-4 rounded-2xl border border-border/70 bg-muted/30 p-4">
                <div class="space-y-1">
                  <p class="text-sm font-medium">{copy.syncEnabled}</p>
                  <p class="text-sm text-muted-foreground">{copy.noPairedDevicesHint}</p>
                </div>
                <Switch
                  checked={snapshot.settings.syncEnabled}
                  disabled={settingsBusy}
                  on:change={(event) => toggleClipboardSync(checkedFromEvent(event))}
                />
              </div>

              {#if runtimeCapabilities.backgroundSync}
                <div class="flex items-start justify-between gap-4 rounded-2xl border border-border/70 bg-muted/30 p-4">
                  <div class="space-y-1">
                    <p class="text-sm font-medium">{copy.backgroundSync}</p>
                    <p class="text-sm text-muted-foreground">{copy.backgroundSyncHint}</p>
                  </div>
                  <Switch
                    checked={snapshot.settings.backgroundSyncEnabled}
                    disabled={settingsBusy}
                    on:change={(event) =>
                      patchSettings({ backgroundSyncEnabled: checkedFromEvent(event) })}
                  />
                </div>
              {/if}
            </CardContent>
          </Card>

          <Card class="border-border/70 shadow-sm">
            <CardHeader class="pb-2">
              <CardTitle>{copy.permissions}</CardTitle>
              <CardDescription>{copy.permissionsHint}</CardDescription>
            </CardHeader>
            <CardContent class="space-y-3">
              {#each permissionRows as item}
                <div class="flex items-center justify-between gap-3 rounded-2xl border border-border/70 bg-muted/30 p-4">
                  <div class="min-w-0">
                    <p class="text-sm font-medium">{item.label}</p>
                    <p class="mt-1 text-sm text-muted-foreground">
                      {copy.permissionState(item.state)}
                    </p>
                  </div>
                  <Badge
                    variant="outline"
                    class={cn(
                      'rounded-full px-3 py-1 text-xs font-medium',
                      permissionBadgeClass(item.state),
                    )}
                  >
                    {#if item.state === 'granted'}
                      <Check size={14} />
                    {:else if item.state === 'denied'}
                      <X size={14} />
                    {/if}
                    <span>{copy.permissionState(item.state)}</span>
                  </Badge>
                </div>
              {/each}

              {#if permissionsNeedAttention}
                <Button class="w-full" disabled={permissionsBusy} on:click={refreshPermissions}>
                  {permissionsBusy ? copy.saving : copy.requestPermissions}
                </Button>
              {/if}
            </CardContent>
          </Card>

          <Card class="border-border/70 shadow-sm xl:col-span-2">
            <CardHeader class="pb-2">
              <CardTitle>{copy.environment}</CardTitle>
              <CardDescription>{ui.runtimeHint}</CardDescription>
            </CardHeader>
            <CardContent class="grid gap-4 md:grid-cols-3">
              <div class="rounded-2xl border border-border/70 bg-muted/30 p-4">
                <div class="text-xs uppercase tracking-wide text-muted-foreground">
                  {copy.environment}
                </div>
                <div class="mt-2 text-sm font-semibold">
                  {copy.runtimePlatform(runtimePlatform)}
                </div>
              </div>
              <div class="rounded-2xl border border-border/70 bg-muted/30 p-4">
                <div class="text-xs uppercase tracking-wide text-muted-foreground">
                  {copy.backgroundSync}
                </div>
                <div class="mt-2 text-sm font-semibold">
                  {copy.backgroundMode(
                    backgroundSyncState?.mode ?? 'unsupported',
                    backgroundSyncState?.active ?? false,
                  )}
                </div>
              </div>
              <div class="rounded-2xl border border-border/70 bg-muted/30 p-4">
                <div class="text-xs uppercase tracking-wide text-muted-foreground">
                  {ui.protocol}
                </div>
                <div class="mt-2 text-sm font-semibold">{localDevice?.protocolVersion}</div>
              </div>

              {#if backgroundSyncState?.message}
                <p class="text-sm text-muted-foreground md:col-span-2">
                  {backgroundSyncState.message}
                </p>
              {/if}

              {#if runtimeCapabilities.openCacheDirectory}
                <div class="md:col-span-3">
                  <Button variant="outline" on:click={openCache}>
                    {copy.openCacheDirectory}
                  </Button>
                </div>
              {/if}
            </CardContent>
          </Card>
        </div>
      {/if}
    </div>
  </div>
{:else}
  <div class="flex min-h-[100dvh] items-center justify-center bg-[radial-gradient(circle_at_top,rgba(15,23,42,0.05),transparent_42%),linear-gradient(180deg,rgba(248,250,252,0.96),rgba(241,245,249,0.88))] px-4">
    <Card class="w-full max-w-sm border-border/70 shadow-sm">
      <CardContent class="flex flex-col items-center gap-4 px-6 py-10 text-center">
        <div class="flex h-12 w-12 animate-pulse items-center justify-center rounded-2xl bg-primary/10 text-primary">
          <Link2 size={22} />
        </div>
        <div class="space-y-2">
          <div class="text-lg font-semibold">{copy.title}</div>
          <p class="text-sm text-muted-foreground">
            {currentLanguage === 'zh-CN' ? '正在加载同步面板…' : 'Loading the sync workspace...'}
          </p>
        </div>
      </CardContent>
    </Card>
  </div>
{/if}
