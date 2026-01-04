<script>
  import { createEventDispatcher } from 'svelte'

  export let folder = ''
  export let message = ''
  export let scanning = false
  export let progress = 0
  export let progressLabel = ''

  const dispatch = createEventDispatcher()
</script>

<div class="fields">
  <label>
    <span>Dossier à analyser</span>
    <input
      type="text"
      placeholder="/chemin/vers/ta/musique"
      bind:value={folder}
    />
  </label>
  <div class="actions" style="gap: 8px;">
    <button class="btn ghost" on:click={() => dispatch('pick')}>Parcourir</button>
    <button class="btn ghost" disabled={scanning} on:click={() => dispatch('scan')}>
      {scanning ? 'Scan…' : 'Lancer le scan'}
    </button>
  </div>
</div>
{#if message}
  <p class="hint">{message}</p>
{/if}
{#if scanning}
  <div class="progress-bar">
    <div class="fill" style={`width:${progress}%`} aria-label={progressLabel || `${Math.round(progress)}%`}></div>
    {#if progressLabel}
      <span class="progress-text">{progressLabel}</span>
    {/if}
  </div>
{/if}
