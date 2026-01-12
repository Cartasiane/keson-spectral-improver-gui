<script>
  import { invoke } from "@tauri-apps/api/core";
  import { downloadDir } from "@tauri-apps/api/path";
  import { onMount } from "svelte";
  import { isDesktop } from "../services/settingsService";
  import { Music, Clock, Folder } from "lucide-svelte";

  let url = "";
  let message = "";
  let busy = false;
  let outputDir = "";
  let downloads = [];
  let queue = { active: 0, pending: 0 };

  // mock for browser dev
  // mock for browser dev
  async function mockDownload(payload) {
    if (import.meta.env.DEV) {
      await new Promise((r) => setTimeout(r, 2000));
      return {
        title: "Mock Song - " + payload.url,
        artist: "Mock Artist",
        cover_url: "",
        quality: "High",
        saved_to: "/tmp/mock.mp3",
        warning: "",
      };
    }
  }

  async function refreshQueue() {
    if (isDesktop && !busy) {
      try {
        queue = await invoke("get_queue_status");
      } catch (e) {}
    }
  }

  onMount(async () => {
    setInterval(refreshQueue, 2000);
    if (isDesktop) {
      try {
        outputDir = await downloadDir();
      } catch (e) {
        console.error("Failed to get download dir", e);
      }
    }
    refreshQueue();
  });

  async function handleDownload() {
    if (!url.trim()) return;
    busy = true;
    message = "Préparation…";
    const payload = { url, outputDir };
    try {
      let result;
      if (!isDesktop) {
        result = await mockDownload(payload);
      } else {
        result = await invoke("download_link", payload);
      }
      downloads = [result, ...downloads].slice(0, 12);
      message = result.warning || "Téléchargé";
    } catch (error) {
      console.error(error);
      const errMsg = typeof error === "string" ? error : error?.message;
      message = errMsg || "Erreur lors du DL";
    } finally {
      busy = false;
    }
  }
</script>

<section class="panel">
  <div class="fields">
    <label>
      Lien
      <input
        type="text"
        placeholder="https://soundcloud.com/..."
        bind:value={url}
        disabled={busy}
      />
    </label>
    <label>
      Dossier de sortie (optionnel)
      <input type="text" placeholder={outputDir} bind:value={outputDir} />
    </label>
    <button class="btn primary" on:click={handleDownload} disabled={busy}>
      {#if busy}Charges...{:else}DOWNLOAD{/if}
    </button>
  </div>
  {#if message}
    <p class="hint">{message}</p>
  {/if}
</section>

<section class="panel">
  <div class="panel-head">
    <h2>Derniers téléchargements</h2>
    <span class="badge">{downloads.length}</span>
  </div>
  {#if !downloads.length}
    <p class="hint">Rien encore. Colle un lien pour commencer.</p>
  {:else}
    <div class="card-grid">
      {#each downloads as item, idx}
        <article class="card">
          <div style="display:flex; gap:12px; align-items:flex-start;">
            {#if item.cover_url}
              <img
                src={item.cover_url}
                alt="Cover"
                style="width:80px; height:80px; object-fit:cover; border-radius:6px; flex-shrink:0;"
              />
            {:else}
              <div
                style="width:80px; height:80px; background:#333; border-radius:6px; display:flex; align-items:center; justify-content:center; color:#666; font-size:10px;"
              >
                No Cover
              </div>
            {/if}
            <div style="flex:1; width: 100%; display:grid; gap:4px;">
              <div class="card-top">
                <span class="pill">#{downloads.length - idx}</span>
                {#if item.quality}<span class="pill ghost">{item.quality}</span
                  >{/if}
                {#if item.source}<span class="pill ghost">{item.source}</span
                  >{/if}
              </div>
              <h3 style="word-break:break-word;">{item.title || "Track"}</h3>
              {#if item.artist}
                <p class="muted">Artiste : {item.artist}</p>
              {/if}
              {#if item.album}
                <p class="muted">Album : {item.album}</p>
              {/if}
              <div style="display: flex; gap: 12px; flex-wrap: wrap;">
                {#if item.bitrate}
                  <span
                    class="muted"
                    style="display:flex; align-items:center; gap:4px;"
                    ><Music size={14} /> {item.bitrate} kbps</span
                  >
                {/if}
                {#if item.duration}
                  <span
                    class="muted"
                    style="display:flex; align-items:center; gap:4px;"
                    ><Clock size={14} />
                    {Math.floor(item.duration / 60)}:{String(
                      Math.floor(item.duration % 60)
                    ).padStart(2, "0")}</span
                  >
                {/if}
              </div>
              {#if item.warning}
                <p class="warn">{item.warning}</p>
              {/if}
              {#if item.saved_to}
                <p class="muted" style="font-size:11px; word-break:break-all;">
                  <Folder size={12} />
                  {item.saved_to}
                </p>
              {/if}
            </div>
          </div>
        </article>
      {/each}
    </div>
  {/if}
</section>

<style>
  .panel {
    display: grid;
    gap: 1.5rem;
  }
  .panel-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .fields {
    display: grid;
    gap: 1rem;
  }
  label {
    display: grid;
    gap: 0.5rem;
    font-size: 0.9rem;
    font-weight: 500;
  }
  input {
    background: var(--surface-2);
    border: 1px solid var(--border);
    padding: 0.75rem;
    border-radius: 6px;
    color: inherit;
  }
  input:focus {
    border-color: var(--primary);
    outline: none;
  }
  .card-grid {
    display: grid;
    gap: 1rem;
  }
  .card {
    background: var(--surface-2);
    padding: 1rem;
    border-radius: 8px;
    border: 1px solid var(--border);
  }
  .card-top {
    display: flex;
    justify-content: space-between;
    margin-bottom: 0.5rem;
  }
  h3 {
    margin: 0;
    font-size: 1rem;
    line-height: 1.4;
  }
  .muted {
    color: var(--text-muted);
    font-size: 0.9rem;
    margin: 0;
  }
  .pill {
    background: var(--primary);
    color: var(--background);
    padding: 2px 8px;
    border-radius: 12px;
    font-size: 0.75rem;
    font-weight: bold;
  }
  .pill.ghost {
    background: transparent;
    color: var(--text-muted);
    border: 1px solid var(--border);
  }
  .warn {
    color: #ffaa00;
    font-size: 0.9rem;
  }
</style>
