<script>
  import { onMount, onDestroy } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";

  export let pathA = ""; // Original/backup file
  export let pathB = ""; // New downloaded file
  export let labelA = "Avant";
  export let labelB = "Après";

  let audioA;
  let audioB;
  let isPlaying = false;
  let activeTrack = "B"; // Start with the "after" track
  let currentTime = 0;
  let duration = 0;
  let seeking = false;

  function formatTime(seconds) {
    if (!seconds || isNaN(seconds)) return "0:00";
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  function togglePlay() {
    if (isPlaying) {
      audioA?.pause();
      audioB?.pause();
    } else {
      // Sync silenced track to active track before playing
      if (activeTrack === "A" && audioA && audioB) {
        audioB.currentTime = audioA.currentTime;
      } else if (activeTrack === "B" && audioA && audioB) {
        audioA.currentTime = audioB.currentTime;
      }

      audioA?.play();
      audioB?.play();
    }
    isPlaying = !isPlaying;
  }

  function switchTrack(track) {
    if (track === activeTrack) return;

    // No sync on switch - rely purely on background Smart Sync to keep them aligned.
    // This ensures switching is instant (just a mute toggle).

    activeTrack = track;
    updateMuting();
  }

  function updateMuting() {
    if (audioA) audioA.muted = activeTrack !== "A";
    if (audioB) audioB.muted = activeTrack !== "B";
  }

  function handleTimeUpdate() {
    if (!seeking) {
      currentTime = audioB?.currentTime || audioA?.currentTime || 0;
    }
  }

  function handleLoadedMetadata() {
    duration = Math.max(audioA?.duration || 0, audioB?.duration || 0);
  }

  function handleSeek(e) {
    const rect = e.target.getBoundingClientRect();
    const percent = (e.clientX - rect.left) / rect.width;
    const newTime = percent * duration;

    if (audioA) audioA.currentTime = newTime;
    if (audioB) audioB.currentTime = newTime;
    currentTime = newTime;
  }

  function handleEnded() {
    isPlaying = false;
    currentTime = 0;
    if (audioA) audioA.currentTime = 0;
    if (audioB) audioB.currentTime = 0;
  }

  let syncInterval;

  onMount(() => {
    updateMuting();

    // Check for drift every second
    // Check for drift every 100ms
    syncInterval = setInterval(() => {
      if (isPlaying && audioA && audioB && !seeking) {
        const diff = audioB.currentTime - audioA.currentTime; // Positive if B is ahead
        const absDiff = Math.abs(diff);

        if (absDiff > 0.5) {
          // Hard sync if drift is huge (> 500ms)
          if (activeTrack === "A") {
            audioB.currentTime = audioA.currentTime;
          } else {
            audioA.currentTime = audioB.currentTime;
          }
          audioA.playbackRate = 1;
          audioB.playbackRate = 1;
        } else if (absDiff > 0.02) {
          // Soft nudge if drift is small (> 20ms)
          // If B is ahead (diff > 0), slow B down or speed A up
          // We only adjust the silenced track to avoid pitch shifting correct track

          if (activeTrack === "A") {
            // A is Master. Adjust B.
            if (diff > 0) {
              audioB.playbackRate = 0.95; // Slow down B to let A catch up
            } else {
              audioB.playbackRate = 1.05; // Speed up B to catch A
            }
            audioA.playbackRate = 1;
          } else {
            // B is Master. Adjust A.
            if (diff > 0) {
              audioA.playbackRate = 1.05; // Speed up A to catch B
            } else {
              audioA.playbackRate = 0.95; // Slow down A to let B catch up
            }
            audioB.playbackRate = 1;
          }
        } else {
          // In sync
          audioA.playbackRate = 1;
          audioB.playbackRate = 1;
        }
      }
    }, 100);
  });

  onDestroy(() => {
    if (syncInterval) clearInterval(syncInterval);
  });

  $: progress = duration > 0 ? (currentTime / duration) * 100 : 0;
</script>

<div class="ab-player">
  <div class="track-switcher">
    <button
      class="track-btn"
      class:active={activeTrack === "A"}
      on:click={() => switchTrack("A")}
    >
      <span class="indicator bad"></span>
      {labelA}
    </button>
    <button
      class="track-btn"
      class:active={activeTrack === "B"}
      on:click={() => switchTrack("B")}
    >
      <span class="indicator good"></span>
      {labelB}
    </button>
  </div>

  <div class="controls">
    <button class="play-btn" on:click={togglePlay}>
      {#if isPlaying}
        <svg viewBox="0 0 24 24" fill="currentColor" width="24" height="24">
          <rect x="6" y="5" width="4" height="14" rx="1" />
          <rect x="14" y="5" width="4" height="14" rx="1" />
        </svg>
      {:else}
        <svg viewBox="0 0 24 24" fill="currentColor" width="24" height="24">
          <path d="M8 5.14v14l11-7-11-7z" />
        </svg>
      {/if}
    </button>

    <!-- svelte-ignore a11y-click-events-have-key-events -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="progress-container" on:click={handleSeek}>
      <div class="progress-bar">
        <div class="progress-fill" style="width: {progress}%"></div>
        <div class="progress-thumb" style="left: {progress}%"></div>
      </div>
    </div>

    <div class="time">
      {formatTime(currentTime)} / {formatTime(duration)}
    </div>
  </div>

  <!-- Hidden audio elements -->
  <audio
    bind:this={audioA}
    src={pathA ? convertFileSrc(pathA) : ""}
    on:timeupdate={handleTimeUpdate}
    on:loadedmetadata={handleLoadedMetadata}
    on:ended={handleEnded}
    preload="metadata"
  ></audio>
  <audio
    bind:this={audioB}
    src={pathB ? convertFileSrc(pathB) : ""}
    on:timeupdate={handleTimeUpdate}
    on:loadedmetadata={handleLoadedMetadata}
    on:ended={handleEnded}
    preload="metadata"
  ></audio>
</div>

<style>
  .ab-player {
    background: linear-gradient(
      145deg,
      rgba(30, 30, 50, 0.9),
      rgba(20, 20, 35, 1)
    );
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 16px;
  }

  .track-switcher {
    display: flex;
    gap: 8px;
    margin-bottom: 16px;
  }

  .track-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 10px 16px;
    background: rgba(255, 255, 255, 0.05);
    border: 2px solid transparent;
    border-radius: 8px;
    color: var(--text-muted, #888);
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
  }

  .track-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .track-btn.active {
    border-color: var(--primary, #646cff);
    color: var(--text, #fff);
    background: rgba(100, 108, 255, 0.1);
  }

  .indicator {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .indicator.bad {
    background: #f87171;
  }

  .indicator.good {
    background: #34d399;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .play-btn {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: linear-gradient(135deg, var(--primary, #646cff), #8b5cf6);
    border: none;
    color: white;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition:
      transform 0.15s,
      box-shadow 0.15s;
  }

  .play-btn:hover {
    transform: scale(1.05);
    box-shadow: 0 4px 20px rgba(100, 108, 255, 0.4);
  }

  .play-btn:active {
    transform: scale(0.95);
  }

  .progress-container {
    flex: 1;
    height: 24px;
    display: flex;
    align-items: center;
    cursor: pointer;
  }

  .progress-bar {
    width: 100%;
    height: 6px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 3px;
    position: relative;
    overflow: visible;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--primary, #646cff), #8b5cf6);
    border-radius: 3px;
    transition: width 0.1s;
  }

  .progress-thumb {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 14px;
    height: 14px;
    background: white;
    border-radius: 50%;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
    transition: left 0.1s;
  }

  .progress-container:hover .progress-thumb {
    transform: translate(-50%, -50%) scale(1.2);
  }

  .time {
    font-size: 0.8rem;
    color: var(--text-muted, #888);
    font-variant-numeric: tabular-nums;
    min-width: 80px;
    text-align: right;
  }

  audio {
    display: none;
  }
</style>
