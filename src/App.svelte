<script lang="ts">
  import { onMount } from 'svelte'
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
    MessageCircle,
    Users,
    Settings,
    Smartphone,
    Laptop2,
    Check,
    X,
    Link2,
    Wifi,
    WifiOff,
    ChevronRight,
    RotateCcw,
    Download,
    Share2,
    Trash2,
    AlertCircle,
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
  let showDeviceDetail: string | null = null
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
  $: permissionsNeedAttention = [
    runtimePermissions.notifications,
    runtimePermissions.localNetwork,
    runtimePermissions.clipboard,
    runtimePermissions.fileAccess,
    runtimePermissions.backgroundSync,
  ].some((state) => ['prompt', 'denied'].includes(state))

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
  <div class="wechat-app">
    <!-- Header -->
    <header class="wechat-header">
      <div class="header-title">RelayClip</div>
      <div class="header-actions">
        {#if syncStatus?.state === 'syncing'}
          <span class="sync-indicator">
            <span class="sync-dot"></span>
            同步中
          </span>
        {:else if syncStatus?.state === 'connected'}
          <span class="sync-status connected">已连接</span>
        {:else}
          <span class="sync-status">未连接</span>
        {/if}
      </div>
    </header>

    <!-- Main Content -->
    <main class="wechat-content">
      {#if errorBanner}
        <div class="error-toast">
          <AlertCircle class="error-icon" size={16} />
          <span>{errorBanner}</span>
        </div>
      {/if}

      <!-- Devices Tab -->
      {#if activeTab === 'devices'}
        <div class="wechat-list">
          <!-- My Device -->
          <div class="list-section">
            <div class="section-title">我的设备</div>
            <div class="wechat-card">
              <div class="device-item self">
                <div class="device-avatar green">
                  {#if isMobilePlatform(localDevice?.platform ?? '')}
                    <Smartphone size={20} />
                  {:else}
                    <Laptop2 size={20} />
                  {/if}
                </div>
                <div class="device-info">
                  <div class="device-name">{snapshot.settings.deviceName}</div>
                  <div class="device-meta">{platformLabel(localDevice?.platform ?? '')} · 本机</div>
                </div>
                <div class="device-action">
                  <span class="badge primary">在线</span>
                </div>
              </div>
            </div>
          </div>

          <!-- Paired Devices -->
          {#if pairedDevices.length > 0}
            <div class="list-section">
              <div class="section-title">已配对设备 ({pairedDevices.length})</div>
              <div class="wechat-card">
                {#each pairedDevices as device, i}
                  <div class="device-item" class:border-bottom={i < pairedDevices.length - 1}>
                    <div class="device-avatar" class:online={device.isOnline}>
                      {#if isMobilePlatform(device.platform)}
                        <Smartphone size={20} />
                      {:else}
                        <Laptop2 size={20} />
                      {/if}
                    </div>
                    <div class="device-info">
                      <div class="device-name">{device.name}</div>
                      <div class="device-meta">
                        {platformLabel(device.platform)} · {device.isOnline ? '在线' : '离线'}
                      </div>
                    </div>
                    <div class="device-action">
                      <button
                        class="btn-text danger"
                        disabled={busyPairingId === device.deviceId}
                        on:click={() => updatePairing(device.deviceId, false)}
                      >
                        取消配对
                      </button>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {/if}

          <!-- Nearby Devices -->
          {#if nearbyDevices.length > 0}
            <div class="list-section">
              <div class="section-title">附近设备</div>
              <div class="wechat-card">
                {#each nearbyDevices as device, i}
                  <div class="device-item" class:border-bottom={i < nearbyDevices.length - 1}>
                    <div class="device-avatar gray">
                      {#if isMobilePlatform(device.platform)}
                        <Smartphone size={20} />
                      {:else}
                        <Laptop2 size={20} />
                      {/if}
                    </div>
                    <div class="device-info">
                      <div class="device-name">{device.name}</div>
                      <div class="device-meta">{platformLabel(device.platform)}</div>
                    </div>
                    <div class="device-action">
                      <button
                        class="btn-text primary"
                        disabled={busyPairingId === device.deviceId}
                        on:click={() => updatePairing(device.deviceId, true)}
                      >
                        配对
                      </button>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {:else if pairedDevices.length === 0}
            <div class="empty-state">
              <div class="empty-icon">
                <WifiOff size={48} />
              </div>
              <div class="empty-text">未发现附近设备</div>
              <div class="empty-subtext">确保其他设备已开启 RelayClip 并在同一网络</div>
            </div>
          {/if}
        </div>

        <!-- Active Transfers -->
        {#if visibleTransferJobs.length > 0}
          <div class="list-section">
            <div class="section-title">传输任务</div>
            <div class="wechat-card">
              {#each visibleTransferJobs as job, i}
                <div class="transfer-item" class:border-bottom={i < visibleTransferJobs.length - 1}>
                  <div class="transfer-header">
                    <span class="transfer-name">{job.displayName}</span>
                    <span class="transfer-time">{formatTime(job.startedAt)}</span>
                  </div>
                  <div class="transfer-progress">
                    <div class="progress-bar">
                      <div class="progress-fill" style="width: {progress(job)}%"></div>
                    </div>
                    <span class="progress-text">{progress(job)}%</span>
                  </div>
                  <div class="transfer-meta">
                    <span>{formatBytes(job.completedBytes)} / {formatBytes(job.totalBytes)}</span>
                    <span class="transfer-status">
                      {#if job.stage === 'ready'}
                        {#if job.direction === 'inbound'}
                          <span class="status-ready">待接收</span>
                        {:else}
                          <span class="status-done">已完成</span>
                        {/if}
                      {:else if job.stage === 'downloading'}
                        <span class="status-active">传输中</span>
                      {:else if job.stage === 'preparing'}
                        <span>准备中</span>
                      {:else if job.errorMessage}
                        <span class="status-error">失败</span>
                      {/if}
                    </span>
                  </div>
                  {#if job.stage === 'ready' && job.direction === 'inbound'}
                    <div class="transfer-actions">
                      {#if hasAction(job, 'placeOnClipboard')}
                        <button
                          class="btn-primary"
                          disabled={busyTransferId === job.transferId}
                          on:click={() => placeTransfer(job)}
                        >
                          复制到剪贴板
                        </button>
                      {/if}
                      {#if hasAction(job, 'shareExternally')}
                        <button
                          class="btn-secondary"
                          disabled={busyTransferId === job.transferId}
                          on:click={() => shareTransfer(job)}
                        >
                          <Share2 size={14} />
                          分享
                        </button>
                      {/if}
                      {#if hasAction(job, 'exportToFiles')}
                        <button
                          class="btn-secondary"
                          disabled={busyTransferId === job.transferId}
                          on:click={() => exportTransfer(job)}
                        >
                          <Download size={14} />
                          保存
                        </button>
                      {/if}
                      <button
                        class="btn-ghost"
                        disabled={busyTransferId === job.transferId}
                        on:click={() => dismissTransfer(job)}
                      >
                        忽略
                      </button>
                    </div>
                  {:else if ['preparing', 'queued', 'downloading', 'verifying'].includes(job.stage)}
                    <div class="transfer-actions">
                      <button
                        class="btn-ghost"
                        disabled={busyTransferId === job.transferId}
                        on:click={() => cancelTransfer(job)}
                      >
                        取消
                      </button>
                    </div>
                  {/if}
                </div>
              {/each}
            </div>
          </div>
        {/if}
      {/if}

      <!-- History Tab -->
      {#if activeTab === 'history'}
        <div class="wechat-list">
          {#if historyGroups.length === 0}
            <div class="empty-state">
              <div class="empty-icon">
                <MessageCircle size={48} />
              </div>
              <div class="empty-text">暂无剪贴板记录</div>
              <div class="empty-subtext">同步开启后将自动记录剪贴板内容</div>
            </div>
          {:else}
            {#each historyGroups as group}
              <div class="list-section">
                <div class="section-title">
                  {group.deviceName}
                  {#if group.isLocal}
                    <span class="badge">本机</span>
                  {/if}
                </div>
                <div class="wechat-card">
                  {#each group.entries as entry, i}
                    <div class="history-item" class:border-bottom={i < group.entries.length - 1}>
                      <div class="history-content">
                        <div class="history-title">{entry.displayName}</div>
                        <div class="history-preview">{historyPreview(entry)}</div>
                        <div class="history-meta">
                          <span>{formatTime(entry.createdAt)}</span>
                          <span>·</span>
                          <span>{formatBytes(entry.size)}</span>
                        </div>
                      </div>
                      <button
                        class="btn-icon"
                        disabled={busyHistoryId === entry.entryId}
                        on:click={() => restoreHistory(entry)}
                        title="恢复"
                      >
                        <RotateCcw size={18} />
                      </button>
                    </div>
                  {/each}
                </div>
              </div>
            {/each}
          {/if}
        </div>
      {/if}

      <!-- Settings Tab -->
      {#if activeTab === 'settings'}
        <div class="wechat-list">
          <!-- Sync Settings -->
          <div class="list-section">
            <div class="section-title">同步设置</div>
            <div class="wechat-card">
              <div class="setting-item">
                <div class="setting-info">
                  <div class="setting-name">设备名称</div>
                </div>
                <div class="setting-control">
                  <input
                    type="text"
                    class="wechat-input"
                    value={snapshot.settings.deviceName}
                    on:change={(e) => patchSettings({ deviceName: e.currentTarget.value })}
                    disabled={settingsBusy}
                  />
                </div>
              </div>
              <div class="setting-item">
                <div class="setting-info">
                  <div class="setting-name">剪贴板同步</div>
                  <div class="setting-desc">自动同步剪贴板内容到配对设备</div>
                </div>
                <div class="setting-control">
                  <label class="wechat-switch">
                    <input
                      type="checkbox"
                      checked={snapshot.settings.syncEnabled}
                      on:change={(e) => toggleClipboardSync(e.currentTarget.checked)}
                      disabled={settingsBusy}
                    />
                    <span class="switch-slider"></span>
                  </label>
                </div>
              </div>
              {#if runtimeCapabilities.backgroundSync}
                <div class="setting-item">
                  <div class="setting-info">
                    <div class="setting-name">后台同步</div>
                    <div class="setting-desc">应用后台运行时保持同步</div>
                  </div>
                  <div class="setting-control">
                    <label class="wechat-switch">
                      <input
                        type="checkbox"
                        checked={snapshot.settings.backgroundSyncEnabled}
                        on:change={(e) => patchSettings({ backgroundSyncEnabled: e.currentTarget.checked })}
                        disabled={settingsBusy}
                      />
                      <span class="switch-slider"></span>
                    </label>
                  </div>
                </div>
              {/if}
            </div>
          </div>

          <!-- Permissions -->
          <div class="list-section">
            <div class="section-title">权限状态</div>
            <div class="wechat-card">
              {#each [
                ['通知权限', runtimePermissions.notifications],
                ['本地网络', runtimePermissions.localNetwork],
                ['剪贴板访问', runtimePermissions.clipboard],
                ['文件访问', runtimePermissions.fileAccess],
                ['后台同步', runtimePermissions.backgroundSync],
              ] as [label, state]}
                <div class="permission-item">
                  <span class="permission-name">{label}</span>
                  <span class="permission-status" class:granted={state === 'granted'}>
                    {#if state === 'granted'}
                      <Check size={14} />
                      已授权
                    {:else if state === 'denied'}
                      <X size={14} />
                      已拒绝
                    {:else if state === 'prompt'}
                      待请求
                    {:else}
                      不支持
                    {/if}
                  </span>
                </div>
              {/each}
              {#if permissionsNeedAttention}
                <div class="permission-action">
                  <button
                    class="btn-primary full"
                    disabled={permissionsBusy}
                    on:click={refreshPermissions}
                  >
                    请求权限
                  </button>
                </div>
              {/if}
            </div>
          </div>

          <!-- About -->
          <div class="list-section">
            <div class="section-title">关于</div>
            <div class="wechat-card">
              <div class="info-item">
                <span>版本</span>
                <span class="info-value">2026.3.14</span>
              </div>
              <div class="info-item">
                <span>平台</span>
                <span class="info-value">{runtimePlatform}</span>
              </div>
            </div>
          </div>
        </div>
      {/if}
    </main>

    <!-- Tab Bar -->
    <nav class="wechat-tabbar">
      <button
        class="tab-item"
        class:active={activeTab === 'devices'}
        on:click={() => activeTab = 'devices'}
      >
        <div class="tab-icon">
          <Link2 size={24} />
        </div>
        <span class="tab-label">设备</span>
      </button>
      <button
        class="tab-item"
        class:active={activeTab === 'history'}
        on:click={() => activeTab = 'history'}
      >
        <div class="tab-icon">
          <MessageCircle size={24} />
        </div>
        <span class="tab-label">记录</span>
      </button>
      <button
        class="tab-item"
        class:active={activeTab === 'settings'}
        on:click={() => activeTab = 'settings'}
      >
        <div class="tab-icon">
          <Settings size={24} />
        </div>
        <span class="tab-label">设置</span>
      </button>
    </nav>
  </div>
{:else}
  <div class="wechat-app loading">
    <div class="loading-spinner"></div>
    <span>RelayClip</span>
  </div>
{/if}

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
    background: #ededed;
    -webkit-font-smoothing: antialiased;
  }

  .wechat-app {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
    max-width: 430px;
    margin: 0 auto;
    background: #ededed;
    position: relative;
  }

  .wechat-app.loading {
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: #999;
  }

  .loading-spinner {
    width: 24px;
    height: 24px;
    border: 2px solid #e0e0e0;
    border-top-color: #07c160;
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* Header */
  .wechat-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    background: #ededed;
    border-bottom: 0.5px solid #d9d9d9;
    position: sticky;
    top: 0;
    z-index: 100;
  }

  .header-title {
    font-size: 17px;
    font-weight: 600;
    color: #000;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .sync-indicator {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 13px;
    color: #07c160;
  }

  .sync-dot {
    width: 6px;
    height: 6px;
    background: #07c160;
    border-radius: 50%;
    animation: pulse 1.5s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  .sync-status {
    font-size: 13px;
    color: #999;
  }

  .sync-status.connected {
    color: #07c160;
  }

  /* Content */
  .wechat-content {
    flex: 1;
    overflow-y: auto;
    padding-bottom: 80px;
  }

  .error-toast {
    margin: 12px 16px;
    padding: 10px 12px;
    background: #fa5151;
    color: white;
    border-radius: 6px;
    font-size: 13px;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .error-icon {
    flex-shrink: 0;
  }

  /* List Sections */
  .wechat-list {
    padding-top: 12px;
  }

  .list-section {
    margin-bottom: 16px;
  }

  .section-title {
    padding: 0 16px 8px;
    font-size: 13px;
    color: #999;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  /* Cards */
  .wechat-card {
    margin: 0 16px;
    background: white;
    border-radius: 8px;
    overflow: hidden;
    box-shadow: 0 1px 2px rgba(0,0,0,0.04);
  }

  /* Device Items */
  .device-item {
    display: flex;
    align-items: center;
    padding: 12px 16px;
    gap: 12px;
  }

  .device-item.border-bottom {
    border-bottom: 0.5px solid #e5e5e5;
  }

  .device-avatar {
    width: 40px;
    height: 40px;
    border-radius: 6px;
    background: #e5e5e5;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #999;
    flex-shrink: 0;
  }

  .device-avatar.green {
    background: #07c160;
    color: white;
  }

  .device-avatar.online {
    background: #e8f5e9;
    color: #07c160;
  }

  .device-avatar.gray {
    background: #f5f5f5;
    color: #999;
  }

  .device-info {
    flex: 1;
    min-width: 0;
  }

  .device-name {
    font-size: 16px;
    font-weight: 500;
    color: #000;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .device-meta {
    font-size: 13px;
    color: #999;
    margin-top: 2px;
  }

  .device-action {
    flex-shrink: 0;
  }

  /* Transfer Items */
  .transfer-item {
    padding: 16px;
  }

  .transfer-item.border-bottom {
    border-bottom: 0.5px solid #e5e5e5;
  }

  .transfer-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
  }

  .transfer-name {
    font-size: 15px;
    font-weight: 500;
    color: #000;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 70%;
  }

  .transfer-time {
    font-size: 12px;
    color: #999;
  }

  .transfer-progress {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }

  .progress-bar {
    flex: 1;
    height: 4px;
    background: #e5e5e5;
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: #07c160;
    border-radius: 2px;
    transition: width 0.3s ease;
  }

  .progress-text {
    font-size: 12px;
    color: #999;
    min-width: 36px;
    text-align: right;
  }

  .transfer-meta {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: #999;
    margin-bottom: 12px;
  }

  .transfer-status {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .status-ready {
    color: #ff9500;
  }

  .status-done {
    color: #07c160;
  }

  .status-active {
    color: #576b95;
  }

  .status-error {
    color: #fa5151;
  }

  .transfer-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  /* History Items */
  .history-item {
    display: flex;
    align-items: flex-start;
    padding: 12px 16px;
    gap: 12px;
  }

  .history-item.border-bottom {
    border-bottom: 0.5px solid #e5e5e5;
  }

  .history-content {
    flex: 1;
    min-width: 0;
  }

  .history-title {
    font-size: 15px;
    font-weight: 500;
    color: #000;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-bottom: 4px;
  }

  .history-preview {
    font-size: 13px;
    color: #666;
    line-height: 1.4;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    margin-bottom: 4px;
  }

  .history-meta {
    font-size: 12px;
    color: #999;
    display: flex;
    gap: 4px;
  }

  /* Settings */
  .setting-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    border-bottom: 0.5px solid #e5e5e5;
  }

  .setting-item:last-child {
    border-bottom: none;
  }

  .setting-info {
    flex: 1;
  }

  .setting-name {
    font-size: 16px;
    color: #000;
  }

  .setting-desc {
    font-size: 13px;
    color: #999;
    margin-top: 2px;
  }

  .setting-control {
    flex-shrink: 0;
  }

  .wechat-input {
    width: 140px;
    padding: 6px 10px;
    border: 0.5px solid #e5e5e5;
    border-radius: 4px;
    font-size: 15px;
    text-align: right;
    background: #f5f5f5;
  }

  .wechat-input:focus {
    outline: none;
    border-color: #07c160;
    background: white;
  }

  /* Switch */
  .wechat-switch {
    position: relative;
    display: inline-block;
    width: 50px;
    height: 30px;
  }

  .wechat-switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .switch-slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: #e5e5e5;
    transition: 0.3s;
    border-radius: 30px;
  }

  .switch-slider:before {
    position: absolute;
    content: "";
    height: 26px;
    width: 26px;
    left: 2px;
    bottom: 2px;
    background-color: white;
    transition: 0.3s;
    border-radius: 50%;
    box-shadow: 0 1px 3px rgba(0,0,0,0.2);
  }

  input:checked + .switch-slider {
    background-color: #07c160;
  }

  input:checked + .switch-slider:before {
    transform: translateX(20px);
  }

  /* Permissions */
  .permission-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 0.5px solid #e5e5e5;
  }

  .permission-item:last-child {
    border-bottom: none;
  }

  .permission-name {
    font-size: 16px;
    color: #000;
  }

  .permission-status {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 14px;
    color: #999;
  }

  .permission-status.granted {
    color: #07c160;
  }

  .permission-action {
    padding: 12px 16px;
    border-top: 0.5px solid #e5e5e5;
  }

  /* Info Items */
  .info-item {
    display: flex;
    justify-content: space-between;
    padding: 12px 16px;
    font-size: 16px;
    color: #000;
    border-bottom: 0.5px solid #e5e5e5;
  }

  .info-item:last-child {
    border-bottom: none;
  }

  .info-value {
    color: #999;
  }

  /* Buttons */
  .btn-primary {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 8px 16px;
    background: #07c160;
    color: white;
    border: none;
    border-radius: 4px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
  }

  .btn-primary:active {
    background: #06ad56;
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-primary.full {
    width: 100%;
  }

  .btn-secondary {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 8px 14px;
    background: #f5f5f5;
    color: #333;
    border: 0.5px solid #e5e5e5;
    border-radius: 4px;
    font-size: 14px;
    cursor: pointer;
  }

  .btn-secondary:active {
    background: #e8e8e8;
  }

  .btn-ghost {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 8px 14px;
    background: transparent;
    color: #576b95;
    border: none;
    font-size: 14px;
    cursor: pointer;
  }

  .btn-text {
    background: none;
    border: none;
    font-size: 14px;
    cursor: pointer;
    padding: 4px 8px;
  }

  .btn-text.primary {
    color: #576b95;
  }

  .btn-text.danger {
    color: #fa5151;
  }

  .btn-icon {
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #f5f5f5;
    border: none;
    border-radius: 50%;
    color: #666;
    cursor: pointer;
    flex-shrink: 0;
  }

  .btn-icon:active {
    background: #e8e8e8;
  }

  /* Badges */
  .badge {
    display: inline-flex;
    align-items: center;
    padding: 2px 6px;
    background: #e5e5e5;
    color: #666;
    font-size: 11px;
    border-radius: 3px;
  }

  .badge.primary {
    background: #e8f5e9;
    color: #07c160;
  }

  /* Empty State */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 60px 32px;
    text-align: center;
  }

  .empty-icon {
    color: #d9d9d9;
    margin-bottom: 16px;
  }

  .empty-text {
    font-size: 16px;
    color: #999;
    margin-bottom: 8px;
  }

  .empty-subtext {
    font-size: 13px;
    color: #bbb;
    line-height: 1.5;
  }

  /* Tab Bar */
  .wechat-tabbar {
    position: fixed;
    bottom: 0;
    left: 50%;
    transform: translateX(-50%);
    width: 100%;
    max-width: 430px;
    display: flex;
    background: #f7f7f7;
    border-top: 0.5px solid #d9d9d9;
    padding-bottom: env(safe-area-inset-bottom, 0);
  }

  .tab-item {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 6px 0;
    background: none;
    border: none;
    cursor: pointer;
    color: #999;
  }

  .tab-item.active {
    color: #07c160;
  }

  .tab-icon {
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .tab-label {
    font-size: 10px;
    margin-top: 2px;
  }

  /* Scrollbar */
  ::-webkit-scrollbar {
    width: 0;
    height: 0;
  }
</style>
