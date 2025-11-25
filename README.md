# Keson Spectral Improver GUI

Tauri + Svelte desktop app styled with system.css. It calls the shared core package (`keson-spectral-improver-core`) to download SoundCloud tracks and display quality info.

## Dev quickstart

```bash
cd packages/gui
npm install
npm run dev      # Vite dev server (for frontend only)
# in another shell
npm run tauri    # launches desktop app
```

> You need Rust + cargo installed for Tauri.

## Bridging to the core
- `src-tauri/src/main.rs` exposes `download_link` and `queue_stats` commands. Wire these to the Node core by spawning a sidecar process or embedding a small Node runner.
- The Svelte UI (`src/App.svelte`) calls these commands via `@tauri-apps/api/tauri`.

## Styling
- Uses [`system.css`](https://github.com/sakofchit/system.css) for the NeXT-like window chrome.

## Next steps
- Replace the placeholder implementations in `main.rs` with real calls to `downloadTrack` (via a Node sidecar or a Rust port).
- Persist output directory choice and remember recent downloads.
- Surface playlist chunk prompts and bitrate warnings in the UI.
