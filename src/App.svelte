<script>
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/tauri'

  let url = ''
  let outputDir = ''
  let downloads = []
  let queue = { active: 0, pending: 0 }
  let message = ''
  let busy = false
  let isMock = true
  let activeTab = 'download'

  // quality scan state
  let scanFolder = ''
  let scanning = false
  let scanResults = []
  let scanMessage = ''

  onMount(() => {
    isMock = typeof window !== 'undefined' && !window.__TAURI__
    refreshQueue()
  })

  async function handleDownload() {
    if (!url.trim()) return
    busy = true
    message = 'Préparation…'
    const payload = { url, outputDir }
    try {
      let result
      if (isMock) {
        result = await mockDownload(payload)
      } else {
        result = await invoke('download_link', payload)
      }
      downloads = [result, ...downloads].slice(0, 12)
      message = result.warning || 'Téléchargé'
    } catch (error) {
      console.error(error)
      message = error?.message || 'Erreur lors du DL'
    } finally {
      busy = false
    }
  }

  async function refreshQueue() {
    try {
      let stats
      if (isMock) {
        stats = { active: downloads.length ? 1 : 0, pending: Math.max(downloads.length - 1, 0) }
      } else {
        stats = await invoke('queue_stats')
      }
      queue = stats || { active: 0, pending: 0 }
    } catch (error) {
      console.warn('Queue stats failed', error)
    }
    setTimeout(refreshQueue, 2000)
  }

  async function mockDownload(payload) {
    await new Promise(r => setTimeout(r, 800))
    return {
      title: 'Mock track',
      caption: payload.url,
      size: '5.2 MB',
      quality: 'Authentique',
      warning: '',
      savedTo: outputDir || '~/Music/Keson'
    }
  }

  async function pickFolder() {
    if (!window.__TAURI__) return
    const { open } = await import('@tauri-apps/api/dialog')
    const choice = await open({ directory: true })
    if (choice && typeof choice === 'string') {
      scanFolder = choice
    }
  }

  async function runScan() {
    if (!scanFolder) {
      scanMessage = 'Choisis un dossier à analyser.'
      return
    }
    if (!window.__TAURI__) {
      scanMessage = 'Scan dispo seulement en mode desktop.'
      return
    }
    scanning = true
    scanMessage = 'Analyse en cours...'
    try {
      const results = await invoke('scan_folder', { folder: scanFolder, minKbps: 256 })
      scanResults = results
      const bad = results.filter(r => r.status === 'bad').length
      scanMessage = bad
        ? `${bad} fichier(s) sous 256 kbps`
        : 'Tout est au-dessus de 256 kbps'
    } catch (error) {
      console.error(error)
      scanMessage = error?.message || 'Échec de l’analyse'
    } finally {
      scanning = false
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
        <span class="stat-value">{queue.active}</span>
      </div>
      <div>
        <span class="stat-label">En attente</span>
        <span class="stat-value">{queue.pending}</span>
      </div>
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
    <section class="panel">
    <div class="fields">
      <label>
        <span>Lien</span>
        <input
          id="url"
          type="text"
          placeholder="SoundCloud, Spotify, Apple Music…"
          bind:value={url}
          on:keydown={(e) => e.key === 'Enter' && handleDownload()}
        />
      </label>
      <label>
        <span>Dossier de sortie (optionnel)</span>
        <input
          id="output"
          type="text"
          placeholder="~/Music/Keson"
          bind:value={outputDir}
        />
      </label>
      <div class="actions">
        <button class="btn primary" disabled={busy} on:click={handleDownload}>
          {busy ? '…' : 'Download'}
        </button>
      </div>
    </div>
    {#if message}
      <p class="hint">{message}</p>
    {/if}
    </section>

    <section class="panel">
    <div class="panel-head">
      <h2>Derniers téléchargements</h2>
      <span class="badge">{downloads.length}</span>
    </div>
    {#if !downloads.length}
      <p class="hint">Rien encore. Colle un lien pour commencer.</p>
    {:else}
      <div class="card-grid">
        {#each downloads as item, idx}
          <article class="card">
            <div class="card-top">
              <span class="pill">#{downloads.length - idx}</span>
              {#if item.quality}<span class="pill ghost">{item.quality}</span>{/if}
            </div>
            <h3>{item.title || 'Track'}</h3>
            <p class="muted">{item.caption}</p>
            {#if item.warning}
              <p class="warn">{item.warning}</p>
            {/if}
            {#if item.savedTo}
              <p class="muted">Sauvé : {item.savedTo}</p>
            {/if}
          </article>
        {/each}
      </div>
    {/if}
    </section>
  {:else}
    <section class="panel">
      <div class="fields">
        <label>
          <span>Dossier à analyser</span>
          <input
            type="text"
            placeholder="/chemin/vers/ta/musique"
            bind:value={scanFolder}
          />
        </label>
        <div class="actions">
          <button class="btn ghost" on:click={pickFolder}>Parcourir</button>
          <button class="btn primary" disabled={scanning} on:click={runScan}>
            {scanning ? 'Scan…' : 'Lancer le scan'}
          </button>
        </div>
      </div>
      {#if scanMessage}
        <p class="hint">{scanMessage}</p>
      {/if}
      {#if scanResults.length}
        <div class="card-grid">
          {#each scanResults as item}
            <article class="card {item.status}">
              <div class="card-top">
                <span class="pill">{item.bitrate ? `${item.bitrate} kbps` : 'n/a'}</span>
                <span class="pill ghost">{item.status === 'bad' ? 'Low' : 'OK'}</span>
              </div>
              <h3>{item.name}</h3>
              <p class="muted">{item.path}</p>
            </article>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</main>
