<script>
  import { createEventDispatcher } from 'svelte'

  export let item
  export let spectrumUrl
  export let loading = false

  const dispatch = createEventDispatcher()
</script>

<div class={`scan-row ${item.status}`}>
  <div class="status-dot">
    {#if item.status === 'bad'}⚠️{:else if item.status === 'ok'}✅{:else}⚙️{/if}
  </div>
  <div class="bitrate">{item.bitrate ? `${item.bitrate} kbps` : 'n/a'}</div>
  <div class="bitrate">{item.is_lossless === true ? 'Yes' : item.is_lossless === false ? 'No' : 'n/a'}</div>
  <div class="name">{item.name}</div>
  <div class="path" title={item.path}>{item.path}</div>
  <div class="actions actions-inline">
    <button class="btn mini ghost" on:click={() => dispatch('reveal', item.path)}>Voir</button>
    <button class="btn mini" on:click={() => dispatch('spectrum', item.path)}>Spectre</button>
  </div>
  {#if loading || spectrumUrl}
    <div class="spectrum" style="grid-column: 1 / -1;">
      {#if loading}
        <div class="skeleton"></div>
      {:else if spectrumUrl === 'error'}
        <div class="skeleton error">Spectre indisponible</div>
      {:else}
        <img src={spectrumUrl} alt={`Spectrogramme ${item.name}`} loading="lazy" />
      {/if}
    </div>
  {/if}
</div>
