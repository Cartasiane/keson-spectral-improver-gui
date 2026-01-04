<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { isDesktop } from "../services/settingsService";

  let url = "";
  let outputDir = "";
  let downloads = [];
  let queue = { active: 0, pending: 0 };
  let message = "";
  let busy = false;

  onMount(() => {
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
      message = error?.message || "Erreur lors du DL";
    } finally {
      busy = false;
    }
  }

  async function refreshQueue() {
    try {
      let stats;
      if (!isDesktop) {
        stats = {
          active: downloads.length ? 1 : 0,
          pending: Math.max(downloads.length - 1, 0),
        };
      } else {
        stats = await invoke("queue_stats");
      }
      queue = stats || { active: 0, pending: 0 };
    } catch (error) {
      console.warn("Queue stats failed", error);
    }
    setTimeout(refreshQueue, 2000);
  }

  async function mockDownload(payload) {
    await new Promise((r) => setTimeout(r, 800));
    return {
      title: "Mock track",
      caption: payload.url,
      size: "5.2 MB",
      quality: "Authentique",
      warning: "",
      savedTo: outputDir || "~/Music/Keson",
    };
  }
</script>

<section class="panel">
  <div class="fields">
    <label>
      <span>Lien</span>
      <input
        type="text"
        placeholder="SoundCloud, Spotify, Apple Music…"
        bind:value={url}
        on:keydown={(e) => e.key === "Enter" && handleDownload()}
      />
    </label>
    <label>
      <span>Dossier de sortie (optionnel)</span>
      <input type="text" placeholder="~/Music/Keson" bind:value={outputDir} />
    </label>
    <div class="actions">
      <button class="btn primary" disabled={busy} on:click={handleDownload}>
        {busy ? "…" : "Download"}
      </button>
    </div>
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
  <div class="panel-head">
    <span class="muted">Actifs : {queue.active}</span>
    <span class="muted">En attente : {queue.pending}</span>
  </div>
  {#if !downloads.length}
    <p class="hint">Rien encore. Colle un lien pour commencer.</p>
  {:else}
    <div class="card-grid">
      {#each downloads as item, idx}
        <article class="card">
          <div class="card-top">
            <span class="pill">#{downloads.length - idx}</span>
            {#if item.quality}<span class="pill ghost">{item.quality}</span
              >{/if}
          </div>
          <h3>{item.title || "Track"}</h3>
          <p class="muted">{item.caption}</p>
          {#if item.warning}
            <p class="warn">{item.warning}</p>
          {/if}
          {#if item.savedTo}
            <p class="muted">Sauvé : {item.savedTo}</p>
          {/if}
        </article>
      {/each}
    </div>
  {/if}
</section>
