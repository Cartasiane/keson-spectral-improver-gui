<script>
  import { createEventDispatcher } from "svelte";
  import { Link } from "lucide-svelte";

  export let track = null;
  export let error = "";

  const dispatch = createEventDispatcher();

  let urlInput = "";

  $: if (track) {
    urlInput = "";
    error = "";
  }

  function close() {
    dispatch("close");
  }

  function submit() {
    const url = urlInput.trim();

    // Validate URL format
    const isValid =
      url.includes("tidal.com/") || url.includes("soundcloud.com/");
    if (!isValid) {
      dispatch("error", "URL invalide. Utilisez une URL Tidal ou SoundCloud.");
      return;
    }

    dispatch("submit", url);
  }

  function handleKeydown(e) {
    if (e.key === "Escape") close();
    if (e.key === "Enter") submit();
  }
</script>

{#if track}
  <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
  <div
    class="modal-overlay"
    on:click={close}
    on:keydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <div class="modal" on:click|stopPropagation role="document">
      <h3><Link size={20} /> Piste non trouvée</h3>
      <p class="muted">
        "{track.name || track.path.split("/").pop()}" n'a pas été trouvé
        automatiquement.
      </p>
      <p>Entrez une URL Tidal ou SoundCloud :</p>
      <input
        type="url"
        bind:value={urlInput}
        placeholder="https://tidal.com/... ou https://soundcloud.com/..."
        class="url-input"
        on:keydown={handleKeydown}
      />
      {#if error}
        <p class="error">{error}</p>
      {/if}
      <div class="modal-actions">
        <button class="btn primary" on:click={submit}> Télécharger </button>
        <button class="btn ghost" on:click={close}> Ignorer </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: var(--bg-card, #1a1a2e);
    border-radius: 12px;
    padding: 24px;
    max-width: 500px;
    width: 90%;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  }

  .modal h3 {
    margin: 0 0 12px;
    font-size: 1.3rem;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .modal .muted {
    color: var(--text-muted, #888);
    font-size: 0.9rem;
    margin-bottom: 16px;
  }

  .url-input {
    width: 100%;
    padding: 12px;
    border: 1px solid var(--border, #333);
    border-radius: 8px;
    background: var(--bg-input, #0f0f1a);
    color: var(--text, #fff);
    font-size: 1rem;
    margin-bottom: 8px;
    box-sizing: border-box;
  }

  .url-input:focus {
    outline: none;
    border-color: var(--primary, #646cff);
  }

  .error {
    color: #ff6b6b;
    font-size: 0.85rem;
    margin: 8px 0;
  }

  .modal-actions {
    display: flex;
    gap: 10px;
    margin-top: 16px;
    justify-content: flex-end;
  }
</style>
