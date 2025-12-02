<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/tauri'
  import { listen } from '@tauri-apps/api/event'

  let scanFolder = ''
  let scanning = false
  let scanResults = []
  let scanMessage = ''
  let isMock = typeof window !== 'undefined' && !window.__TAURI__
  let progress = 0
  let unlistenProgress
  let spectra = {}

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
    progress = 0
    await startProgressListener()
    try {
      const results = await invoke('scan_folder', { folder: scanFolder, minKbps: 256 })
      scanResults = results
      const bad = results.filter(r => r.status === 'bad').length
      scanMessage = bad ? `${bad} fichier(s) sous 256 kbps` : 'Tout est au-dessus de 256 kbps'
    } catch (error) {
      console.error(error)
      scanMessage = error?.message || 'Échec de l’analyse'
    } finally {
      scanning = false
      stopProgressListener()
      progress = 100
    }
  }

  async function startProgressListener() {
    stopProgressListener()
    if (!window.__TAURI__) return
    unlistenProgress = await listen('scan_progress', event => {
      const val = Number(event.payload)
      if (Number.isFinite(val)) {
        progress = Math.max(0, Math.min(100, val))
      }
    })
    progress = 1
  }

  function stopProgressListener() {
    if (unlistenProgress) {
      unlistenProgress()
      unlistenProgress = null
    }
  }

  async function reveal(path) {
    if (!window.__TAURI__) return
    try {
      await invoke('reveal_in_folder', { path })
    } catch (err) {
      console.error(err)
      scanMessage = err?.message || "Impossible d’ouvrir le dossier."
    }
  }

  async function spectrum(path) {
    if (!window.__TAURI__) return
    try {
      const imgPath = await invoke('open_spectrum', { path })
      const url = convertFileSrc(imgPath)
      spectra = { ...spectra, [path]: url }
      scanMessage = 'Spectre généré'
    } catch (err) {
      console.error(err)
      scanMessage = err?.message || 'Échec génération spectre.'
    }
  }
</script>

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
    <div class="actions" style="gap: 8px;">
      <button class="btn ghost" on:click={pickFolder}>Parcourir</button>
      <button class="btn primary" disabled={scanning} on:click={runScan}>
        {scanning ? 'Scan…' : 'Lancer le scan'}
      </button>
    </div>
  </div>
  {#if scanMessage}
    <p class="hint">{scanMessage}</p>
  {/if}
  {#if scanning}
    <div class="progress-bar">
      <div class="fill" style={`width:${progress}%`}></div>
    </div>
  {/if}
  {#if scanResults.length}
    <div class="scan-summary">
      <span class="pill ghost">Total {scanResults.length}</span>
      <span class="pill warn">Low {scanResults.filter(r => r.status === 'bad').length}</span>
      <span class="pill">OK {scanResults.filter(r => r.status === 'ok').length}</span>
      <span class="pill">Err {scanResults.filter(r => r.status === 'error').length}</span>
    </div>
    <div class="scan-table">
      <div class="scan-row head">
        <div>Statut</div>
        <div>Bitrate</div>
        <div>Lossless</div>
        <div>Nom</div>
        <div>Chemin</div>
        <div>Actions</div>
      </div>
      {#each scanResults as item}
        <div class={`scan-row ${item.status}`}>
          <div class="status-dot">
            {#if item.status === 'bad'}⚠️{:else if item.status === 'ok'}✅{:else}⚙️{/if}
          </div>
          <div class="bitrate">{item.bitrate ? `${item.bitrate} kbps` : 'n/a'}</div>
          <div class="bitrate">{item.is_lossless === true ? 'Yes' : item.is_lossless === false ? 'No' : 'n/a'}</div>
          <div class="name">{item.name}</div>
          <div class="path" title={item.path}>{item.path}</div>
          <div class="actions actions-inline">
            <button class="btn mini ghost" on:click={() => reveal(item.path)}>Voir</button>
            <button class="btn mini" on:click={() => spectrum(item.path)}>Spectre</button>
          </div>
          {#if spectra[item.path]}
            <div class="spectrum" style="grid-column: 1 / -1;">
              <img src={spectra[item.path]} alt={`Spectrogramme ${item.name}`} loading="lazy" />
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</section>
