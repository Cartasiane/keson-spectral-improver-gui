#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use tauri::Manager;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

#[derive(Serialize)]
struct QueueStats {
    active: u32,
    pending: u32,
}

#[derive(Serialize)]
struct DownloadResult {
    title: String,
    caption: String,
    quality: String,
    warning: String,
    saved_to: String,
}

#[derive(Serialize)]
struct ScanResult {
    path: String,
    name: String,
    bitrate: Option<u32>,
    status: String, // "ok" | "bad" | "error"
}

#[tauri::command]
fn queue_stats() -> QueueStats {
    QueueStats { active: 0, pending: 0 }
}

#[tauri::command]
fn download_link(url: String, output_dir: Option<String>) -> Result<DownloadResult, String> {
    // TODO: bridge to Node core (spawn a sidecar or call a background service)
    let target = output_dir.unwrap_or_else(|| "~/Music".to_string());
    Ok(DownloadResult {
        title: "Placeholder".into(),
        caption: url,
        quality: "Unknown".into(),
        warning: String::new(),
        saved_to: target,
    })
}

#[tauri::command]
fn scan_folder(
    folder: String,
    min_kbps: Option<u32>,
    app: tauri::AppHandle,
) -> Result<Vec<ScanResult>, String> {
    let min = min_kbps.unwrap_or(256);
    let root = Path::new(&folder);
    if !root.exists() {
        return Err("Dossier introuvable".into());
    }

    // Phase 1: discovery with lightweight progress tick
    let mut audio_entries = Vec::new();
    let mut discovered = 0usize;
    let mut tick = 0u32;
    let _ = app.emit_all("scan_progress", 5u32); // start

    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            discovered += 1;
            if is_audio(entry.path()) {
                audio_entries.push(entry);
            }
            // emit small progress ticks during discovery
            let pct = 5 + ((discovered as f64).sqrt() as u32 % 10); // gentle movement up to ~15%
            if pct != tick {
                tick = pct;
                let _ = app.emit_all("scan_progress", pct.min(15));
            }
        }
    }

    if audio_entries.is_empty() {
        let _ = app.emit_all("scan_progress", 100u32);
        return Ok(Vec::new());
    }

    let total = audio_entries.len();
    let mut results = Vec::with_capacity(total);

    for (idx, entry) in audio_entries.into_iter().enumerate() {
        let path = entry.path();
        let bitrate = probe_bitrate(path).ok();
        let status = match bitrate {
            Some(b) if b < min => "bad".to_string(),
            Some(_) => "ok".to_string(),
            None => "error".to_string(),
        };
        results.push(ScanResult {
            path: path.display().to_string(),
            name: entry.file_name().to_string_lossy().into(),
            bitrate,
            status,
        });

        // emit progress: map processing into 15-100 range
        let percent = 15.0 + ((idx + 1) as f64 / total as f64) * 85.0;
        let _ = app.emit_all("scan_progress", percent.round() as u32);
    }
    Ok(results)
}

fn is_audio(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()) {
        Some(ext) => matches!(
            ext.as_str(),
            "mp3" | "m4a" | "aac" | "wav" | "flac" | "ogg" | "opus" | "webm"
        ),
        None => false,
    }
}

fn probe_bitrate(path: &Path) -> Result<u32, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=bit_rate",
            "-of",
            "default=nk=1:nw=1",
            path.to_str().unwrap_or_default(),
        ])
        .output()
        .map_err(|e| format!("ffprobe: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let txt = String::from_utf8_lossy(&output.stdout);
    let val: f64 = txt.trim().parse().map_err(|_| "parse failed".to_string())?;
    Ok((val / 1000.0).round() as u32)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            queue_stats,
            download_link,
            scan_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
