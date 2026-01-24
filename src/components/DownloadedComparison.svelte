<script>
  import {
    revertReplacement,
    extractCover,
    getCoverSrc,
    toAssetUrl,
  } from "../services/scanService";
  import { createEventDispatcher, onMount } from "svelte";
  import { Check, Trash2, RotateCcw, Music } from "lucide-svelte";

  export let items = [];

  const dispatch = createEventDispatcher();

  // Store extracted cover paths: { [backupPath]: coverUrl | null }
  let backupCovers = {};

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

  // Extract covers for backup files
  async function loadBackupCovers() {
    for (const item of items) {
      if (item.backup_enabled) {
        const backupPath = getBackupPath(item.original_path);
        if (backupPath && !(backupPath in backupCovers)) {
          try {
            const coverPath = await extractCover(backupPath);
            if (coverPath) {
              backupCovers[backupPath] = toAssetUrl(coverPath);
            } else {
              backupCovers[backupPath] = null;
            }
            backupCovers = { ...backupCovers }; // trigger reactivity
          } catch (e) {
            console.error("Cover extraction failed:", e);
            backupCovers[backupPath] = null;
            backupCovers = { ...backupCovers };
          }
        }
      }
    }
  }

  // Load covers on mount and when items change
  $: if (items.length > 0) loadBackupCovers();
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
                {#if getCoverSrc(item.cover_url)}
                  <img
                    src={getCoverSrc(item.cover_url)}
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
                <button
                  class="action-btn revert"
                  on:click={() => handleRevert(item)}
                >
                  <RotateCcw size={14} /> Restaurer l'original
                </button>
              {/if}
            </div>
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
</style>
