<script>
  import { createEventDispatcher } from "svelte";
  import {
    Clock,
    Loader,
    Check,
    X,
    Link,
    AlertTriangle,
    Cog,
    RefreshCw,
    Send,
    BadgeCheck,
  } from "lucide-svelte";
  import JmodDrawable from "./JmodDrawable.svelte";

  export let item;
  export let spectrumUrl;
  export let loading = false;
  export let downloadStatus = null; // 'pending' | 'downloading' | 'done' | 'error' | 'no-match' | null

  const dispatch = createEventDispatcher();

  let manualUrl = "";
  let submitting = false;

  function getStatusIcon(status) {
    if (status?.startsWith("retry-")) return RefreshCw;
    switch (status) {
      case "pending":
        return Clock;
      case "downloading":
        return Loader;
      case "done":
        return Check;
      case "error":
      case "queue-full":
        return X;
      case "no-match":
        return Link;
      default:
        return null;
    }
  }

  function getStatusClass(status) {
    if (status?.startsWith("retry-")) return "retry";
    if (status === "queue-full") return "queue-full";
    return status;
  }

  function submitManualUrl() {
    const url = manualUrl.trim();
    if (!url) return;

    // Validate URL format
    const isValid =
      url.includes("tidal.com/") || url.includes("soundcloud.com/");
    if (!isValid) {
      return; // Invalid URL, don't submit
    }

    submitting = true;
    dispatch("manualUrl", { path: item.path, url });
  }

  // Reset submitting state when downloadStatus changes
  $: if (downloadStatus !== "no-match") {
    submitting = false;
    manualUrl = "";
  }
</script>

<div
  class={`scan-row ${item.status} ${downloadStatus === "no-match" ? "no-match-row" : ""}`}
>
  <div class="status-dot">
    {#if downloadStatus}
      <span
        class="dl-status"
        class:spin={downloadStatus === "downloading" ||
          downloadStatus?.startsWith("retry-")}
      >
        <svelte:component this={getStatusIcon(downloadStatus)} size={16} />
      </span>
    {:else if item.status === "bad"}
      <AlertTriangle size={16} />
    {:else if item.status === "ok"}
      <Check size={16} />
    {:else}
      <Cog size={16} />
    {/if}
  </div>
  <div class="bitrate">
    {item.bitrate
      ? `${item.bitrate} kbps`
      : item.is_lossless
        ? "Lossless"
        : "n/a"}
  </div>
  <div class="bitrate">
    {item.is_lossless === true
      ? "Yes"
      : item.is_lossless === false
        ? "No"
        : "n/a"}
  </div>
  <div class="name">
    {item.name}
    {#if item.replaced || item.status === "replaced"}
      <span class="replaced-badge" title="Déjà replacé par Keson">
        <BadgeCheck size={14} />
      </span>
    {/if}
  </div>
  <div
    class="path"
    class:has-error={item.status === "error"}
    title={item.status === "error" ? item.note || "Erreur inconnue" : item.path}
  >
    {#if item.status === "error"}
      <span class="error-msg">{item.note || "Erreur inconnue"}</span>
    {:else}
      {item.path}
    {/if}
  </div>
  <div class="actions actions-inline">
    {#if downloadStatus === "no-match"}
      <!-- Inline URL input for failed matches -->
      <div class="inline-url-form">
        <input
          type="text"
          class="url-input"
          placeholder="URL Tidal ou SoundCloud..."
          bind:value={manualUrl}
          disabled={submitting}
          on:keydown={(e) => e.key === "Enter" && submitManualUrl()}
        />
        <div class="btn-row">
          <button
            class="btn mini primary"
            on:click={submitManualUrl}
            disabled={submitting || !manualUrl.trim()}
            title="Télécharger avec cette URL"
          >
            {#if submitting}
              <Loader size={14} class="spin" />
            {:else}
              <Send size={14} />
            {/if}
          </button>
          <button
            class="btn mini ghost"
            on:click={() => dispatch("reveal", item.path)}>Voir</button
          >
        </div>
      </div>
    {:else}
      {#if item.status === "bad" && !downloadStatus}
        <button
          class="btn mini primary"
          on:click={() => dispatch("redownload", item)}
          title="Retélécharger ce fichier"
        >
          <RefreshCw size={14} />
        </button>
      {/if}
      <button
        class="btn mini ghost"
        on:click={() => dispatch("reveal", item.path)}>Voir</button
      >
      <button class="btn mini" on:click={() => dispatch("spectrum", item.path)}
        >Spectre</button
      >
    {/if}
  </div>
  {#if loading || spectrumUrl}
    <div class="spectrum" style="grid-column: 1 / -1;">
      {#if loading}
        <div class="spectrum-loading">
          <div class="jmod-wrapper">
            <JmodDrawable
              strokeColor="var(--accent, #39ff14)"
              strokeWidth={1}
              duration={10000}
              loop={true}
            />
          </div>
          <div class="jmod-wrapper">
            <JmodDrawable
              strokeColor="var(--accent, #39ff14)"
              strokeWidth={10}
              duration={10000}
              loop={true}
            />
          </div>
        </div>
      {:else if spectrumUrl === "error"}
        <div class="skeleton error">Spectre indisponible</div>
      {:else}
        <img
          src={spectrumUrl}
          alt={`Spectrogramme ${item.name}`}
          loading="lazy"
        />
      {/if}
    </div>
  {/if}
</div>

<style>
  .dl-status {
    display: inline-block;
  }
  .dl-status.spin {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  .no-match-row {
    background: rgba(245, 158, 11, 0.1) !important;
    border-left: 2px solid #f59e0b !important;
  }

  /* When in no-match state, make the row display differently */
  .no-match-row .actions {
    grid-column: span 2;
  }

  .inline-url-form {
    display: flex;
    gap: 6px;
    width: 100%;
    flex-wrap: wrap;
  }

  .url-input {
    flex: 1;
    min-width: 150px;
    padding: 6px 10px;
    border: 1px solid rgba(57, 255, 20, 0.4);
    background: var(--panel, #000);
    color: var(--text, #fff);
    font-size: 12px;
    outline: none;
  }

  .url-input:focus {
    border-color: var(--accent, #39ff14);
  }

  .url-input::placeholder {
    color: var(--muted, #888);
  }

  .btn-row {
    display: flex;
    gap: 6px;
  }

  .replaced-badge {
    display: inline-flex;
    align-items: center;
    margin-left: 6px;
    color: #8b5cf6;
    vertical-align: middle;
  }

  .error-msg {
    color: #ef4444;
    font-weight: 500;
  }

  .path.has-error {
    white-space: normal;
    overflow: visible;
    word-break: break-all; /* Ensure long error codes break */
  }

  .spectrum-loading {
    display: flex;
    align-items: center;
    flex-direction: row;
    justify-content: space-around;
    height: 300px; /* Typical spectrum height */
    width: 100%;
    background: rgba(255, 255, 255, 0.02);
    border-radius: 4px;
  }

  .jmod-wrapper {
    width: 300px;
    opacity: 0.9;
  }
</style>
