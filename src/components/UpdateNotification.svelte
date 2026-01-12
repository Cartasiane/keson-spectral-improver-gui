<script>
  import { createEventDispatcher } from "svelte";
  import { Rocket } from "lucide-svelte";

  export let version = "";
  export let notes = "";
  export let downloading = false;
  export let progress = 0;

  const dispatch = createEventDispatcher();
</script>

<div class="update-banner">
  <div class="update-content">
    <div class="update-icon"><Rocket size={24} /></div>
    <div class="update-info">
      <strong>Mise à jour disponible</strong>
      {#if version}
        <span class="version">v{version}</span>
      {/if}
      {#if notes}
        <p class="notes">{notes}</p>
      {/if}
    </div>
  </div>

  <div class="update-actions">
    {#if downloading}
      <div class="progress-container">
        <div class="progress-bar" style="width: {progress}%"></div>
        <span class="progress-text">{progress}%</span>
      </div>
    {:else}
      <button class="btn-update" on:click={() => dispatch("install")}>
        Installer
      </button>
      <button class="btn-dismiss" on:click={() => dispatch("dismiss")}>
        Plus tard
      </button>
    {/if}
  </div>
</div>

<style>
  .update-banner {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
    border-bottom: 2px solid #00ff88;
    padding: 12px 20px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    z-index: 9999;
    box-shadow: 0 4px 20px rgba(0, 255, 136, 0.2);
    animation: slideDown 0.3s ease-out;
  }

  @keyframes slideDown {
    from {
      transform: translateY(-100%);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  .update-content {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .update-icon {
    font-size: 24px;
  }

  .update-info {
    color: #fff;
  }

  .update-info strong {
    font-size: 14px;
    color: #00ff88;
  }

  .version {
    margin-left: 8px;
    font-size: 12px;
    background: rgba(0, 255, 136, 0.2);
    padding: 2px 8px;
    border-radius: 4px;
    color: #00ff88;
  }

  .notes {
    margin: 4px 0 0 0;
    font-size: 12px;
    color: #888;
    max-width: 400px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .update-actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .btn-update {
    background: #00ff88;
    color: #000;
    border: none;
    padding: 8px 16px;
    border-radius: 4px;
    font-weight: bold;
    cursor: pointer;
    transition: all 0.2s;
  }

  .btn-update:hover {
    background: #00cc6a;
    transform: scale(1.02);
  }

  .btn-dismiss {
    background: transparent;
    color: #888;
    border: 1px solid #444;
    padding: 8px 16px;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .btn-dismiss:hover {
    border-color: #666;
    color: #fff;
  }

  .progress-container {
    width: 150px;
    height: 24px;
    background: #333;
    border-radius: 4px;
    position: relative;
    overflow: hidden;
  }

  .progress-bar {
    height: 100%;
    background: linear-gradient(90deg, #00ff88, #00cc6a);
    transition: width 0.3s ease;
  }

  .progress-text {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    font-size: 12px;
    font-weight: bold;
    color: #fff;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
  }
</style>
