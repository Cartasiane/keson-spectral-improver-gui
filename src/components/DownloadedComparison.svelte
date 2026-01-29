<script>
  import {
    revertReplacement,
    extractCover,
    getCoverSrc,
    toAssetUrl,
    openSpectrum,
  } from "../services/scanService";
  import { createEventDispatcher, onMount } from "svelte";
  import { Check, Trash2, RotateCcw, Music, Loader } from "lucide-svelte";

  export let items = [];

  const dispatch = createEventDispatcher();

  // Store extracted cover paths: { [backupPath]: coverUrl | null }
  let backupCovers = {};
  let newCovers = {}; // path -> assetUrl
  let loadVersion = 0;

  // Spectrum state: { [path]: url | 'error' }
  let spectrumUrls = {};
  // Spectrum loading state: { [path]: boolean }
  let spectrumLoading = {};

  async function generateSpectrum(path) {
    if (!path || spectrumLoading[path]) return;
    spectrumLoading[path] = true;
    spectrumLoading = { ...spectrumLoading };

    try {
      const url = await openSpectrum(path);
      spectrumUrls[path] = url;
    } catch (e) {
      console.error("Spectrum generation failed:", e);
      spectrumUrls[path] = "error";
    } finally {
      spectrumLoading[path] = false;
      spectrumLoading = { ...spectrumLoading };
      spectrumUrls = { ...spectrumUrls };
    }
  }

  function formatDuration(seconds) {
    if (!seconds) return "--:--";
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  function getFilename(path) {
    return path?.split(/[/\\]/).pop() || "Unknown";
  }

  function getBackupPath(originalPath) {
    if (!originalPath) return null;
    // Split by either / or \ to handle Windows paths
    const parts = originalPath.split(/[/\\]/);
    const filename = parts.pop();
    // Rejoin with the detected separator or default to /
    // Using / is generally safe in JS contexts, but to preserve OS style we can try:
    const isWindows = originalPath.includes("\\");
    const sep = isWindows ? "\\" : "/";
    return [...parts, "backup-ksi", filename].join(sep);
  }

  async function handleRevert(item) {
    try {
      const confirm = await window.confirm(
        "Voulez-vous vraiment restaurer le fichier original ?",
      );
      if (!confirm) return;

      await revertReplacement(item.original_path);
      dismiss(item);
    } catch (e) {
      console.error("Revert failed", e);
      alert("Erreur lors de la restauration: " + e);
    }
  }

  function dismiss(item) {
    dispatch("dismiss", item);
  }

  const MAX_CONCURRENT = 3;

  async function loadCovers() {
    loadVersion++;
    const currentVersion = loadVersion;

    // Create a list of all paths we need to check (backups AND new files)
    const toProcess = [];

    // 1. Check backup covers
    for (const item of items) {
      if (item.backup_enabled) {
        const backupPath = getBackupPath(item.original_path);
        if (backupPath && !backupCovers[backupPath]) {
          toProcess.push({ path: backupPath, type: "backup" });
        }
      }
    }

    // 2. Check new file covers (fallback if no URL)
    for (const item of items) {
      if (!item.cover_url && item.new_path && !newCovers[item.new_path]) {
        toProcess.push({ path: item.new_path, type: "new" });
      }
    }

    if (toProcess.length === 0) return;

    // Process in chunks
    for (let i = 0; i < toProcess.length; i += MAX_CONCURRENT) {
      if (loadVersion !== currentVersion) return;

      const chunk = toProcess.slice(i, i + MAX_CONCURRENT);
      await Promise.all(
        chunk.map(async (task) => {
          if (loadVersion !== currentVersion) return;

          try {
            console.log(
              `[CoverLogic] Starting extraction for ${task.type}: ${task.path}`,
            );
            const coverPath = await extractCover(task.path);
            console.log(
              `[CoverLogic] Extracted ${task.type} path: ${coverPath}`,
            );

            if (loadVersion === currentVersion) {
              const finalUrl = coverPath ? toAssetUrl(coverPath) : null;

              if (task.type === "backup") {
                backupCovers[task.path] = finalUrl;
                backupCovers = { ...backupCovers };
              } else {
                newCovers[task.path] = finalUrl;
                newCovers = { ...newCovers };
              }
            }
          } catch (e) {
            console.error(`[CoverLogic] ${task.type} extraction failed:`, e);
            if (loadVersion === currentVersion) {
              if (task.type === "backup") {
                backupCovers[task.path] = null;
                backupCovers = { ...backupCovers };
              } else {
                newCovers[task.path] = null;
                newCovers = { ...newCovers };
              }
            }
          }
        }),
      );
    }
  }

  // Load covers on mount and when items change
  $: if (items.length > 0) loadCovers();
</script>

{#if items.length > 0}
  <div class="comparison-section">
    <div class="comparison-grid">
      {#each items as item}
        <div class="comparison-card">
          <div class="card-header">
            <span class="filename">{getFilename(item.new_path)}</span>
            <button
              class="dismiss-btn"
              on:click={() => dismiss(item)}
              title="Retirer">×</button
            >
          </div>

          <!-- Stats comparison -->
          <div class="stats-row">
            <div class="stat-box before">
              <span class="stat-label">Avant</span>

              <!-- Cover Art -->
              <div class="cover-container">
                {#if item.backup_enabled}
                  {@const backupPath = getBackupPath(item.original_path)}
                  {#if backupCovers[backupPath]}
                    <img
                      src={backupCovers[backupPath]}
                      class="cover-img"
                      alt="Original Cover"
                    />
                  {:else}
                    <div class="cover-placeholder">
                      <Music size={24} />
                    </div>
                  {/if}
                {:else}
                  <div class="cover-placeholder">
                    <Music size={24} />
                  </div>
                {/if}
              </div>
              <span class="stat-value bad"
                >{item.original_bitrate || "?"} kbps</span
              >
              <span class="stat-duration"
                >{formatDuration(item.original_duration)}</span
              >
              {#if !item.backup_enabled}
                <span class="deleted-tag"><Trash2 size={12} /></span>
              {/if}
            </div>
            <div class="arrow">→</div>
            <div class="stat-box after">
              <span class="stat-label">Après</span>

              <!-- Cover Art -->
              <div class="cover-container">
                {#if getCoverSrc(item.cover_url) || newCovers[item.new_path]}
                  <img
                    src={getCoverSrc(item.cover_url) ||
                      newCovers[item.new_path]}
                    class="cover-img"
                    alt="New Cover"
                  />
                {:else}
                  <div class="cover-placeholder">
                    <Music size={24} />
                  </div>
                {/if}
              </div>
              <span class="stat-value good">{item.new_bitrate || "FLAC"}</span>
              <span class="stat-duration"
                >{formatDuration(item.new_duration)}</span
              >
            </div>
          </div>

          <!-- A/B Comparison Player -->
          <!-- Simple Players & Actions -->
          <div class="players-container">
            {#if item.backup_enabled}
              <div class="player-row">
                <span class="player-label">Avant (Backup):</span>
                <audio
                  controls
                  src={toAssetUrl(getBackupPath(item.original_path))}
                ></audio>
              </div>
            {/if}
            <div class="player-row">
              <span class="player-label">Après (Nouveau):</span>
              <audio controls src={toAssetUrl(item.new_path)}></audio>
            </div>

            <div class="actions-row">
              {#if item.backup_enabled}
                {@const backupPath = getBackupPath(item.original_path)}
                <button
                  class="action-btn"
                  on:click={() => generateSpectrum(backupPath)}
                  disabled={spectrumLoading[backupPath]}
                >
                  {#if spectrumLoading[backupPath]}
                    <Loader size={14} class="spin" />
                  {:else}
                    Spectre Avant
                  {/if}
                </button>
              {/if}
              <button
                class="action-btn"
                on:click={() => generateSpectrum(item.new_path)}
                disabled={spectrumLoading[item.new_path]}
              >
                {#if spectrumLoading[item.new_path]}
                  <Loader size={14} class="spin" />
                {:else}
                  Spectre Après
                {/if}
              </button>
              {#if item.backup_enabled}
                <button
                  class="action-btn revert"
                  on:click={() => handleRevert(item)}
                >
                  <RotateCcw size={14} /> Restaurer l'original
                </button>
              {/if}
            </div>

            <!-- Spectrum Display -->
            {#if item.backup_enabled}
              {@const backupPath = getBackupPath(item.original_path)}
              {#if spectrumUrls[backupPath] || spectrumUrls[item.new_path] || spectrumLoading[backupPath] || spectrumLoading[item.new_path]}
                <div class="spectrum-comparison">
                  <div class="spectrum-box">
                    <span class="spectrum-label">Spectre Avant</span>
                    {#if spectrumLoading[backupPath]}
                      <div class="skeleton"></div>
                    {:else if spectrumUrls[backupPath] === "error"}
                      <div class="skeleton error">Spectre indisponible</div>
                    {:else if spectrumUrls[backupPath]}
                      <img
                        src={spectrumUrls[backupPath]}
                        alt="Spectrum Before"
                        loading="lazy"
                      />
                    {:else}
                      <div class="skeleton empty">
                        Cliquez sur "Spectre Avant"
                      </div>
                    {/if}
                  </div>
                  <div class="spectrum-box">
                    <span class="spectrum-label">Spectre Après</span>
                    {#if spectrumLoading[item.new_path]}
                      <div class="skeleton"></div>
                    {:else if spectrumUrls[item.new_path] === "error"}
                      <div class="skeleton error">Spectre indisponible</div>
                    {:else if spectrumUrls[item.new_path]}
                      <img
                        src={spectrumUrls[item.new_path]}
                        alt="Spectrum After"
                        loading="lazy"
                      />
                    {:else}
                      <div class="skeleton empty">
                        Cliquez sur "Spectre Après"
                      </div>
                    {/if}
                  </div>
                </div>
              {/if}
            {:else if spectrumUrls[item.new_path] || spectrumLoading[item.new_path]}
              <div class="spectrum-single">
                <span class="spectrum-label">Spectre</span>
                {#if spectrumLoading[item.new_path]}
                  <div class="skeleton"></div>
                {:else if spectrumUrls[item.new_path] === "error"}
                  <div class="skeleton error">Spectre indisponible</div>
                {:else if spectrumUrls[item.new_path]}
                  <img
                    src={spectrumUrls[item.new_path]}
                    alt="Spectrum"
                    loading="lazy"
                  />
                {/if}
              </div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .comparison-section {
    margin: 20px 0;
  }

  .comparison-grid {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .comparison-card {
    background: linear-gradient(
      145deg,
      rgba(30, 30, 50, 0.8),
      rgba(20, 20, 35, 0.9)
    );
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    padding: 20px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    backdrop-filter: blur(10px);
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
    padding-bottom: 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  }

  .filename {
    font-weight: 600;
    color: var(--text, #fff);
    font-size: 0.95rem;
  }

  .dismiss-btn {
    background: rgba(255, 255, 255, 0.1);
    border: none;
    color: var(--text-muted, #888);
    width: 28px;
    height: 28px;
    border-radius: 50%;
    cursor: pointer;
    font-size: 1.2rem;
    line-height: 1;
    transition: all 0.2s;
  }

  .dismiss-btn:hover {
    background: rgba(239, 68, 68, 0.2);
    color: #ef4444;
  }

  .stats-row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 16px;
    margin-bottom: 16px;
  }

  .stat-box {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 12px 20px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 10px;
    min-width: 120px;
  }

  .stat-box.before {
    border: 1px solid rgba(248, 113, 113, 0.3);
  }

  .stat-box.after {
    border: 1px solid rgba(52, 211, 153, 0.3);
  }

  .stat-label {
    font-size: 0.7rem;
    color: var(--text-muted, #666);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .stat-value {
    font-weight: 600;
    font-size: 1rem;
  }

  .stat-value.bad {
    color: #f87171;
  }

  .stat-value.good {
    color: #34d399;
  }

  .stat-duration {
    font-size: 0.8rem;
    color: var(--text-muted, #888);
  }

  .deleted-tag {
    font-size: 0.7rem;
  }

  .arrow {
    font-size: 1.5rem;
    color: var(--text-muted, #666);
    opacity: 0.5;
  }

  .players-container {
    margin-top: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .player-row {
    display: flex;
    align-items: center;
    gap: 12px;
    background: rgba(0, 0, 0, 0.2);
    padding: 10px;
    border-radius: 8px;
  }

  .player-label {
    min-width: 120px;
    font-size: 0.9rem;
    color: var(--text-muted, #999);
    text-align: right;
  }

  audio {
    flex: 1;
    height: 36px;
  }

  .actions-row {
    display: flex;
    justify-content: flex-end;
    margin-top: 8px;
  }

  .action-btn {
    padding: 8px 16px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.05);
    color: var(--text, #fff);
    cursor: pointer;
    font-size: 0.9rem;
    transition: all 0.2s;
  }

  .action-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .action-btn.revert:hover {
    background: rgba(248, 113, 113, 0.1);
    border-color: rgba(248, 113, 113, 0.3);
    color: #f87171;
  }

  .cover-container {
    margin: 8px 0;
  }

  .cover-placeholder {
    width: 64px;
    height: 64px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    border: 1px dashed rgba(255, 255, 255, 0.1);
  }

  .cover-img {
    width: 64px;
    height: 64px;
    object-fit: cover;
    border-radius: 6px;
    box-shadow: 0 4px 8px rgba(0, 0, 0, 0.25);
  }

  /* Spectrum styles */
  .spectrum-comparison {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    margin-top: 16px;
  }

  .spectrum-single {
    margin-top: 16px;
  }

  .spectrum-box {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .spectrum-label {
    font-size: 0.75rem;
    color: var(--text-muted, #888);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .spectrum-comparison img,
  .spectrum-single img {
    width: 100%;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .skeleton {
    height: 120px;
    background: linear-gradient(
      90deg,
      rgba(255, 255, 255, 0.05) 25%,
      rgba(255, 255, 255, 0.1) 50%,
      rgba(255, 255, 255, 0.05) 75%
    );
    background-size: 200% 100%;
    animation: shimmer 1.5s infinite;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted, #888);
    font-size: 0.8rem;
  }

  .skeleton.error {
    background: rgba(248, 113, 113, 0.1);
    animation: none;
    color: #f87171;
  }

  .skeleton.empty {
    background: rgba(255, 255, 255, 0.03);
    animation: none;
  }

  @keyframes shimmer {
    0% {
      background-position: 200% 0;
    }
    100% {
      background-position: -200% 0;
    }
  }

  /* Spin animation for loader */
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

  .actions-row {
    gap: 8px;
  }

  .action-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
