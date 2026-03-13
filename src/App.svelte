<script lang="ts">
  import { onMount } from 'svelte'
  import {
    cancelTransferJob,
    dismissTransferJob,
    getAppState,
    isAutostartEnabled,
    listTransferJobs,
    onClipboardError,
    onDevicesUpdated,
    onSyncStatusChanged,
    onTransferJobsUpdated,
    onTransferReady,
    placeReceivedTransferOnClipboard,
    setActiveDevice,
    syncAutostart,
    toggleSync,
    updateSettings,
  } from './lib/api'
  import { getMessages, normalizeLanguage } from './lib/i18n'
  import type { AppStateSnapshot, TransferJob } from './lib/types'
  import {
    isPermissionGranted,
    requestPermission,
    sendNotification,
  } from '@tauri-apps/plugin-notification'

  let snapshot: AppStateSnapshot | null = null
  let transferJobs: TransferJob[] = []
  let busyTransferId: string | null = null
  let settingsBusy = false
  let errorBanner: string | null = null
  const notifiedReady = new Set<string>()

  $: currentLanguage =
    snapshot?.settings.language ??
    normalizeLanguage(typeof navigator === 'undefined' ? undefined : navigator.language)
  $: copy = getMessages(currentLanguage)
  $: devices = snapshot?.devices ?? []
  $: activeDevice = devices.find((device) => device.isActive) ?? null
  $: nearbyDevices = devices.filter(
    (device) =>
      !device.isActive &&
      device.isOnline &&
      device.deviceId !== (snapshot?.localDevice.deviceId ?? ''),
  )
  $: visibleTransferJobs = transferJobs
  $: syncStatus = snapshot?.syncStatus

  async function refresh() {
    const [state, jobs] = await Promise.all([getAppState(), listTransferJobs()])
    snapshot = state
    transferJobs = jobs

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
      errorBanner = error instanceof Error ? error.message : String(error)
    }
  }

  async function patchSettings(patch: Parameters<typeof updateSettings>[0]) {
    if (!snapshot) return
    settingsBusy = true
    errorBanner = null

    try {
      snapshot = await updateSettings(patch)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      settingsBusy = false
    }
  }

  async function pairDevice(deviceId: string) {
    errorBanner = null
    try {
      snapshot = await setActiveDevice(deviceId)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
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
  <main class="shell">
    <section class="toolbox glass">
      <header class="topbar">
        <div class="title-stack">
          <p class="eyebrow">{copy.title}</p>
          <input
            class="device-name-input"
            disabled={settingsBusy}
            on:change={(event) =>
              patchSettings({ deviceName: (event.currentTarget as HTMLInputElement).value })}
            spellcheck="false"
            type="text"
            value={snapshot.settings.deviceName}
          />
        </div>
        <div class={`status-pill ${syncStatus?.state ?? 'idle'}`}>
          <strong>{copy.syncState(syncStatus?.state ?? 'idle')}</strong>
          <small>{syncStatus?.message ?? ''}</small>
        </div>
      </header>

      {#if errorBanner}
        <div class="banner">{errorBanner}</div>
      {/if}

      <section class="card glass inner">
        <div class="section-head">
          <span>{copy.currentPair}</span>
        </div>
        {#if activeDevice}
          <article class="pair-card">
            <div>
              <strong>{activeDevice.name}</strong>
              <small>{activeDevice.platform} · {activeDevice.isOnline ? copy.online : copy.offline}</small>
            </div>
            <span class="pill success">{copy.activeNow}</span>
          </article>
        {:else}
          <article class="empty-card">
            <strong>{copy.noActivePair}</strong>
            <small>{copy.noActivePairHint}</small>
          </article>
        {/if}
      </section>

      <section class="card glass inner">
        <div class="section-head">
          <span>{copy.nearby}</span>
        </div>
        <div class="device-list">
          {#if nearbyDevices.length === 0}
            <article class="empty-card compact">
              <strong>{copy.waitingDevices}</strong>
              <small>{copy.waitingDevicesHint}</small>
            </article>
          {:else}
            {#each nearbyDevices as device}
              <article class="device-row">
                <div class="device-meta">
                  <strong>{device.name}</strong>
                  <small>{device.platform}</small>
                </div>
                <button class="primary compact" on:click={() => pairDevice(device.deviceId)} type="button">
                  {copy.makeActive}
                </button>
              </article>
            {/each}
          {/if}
        </div>
      </section>

      <section class="card glass inner">
        <div class="section-head">
          <span>{copy.transfers}</span>
        </div>
        <div class="transfer-list">
          {#if visibleTransferJobs.length === 0}
            <article class="empty-card compact">
              <strong>{copy.noTransfers}</strong>
            </article>
          {:else}
            {#each visibleTransferJobs as job}
              <article class:ready={job.stage === 'ready' && job.readyActionState === 'pendingPrompt'} class="transfer-row">
                <div class="transfer-top">
                  <div>
                    <strong>{job.displayName}</strong>
                    <small>{copy.transferState(job.stage, job.direction)}</small>
                  </div>
                  <small>{copy.transferSummary(job.completedEntries, job.totalEntries)}</small>
                </div>

                <div class="meter">
                  <div class="meter-bar" style={`width: ${progress(job)}%`}></div>
                </div>

                <div class="transfer-meta">
                  <small>{formatBytes(job.completedBytes)} / {formatBytes(job.totalBytes)}</small>
                  {#if job.warningMessage}
                    <small>{job.warningMessage}</small>
                  {:else if job.errorMessage}
                    <small>{job.errorMessage}</small>
                  {:else if job.stage === 'ready' && job.direction === 'inbound'}
                    <small>{copy.readyToPaste}</small>
                  {:else if job.stage === 'ready'}
                    <small>{copy.hiddenReadyState(job.readyActionState)}</small>
                  {/if}
                </div>

                {#if job.stage === 'ready' && job.direction === 'inbound' && job.readyActionState === 'pendingPrompt'}
                  <div class="action-row">
                    <button
                      class="primary"
                      disabled={busyTransferId === job.transferId}
                      on:click={() => placeTransfer(job)}
                      type="button"
                    >
                      {copy.placeOnClipboard}
                    </button>
                    <button
                      class="ghost"
                      disabled={busyTransferId === job.transferId}
                      on:click={() => dismissTransfer(job)}
                      type="button"
                    >
                      {copy.dismiss}
                    </button>
                  </div>
                  <small class="warning">{copy.replaceWarning}</small>
                {:else if ['preparing', 'queued', 'downloading', 'verifying'].includes(job.stage)}
                  <div class="action-row">
                    <button
                      class="ghost"
                      disabled={busyTransferId === job.transferId}
                      on:click={() => cancelTransfer(job)}
                      type="button"
                    >
                      {copy.cancel}
                    </button>
                  </div>
                {/if}
              </article>
            {/each}
          {/if}
        </div>
      </section>

      <footer class="card glass inner footer">
        <label class="toggle-row">
          <span>{copy.syncEnabled}</span>
          <input
            checked={snapshot.settings.syncEnabled}
            disabled={settingsBusy}
            on:change={(event) => toggleClipboardSync((event.currentTarget as HTMLInputElement).checked)}
            type="checkbox"
          />
        </label>
      </footer>
    </section>
  </main>
{:else}
  <main class="loading">
    <div class="glass loader">RelayClip</div>
  </main>
{/if}
