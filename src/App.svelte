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
</script>

<div class="app-shell window">
  <header class="title-bar">
    <div class="title">Keson Spectral Improver</div>
    <div class="controls">
      <button aria-label="minimize" class="button">–</button>
      <button aria-label="close" class="button">×</button>
    </div>
  </header>

  <section class="window-body">
    <div class="toolbar">
      <input
        class="url-input"
        type="text"
        placeholder="Colle un lien SoundCloud, Spotify, Apple Music…"
        bind:value={url}
        on:keydown={(e) => e.key === 'Enter' && handleDownload()}
      />
      <input
        class="url-input"
        type="text"
        placeholder="Dossier de sortie (optionnel)"
        bind:value={outputDir}
      />
      <button class="button accent" disabled={busy} on:click={handleDownload}>
        {busy ? '…' : 'Download'}
      </button>
    </div>

    {#if message}
      <p class="small">{message}</p>
    {/if}

    <div class="progress-row">
      <div>Actifs: {queue.active}</div>
      <div>En attente: {queue.pending}</div>
    </div>

    <h3>Derniers téléchargements</h3>
    {#if !downloads.length}
      <p class="small">Rien encore. Colle un lien pour commencer.</p>
    {:else}
      <div class="card-grid">
        {#each downloads as item, idx}
          <div class="list-card">
            <div class="badge">#{downloads.length - idx}</div>
            <h4>{item.title || 'Track'}</h4>
            <p class="small">{item.caption}</p>
            {#if item.quality}
              <p class="small">Qualité: {item.quality}</p>
            {/if}
            {#if item.warning}
              <p class="small warning">{item.warning}</p>
            {/if}
            {#if item.savedTo}
              <p class="small">Sauvé: {item.savedTo}</p>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>
