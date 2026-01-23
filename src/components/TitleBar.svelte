<script>
  import { getCurrentWindow } from "@tauri-apps/api/window";

  const appWindow = getCurrentWindow();

  function minimize() {
    appWindow.minimize();
  }

  async function toggleMaximize() {
    await appWindow.toggleMaximize();
  }

  function close() {
    appWindow.close();
  }
</script>

<div class="titlebar" data-tauri-drag-region>
  <div class="drag-handle" data-tauri-drag-region></div>
  <div class="controls">
    <button class="titlebar-button" on:click={minimize} title="Minimize">
      _
    </button>
    <button class="titlebar-button" on:click={toggleMaximize} title="Maximize">
      □
    </button>
    <button class="titlebar-button close" on:click={close} title="Close">
      ×
    </button>
  </div>
</div>

<style>
  .titlebar {
    height: 32px;
    display: flex;
    justify-content: space-between;
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    z-index: 99999;
    /* Background is transparent to show content/hero behind it */
  }

  .drag-handle {
    flex-grow: 1;
    height: 100%;
  }

  .controls {
    display: flex;
    -webkit-app-region: no-drag; /* For safety, though buttons usually capture events */
    padding-right: 6px;
  }

  .titlebar-button {
    display: inline-flex;
    justify-content: center;
    align-items: center;
    width: 40px;
    height: 32px;
    background: transparent;
    border: none;
    color: var(--muted, #aaa);
    cursor: pointer;
    transition:
      background 0.2s,
      color 0.2s,
      text-shadow 0.2s,
      opacity 0.2s;
    outline: none;
    font-family: "VT323", monospace;
    font-size: 20px;
    line-height: 1;
    opacity: 0.7;
  }

  .titlebar-button:hover {
    opacity: 1;
    background: rgba(57, 255, 20, 0.1);
    color: var(--accent);
    text-shadow: 0 0 5px var(--accent);
  }

  .titlebar-button.close {
    font-size: 24px;
    padding-bottom: 2px;
  }

  .titlebar-button.close:hover {
    background: rgba(232, 17, 35, 0.15);
    color: #ff4444;
    text-shadow: 0 0 8px #ff4444;
  }
</style>
