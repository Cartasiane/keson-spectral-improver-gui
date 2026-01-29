<script>
  import { createEventDispatcher } from "svelte";
  import { FolderSearch } from "lucide-svelte";

  export let folder = "";
  export let message = "";
  export let scanning = false;
  export let progress = 0;
  export let progressLabel = "";

  const dispatch = createEventDispatcher();
</script>

<div class="fields">
  <label>
    <span>Dossier à analyser</span>
    <div class="input-with-button">
      <input
        type="text"
        placeholder="/chemin/vers/ta/musique"
        bind:value={folder}
      />
      <button
        class="btn icon-only"
        title="Choisir un dossier"
        on:click={() => dispatch("pick")}
      >
        <FolderSearch size={16} />
      </button>
    </div>
  </label>
  <div class="actions" style="gap: 8px;">
    <button
      class="btn ghost"
      class:scanning
      disabled={scanning}
      on:click={() => dispatch("scan")}
    >
      {#if scanning}
        Analyse en cours...
      {:else}
        Lancer le scan
      {/if}
    </button>
  </div>
</div>
{#if message}
  <p class="hint">{message}</p>
{/if}
{#if scanning}
  <div class="progress-bar">
    <div
      class="fill"
      style={`width:${progress}%`}
      aria-label={progressLabel || `${Math.round(progress)}%`}
    ></div>
    {#if progressLabel}
      <span class="progress-text">{progressLabel}</span>
    {/if}
  </div>
{/if}

<style>
  .input-with-button {
    display: flex;
    gap: 0.5rem;
    align-items: stretch;
  }
  .input-with-button input {
    flex: 1;
  }
  .btn.icon-only {
    padding: 12px;
    background: var(--surface-1, #111);
    border: 1px solid var(--border, #333);
    color: var(--text, #fff);
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;
  }
  .btn.icon-only:hover {
    background: var(--surface-3, #222);
    border-color: var(--accent, #39ff14);
    color: var(--accent, #39ff14);
  }

  .btn.ghost.scanning {
    min-width: 140px;
    padding: 8px 16px;
  }
</style>
