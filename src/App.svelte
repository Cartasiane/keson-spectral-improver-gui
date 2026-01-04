<script>
  import DownloadTab from './components/DownloadTab.svelte'
  import QualityTab from './components/QualityTab.svelte'
  import SettingsModal from './components/SettingsModal.svelte'
  import { onMount } from 'svelte'
  import { fetchSettings, persistSettings, isDesktop } from './services/settingsService'
  import { startMatrix } from './services/matrixRain'

  let activeTab = 'quality'
  let showSettings = false
  let settings = {
    min_bitrate: 256,
    analysis_window_seconds: 100,
    rayon_threads: 0,
    cache_enabled: true,
    cache_max_entries: 10000
  }
  let settingsLoading = false
  let settingsMessage = ''
  let matrixCanvas
  let stopMatrix = () => {}

  onMount(() => {
    loadSettings()
    stopMatrix = startMatrix(matrixCanvas)
    return () => stopMatrix()
  })

  async function loadSettings() {
    if (!isDesktop) return
    settingsLoading = true
    settingsMessage = ''
    try {
      const res = await fetchSettings()
      if (res) settings = { ...settings, ...res }
    } catch (err) {
      console.error(err)
      settingsMessage = err?.message || 'Impossible de charger les paramètres'
    } finally {
      settingsLoading = false
    }
  }

  async function saveSettings() {
    if (!isDesktop) {
      showSettings = false
      return
    }
    settingsLoading = true
    settingsMessage = ''
    try {
      await persistSettings(settings)
      settingsMessage = 'Paramètres sauvegardés'
      showSettings = false
    } catch (err) {
      console.error(err)
      settingsMessage = err?.message || 'Sauvegarde impossible'
    } finally {
      settingsLoading = false
    }
  }
</script>

<canvas class="matrix-bg" bind:this={matrixCanvas} aria-hidden="true"></canvas>

<main class="app">
<header class="hero">
  <div>
    <p class="eyebrow">Keson</p>
    <h1>Spectral Improver</h1>
    <p class="sub">Awaken ton KESON avec des fichiers large SPECTRE </p>
  </div>
  <div class="hero-right">
    <nav class="top-menu">
      <button class="menu-link" on:click={() => showSettings = true}>SETTINGS</button>
    </nav>
    <div class="hero-stats">
      <div>
        <span class="stat-label">Actifs</span>
        <span class="stat-value">0</span>
      </div>
      <div>
        <span class="stat-label">En attente</span>
        <span class="stat-value">0</span>
      </div>
    </div>
  </div>
</header>

  <div class="tabs">
    <button class:active={activeTab === 'quality'} on:click={() => (activeTab = 'quality')}>
      Spectral Checker
    </button>
    <button class:active={activeTab === 'download'} on:click={() => (activeTab = 'download')}>
      Download
    </button>
  </div>

{#if activeTab === 'download'}
  <DownloadTab />
{:else}
  <QualityTab />
{/if}

  {#if showSettings}
    <SettingsModal
      bind:settings
      loading={settingsLoading}
      message={settingsMessage}
      on:close={() => (showSettings = false)}
      on:save={saveSettings}
    />
  {/if}
</main>
