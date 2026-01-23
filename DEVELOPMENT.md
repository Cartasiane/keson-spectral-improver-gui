# Development Guide

## Prerequisites

- **Node.js**: v20 or later (Use `nvm` to manage versions)
- **Rust**: Stable toolchain (via `rustup`)
- **Python**: v3.11+ (Required for script utilities)

## Setting up the Environment

### Windows Development

Because the required FFmpeg binaries are **>100MB**, they are **not committed** to the git repository (except for macOS). You must download them manually using the provided script.

1. **Install Python Dependencies**:
   The script uses `py7zr` to handle archives.

   ```powershell
   pip install py7zr
   ```

2. **Download Binaries**:
   Run the utility script to fetch FFmpeg/FFprobe for Windows (and other platforms).

   ```powershell
   python download_binaries.py
   ```

   > **Note**: This will create `src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe` and `ffprobe`.

3. **Run the App**:
   Now you can start the Tauri development server.
   ```powershell
   npm run tauri dev
   ```

### macOS Development

macOS binaries are smaller (<100MB) and are **committed to the repository**. You typically don't need to run the download script unless you want to update them or fetch Intel/ARM variants manually.

Simply run:

```bash
npm run tauri dev
```

### Linux Development

Linux binaries are also large and ignored by git. Use the same script as Windows:

```bash
pip install py7zr
python3 download_binaries.py
npm run tauri dev
```

## Verify Sidecars

To ensure sidecars are correctly detected:

1. Run the app in dev mode.
2. Go to `Quality` tab.
3. If errors occur regarding "command not found" or "permission denied", ensure:
   - `src-tauri/binaries/` contains the binary with the correct target triple suffix.
   - `src-tauri/capabilities/desktop.json` allows the binary execution.
