<script lang="ts">
  import { onMount } from 'svelte'
  import {
    getAppState,
    isAutostartEnabled,
    onClipboardError,
    onDevicesUpdated,
    onSyncStatusChanged,
    setActiveDevice,
    syncAutostart,
    toggleSync,
    updateSettings,
  } from './lib/api'
  import { getMessages, languageLabel, normalizeLanguage } from './lib/i18n'
  import type { AppLanguage, AppStateSnapshot, SyncStatus } from './lib/types'

  const languageOptions: AppLanguage[] = ['en', 'zh-CN']

  let snapshot: AppStateSnapshot | null = null
  let selectedPane: 'overview' | 'device' = 'overview'
  let selectedDeviceId: string | null = null
  let busy = false
  let errorBanner: string | null = null

  $: currentLanguage =
    snapshot?.settings.language ??
    normalizeLanguage(typeof navigator === 'undefined' ? undefined : navigator.language)
  $: copy = getMessages(currentLanguage)
  $: devices = snapshot?.devices ?? []
  $: syncStatus = snapshot?.syncStatus
  $: selectedDevice =
    devices.find((device) => device.deviceId === selectedDeviceId) ??
    devices.find((device) => device.isActive) ??
    null
  $: if (!selectedDevice && devices.length === 0) {
    selectedPane = 'overview'
  }

  function applySnapshot(next: AppStateSnapshot) {
    snapshot = next

    if (!selectedDeviceId) {
      selectedDeviceId = next.settings.activeDeviceId ?? next.devices[0]?.deviceId ?? null
    }

    if (
      selectedDeviceId &&
      !next.devices.some((device) => device.deviceId === selectedDeviceId)
    ) {
      selectedDeviceId = next.settings.activeDeviceId ?? next.devices[0]?.deviceId ?? null
    }
  }

  async function refresh() {
    const state = await getAppState()
    applySnapshot(state)

    const autostartEnabled = await isAutostartEnabled().catch(() => state.settings.launchOnLogin)
    if (autostartEnabled !== state.settings.launchOnLogin) {
      const corrected = await updateSettings({ launchOnLogin: autostartEnabled })
      applySnapshot(corrected)
    }
  }

  async function patchSettings(patch: Parameters<typeof updateSettings>[0]) {
    busy = true
    errorBanner = null

    try {
      if (typeof patch.launchOnLogin === 'boolean') {
        await syncAutostart(patch.launchOnLogin)
      }

      const next = await updateSettings(patch)
      applySnapshot(next)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      busy = false
    }
  }

  async function changeActiveDevice(deviceId: string) {
    busy = true
    errorBanner = null

    try {
      const next = await setActiveDevice(deviceId)
      selectedPane = 'device'
      selectedDeviceId = deviceId
      applySnapshot(next)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      busy = false
    }
  }

  async function flipSync(enabled: boolean) {
    busy = true
    errorBanner = null

    try {
      const next = await toggleSync(enabled)
      applySnapshot(next)
    } catch (error) {
      errorBanner = error instanceof Error ? error.message : String(error)
    } finally {
      busy = false
    }
  }

  function statusTone(status?: SyncStatus) {
    switch (status?.state) {
      case 'connected':
        return 'good'
      case 'syncing':
        return 'live'
      case 'paused':
        return 'muted'
      case 'error':
        return 'danger'
      default:
        return 'default'
    }
  }

  function prettyDate(value: string | null | undefined) {
    if (!value) {
      return copy.never
    }

    const date = new Date(value)
    return Number.isNaN(date.getTime()) ? value : date.toLocaleString(currentLanguage)
  }

  onMount(() => {
    let disposed = false

    void refresh()

    const unlisten = Promise.all([
      onDevicesUpdated((devices) => {
        if (!snapshot || disposed) {
          return
        }

        snapshot = { ...snapshot, devices }
      }),
      onSyncStatusChanged((status) => {
        if (!snapshot || disposed) {
          return
        }

        snapshot = { ...snapshot, syncStatus: status }
      }),
      onClipboardError((message) => {
        if (!disposed) {
          errorBanner = message
        }
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
    <aside class="sidebar glass">
      <div class="brand">
        <div class="brand-mark">RC</div>
        <div>
          <p>RelayClip</p>
          <small>{copy.relayHub(snapshot.localDevice.platform)}</small>
        </div>
      </div>

      <button
        class:selected={selectedPane === 'overview'}
        class="nav-card"
        on:click={() => {
          selectedPane = 'overview'
          selectedDeviceId = null
        }}
        type="button"
      >
        <strong>{copy.overview}</strong>
        <span>{copy.trustedDeviceCount(devices.length)}</span>
      </button>

      <div class="device-stack">
        <div class="stack-head">
          <span>{copy.activePairing}</span>
          <small>
            {snapshot.settings.activeDeviceId ? copy.switchTarget : copy.chooseFirstDevice}
          </small>
        </div>

        {#if devices.length === 0}
          <div class="empty-state">
            <strong>{copy.waitingNearbyDevices}</strong>
            <p>{copy.waitingNearbyDevicesDesc}</p>
          </div>
        {:else}
          {#each devices as device}
            <button
              class:selected={selectedDeviceId === device.deviceId}
              class="device-card"
              on:click={() => {
                selectedPane = 'device'
                selectedDeviceId = device.deviceId
              }}
              type="button"
            >
              <div class="device-card-top">
                <strong>{device.name}</strong>
                {#if device.isActive}
                  <span class="pill accent">{copy.current}</span>
                {/if}
              </div>
              <span>{device.platform}</span>
              <small>
                {device.isOnline ? copy.onlineNow : copy.offline} ·
                {device.autoTrusted ? copy.autoTrusted : copy.knownPeer}
              </small>
            </button>
          {/each}
        {/if}
      </div>
    </aside>

    <section class="workspace">
      <header class="workspace-head glass">
        <div>
          <p class="eyebrow">{copy.status}</p>
          <h1>{copy.headline}</h1>
          <small>{copy.subheadline}</small>
        </div>

        <div class={`status-pill ${statusTone(syncStatus)}`}>
          <span>{copy.statusState(syncStatus?.state ?? 'idle')}</span>
          <small>{syncStatus?.message ?? copy.waitingForActivity}</small>
        </div>
      </header>

      {#if errorBanner}
        <div class="banner error">
          <strong>{copy.clipboardEvent}</strong>
          <span>{errorBanner}</span>
        </div>
      {/if}

      {#if selectedPane === 'overview'}
        <div class="panel-grid">
          <article class="panel glass hero">
            <div class="hero-copy">
              <p class="eyebrow">{copy.localDevice}</p>
              <h2>{snapshot.localDevice.deviceName}</h2>
              <small>
                {snapshot.localDevice.platform} · {snapshot.localDevice.protocolVersion} ·
                {snapshot.localDevice.capabilities.join(', ')}
              </small>
            </div>

            <dl class="specs">
              <div>
                <dt>{copy.fingerprint}</dt>
                <dd>{snapshot.localDevice.fingerprint.slice(0, 16)}...</dd>
              </div>
              <div>
                <dt>{copy.lastPayload}</dt>
                <dd>
                  {#if syncStatus?.lastPayload}
                    {copy.payload(
                      syncStatus.lastPayload.kind,
                      Math.round(syncStatus.lastPayload.size / 1024),
                    )}
                  {:else}
                    {copy.noRelayYet}
                  {/if}
                </dd>
              </div>
              <div>
                <dt>{copy.updated}</dt>
                <dd>{prettyDate(syncStatus?.updatedAt)}</dd>
              </div>
            </dl>
          </article>

          <article class="panel glass settings">
            <div class="panel-head">
              <div>
                <p class="eyebrow">{copy.preferences}</p>
                <h2>{copy.systemBehavior}</h2>
              </div>
            </div>

            <div class="setting-list">
              <label class="setting-row">
                <div>
                  <strong>{copy.language}</strong>
                  <small>{copy.languageDescription}</small>
                </div>
                <div class="segment-group" role="group" aria-label={copy.language}>
                  {#each languageOptions as language}
                    <button
                      class:active={snapshot.settings.language === language}
                      class="segment-button"
                      disabled={busy}
                      on:click={() => patchSettings({ language })}
                      type="button"
                    >
                      {languageLabel(language)}
                    </button>
                  {/each}
                </div>
              </label>

              <label class="setting-row">
                <div>
                  <strong>{copy.deviceName}</strong>
                  <small>{copy.deviceNameDescription}</small>
                </div>
                <input
                  disabled={busy}
                  on:change={(event) =>
                    patchSettings({ deviceName: (event.currentTarget as HTMLInputElement).value })
                  }
                  type="text"
                  value={snapshot.settings.deviceName}
                />
              </label>

              <label class="setting-row">
                <div>
                  <strong>{copy.launchOnLogin}</strong>
                  <small>{copy.launchOnLoginDescription}</small>
                </div>
                <input
                  checked={snapshot.settings.launchOnLogin}
                  disabled={busy}
                  on:change={(event) =>
                    patchSettings({ launchOnLogin: (event.currentTarget as HTMLInputElement).checked })
                  }
                  type="checkbox"
                />
              </label>

              <label class="setting-row">
                <div>
                  <strong>{copy.lanDiscovery}</strong>
                  <small>{copy.lanDiscoveryDescription}</small>
                </div>
                <input
                  checked={snapshot.settings.discoveryEnabled}
                  disabled={busy}
                  on:change={(event) =>
                    patchSettings({ discoveryEnabled: (event.currentTarget as HTMLInputElement).checked })
                  }
                  type="checkbox"
                />
              </label>

              <label class="setting-row">
                <div>
                  <strong>{copy.clipboardSync}</strong>
                  <small>{copy.clipboardSyncDescription}</small>
                </div>
                <input
                  checked={snapshot.settings.syncEnabled}
                  disabled={busy}
                  on:change={(event) => flipSync((event.currentTarget as HTMLInputElement).checked)}
                  type="checkbox"
                />
              </label>
            </div>
          </article>

          <article class="panel glass roster">
            <div class="panel-head">
              <div>
                <p class="eyebrow">{copy.trustedPeers}</p>
                <h2>{copy.availableDevices}</h2>
              </div>
              <span class="pill">{copy.onlineCount(devices.filter((device) => device.isOnline).length)}</span>
            </div>

            <div class="roster-list">
              {#each devices as device}
                <div class="roster-row">
                  <div>
                    <strong>{device.name}</strong>
                    <small>{device.platform} · {device.addresses.join(', ') || copy.noAddressYet}</small>
                  </div>
                  <div class="roster-actions">
                    <span class={`pill ${device.isOnline ? 'accent' : ''}`}>
                      {device.isOnline ? copy.reachable : copy.offline}
                    </span>
                    <button
                      class="ghost-button"
                      disabled={busy}
                      on:click={() => changeActiveDevice(device.deviceId)}
                      type="button"
                    >
                      {device.isActive ? copy.selected : copy.setActive}
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          </article>
        </div>
      {:else if selectedDevice}
        <div class="panel-grid detail">
          <article class="panel glass hero">
            <div class="panel-head">
              <div>
                <p class="eyebrow">{copy.deviceDetail}</p>
                <h2>{selectedDevice.name}</h2>
              </div>
              <div class="chip-row">
                <span class={`pill ${selectedDevice.isOnline ? 'accent' : ''}`}>
                  {selectedDevice.isOnline ? copy.onlineNow : copy.offline}
                </span>
                {#if selectedDevice.isActive}
                  <span class="pill accent">{copy.currentTarget}</span>
                {/if}
              </div>
            </div>

            <dl class="detail-grid">
              <div>
                <dt>{copy.platform}</dt>
                <dd>{selectedDevice.platform}</dd>
              </div>
              <div>
                <dt>{copy.capabilities}</dt>
                <dd>{selectedDevice.capabilities.join(', ')}</dd>
              </div>
              <div>
                <dt>{copy.host}</dt>
                <dd>{selectedDevice.hostName ?? copy.unknown}</dd>
              </div>
              <div>
                <dt>{copy.port}</dt>
                <dd>{selectedDevice.port ?? copy.pending}</dd>
              </div>
              <div>
                <dt>{copy.lastSeen}</dt>
                <dd>{prettyDate(selectedDevice.lastSeen)}</dd>
              </div>
              <div>
                <dt>{copy.lastSync}</dt>
                <dd>{selectedDevice.lastSyncStatus ?? copy.noSyncHistoryYet}</dd>
              </div>
            </dl>
          </article>

          <article class="panel glass settings">
            <div class="panel-head">
              <div>
                <p class="eyebrow">{copy.routing}</p>
                <h2>{copy.pairThisDevice}</h2>
              </div>
            </div>

            <p class="detail-copy">{copy.routingDescription}</p>

            <div class="action-row">
              <button
                class="primary-button"
                disabled={busy || selectedDevice.isActive}
                on:click={() => changeActiveDevice(selectedDevice.deviceId)}
                type="button"
              >
                {selectedDevice.isActive ? copy.alreadyActive : copy.makeActiveDevice}
              </button>

              <button
                class="ghost-button"
                on:click={() => {
                  selectedPane = 'overview'
                  selectedDeviceId = null
                }}
                type="button"
              >
                {copy.backToOverview}
              </button>
            </div>
          </article>
        </div>
      {/if}
    </section>
  </main>
{:else}
  <main class="loading">
    <div class="loader glass">
      <strong>{copy.loadingTitle}</strong>
      <small>{copy.loadingSubtitle}</small>
    </div>
  </main>
{/if}
