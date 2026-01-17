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

  // Redownload modal state
  let showRedownloadModal = false;
  let redownloadTargets = []; // paths to redownload
  let downloadStatus = {}; // { [path]: 'pending' | 'downloading' | 'done' | 'error' }

  // Downloaded comparison state
  let downloadedItems = []; // Array of comparison objects

  $: noMatchCount = Object.values(downloadStatus).filter(
    (s) => s === "no-match",
  ).length;

  $: filteredResults =
    filter === "all"
      ? scanResults
      : filter === "no-match"
        ? scanResults.filter((r) => downloadStatus[r.path] === "no-match")
        : filter === "replaced"
          ? scanResults.filter((r) => r.status === "replaced" || r.replaced)
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
      let noMatchesCount = 0;

      for (const filePath of redownloadTargets) {
        downloadStatus = { ...downloadStatus, [filePath]: "downloading" };

        try {
          const saved = await redownloadBad([filePath], { source, backup });

          // Check if no match was found (empty results)
          if (saved.length === 0) {
            noMatchesCount++;
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
                (s) => s.path === r.original_path,
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
                (s) => s.path !== r.original_path,
              );
            }
          }
        } catch (err) {
          console.error(`Failed: ${filePath}`, err);
          const errMsg = typeof err === "string" ? err : err?.message || "";

          // Check for QUEUE_FULL - retry with exponential backoff
          if (errMsg.startsWith("QUEUE_FULL:")) {
            let retrySuccess = false;
            for (let attempt = 1; attempt <= 2; attempt++) {
              const delay = attempt * 2000; // 2s, 4s
              downloadStatus = {
                ...downloadStatus,
                [filePath]: `retry-${attempt}`,
              };
              await new Promise((r) => setTimeout(r, delay));

              try {
                const saved = await redownloadBad([filePath], {
                  source,
                  backup,
                });
                if (saved.length > 0) {
                  results.push(...saved);
                  downloadStatus = { ...downloadStatus, [filePath]: "done" };
                  retrySuccess = true;
                  break;
                }
              } catch (retryErr) {
                const retryMsg =
                  typeof retryErr === "string"
                    ? retryErr
                    : retryErr?.message || "";
                if (!retryMsg.startsWith("QUEUE_FULL:")) {
                  break; // Different error, stop retrying
                }
              }
            }
            if (!retrySuccess) {
              downloadStatus = { ...downloadStatus, [filePath]: "queue-full" };
            }
          } else {
            downloadStatus = { ...downloadStatus, [filePath]: "error" };
          }
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
      if (noMatchesCount > 0) {
        msgParts.push(`${noMatchesCount} non trouvé(s)`);
        // Switch to no-match filter so user sees them
        filter = "no-match";
      }
      scanMessage = msgParts.join(", ") || "Terminé.";
    } catch (err) {
      console.error(err);
      scanMessage =
        typeof err === "string"
          ? err
          : err?.message || "Échec du retéléchargement";
    } finally {
      retrying = false;
      redownloadTargets = [];
      // Don't clear status automatically - let user handle no-match items
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
          origUrl,
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
    // Set status to no-match so user can try again with a different URL
    downloadStatus = { ...downloadStatus, [entry.original_path]: "no-match" };
    reviewQueue = reviewQueue.filter((r) => r !== entry);
    scanMessage = "Ignoré - vous pouvez réessayer avec une autre URL.";
  }

  // Handler for inline manual URL submissions from ScanRow
  async function handleInlineManualUrl(event) {
    const { path, url } = event.detail;

    // Update status to downloading
    downloadStatus = { ...downloadStatus, [path]: "downloading" };
    scanMessage = "Téléchargement en cours...";

    try {
      const result = await downloadWithUrl(path, url, true);

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
            (s) => s.path === result.original_path,
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
            (s) => s.path !== result.original_path,
          );

          // Clear the no-match status
          downloadStatus = { ...downloadStatus, [path]: "done" };
          filter = "downloaded";
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
          downloadStatus = { ...downloadStatus, [path]: "done" };
          scanMessage = "Téléchargé - vérifiez la durée.";
        }
      } else {
        downloadStatus = { ...downloadStatus, [path]: "no-match" };
        scanMessage = "Échec du téléchargement.";
      }
    } catch (err) {
      console.error(err);
      downloadStatus = { ...downloadStatus, [path]: "no-match" };
      scanMessage = err?.message || "Échec du téléchargement";
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
      reviewCount={reviewQueue.length}
      {noMatchCount}
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
              (i) => i.original_path !== e.detail.original_path,
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

    <!-- Review queue - only show when review filter is active -->
    {#if filter === "review" && reviewQueue.length > 0}
      <div class="scan-table">
        <div class="scan-row head">
          <div>Durées</div>
          <div style="grid-column: span 2;">Ancienne</div>
          <div style="grid-column: span 2;">Nouvelle</div>
          <div>Actions</div>
        </div>
        {#each reviewQueue as item}
          <div class="scan-row review-row">
            <div class="duration-info">
              <span class="warn"
                >{item.original_duration?.toFixed(1) || "?"}s → {item.new_duration?.toFixed(
                  1,
                ) || "?"}s</span
              >
            </div>
            <div style="grid-column: span 2;">
              <audio
                controls
                src={item.origUrl}
                style="width: 100%; max-width: 200px;"
              ></audio>
            </div>
            <div style="grid-column: span 2;">
              <audio
                controls
                src={item.newUrl}
                style="width: 100%; max-width: 200px;"
              ></audio>
            </div>
            <div class="actions actions-inline">
              <button
                class="btn mini primary"
                on:click={() => acceptReplacement(item)}>Accepter</button
              >
              <button
                class="btn mini ghost"
                on:click={() => ignoreReplacement(item)}>Ignorer</button
              >
            </div>
          </div>
        {/each}
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
          on:manualUrl={handleInlineManualUrl}
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
</section>

<style>
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

  .review-row {
    background: rgba(59, 130, 246, 0.1) !important;
    border-left: 2px solid #3b82f6 !important;
  }

  .duration-info {
    font-size: 13px;
  }
</style>
