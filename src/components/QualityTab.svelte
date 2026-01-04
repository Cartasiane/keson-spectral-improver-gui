<script>
  import ScanControls from './ScanControls.svelte'
  import ScanSummary from './ScanSummary.svelte'
  import ScanRow from './ScanRow.svelte'
  import { onDestroy } from 'svelte'
  import { convertFileSrc } from '@tauri-apps/api/tauri'
  import {
    isDesktop,
    pickFolderDialog,
    scanFolder as performScan,
    revealInFolder,
    openSpectrum,
    listenScanProgress,
    redownloadBad,
    acceptRedownload,
    discardFile
  } from '../services/scanService'

  let scanFolder = ''
  let scanning = false
  let scanResults = []
  let scanMessage = ''
  let filter = 'bad'
  let progress = 0
  let progressLabel = ''
  let retrying = false
  let reviewQueue = []
  let unlistenProgress
  let spectra = {}
  let spectroLoading = {}

  $: filteredResults =
    filter === 'all'
      ? scanResults
      : scanResults.filter((r) => r.status === filter)

  async function pickFolder() {
    const choice = await pickFolderDialog()
    if (choice) scanFolder = choice
  }

  async function runScan() {
    if (!scanFolder) {
      scanMessage = 'Choisis un dossier à analyser.'
      return
    }
    if (!isDesktop) {
      scanMessage = 'Scan dispo seulement en mode desktop.'
      return
    }
    scanning = true
    scanMessage = 'Analyse en cours...'
    progress = 0
    await startProgressListener()
    try {
      const results = await performScan(scanFolder, 256)
      scanResults = results
      const bad = results.filter(r => r.status === 'bad').length
      scanMessage = bad ? `${bad} fichier(s) sous 256 kbps` : 'Tout est au-dessus de 256 kbps'
    } catch (error) {
      console.error(error)
      const msg =
        (error && (error.message || error.toString?.())) ||
        (typeof error === 'string' ? error : '') ||
        ''
      scanMessage = msg.trim() || 'Échec de l’analyse'
    } finally {
      scanning = false
      stopProgressListener()
      progress = 100
      progressLabel = ''
    }
  }

  async function startProgressListener() {
    stopProgressListener()
    if (!isDesktop) return
    unlistenProgress = await listenScanProgress(payload => {
      // Accept payload as number, string "x/y", or object { current, total, percent }
      if (typeof payload === 'number' && Number.isFinite(payload)) {
        progress = clampPercent(payload)
        progressLabel = `${Math.round(progress)}%`
        return
      }
      if (typeof payload === 'string') {
        const match = payload.match(/^(\d+)\s*\/\s*(\d+)$/)
        if (match) {
          const current = Number(match[1])
          const total = Number(match[2]) || 0
          progress = clampPercent((current / Math.max(total, 1)) * 100)
          progressLabel = `${current}/${total || '?'}`
          return
        }
        const asNumber = Number(payload)
        if (Number.isFinite(asNumber)) {
          progress = clampPercent(asNumber)
          progressLabel = `${Math.round(progress)}%`
        }
        return
      }
      if (payload && typeof payload === 'object') {
        const { current, total, percent } = payload
        if (Number.isFinite(percent)) {
          progress = clampPercent(percent)
          progressLabel = `${Math.round(progress)}%`
          return
        }
        if (Number.isFinite(current) && Number.isFinite(total)) {
          progress = clampPercent((current / Math.max(total, 1)) * 100)
          progressLabel = `${current}/${total || '?'}`
        }
      }
    })
    progress = 1
    progressLabel = '0/?'
  }

  const clampPercent = (val) => Math.max(0, Math.min(100, val))

  function stopProgressListener() {
    if (unlistenProgress) {
      unlistenProgress()
      unlistenProgress = null
    }
  }

  async function reveal(path) {
    try {
      await revealInFolder(path)
    } catch (err) {
      console.error(err)
      scanMessage = err?.message || "Impossible d’ouvrir le dossier."
    }
  }

  async function spectrum(path) {
    if (!isDesktop) return
    try {
      spectroLoading = { ...spectroLoading, [path]: true }
      spectra = { ...spectra, [path]: undefined }
      const url = await openSpectrum(path)
      spectra = { ...spectra, [path]: url }
      scanMessage = 'Spectre généré'
    } catch (err) {
      console.error(err)
      spectra = { ...spectra, [path]: 'error' }
      scanMessage = err?.message || 'Échec génération spectre.'
    } finally {
      spectroLoading = { ...spectroLoading, [path]: false }
    }
  }

  async function redownloadLow() {
    const badPaths = scanResults.filter((r) => r.status === 'bad').map((r) => r.path)
    if (!badPaths.length) {
      scanMessage = 'Aucun fichier à retélécharger.'
      return
    }
    if (!isDesktop) {
      scanMessage = 'Retéléchargement dispo seulement en desktop.'
      return
    }
    retrying = true
    scanMessage = 'Recherche SoundCloud en cours...'
    reviewQueue = []
    try {
      const saved = await redownloadBad(badPaths)
      const reviewed = buildReviewQueue(saved)
      reviewQueue = reviewed
      const flagged = reviewed.filter((r) => r.mismatch).length
      scanMessage = flagged
        ? `Retéléchargé ${saved.length}, ${flagged} à vérifier (durée différente).`
        : `Retéléchargé ${saved.length} fichier(s) (durées ok).`
    } catch (err) {
      console.error(err)
      scanMessage = typeof err === 'string' ? err : err?.message || 'Échec du retéléchargement'
    } finally {
      retrying = false
    }
  }

  function buildReviewQueue(results) {
    const toleranceSec = 2
    const tolerancePct = 0.05
    return results.map((item) => {
      if (!item.original_path || !item.new_path) return null
      if (item.new_path === 'NA') return null
      const orig = item.original_duration ?? 0
      const fresh = item.new_duration ?? 0
      const diff = Math.abs(orig - fresh)
      const rel = orig > 0 ? diff / orig : 1
      const mismatch = diff > toleranceSec && rel > tolerancePct
      return {
        ...item,
        mismatch,
        origUrl: convertFileSrc(item.original_path),
        newUrl: convertFileSrc(item.new_path)
      }
    }).filter((i) => i && i.mismatch)
  }

  async function acceptReplacement(entry) {
    try {
      await acceptRedownload(entry.original_path, entry.new_path)
      reviewQueue = reviewQueue.filter((r) => r !== entry)
      scanMessage = 'Remplacé par la version SoundCloud.'
    } catch (err) {
      console.error(err)
      scanMessage = err?.message || 'Impossible de remplacer le fichier'
    }
  }

  async function ignoreReplacement(entry) {
    try {
      await discardFile(entry.new_path)
    } catch (err) {
      console.warn('discard failed', err)
    }
    reviewQueue = reviewQueue.filter((r) => r !== entry)
  }

  onDestroy(stopProgressListener)
</script>

<section class="panel">
  <ScanControls
    bind:folder={scanFolder}
    message={scanMessage}
    scanning={scanning}
    progress={progress}
    progressLabel={progressLabel}
    on:pick={pickFolder}
    on:scan={runScan}
  />

  {#if scanResults.length}
    <ScanSummary
      results={scanResults}
      active={filter}
      on:filter={(e) => (filter = e.detail)}
    />
    <div class="actions" style="justify-content:flex-start; margin-bottom: 10px; gap:8px;">
      <button class="btn primary" disabled={retrying} on:click={redownloadLow}>
        {retrying ? 'Recherche SC…' : 'Retélécharger les LOW via SoundCloud'}
      </button>
    </div>

    {#if reviewQueue.length}
      <div class="panel review-block">
        <div class="panel-head" style="margin-bottom:6px;">
          <h2>Vérifier les durées</h2>
          <span class="badge">{reviewQueue.length}</span>
        </div>
        <div class="review-list">
          {#each reviewQueue as item}
            <div class="review-card">
              <div class="review-meta">
                <div>Durée originale : {item.original_duration ? item.original_duration.toFixed(1) + 's' : 'n/a'}</div>
                <div>Nouvelle : {item.new_duration ? item.new_duration.toFixed(1) + 's' : 'n/a'}</div>
                {#if item.mismatch}
                  <div class="warn">Durées différentes</div>
                {/if}
              </div>
              <div class="players">
                <div>
                  <p class="muted">Ancienne</p>
                  <audio controls src={item.origUrl}></audio>
                </div>
                <div>
                  <p class="muted">Nouvelle (SoundCloud)</p>
                  <audio controls src={item.newUrl}></audio>
                </div>
              </div>
              <div class="actions" style="gap:10px;">
                <button class="btn primary" on:click={() => acceptReplacement(item)}>Accepter</button>
                <button class="btn ghost" on:click={() => ignoreReplacement(item)}>Ignorer</button>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <div class="scan-table">
      <div class="scan-row head">
        <div>Statut</div>
        <div>Bitrate</div>
        <div>Lossless</div>
        <div>Nom</div>
        <div>Chemin</div>
        <div>Actions</div>
      </div>
      {#each filteredResults as item}
        <ScanRow
          {item}
          spectrumUrl={spectra[item.path]}
          loading={!!spectroLoading[item.path]}
          on:reveal={(e) => reveal(e.detail)}
          on:spectrum={(e) => spectrum(e.detail)}
        />
      {/each}
    </div>
  {/if}
</section>
