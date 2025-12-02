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

<div class="desktop">
  <div class="app-shell window">
    <header class="title-bar">
      <div class="title">Keson Spectral Improver</div>
      <div class="title-buttons">
        <button aria-label="minimize" class="button">–</button>
        <button aria-label="close" class="button">×</button>
      </div>
    </header>

    <section class="window-body">
      <div class="toolbar">
        <div class="field-row stretch">
          <label for="url">Lien</label>
          <input
            id="url"
            type="text"
            placeholder="SoundCloud, Spotify, Apple Music…"
            bind:value={url}
            on:keydown={(e) => e.key === 'Enter' && handleDownload()}
          />
        </div>
        <div class="field-row stretch">
          <label for="output">Dossier</label>
          <input
            id="output"
            type="text"
            placeholder="~/Music/Keson (optionnel)"
            bind:value={outputDir}
          />
        </div>
        <div class="toolbar-actions">
          <button class="button primary" disabled={busy} on:click={handleDownload}>
            {busy ? '…' : 'Download'}
          </button>
        </div>
      </div>

      <div class="status-bar">
        <span>Actifs : {queue.active}</span>
        <span>En attente : {queue.pending}</span>
        {#if message}<span>{message}</span>{/if}
      </div>

      <section class="group-box">
        <header>Derniers téléchargements</header>
        {#if !downloads.length}
          <p class="muted">Rien encore. Colle un lien pour commencer.</p>
        {:else}
          <div class="card-grid">
            {#each downloads as item, idx}
              <article class="download-card">
                <div class="card-head">
                  <span class="pill">#{downloads.length - idx}</span>
                  {#if item.quality}<span class="pill ghost">{item.quality}</span>{/if}
                </div>
                <h4>{item.title || 'Track'}</h4>
                <p class="muted">{item.caption}</p>
                {#if item.warning}
                  <p class="warning">{item.warning}</p>
                {/if}
                {#if item.savedTo}
                  <p class="muted">Sauvé : {item.savedTo}</p>
                {/if}
              </article>
            {/each}
          </div>
        {/if}
      </section>
    </section>
  </div>
</div>
