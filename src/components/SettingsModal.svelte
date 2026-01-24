<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  export let settings;
  export let loading = false;
  export let message = "";

  const dispatch = createEventDispatcher();

  async function openLogs() {
    try {
      await invoke("open_logs_folder");
    } catch (e) {
      alert("Erreur: " + e);
    }
  }

  function updateSetting(key, value) {
    settings = { ...settings, [key]: value };
  }

  function closeOnBackdrop(event) {
    if (event.type === "click" || event.key === "Enter" || event.key === " ") {
      dispatch("close");
    }
  }
</script>

<div
  class="modal-backdrop"
  role="button"
  tabindex="0"
  aria-label="Fermer les paramètres"
  on:click={closeOnBackdrop}
  on:keydown={closeOnBackdrop}
/>
<div class="modal">
  <div class="modal-head">
    <h3>Paramètres</h3>
    <button class="icon-btn" on:click={() => dispatch("close")}>[X]</button>
  </div>
  <div class="settings-grid">
    <label>
      <span>Seuil bas (kbps)</span>
      <small class="muted"
        >Sous ce débit, le fichier est signalé comme low.</small
      >
      <input
        type="number"
        min="32"
        max="1000"
        value={settings.min_bitrate}
        on:input={(e) => updateSetting("min_bitrate", Number(e.target.value))}
      />
    </label>
    <label>
      <span>Fenêtre d'analyse (s)</span>
      <small class="muted">Durée échantillonnée pour estimer le bitrate.</small>
      <input
        type="number"
        min="10"
        max="300"
        value={settings.analysis_window_seconds}
        on:input={(e) =>
          updateSetting("analysis_window_seconds", Number(e.target.value))}
      />
    </label>
    <label>
      <span>Threads Rayon (0 = auto)</span>
      <small class="muted">Nombre de threads CPU pour l'analyse.</small>
      <input
        type="number"
        min="0"
        max="64"
        value={settings.rayon_threads}
        on:input={(e) => updateSetting("rayon_threads", Number(e.target.value))}
      />
    </label>
    <label class="row">
      <input
        type="checkbox"
        checked={settings.cache_enabled}
        on:change={(e) => updateSetting("cache_enabled", e.target.checked)}
      />
      <span>Activer le cache d'analyse</span>
      <small class="muted">Stocke les résultats pour éviter de rescanner.</small
      >
    </label>

    <hr
      style="grid-column: 1 / -1; margin: 10px 0; border:0; border-top:1px solid #333;"
    />
    <label>
      <span>Taille max du cache (entrées)</span>
      <small class="muted">Nombre maximum de fichiers conservés en cache.</small
      >
      <input
        type="number"
        min="0"
        max="200000"
        value={settings.cache_max_entries}
        on:input={(e) =>
          updateSetting("cache_max_entries", Number(e.target.value))}
      />
    </label>

    {#if settings.client_token}
      <div class="status-row">
        <span class="status-badge registered">✓ Enregistré</span>
      </div>
    {/if}
  </div>
  {#if message}
    <p class="hint">{message}</p>
  {/if}
  <div
    class="actions"
    style="justify-content:flex-end; gap:8px; margin-top:10px;"
  >
    <button class="btn secondary" style="margin-right:auto" on:click={openLogs}
      >Ouvrir les logs</button
    >
    <button class="btn ghost" on:click={() => dispatch("close")}>Annuler</button
    >
    <button
      class="btn primary"
      disabled={loading}
      on:click={() => dispatch("save")}
    >
      {loading ? "..." : "Enregistrer"}
    </button>
  </div>
</div>
