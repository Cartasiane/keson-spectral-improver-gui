<script>
  import DownloadTab from './components/DownloadTab.svelte'
  import QualityTab from './components/QualityTab.svelte'
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/tauri'

  let activeTab = 'download'
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

  onMount(loadSettings)

  async function loadSettings() {
    if (!window.__TAURI__) return
    settingsLoading = true
    settingsMessage = ''
    try {
      const res = await invoke('get_settings')
      settings = { ...settings, ...res }
    } catch (err) {
      console.error(err)
      settingsMessage = err?.message || 'Impossible de charger les paramètres'
    } finally {
      settingsLoading = false
    }
  }

  async function saveSettings() {
    if (!window.__TAURI__) {
      showSettings = false
      return
    }
    settingsLoading = true
    settingsMessage = ''
    try {
      await invoke('save_settings', { settings })
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

<main class="app">
<header class="hero">
  <div>
    <p class="eyebrow">Keson</p>
    <h1>Spectral Improver</h1>
    <p class="sub">Colle un lien, choisis un dossier, on s’occupe du DL.</p>
  </div>
  <div class="hero-stats">
    <div>
      <span class="stat-label">Actifs</span>
      <span class="stat-value">0</span>
    </div>
    <div>
      <span class="stat-label">En attente</span>
      <span class="stat-value">0</span>
    </div>
    <button class="icon-btn" aria-label="Paramètres" on:click={() => showSettings = true}>⚙️</button>
  </div>
</header>

  <div class="tabs">
    <button class:active={activeTab === 'download'} on:click={() => (activeTab = 'download')}>
      Download
    </button>
    <button class:active={activeTab === 'quality'} on:click={() => (activeTab = 'quality')}>
      Scan qualité
    </button>
  </div>

{#if activeTab === 'download'}
  <DownloadTab />
{:else}
  <QualityTab />
{/if}

  {#if showSettings}
    <div
      class="modal-backdrop"
      role="button"
      tabindex="0"
      aria-label="Fermer les paramètres"
      on:click={() => (showSettings = false)}
      on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && (showSettings = false)}
    />
    <div class="modal">
      <div class="modal-head">
        <h3>Paramètres</h3>
        <button class="icon-btn" on:click={() => (showSettings = false)}>✖️</button>
      </div>
      <div class="settings-grid">
        <label>
          <span>Seuil bas (kbps)</span>
          <input type="number" min="32" max="1000" bind:value={settings.min_bitrate} />
        </label>
        <label>
          <span>Fenêtre d’analyse (s)</span>
          <input type="number" min="10" max="300" bind:value={settings.analysis_window_seconds} />
        </label>
        <label>
          <span>Threads Rayon (0 = auto)</span>
          <input type="number" min="0" max="64" bind:value={settings.rayon_threads} />
        </label>
        <label class="row">
          <input type="checkbox" bind:checked={settings.cache_enabled} />
          <span>Activer le cache d’analyse</span>
        </label>
        <label>
          <span>Taille max du cache (entrées)</span>
          <input type="number" min="0" max="200000" bind:value={settings.cache_max_entries} />
        </label>
      </div>
      {#if settingsMessage}
        <p class="hint">{settingsMessage}</p>
      {/if}
      <div class="actions" style="justify-content:flex-end; gap:8px; margin-top:10px;">
        <button class="btn ghost" on:click={() => showSettings = false}>Annuler</button>
        <button class="btn primary" disabled={settingsLoading} on:click={saveSettings}>
          {settingsLoading ? '...':'Enregistrer'}
        </button>
      </div>
    </div>
  {/if}
</main>
