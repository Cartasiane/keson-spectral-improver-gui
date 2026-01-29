<script>
  import { createEventDispatcher, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { isDesktop } from "../services/scanService";
  import {
    Search,
    Music,
    Download,
    Loader2,
    Cloud,
    Music2,
  } from "lucide-svelte";

  const dispatch = createEventDispatcher();

  let query = "";
  let results = [];
  let loading = false;
  let error = null;
  let debounceTimer = null;
  let selectedIndex = -1;

  // Debounced search
  function handleInput() {
    clearTimeout(debounceTimer);
    error = null;
    selectedIndex = -1;

    if (query.trim().length < 2) {
      results = [];
      return;
    }

    debounceTimer = setTimeout(() => {
      performSearch();
    }, 400);
  }

  async function performSearch() {
    if (!isDesktop) {
      error = "Search only available in desktop mode";
      return;
    }

    loading = true;
    error = null;

    try {
      results = await invoke("search_tracks", { query: query.trim() });
    } catch (err) {
      console.error("Search failed:", err);
      error = err?.message || err || "Search failed";
      results = [];
    } finally {
      loading = false;
    }
  }

  function handleKeydown(event) {
    if (results.length === 0) return;

    if (event.key === "ArrowDown") {
      event.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, results.length - 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
    } else if (event.key === "Enter" && selectedIndex >= 0) {
      event.preventDefault();
      downloadResult(results[selectedIndex]);
    } else if (event.key === "Escape") {
      results = [];
      selectedIndex = -1;
    }
  }

  function downloadResult(result) {
    // Dispatch event to trigger download in DownloadTab
    dispatch("download", result.url);
  }

  function formatDuration(seconds) {
    if (!seconds) return "--:--";
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  }

  onDestroy(() => {
    clearTimeout(debounceTimer);
  });
</script>

<section class="search-tab">
  <div class="search-container">
    <div class="search-input-wrapper">
      <Search size={20} class="search-icon" />
      <input
        type="text"
        bind:value={query}
        on:input={handleInput}
        on:keydown={handleKeydown}
        placeholder="Search tracks on Tidal & SoundCloud..."
        class="search-input"
        autocomplete="off"
        spellcheck="false"
      />
      {#if loading}
        <Loader2 size={20} class="loading-icon spin" />
      {/if}
    </div>

    {#if error}
      <div class="error-message">{error}</div>
    {/if}

    {#if results.length > 0}
      <div class="results-dropdown">
        {#each results as result, i}
          <button
            class="result-item"
            class:selected={i === selectedIndex}
            on:click={() => downloadResult(result)}
            on:mouseenter={() => (selectedIndex = i)}
          >
            <div class="result-cover">
              {#if result.cover_url}
                <img src={result.cover_url} alt="" loading="lazy" />
              {:else}
                <div class="cover-placeholder">
                  <Music size={24} />
                </div>
              {/if}
            </div>
            <div class="result-info">
              <div class="result-title">{result.title}</div>
              <div class="result-artist">{result.artist}</div>
            </div>
            <div class="result-meta">
              <span class="result-duration"
                >{formatDuration(result.duration)}</span
              >
              <span
                class="result-source"
                class:tidal={result.source === "tidal"}
              >
                {#if result.source === "tidal"}
                  <Music2 size={14} />
                {:else}
                  <Cloud size={14} />
                {/if}
                {result.source === "tidal" ? "Tidal" : "SoundCloud"}
              </span>
            </div>
            <div class="result-action">
              <Download size={18} />
            </div>
          </button>
        {/each}
      </div>
    {:else if query.length >= 2 && !loading && !error}
      <div class="no-results">No results found</div>
    {/if}
  </div>
</section>

<style>
  .search-tab {
    padding: 1.5rem 0;
  }

  .search-container {
    max-width: 700px;
    margin: 0 auto;
    position: relative;
  }

  .search-input-wrapper {
    position: relative;
    display: flex;
    align-items: center;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 16px;
    padding: 0 1rem;
    transition: all 0.2s ease;
  }

  .search-input-wrapper:focus-within {
    background: rgba(255, 255, 255, 0.12);
    border-color: var(--accent, #7c3aed);
    box-shadow: 0 0 0 3px rgba(124, 58, 237, 0.2);
  }

  .search-input-wrapper :global(.search-icon) {
    color: rgba(255, 255, 255, 0.5);
    flex-shrink: 0;
  }

  .search-input-wrapper :global(.loading-icon) {
    color: var(--accent, #7c3aed);
    flex-shrink: 0;
  }

  .search-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #fff;
    font-size: 1.1rem;
    padding: 1rem 0.75rem;
    font-family: inherit;
  }

  .search-input::placeholder {
    color: rgba(255, 255, 255, 0.4);
  }

  .results-dropdown {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    right: 0;
    background: rgba(20, 20, 30, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 16px;
    overflow: hidden;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
    z-index: 100;
    max-height: 400px;
    overflow-y: auto;
    backdrop-filter: blur(20px);
  }

  .result-item {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    width: 100%;
    border: none;
    background: transparent;
    color: #fff;
    cursor: pointer;
    text-align: left;
    transition: background 0.15s ease;
    font-family: inherit;
  }

  .result-item:hover,
  .result-item.selected {
    background: rgba(124, 58, 237, 0.2);
  }

  .result-item:not(:last-child) {
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .result-cover {
    width: 48px;
    height: 48px;
    border-radius: 8px;
    overflow: hidden;
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.1);
  }

  .result-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .cover-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.3);
  }

  .result-info {
    flex: 1;
    min-width: 0;
  }

  .result-title {
    font-weight: 500;
    font-size: 0.95rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .result-artist {
    font-size: 0.85rem;
    color: rgba(255, 255, 255, 0.6);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .result-meta {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.25rem;
    flex-shrink: 0;
  }

  .result-duration {
    font-size: 0.85rem;
    color: rgba(255, 255, 255, 0.5);
    font-variant-numeric: tabular-nums;
  }

  .result-source {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    background: rgba(255, 140, 0, 0.2);
    color: #ff8c00;
  }

  .result-source.tidal {
    background: rgba(0, 255, 255, 0.15);
    color: #00d4d4;
  }

  .result-action {
    color: rgba(255, 255, 255, 0.4);
    padding: 0.5rem;
    border-radius: 8px;
    transition: all 0.15s ease;
  }

  .result-item:hover .result-action,
  .result-item.selected .result-action {
    color: var(--accent, #7c3aed);
    background: rgba(124, 58, 237, 0.2);
  }

  .no-results,
  .error-message {
    text-align: center;
    padding: 1.5rem;
    color: rgba(255, 255, 255, 0.5);
    font-size: 0.9rem;
  }

  .error-message {
    color: #ff6b6b;
    background: rgba(255, 107, 107, 0.1);
    border-radius: 12px;
    margin-top: 0.5rem;
  }

  :global(.spin) {
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
