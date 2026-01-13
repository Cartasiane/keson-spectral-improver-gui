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
  } from "lucide-svelte";

  export let item;
  export let spectrumUrl;
  export let loading = false;
  export let downloadStatus = null; // 'pending' | 'downloading' | 'done' | 'error' | null

  const dispatch = createEventDispatcher();

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
</script>

<div class={`scan-row ${item.status}`}>
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
  <div class="bitrate">{item.bitrate ? `${item.bitrate} kbps` : "n/a"}</div>
  <div class="bitrate">
    {item.is_lossless === true
      ? "Yes"
      : item.is_lossless === false
        ? "No"
        : "n/a"}
  </div>
  <div class="name">{item.name}</div>
  <div class="path" title={item.path}>{item.path}</div>
  <div class="actions actions-inline">
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
  </div>
  {#if loading || spectrumUrl}
    <div class="spectrum" style="grid-column: 1 / -1;">
      {#if loading}
        <div class="skeleton"></div>
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
</style>
