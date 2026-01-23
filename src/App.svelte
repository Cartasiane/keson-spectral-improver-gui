<script>
  import DownloadTab from "./components/DownloadTab.svelte";
  import QualityTab from "./components/QualityTab.svelte";
  import SettingsModal from "./components/SettingsModal.svelte";
  import RegistrationModal from "./components/RegistrationModal.svelte";
  import UpdateNotification from "./components/UpdateNotification.svelte";
  import TitleBar from "./components/TitleBar.svelte";
  import { onMount } from "svelte";
  import { type as osType } from "@tauri-apps/plugin-os";
  import { invoke } from "@tauri-apps/api/core";
  import { fetchSettings, persistSettings } from "./services/settingsService";
  import { isDesktop } from "./services/scanService";
  import { startMatrix } from "./services/matrixRain";

  let activeTab = "quality";
  let showSettings = false;
  let showTitleBar = false;
  let settings = {
    min_bitrate: 256,
    analysis_window_seconds: 100,
    rayon_threads: 0,
    cache_enabled: true,
    cache_max_entries: 10000,
    client_token: null,
  };
  let settingsLoading = false;
  let settingsMessage = "";
  let matrixCanvas;
  let matrixCleanup = () => {};

  // Auth state
  let authChecked = false;
  let isRegistered = false;
  let slotsRemaining = null;

  // Update state
  let updateAvailable = null;
  let updateDownloading = false;
  let updateProgress = 0;
  let updateDismissed = false;

  onMount(async () => {
    if (isDesktop) {
      const type = await osType();
      showTitleBar = true; // Use custom titlebar on all desktop platforms since decorations are hidden
    }
    loadSettings();
    checkAuth();
    const { cleanup } = startMatrix(matrixCanvas);
    matrixCleanup = cleanup;
    checkForUpdates();
    return () => matrixCleanup();
  });

  async function checkAuth() {
    if (!isDesktop) {
      authChecked = true;
      isRegistered = true; // Always registered in browser dev mode
      return;
    }

    try {
      const status = await invoke("check_auth_status");
      isRegistered = status.registered;
      slotsRemaining = status.slots_remaining;
    } catch (err) {
      console.error("Auth check failed:", err);
      isRegistered = false;
    } finally {
      authChecked = true;
    }
  }

  function handleRegistered() {
    isRegistered = true;
    loadSettings(); // Reload settings to get the new token
  }

  async function checkForUpdates() {
    if (!isDesktop) return;
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (update) {
        updateAvailable = update;
        console.log("Update available:", update.version);
      }
    } catch (err) {
      console.log("Update check skipped:", err?.message || err);
    }
  }

  async function installUpdate() {
    if (!updateAvailable) return;
    updateDownloading = true;
    updateProgress = 0;
    try {
      const { relaunch } = await import("@tauri-apps/plugin-process");

      await updateAvailable.downloadAndInstall((event) => {
        if (event.event === "Progress") {
          const total = event.data.contentLength || 1;
          updateProgress = Math.round((event.data.chunkLength / total) * 100);
        }
      });
      updateProgress = 100;
      await relaunch();
    } catch (err) {
      console.error("Update failed:", err);
      updateDownloading = false;
    }
  }

  function dismissUpdate() {
    updateDismissed = true;
  }

  async function loadSettings() {
    if (!isDesktop) return;
    settingsLoading = true;
    settingsMessage = "";
    try {
      const res = await fetchSettings();
      if (res) settings = { ...settings, ...res };
    } catch (err) {
      console.error(err);
      settingsMessage = err?.message || "Impossible de charger les paramètres";
    } finally {
      settingsLoading = false;
    }
  }

  async function saveSettings() {
    if (!isDesktop) {
      showSettings = false;
      return;
    }
    settingsLoading = true;
    settingsMessage = "";
    try {
      await persistSettings(settings);
      settingsMessage = "Paramètres sauvegardés";
      showSettings = false;
    } catch (err) {
      console.error(err);
      settingsMessage = err?.message || "Sauvegarde impossible";
    } finally {
      settingsLoading = false;
    }
  }
</script>

<canvas class="matrix-bg" bind:this={matrixCanvas} aria-hidden="true"></canvas>

<!-- Registration Modal - shown when not registered -->
<RegistrationModal
  show={authChecked && !isRegistered}
  {slotsRemaining}
  on:registered={handleRegistered}
/>

{#if updateAvailable && !updateDismissed}
  <UpdateNotification
    version={updateAvailable.version}
    notes={updateAvailable.body}
    downloading={updateDownloading}
    progress={updateProgress}
    on:install={installUpdate}
    on:dismiss={dismissUpdate}
  />
{/if}

{#if showTitleBar}
  <TitleBar />
{/if}

<main
  class="app"
  style={showTitleBar ? "padding-top: 50px;" : "padding-top: 20px;"}
>
  <header class="hero" data-tauri-drag-region>
    <div>
      <p class="eyebrow">Keson</p>
      <h1>Spectral Improver</h1>
      <p class="sub">Awaken ton KESON avec des fichiers large SPECTRE</p>
    </div>
    <div class="hero-right">
      <nav class="top-menu">
        <button class="menu-link" on:click={() => (showSettings = true)}
          >SETTINGS</button
        >
      </nav>
    </div>
  </header>

  <div class="tabs">
    <button
      class:active={activeTab === "quality"}
      on:click={() => (activeTab = "quality")}
    >
      Spectral Checker
    </button>
    <button
      class:active={activeTab === "download"}
      on:click={() => (activeTab = "download")}
    >
      Download
    </button>
  </div>

  <div style="display: {activeTab === 'download' ? 'block' : 'none'}">
    <DownloadTab />
  </div>
  <div style="display: {activeTab === 'quality' ? 'block' : 'none'}">
    <QualityTab />
  </div>

  {#if showSettings}
    <SettingsModal
      bind:settings
      loading={settingsLoading}
      message={settingsMessage}
      on:close={() => (showSettings = false)}
      on:save={saveSettings}
    />
  {/if}
</main>
