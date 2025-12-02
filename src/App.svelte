<script>
  import DownloadTab from './components/DownloadTab.svelte'
  import QualityTab from './components/QualityTab.svelte'

  let activeTab = 'download'
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
</main>
