<script>
  import ScanControls from "./ScanControls.svelte";
  import ScanSummary from "./ScanSummary.svelte";
  import ScanRow from "./ScanRow.svelte";
  import RedownloadModal from "./RedownloadModal.svelte";
  import DownloadedComparison from "./DownloadedComparison.svelte";
  import { onDestroy } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import {
    isDesktop,
    pickFolderDialog,
    scanFolder as performScan,
    revealInFolder,
    openSpectrum,
    listenScanProgress,
    redownloadBad,
    downloadWithUrl,
    acceptRedownload,
    discardFile,
  } from "../services/scanService";
  import { Link } from "lucide-svelte";

  let scanFolder = "";
  let scanning = false;
  let scanResults = [];
  let scanMessage = "";
  let filter = "bad";
  let progress = 0;
  let progressLabel = "";
  let retrying = false;
  let reviewQueue = [];
  let unlistenProgress;
  let spectra = {};
  let spectroLoading = {};

  // Manual URL input state
  let manualUrlTrack = null; // Track needing manual URL
  let manualUrlInput = "";
  let manualUrlError = "";
  let failedMatches = []; // Queue of tracks that failed auto-match

  // Redownload modal state
  let showRedownloadModal = false;
  let redownloadTargets = []; // paths to redownload
  let downloadStatus = {}; // { [path]: 'pending' | 'downloading' | 'done' | 'error' }

  // Downloaded comparison state
  let downloadedItems = []; // Array of comparison objects

  $: filteredResults =
    filter === "all"
      ? scanResults
      : scanResults.filter((r) => r.status === filter);

  async function pickFolder() {
    const choice = await pickFolderDialog();
    if (choice) scanFolder = choice;
  }

  async function runScan() {
    if (!scanFolder) {
      scanMessage = "Choisis un dossier à analyser.";
      return;
    }
    if (!isDesktop) {
      scanMessage = "Scan dispo seulement en mode desktop.";
      return;
    }
    scanning = true;
    scanMessage = "Analyse en cours...";
    progress = 0;
    await startProgressListener();
    try {
      const results = await performScan(scanFolder, 256);
      scanResults = results;
      const bad = results.filter((r) => r.status === "bad").length;
      scanMessage = bad
        ? `${bad} fichier(s) sous 256 kbps`
        : "Tout est au-dessus de 256 kbps";
    } catch (error) {
      console.error(error);
      const msg =
        (error && (error.message || error.toString?.())) ||
        (typeof error === "string" ? error : "") ||
        "";
      scanMessage = msg.trim() || "Échec de l’analyse";
    } finally {
      scanning = false;
      stopProgressListener();
      progress = 100;
      progressLabel = "";
    }
  }

  async function startProgressListener() {
    stopProgressListener();
    if (!isDesktop) return;
    unlistenProgress = await listenScanProgress((payload) => {
      // Accept payload as number, string "x/y", or object { current, total, percent }
      if (typeof payload === "number" && Number.isFinite(payload)) {
        progress = clampPercent(payload);
        progressLabel = `${Math.round(progress)}%`;
        return;
      }
      if (typeof payload === "string") {
        const match = payload.match(/^(\d+)\s*\/\s*(\d+)$/);
        if (match) {
          const current = Number(match[1]);
          const total = Number(match[2]) || 0;
          progress = clampPercent((current / Math.max(total, 1)) * 100);
          progressLabel = `${current}/${total || "?"}`;
          return;
        }
        const asNumber = Number(payload);
        if (Number.isFinite(asNumber)) {
          progress = clampPercent(asNumber);
          progressLabel = `${Math.round(progress)}%`;
        }
        return;
      }
      if (payload && typeof payload === "object") {
        const { current, total, percent } = payload;
        if (Number.isFinite(percent)) {
          progress = clampPercent(percent);
          progressLabel = `${Math.round(progress)}%`;
          return;
        }
        if (Number.isFinite(current) && Number.isFinite(total)) {
          progress = clampPercent((current / Math.max(total, 1)) * 100);
          progressLabel = `${current}/${total || "?"}`;
        }
      }
    });
    progress = 1;
    progressLabel = "0/?";
  }

  const clampPercent = (val) => Math.max(0, Math.min(100, val));

  function stopProgressListener() {
    if (unlistenProgress) {
      unlistenProgress();
      unlistenProgress = null;
    }
  }

  async function reveal(path) {
    try {
      await revealInFolder(path);
    } catch (err) {
      console.error(err);
      scanMessage = err?.message || "Impossible d’ouvrir le dossier.";
    }
  }

  async function spectrum(path) {
    if (!isDesktop) return;
    try {
      spectroLoading = { ...spectroLoading, [path]: true };
      spectra = { ...spectra, [path]: undefined };
      const url = await openSpectrum(path);
      spectra = { ...spectra, [path]: url };
      scanMessage = "Spectre généré";
    } catch (err) {
      console.error(err);
      spectra = { ...spectra, [path]: "error" };
      scanMessage = err?.message || "Échec génération spectre.";
    } finally {
      spectroLoading = { ...spectroLoading, [path]: false };
    }
  }

  // Open modal for batch redownload
  function openRedownloadModal() {
    const badPaths = scanResults
      .filter((r) => r.status === "bad")
      .map((r) => r.path);
    if (!badPaths.length) {
      scanMessage = "Aucun fichier à retélécharger.";
      return;
    }
    redownloadTargets = badPaths;
    showRedownloadModal = true;
  }

  // Open modal for single track redownload
  function handleSingleRedownload(event) {
    const item = event.detail;
    redownloadTargets = [item.path];
    showRedownloadModal = true;
  }

  // Close modal
  function cancelRedownload() {
    showRedownloadModal = false;
    redownloadTargets = [];
  }

  // Execute redownload with options from modal
  async function executeRedownload(event) {
    const { source, backup } = event.detail;
    showRedownloadModal = false;

    if (!isDesktop) {
      scanMessage = "Retéléchargement dispo seulement en desktop.";
      return;
    }

    retrying = true;
    reviewQueue = [];

    // Switch to downloaded view immediately
    filter = "downloaded";

    // Initialize status for all targets
    downloadStatus = {};
    for (const p of redownloadTargets) {
      downloadStatus[p] = "pending";
    }
    downloadStatus = { ...downloadStatus };

    scanMessage = `Téléchargement de ${redownloadTargets.length} fichier(s)...`;

    try {
      // Process one at a time for per-track progress
      const results = [];
      failedMatches = []; // Reset failed matches queue

      for (const filePath of redownloadTargets) {
        downloadStatus = { ...downloadStatus, [filePath]: "downloading" };

        try {
          const saved = await redownloadBad([filePath], { source, backup });

          // Check if no match was found (empty results)
          if (saved.length === 0) {
            const trackItem = scanResults.find((s) => s.path === filePath);
            if (trackItem) {
              failedMatches = [...failedMatches, trackItem];
            }
            downloadStatus = { ...downloadStatus, [filePath]: "no-match" };
            continue;
          }

          results.push(...saved);
          downloadStatus = { ...downloadStatus, [filePath]: "done" };

          // Add to downloadedItems immediately after each successful download
          for (const r of saved) {
            const orig = r.original_duration ?? 0;
            const fresh = r.new_duration ?? 0;
            const diff = Math.abs(orig - fresh);
            const rel = orig > 0 ? diff / orig : 1;
            const isMatch = diff <= 2 || rel <= 0.05;

            if (isMatch) {
              const origItem = scanResults.find(
                (s) => s.path === r.original_path
              );
              downloadedItems = [
                ...downloadedItems,
                {
                  original_path: r.original_path,
                  new_path: r.new_path,
                  original_duration: r.original_duration,
                  new_duration: r.new_duration,
                  original_bitrate: origItem?.bitrate || null,
                  new_bitrate: r.new_bitrate || null,
                  cover_url: r.cover_url || null,
                  backup_enabled: backup,
                },
              ];

              // Remove from scanResults immediately
              scanResults = scanResults.filter(
                (s) => s.path !== r.original_path
              );
            }
          }
        } catch (err) {
          console.error(`Failed: ${filePath}`, err);
          downloadStatus = { ...downloadStatus, [filePath]: "error" };
        }
      }

      const reviewed = buildReviewQueue(results);
      reviewQueue = reviewed;
      const flagged = reviewed.filter((r) => r.mismatch).length;

      // Note: successful replacements are already added to downloadedItems
      // and removed from scanResults in the loop above

      // Build summary message
      let msgParts = [];
      if (results.length > 0) {
        msgParts.push(`Retéléchargé ${results.length}`);
      }
      if (flagged > 0) {
        msgParts.push(`${flagged} à vérifier`);
      }
      if (failedMatches.length > 0) {
        msgParts.push(`${failedMatches.length} non trouvé(s)`);
      }
      scanMessage = msgParts.join(", ") || "Terminé.";

      // If there are failed matches, show manual URL modal for the first one
      if (failedMatches.length > 0) {
        promptManualUrl(failedMatches[0]);
      }
    } catch (err) {
      console.error(err);
      scanMessage =
        typeof err === "string"
          ? err
          : err?.message || "Échec du retéléchargement";
    } finally {
      retrying = false;
      redownloadTargets = [];
      // Clear status after a delay
      setTimeout(() => {
        downloadStatus = {};
      }, 3000);
    }
  }

  function buildReviewQueue(results) {
    const toleranceSec = 2;
    const tolerancePct = 0.05;
    return results
      .map((item) => {
        if (!item.original_path || !item.new_path) return null;
        if (item.new_path === "NA") return null;
        const orig = item.original_duration ?? 0;
        const fresh = item.new_duration ?? 0;
        const diff = Math.abs(orig - fresh);
        const rel = orig > 0 ? diff / orig : 1;
        const mismatch = diff > toleranceSec && rel > tolerancePct;

        const origUrl = convertFileSrc(item.original_path);
        const newUrl = convertFileSrc(item.new_path);
        console.log(
          "[QualityTab] orig path:",
          item.original_path,
          "-> url:",
          origUrl
        );
        console.log("[QualityTab] new path:", item.new_path, "-> url:", newUrl);

        return {
          ...item,
          mismatch,
          origUrl,
          newUrl,
        };
      })
      .filter((i) => i && i.mismatch);
  }

  async function acceptReplacement(entry) {
    try {
      await acceptRedownload(entry.original_path, entry.new_path);
      reviewQueue = reviewQueue.filter((r) => r !== entry);
      scanMessage = "Remplacé par la version SoundCloud.";
    } catch (err) {
      console.error(err);
      scanMessage = err?.message || "Impossible de remplacer le fichier";
    }
  }

  async function ignoreReplacement(entry) {
    try {
      await discardFile(entry.new_path);
    } catch (err) {
      console.warn("discard failed", err);
    }
    reviewQueue = reviewQueue.filter((r) => r !== entry);
  }

  // Manual URL input functions
  function promptManualUrl(track) {
    manualUrlTrack = track;
    manualUrlInput = "";
    manualUrlError = "";
  }

  function closeManualUrl() {
    // Remove current track from failedMatches queue
    if (manualUrlTrack) {
      failedMatches = failedMatches.filter(
        (t) => t.path !== manualUrlTrack.path
      );
    }

    manualUrlTrack = null;
    manualUrlInput = "";
    manualUrlError = "";

    // Show next failed track if any remain
    if (failedMatches.length > 0) {
      setTimeout(() => promptManualUrl(failedMatches[0]), 100);
    }
  }

  async function submitManualUrl() {
    const url = manualUrlInput.trim();

    // Validate URL format
    const isValid =
      url.includes("tidal.com/") || url.includes("soundcloud.com/");
    if (!isValid) {
      manualUrlError = "URL invalide. Utilisez une URL Tidal ou SoundCloud.";
      return;
    }

    manualUrlError = "";
    scanMessage = "Téléchargement en cours...";

    try {
      // Use dedicated manual URL download function
      const result = await downloadWithUrl(manualUrlTrack.path, url, true);

      if (result && result.new_path) {
        // Check if durations match
        const orig = result.original_duration ?? 0;
        const fresh = result.new_duration ?? 0;
        const diff = Math.abs(orig - fresh);
        const rel = orig > 0 ? diff / orig : 1;
        const isMatch = diff <= 2 || rel <= 0.05;

        if (isMatch) {
          // Add to downloadedItems
          const origItem = scanResults.find(
            (s) => s.path === result.original_path
          );
          downloadedItems = [
            ...downloadedItems,
            {
              original_path: result.original_path,
              new_path: result.new_path,
              original_duration: result.original_duration,
              new_duration: result.new_duration,
              original_bitrate: origItem?.bitrate || null,
              new_bitrate: result.new_bitrate || null,
              cover_url: result.cover_url || null,
              backup_enabled: true,
            },
          ];

          // Remove from scanResults
          scanResults = scanResults.filter(
            (s) => s.path !== result.original_path
          );
          filter = "downloaded"; // Switch to downloaded view
          scanMessage = "Téléchargé avec succès.";
        } else {
          // Add to review queue for duration mismatch
          reviewQueue = [
            ...reviewQueue,
            {
              original: result.original_path,
              fresh: result.new_path,
              originalDur: result.original_duration,
              freshDur: result.new_duration,
              mismatch: true,
            },
          ];
          scanMessage = "Téléchargé - vérifiez la durée.";
        }
      } else {
        scanMessage = "Échec du téléchargement.";
      }

      closeManualUrl();
    } catch (err) {
      console.error(err);
      manualUrlError = err?.message || "Échec du téléchargement";
    }
  }

  onDestroy(stopProgressListener);
</script>

<section class="panel">
  <ScanControls
    bind:folder={scanFolder}
    message={scanMessage}
    {scanning}
    {progress}
    {progressLabel}
    on:pick={pickFolder}
    on:scan={runScan}
  />

  {#if scanResults.length || downloadedItems.length > 0}
    <ScanSummary
      results={scanResults}
      active={filter}
      downloadedCount={downloadedItems.length}
      on:filter={(e) => (filter = e.detail)}
    />
    <div
      class="actions"
      style="justify-content:flex-start; margin-bottom: 10px; gap:8px;"
    >
      <button
        class="btn primary"
        disabled={retrying}
        on:click={openRedownloadModal}
      >
        {retrying ? "Téléchargement…" : "Retélécharger les LOW"}
      </button>
    </div>

    <!-- Downloaded comparison view -->
    {#if filter === "downloaded"}
      {#if downloadedItems.length > 0}
        <DownloadedComparison
          items={downloadedItems}
          on:dismiss={(e) => {
            downloadedItems = downloadedItems.filter(
              (i) => i.original_path !== e.detail.original_path
            );
          }}
        />
      {:else if retrying}
        <div class="downloading-placeholder">
          <div class="spinner"></div>
          <p>Téléchargement en cours...</p>
        </div>
      {:else}
        <div class="downloading-placeholder">
          <p class="muted">Aucun fichier téléchargé pour l'instant.</p>
        </div>
      {/if}
    {/if}

    {#if reviewQueue.length}
      <div class="panel review-block">
        <div class="panel-head" style="margin-bottom:6px;">
          <h2>Vérifier les durées</h2>
          <span class="badge">{reviewQueue.length}</span>
        </div>
        <div class="review-list">
          {#each reviewQueue as item}
            <div class="review-card">
              <div class="review-meta">
                <div>
                  Durée originale : {item.original_duration
                    ? item.original_duration.toFixed(1) + "s"
                    : "n/a"}
                </div>
                <div>
                  Nouvelle : {item.new_duration
                    ? item.new_duration.toFixed(1) + "s"
                    : "n/a"}
                </div>
                {#if item.mismatch}
                  <div class="warn">Durées différentes</div>
                {/if}
              </div>
              <div class="players">
                <div>
                  <p class="muted">Ancienne</p>
                  <audio controls src={item.origUrl}></audio>
                </div>
                <div>
                  <p class="muted">Nouvelle (SoundCloud)</p>
                  <audio controls src={item.newUrl}></audio>
                </div>
              </div>
              <div class="actions" style="gap:10px;">
                <button
                  class="btn primary"
                  on:click={() => acceptReplacement(item)}>Accepter</button
                >
                <button
                  class="btn ghost"
                  on:click={() => ignoreReplacement(item)}>Ignorer</button
                >
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <div class="scan-table">
      <div class="scan-row head">
        <div>Statut</div>
        <div>Bitrate</div>
        <div>Lossless</div>
        <div>Nom</div>
        <div>Chemin</div>
        <div>Actions</div>
      </div>
      {#each filteredResults as item}
        <ScanRow
          {item}
          spectrumUrl={spectra[item.path]}
          loading={!!spectroLoading[item.path]}
          downloadStatus={downloadStatus[item.path]}
          on:reveal={(e) => reveal(e.detail)}
          on:spectrum={(e) => spectrum(e.detail)}
          on:redownload={handleSingleRedownload}
        />
      {/each}
    </div>
  {/if}

  <!-- Redownload Options Modal -->
  <RedownloadModal
    show={showRedownloadModal}
    trackCount={redownloadTargets.length}
    on:cancel={cancelRedownload}
    on:start={executeRedownload}
  />

  <!-- Manual URL Input Modal -->
  {#if manualUrlTrack}
    <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
    <div
      class="modal-overlay"
      on:click={closeManualUrl}
      on:keydown={(e) => e.key === "Escape" && closeManualUrl()}
      role="dialog"
      aria-modal="true"
      tabindex="-1"
    >
      <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
      <!-- svelte-ignore a11y-click-events-have-key-events -->
      <div class="modal" on:click|stopPropagation role="document">
        <h3><Link size={20} /> Piste non trouvée</h3>
        <p class="muted">
          "{manualUrlTrack.name || manualUrlTrack.path.split("/").pop()}" n'a
          pas été trouvé automatiquement.
        </p>
        <p>Entrez une URL Tidal ou SoundCloud :</p>
        <input
          type="url"
          bind:value={manualUrlInput}
          placeholder="https://tidal.com/... ou https://soundcloud.com/..."
          class="url-input"
        />
        {#if manualUrlError}
          <p class="error">{manualUrlError}</p>
        {/if}
        <div class="modal-actions">
          <button class="btn primary" on:click={submitManualUrl}>
            Télécharger
          </button>
          <button class="btn ghost" on:click={closeManualUrl}> Ignorer </button>
        </div>
      </div>
    </div>
  {/if}
</section>

<style>
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: var(--bg-card, #1a1a2e);
    border-radius: 12px;
    padding: 24px;
    max-width: 500px;
    width: 90%;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  }

  .modal h3 {
    margin: 0 0 12px;
    font-size: 1.3rem;
  }

  .modal .muted {
    color: var(--text-muted, #888);
    font-size: 0.9rem;
    margin-bottom: 16px;
  }

  .url-input {
    width: 100%;
    padding: 12px;
    border: 1px solid var(--border, #333);
    border-radius: 8px;
    background: var(--bg-input, #0f0f1a);
    color: var(--text, #fff);
    font-size: 1rem;
    margin-bottom: 8px;
  }

  .url-input:focus {
    outline: none;
    border-color: var(--primary, #646cff);
  }

  .error {
    color: #ff6b6b;
    font-size: 0.85rem;
    margin: 8px 0;
  }

  .modal-actions {
    display: flex;
    gap: 10px;
    margin-top: 16px;
    justify-content: flex-end;
  }

  .downloading-placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    padding: 48px 24px;
    background: linear-gradient(
      145deg,
      rgba(30, 30, 50, 0.6),
      rgba(20, 20, 35, 0.7)
    );
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .downloading-placeholder .spinner {
    width: 40px;
    height: 40px;
    border: 3px solid rgba(255, 255, 255, 0.1);
    border-top-color: var(--primary, #646cff);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
