<script>
  import { createEventDispatcher, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { startMatrix } from "../services/matrixRain";

  const dispatch = createEventDispatcher();
  let matrixCanvas;
  let copied = false;
  let loading = false;

  onMount(() => {
    const { cleanup } = startMatrix(matrixCanvas);
    return () => cleanup();
  });

  async function copyLogs() {
    loading = true;
    try {
      const logs = await invoke("get_log_tail", { lines: 200 });
      await writeText(logs);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (e) {
      console.error("Failed to copy logs:", e);
      alert("Erreur lors de la copie des logs: " + e);
    } finally {
      loading = false;
    }
  }

  function closeOnBackdrop(event) {
    if (event.target === event.currentTarget) {
      dispatch("close");
    }
  }
</script>

<div
  class="modal-backdrop"
  role="button"
  tabindex="0"
  aria-label="Fermer"
  on:click={closeOnBackdrop}
  on:keydown={(e) => e.key === "Escape" && dispatch("close")}
>
  <div class="modal matrix-modal">
    <canvas class="matrix-bg-inner" bind:this={matrixCanvas}></canvas>

    <div class="content-overlay">
      <div class="modal-head">
        <h3 class="glitch-text">SIGNALER UN BUG</h3>
        <button class="icon-btn" on:click={() => dispatch("close")}>[X]</button>
      </div>

      <div class="bug-content">
        <ol style="margin-left: 20px; margin-top: 0px; margin-bottom: 40px;">
          <li>Copie les logs avec le bouton.</li>
          <li>Envoie les sur Telegram EWA.</li>
        </ol>

        <div class="actions-vertical">
          <button
            class="btn primary matrix-btn"
            on:click={copyLogs}
            disabled={loading}
          >
            {#if loading}
              EXTRACTION...
            {:else if copied}
              COPIÉ !
            {:else}
              [1] COPIER LOGS
            {/if}
          </button>

          <a
            href="https://t.me/Tayoukak"
            target="_blank"
            class="btn secondary matrix-btn"
          >
            [2] OUVRIR TELEGRAM
          </a>
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .matrix-modal {
    background: #000;
    border: 1px solid #0f0;
    box-shadow: 0 0 20px rgba(0, 255, 0, 0.2);
    overflow: hidden;
    position: relative;
    max-width: 500px;
  }

  .matrix-bg-inner {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    opacity: 0.3;
    pointer-events: none;
  }

  .content-overlay {
    position: relative;
    z-index: 2;
    padding: 20px;
    background: rgba(0, 20, 0, 0.8);
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .glitch-text {
    color: #0f0;
    font-family: monospace;
    text-shadow:
      2px 0 0 rgba(255, 0, 0, 0.5),
      -2px 0 0 rgba(0, 0, 255, 0.5);
    letter-spacing: 2px;
  }

  .bug-content {
    color: #cfc;
    font-family: monospace;
    margin-top: 20px;
  }

  ol {
    margin: 20px 0;
    padding-left: 20px;
    line-height: 1.6;
  }

  li {
    margin-bottom: 10px;
  }

  .actions-vertical {
    display: flex;
    flex-direction: column;
    gap: 15px;
    margin-top: 30px;
  }

  .matrix-btn {
    font-family: monospace;
    text-transform: uppercase;
    letter-spacing: 1px;
    padding: 15px;
    border: 1px solid #0f0;
    background: rgba(0, 50, 0, 0.5);
    color: #0f0;
    transition: all 0.2s;
    text-align: center;
    text-decoration: none;
    display: flex;
    justify-content: center;
    align-items: center;
  }

  .matrix-btn:hover {
    background: #0f0;
    color: #000;
    box-shadow: 0 0 15px #0f0;
  }

  .matrix-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    border-color: #555;
    color: #555;
  }

  .icon-btn {
    color: #0f0;
  }

  .icon-btn:hover {
    text-shadow: 0 0 5px #0f0;
  }
</style>
