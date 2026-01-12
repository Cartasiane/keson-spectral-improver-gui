<script>
  import { createEventDispatcher } from "svelte";

  export let trackCount = 0;
  export let show = false;

  const dispatch = createEventDispatcher();
  import { RefreshCw, Folder, Info } from "lucide-svelte";

  let source = "auto";
  let backup = true;

  const tooltips = {
    source:
      "Auto : cherche d'abord sur Tidal (meilleure qualité), puis SoundCloud en fallback.",
    auto: "Essaie Tidal d'abord (lossless), puis SoundCloud si non trouvé.",
    tidal: "FLAC / Hi-Res. Nécessite un abonnement Tidal HiFi.",
    soundcloud: "MP3 256 kbps max. Plus rapide, mais qualité inférieure.",
    backup:
      "Déplace les fichiers originaux dans un dossier .backup/ avant remplacement.",
  };

  function cancel() {
    dispatch("cancel");
  }

  function start() {
    dispatch("start", { source, backup });
  }

  function handleKeydown(e) {
    if (e.key === "Escape") cancel();
  }
</script>

{#if show}
  <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
  <div
    class="modal-overlay"
    on:click={cancel}
    on:keydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <div class="modal" on:click|stopPropagation role="document">
      <div class="modal-header">
        <h3><RefreshCw size={20} /> Options de retéléchargement</h3>
        <button class="close-btn" on:click={cancel}>×</button>
      </div>

      <div class="modal-body">
        <p class="track-count">
          <Folder size={16} />
          {trackCount} fichier{trackCount > 1 ? "s" : ""} sélectionné{trackCount >
          1
            ? "s"
            : ""}
        </p>

        <div class="option-group">
          <span class="option-label">
            Source
            <span class="tooltip" title={tooltips.source}
              ><Info size={14} /></span
            >
          </span>
          <div class="radio-group">
            <label class="radio-option" title={tooltips.auto}>
              <input type="radio" bind:group={source} value="auto" />
              <span>Auto (Tidal → SoundCloud)</span>
            </label>
            <label class="radio-option" title={tooltips.tidal}>
              <input type="radio" bind:group={source} value="tidal" />
              <span>Tidal uniquement</span>
            </label>
            <label class="radio-option" title={tooltips.soundcloud}>
              <input type="radio" bind:group={source} value="soundcloud" />
              <span>SoundCloud uniquement</span>
            </label>
          </div>
        </div>

        <div class="option-group">
          <label class="checkbox-option" title={tooltips.backup}>
            <input type="checkbox" bind:checked={backup} />
            <span>Sauvegarder les originaux</span>
            <span class="tooltip"><Info size={14} /></span>
          </label>
        </div>
      </div>

      <div class="modal-footer">
        <button class="btn ghost" on:click={cancel}>Annuler</button>
        <button class="btn primary" on:click={start}>Démarrer</button>
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
    background: rgba(0, 0, 0, 0.75);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }

  .modal {
    background: var(--bg-card, #1a1a2e);
    border-radius: 16px;
    padding: 0;
    max-width: 420px;
    width: 90%;
    box-shadow: 0 12px 48px rgba(0, 0, 0, 0.5);
    border: 1px solid var(--border, #333);
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 24px;
    border-bottom: 1px solid var(--border, #333);
  }

  .modal-header h3 {
    margin: 0;
    font-size: 1.2rem;
    font-weight: 600;
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--text-muted, #888);
    font-size: 1.5rem;
    cursor: pointer;
    padding: 0;
    line-height: 1;
    transition: color 0.2s;
  }

  .close-btn:hover {
    color: var(--text, #fff);
  }

  .modal-body {
    padding: 24px;
  }

  .track-count {
    margin: 0 0 20px;
    padding: 12px 16px;
    background: var(--bg-input, #0f0f1a);
    border-radius: 8px;
    font-size: 0.95rem;
  }

  .option-group {
    margin-bottom: 20px;
  }

  .option-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 500;
    margin-bottom: 10px;
    color: var(--text, #fff);
  }

  .tooltip {
    color: var(--text-muted, #888);
    cursor: help;
    font-size: 0.85rem;
  }

  .radio-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .radio-option,
  .checkbox-option {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    background: var(--bg-input, #0f0f1a);
    border-radius: 8px;
    cursor: pointer;
    transition:
      background 0.2s,
      border-color 0.2s;
    border: 1px solid transparent;
  }

  .radio-option:hover,
  .checkbox-option:hover {
    background: var(--bg-hover, #252540);
    border-color: var(--border, #444);
  }

  .radio-option input,
  .checkbox-option input {
    accent-color: var(--primary, #646cff);
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    padding: 16px 24px;
    border-top: 1px solid var(--border, #333);
    background: var(--bg-input, #0f0f1a);
  }
</style>
