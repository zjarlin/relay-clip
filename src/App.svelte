<script lang="ts">
  import { onMount } from 'svelte'
  import {
    cancelTransferJob,
    dismissTransferJob,
    getAppState,
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
    restoreClipboardHistoryEntry,
    setDevicePairing,
    syncAutostart,
    toggleSync,
    updateSettings,
  } from './lib/api'
  import { getMessages, normalizeLanguage } from './lib/i18n'
  import type { AppStateSnapshot, ClipboardHistoryEntry, TransferJob } from './lib/types'
  import {
    isPermissionGranted,
    requestPermission,
    sendNotification,
  } from '@tauri-apps/plugin-notification'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card'
  import { Input } from '$lib/components/ui/input'
  import { Progress } from '$lib/components/ui/progress'
  import { ScrollArea } from '$lib/components/ui/scroll-area'
  import { Separator } from '$lib/components/ui/separator'
  import { Switch } from '$lib/components/ui/switch'
  import {
    ClipboardList,
    Copy,
    Laptop2,
    FolderOpen,
    Image,
    Link2,
    Search,
    Send,
    Smartphone,
    Type,
  } from '@lucide/svelte'

  interface HistoryGroup {
    deviceId: string
    deviceName: string
    isLocal: boolean
    entries: ClipboardHistoryEntry[]
  }

  type ErrorBannerSource = 'action' | 'sync'

  let snapshot: AppStateSnapshot | null = null
  let transferJobs: TransferJob[] = []
  let clipboardHistory: ClipboardHistoryEntry[] = []
  let busyTransferId: string | null = null
  let busyHistoryId: string | null = null
  let busyPairingId: string | null = null
  let settingsBusy = false
  let errorBanner: string | null = null
  let errorBannerSource: ErrorBannerSource | null = null
  const notifiedReady = new Set<string>()

  $: currentLanguage =
    snapshot?.settings.language ??
    normalizeLanguage(typeof navigator === 'undefined' ? undefined : navigator.language)
  $: copy = getMessages(currentLanguage)
  $: devices = snapshot?.devices ?? []
  $: localDevice = snapshot?.localDevice ?? null
  $: pairedDevices = devices.filter((device) => device.isActive)
  $: nearbyDevices = devices.filter(
    (device) => !device.isActive && device.isOnline && device.deviceId !== (localDevice?.deviceId ?? ''),
  )
  $: visibleTransferJobs = transferJobs
  $: syncStatus = snapshot?.syncStatus
  $: historyGroups = groupHistory(clipboardHistory, localDevice?.deviceId ?? '', localDevice?.deviceName ?? copy.thisDevice)
  $: if (errorBannerSource === 'sync' && syncStatus?.state && syncStatus.state !== 'error') {
    clearErrorBanner()
  }

  function clearErrorBanner() {
    errorBanner = null
    errorBannerSource = null
  }

  function showActionError(error: unknown) {
    errorBanner = error instanceof Error ? error.message : String(error)
    errorBannerSource = 'action'
  }

  function showSyncError(message: string) {
    errorBanner = message
    errorBannerSource = 'sync'
  }

  async function refresh() {
    const [state, jobs, history] = await Promise.all([
      getAppState(),
      listTransferJobs(),
      listClipboardHistory(),
    ])
    snapshot = state
    transferJobs = jobs
    clipboardHistory = history

    try {
      let shouldPersistAutostart = state.settings.launchOnLogin !== true
      const autostartEnabled = await isAutostartEnabled().catch(() => state.settings.launchOnLogin)
      if (!autostartEnabled) {
        await syncAutostart(true)
        shouldPersistAutostart = true
      }
      if (shouldPersistAutostart) {
        snapshot = await updateSettings({ launchOnLogin: true })
      }
    } catch (error) {
      showActionError(error)
    }
  }

  async function patchSettings(patch: Parameters<typeof updateSettings>[0]) {
    if (!snapshot) return
    settingsBusy = true
    clearErrorBanner()

    try {
      snapshot = await updateSettings(patch)
    } catch (error) {
      showActionError(error)
    } finally {
      settingsBusy = false
    }
  }

  async function updatePairing(deviceId: string, paired: boolean) {
    busyPairingId = deviceId
    clearErrorBanner()
    try {
      snapshot = await setDevicePairing(deviceId, paired)
    } catch (error) {
      showActionError(error)
    } finally {
      busyPairingId = null
    }
  }

  async function toggleClipboardSync(enabled: boolean) {
    clearErrorBanner()
    try {
      snapshot = await toggleSync(enabled)
    } catch (error) {
      showActionError(error)
    }
  }

  async function placeTransfer(job: TransferJob) {
    busyTransferId = job.transferId
    clearErrorBanner()
    try {
      await placeReceivedTransferOnClipboard(job.transferId)
    } catch (error) {
      showActionError(error)
    } finally {
      busyTransferId = null
    }
  }

  async function dismissTransfer(job: TransferJob) {
    busyTransferId = job.transferId
    clearErrorBanner()
    try {
      await dismissTransferJob(job.transferId)
    } catch (error) {
      showActionError(error)
    } finally {
      busyTransferId = null
    }
  }

  async function cancelTransfer(job: TransferJob) {
    busyTransferId = job.transferId
    clearErrorBanner()
    try {
      await cancelTransferJob(job.transferId)
    } catch (error) {
      showActionError(error)
    } finally {
      busyTransferId = null
    }
  }

  async function restoreHistory(entry: ClipboardHistoryEntry) {
    busyHistoryId = entry.entryId
    clearErrorBanner()
    try {
      await restoreClipboardHistoryEntry(entry.entryId)
    } catch (error) {
      showActionError(error)
    } finally {
      busyHistoryId = null
    }
  }

  async function openCache() {
    clearErrorBanner()
    try {
      await openCacheDirectory()
    } catch (error) {
      showActionError(error)
    }
  }

  async function notifyTransferReady(job: TransferJob) {
    if (notifiedReady.has(job.transferId)) return
    notifiedReady.add(job.transferId)

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
  }

  function groupHistory(entries: ClipboardHistoryEntry[], localDeviceId: string, localDeviceName: string): HistoryGroup[] {
    const groups = new Map<string, HistoryGroup>()

    for (const entry of entries) {
      const isLocal = entry.originDeviceId === localDeviceId || entry.source === 'local'
      const deviceId = entry.originDeviceId || (isLocal ? localDeviceId : `unknown:${entry.entryId}`)
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

  function groupPlatform(group: HistoryGroup) {
    if (group.isLocal) {
      return localDevice?.platform ?? ''
    }

    return devices.find((device) => device.deviceId === group.deviceId)?.platform ?? ''
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
    return new Intl.DateTimeFormat(currentLanguage === 'zh-CN' ? 'zh-CN' : 'en-US', {
      month: 'numeric',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(value))
  }

  function historyPreview(entry: ClipboardHistoryEntry) {
    if (entry.previewText) return entry.previewText
    if (entry.topLevelNames.length > 0) return entry.topLevelNames.join(', ')
    return formatBytes(entry.size)
  }

  function statusVariant(state?: string | null): 'default' | 'secondary' | 'outline' {
    if (state === 'connected' || state === 'syncing') return 'default'
    if (state === 'error') return 'outline'
    return 'secondary'
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
        if (!disposed) showSyncError(message)
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
  <main class="min-h-screen bg-muted/40 p-3 text-sm">
    <div class="mx-auto flex max-w-[430px] flex-col gap-3">
      <Card class="shadow-sm">
        <CardHeader class="gap-3 pb-3">
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0 flex-1 space-y-2">
              <div class="flex items-center gap-2 text-[11px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
                <Link2 class="size-3.5" />
                <span>{copy.title}</span>
              </div>
              <Input
                class="h-9 text-sm font-semibold"
                disabled={settingsBusy}
                on:change={(event) =>
                  patchSettings({ deviceName: (event.target as HTMLInputElement).value })}
                spellcheck="false"
                value={snapshot.settings.deviceName}
              />
            </div>
            <div class="flex max-w-40 flex-col items-end gap-1 text-right">
              <Badge variant={statusVariant(syncStatus?.state)}>{copy.syncState(syncStatus?.state ?? 'idle')}</Badge>
              <p class="text-[11px] leading-4 text-muted-foreground">
                {syncStatus?.message ?? ''}
              </p>
            </div>
          </div>
          {#if errorBanner}
            <div class="rounded-md border border-destructive/25 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {errorBanner}
            </div>
          {/if}
        </CardHeader>

        <CardContent class="space-y-3">
          <div class="space-y-2">
            <div class="flex items-center justify-between gap-3">
              <div>
                <CardTitle>{copy.pairedDevices}</CardTitle>
                <CardDescription class="mt-1">
                  {pairedDevices.length > 0 ? copy.pairedDevicesCount(pairedDevices.length) : copy.noPairedDevicesHint}
                </CardDescription>
              </div>
              <Badge variant="secondary">{copy.pairedDevicesCount(pairedDevices.length)}</Badge>
            </div>

            {#if pairedDevices.length === 0}
              <div class="rounded-lg border border-dashed bg-background px-3 py-3 text-xs text-muted-foreground">
                <div class="font-medium text-foreground">{copy.noPairedDevices}</div>
                <div class="mt-1">{copy.noPairedDevicesHint}</div>
              </div>
            {:else}
              <div class="space-y-2">
                {#each pairedDevices as device}
                  <div class="flex items-center justify-between gap-3 rounded-lg border bg-background px-3 py-2">
                    <div class="flex min-w-0 items-center gap-2">
                      {#if isMobilePlatform(device.platform)}
                        <Smartphone class="size-3.5 text-muted-foreground" />
                      {:else}
                        <Laptop2 class="size-3.5 text-muted-foreground" />
                      {/if}
                      <div class="min-w-0">
                        <div class="truncate font-medium">{device.name}</div>
                        <div class="text-xs text-muted-foreground">{platformLabel(device.platform)} · {device.isOnline ? copy.online : copy.offline}</div>
                      </div>
                    </div>
                    <Button
                      disabled={busyPairingId === device.deviceId}
                      on:click={() => updatePairing(device.deviceId, false)}
                      size="sm"
                      variant="outline"
                    >
                      {copy.unpair}
                    </Button>
                  </div>
                {/each}
              </div>
            {/if}
          </div>

          <Separator />

          <div class="space-y-2">
            <div class="flex items-center gap-2 text-xs font-medium text-muted-foreground">
              <Search class="size-3.5" />
              <span>{copy.nearby}</span>
            </div>

            {#if nearbyDevices.length === 0}
              <div class="rounded-lg border border-dashed bg-background px-3 py-3 text-xs text-muted-foreground">
                <div class="font-medium text-foreground">{copy.noNearbyDevices}</div>
                <div class="mt-1">{copy.nearbyDevicesHint}</div>
              </div>
            {:else}
              <div class="space-y-2">
                {#each nearbyDevices as device}
                  <div class="flex items-center justify-between gap-3 rounded-lg border bg-background px-3 py-2">
                    <div class="flex min-w-0 items-center gap-2">
                      {#if isMobilePlatform(device.platform)}
                        <Smartphone class="size-3.5 text-muted-foreground" />
                      {:else}
                        <Laptop2 class="size-3.5 text-muted-foreground" />
                      {/if}
                      <div class="min-w-0">
                        <div class="truncate font-medium">{device.name}</div>
                        <div class="text-xs text-muted-foreground">{platformLabel(device.platform)}</div>
                      </div>
                    </div>
                    <Button
                      disabled={busyPairingId === device.deviceId}
                      on:click={() => updatePairing(device.deviceId, true)}
                      size="sm"
                      variant="secondary"
                    >
                      {copy.pair}
                    </Button>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </CardContent>
      </Card>

      <Card class="shadow-sm">
        <CardHeader class="pb-3">
          <div class="flex items-center justify-between gap-3">
            <div class="flex items-center gap-2">
              <Send class="size-4 text-muted-foreground" />
              <CardTitle>{copy.transfers}</CardTitle>
            </div>
          </div>
        </CardHeader>
        <CardContent class="pt-0">
          {#if visibleTransferJobs.length === 0}
            <div class="rounded-lg border border-dashed bg-background px-3 py-3 text-xs text-muted-foreground">
              {copy.noTransfers}
            </div>
          {:else}
            <ScrollArea class="max-h-64 space-y-2 pr-1">
              {#each visibleTransferJobs as job}
                <div class="mb-2 space-y-2 rounded-lg border bg-background px-3 py-3">
                  <div class="flex items-start justify-between gap-3">
                    <div class="min-w-0">
                      <div class="truncate font-medium">{job.displayName}</div>
                      <div class="text-xs text-muted-foreground">
                        {copy.transferState(job.stage, job.direction)}
                      </div>
                    </div>
                    <div class="shrink-0 text-[11px] text-muted-foreground">
                      {copy.transferSummary(job.completedEntries, job.totalEntries)}
                    </div>
                  </div>

                  <Progress value={progress(job)} />

                  <div class="flex items-center justify-between gap-3 text-[11px] text-muted-foreground">
                    <span>{formatBytes(job.completedBytes)} / {formatBytes(job.totalBytes)}</span>
                    <span>
                      {#if job.warningMessage}
                        {job.warningMessage}
                      {:else if job.errorMessage}
                        {job.errorMessage}
                      {:else if job.stage === 'ready' && job.direction === 'inbound'}
                        {copy.readyToPaste}
                      {:else if job.stage === 'ready'}
                        {copy.hiddenReadyState(job.readyActionState)}
                      {/if}
                    </span>
                  </div>

                  {#if job.stage === 'ready' && job.direction === 'inbound' && job.readyActionState === 'pendingPrompt'}
                    <div class="flex items-center justify-end gap-2">
                      <Button
                        disabled={busyTransferId === job.transferId}
                        on:click={() => placeTransfer(job)}
                        size="sm"
                      >
                        {copy.placeOnClipboard}
                      </Button>
                      <Button
                        disabled={busyTransferId === job.transferId}
                        on:click={() => dismissTransfer(job)}
                        size="sm"
                        variant="outline"
                      >
                        {copy.dismiss}
                      </Button>
                    </div>
                    <p class="text-[11px] text-muted-foreground">{copy.replaceWarning}</p>
                  {:else if ['preparing', 'queued', 'downloading', 'verifying'].includes(job.stage)}
                    <div class="flex justify-end">
                      <Button
                        disabled={busyTransferId === job.transferId}
                        on:click={() => cancelTransfer(job)}
                        size="sm"
                        variant="outline"
                      >
                        {copy.cancel}
                      </Button>
                    </div>
                  {/if}
                </div>
              {/each}
            </ScrollArea>
          {/if}
        </CardContent>
      </Card>

      <Card class="shadow-sm">
        <CardHeader class="pb-3">
          <div class="flex items-center justify-between gap-3">
            <div class="flex items-center gap-2">
              <ClipboardList class="size-4 text-muted-foreground" />
              <CardTitle>{copy.copyCenter}</CardTitle>
            </div>
            <Button on:click={openCache} size="sm" variant="ghost">
              <FolderOpen class="size-4" />
              {copy.openCacheDirectory}
            </Button>
          </div>
        </CardHeader>
        <CardContent class="pt-0">
          {#if historyGroups.length === 0}
            <div class="rounded-lg border border-dashed bg-background px-3 py-3 text-xs text-muted-foreground">
              {copy.noClipboardHistory}
            </div>
          {:else}
            <ScrollArea class="max-h-[30rem] pr-1">
              <div class="space-y-3">
                {#each historyGroups as group}
                  <div class="rounded-lg border bg-background">
                    <div class="flex items-center justify-between gap-3 border-b px-3 py-2">
                      <div class="flex min-w-0 items-center gap-2">
                        {#if isMobilePlatform(groupPlatform(group))}
                          <Smartphone class="size-3.5 text-muted-foreground" />
                        {:else}
                          <Laptop2 class="size-3.5 text-muted-foreground" />
                        {/if}
                        <div class="truncate font-medium">{group.deviceName}</div>
                        {#if group.isLocal}
                          <Badge variant="secondary">{copy.thisDevice}</Badge>
                        {/if}
                      </div>
                      <div class="text-[11px] text-muted-foreground">{copy.clips(group.entries.length)}</div>
                    </div>

                    <div class="space-y-2 p-2">
                      {#each group.entries as entry}
                        <div class="rounded-md border px-3 py-2">
                          <div class="flex items-start justify-between gap-3">
                            <div class="flex min-w-0 items-start gap-2">
                              <div class="mt-0.5 text-muted-foreground">
                                {#if entry.kind === 'text'}
                                  <Type class="size-4" />
                                {:else if entry.kind === 'image'}
                                  <Image class="size-4" />
                                {:else}
                                  <ClipboardList class="size-4" />
                                {/if}
                              </div>
                              <div class="min-w-0">
                                <div class="truncate font-medium">{entry.displayName}</div>
                                <div class="text-xs text-muted-foreground">
                                  {copy.historyKind(entry.kind, entry.fileCount)} · {copy.historySource(entry.source)}
                                </div>
                              </div>
                            </div>
                            <div class="shrink-0 text-[11px] text-muted-foreground">{formatTime(entry.createdAt)}</div>
                          </div>

                          <p class="mt-2 line-clamp-2 text-xs leading-5 text-muted-foreground">
                            {historyPreview(entry)}
                          </p>

                          <div class="mt-2 flex justify-end">
                            <Button
                              disabled={busyHistoryId === entry.entryId}
                              on:click={() => restoreHistory(entry)}
                              size="sm"
                              variant="outline"
                            >
                              <Copy class="size-3.5" />
                              {copy.restoreHistory}
                            </Button>
                          </div>
                        </div>
                      {/each}
                    </div>
                  </div>
                {/each}
              </div>
            </ScrollArea>
          {/if}
        </CardContent>
      </Card>

      <Card class="shadow-sm">
        <CardContent class="flex items-center justify-between gap-3 py-3">
          <div>
            <div class="font-medium">{copy.syncEnabled}</div>
            <div class="text-xs text-muted-foreground">{syncStatus?.message ?? ''}</div>
          </div>
          <Switch
            checked={snapshot.settings.syncEnabled}
            disabled={settingsBusy}
            on:change={(event) =>
              toggleClipboardSync((event.target as HTMLInputElement).checked)}
          />
        </CardContent>
      </Card>
    </div>
  </main>
{:else}
  <main class="flex min-h-screen items-center justify-center bg-muted/40 p-4">
    <Card class="w-full max-w-sm shadow-sm">
      <CardContent class="py-10 text-center text-sm text-muted-foreground">RelayClip</CardContent>
    </Card>
  </main>
{/if}
