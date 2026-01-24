# GUI Development Guide

This document covers how to build, run, and modify the `keson-spectral-improver-gui`.

## 🛠 Prerequisites

- **Node.js** (v18+)
- **Rust** & Cargo (latest stable)
- **pnpm** (recommended) or npm

## 🚀 Quick Start

1. **Install Dependencies**

   ```bash
   npm install
   ```

2. **Run in Development Mode**
   Start the Vite frontend server:

   ```bash
   npm run dev
   ```

   In a separate terminal, launch the Tauri desktop app:

   ```bash
   npm run tauri dev
   ```

## 🔌 Bridging to Core

The GUI interacts with `keson-spectral-improver-core` for heavy lifting.

- **Rust Backend**: `src-tauri/src/main.rs` exposes commands like `download_link` and `queue_stats`.
- **Frontend**: The Svelte UI (`src/App.svelte`) calls these commands using `@tauri-apps/api`.
- **Sidecars**: Binaries like `ffmpeg` and `ffprobe` are bundled as sidecars to ensure functionality on user machines.

## 📦 Building for Release

To create a production build/installer:

```bash
npm run tauri build
```

Artifacts will be in `src-tauri/target/release/bundle/`.

## 🎨 Styling

The app uses a customized version of [system.css](https://github.com/sakofchit/system.css) to achieve its "Retro/NeXT" aesthetic.
