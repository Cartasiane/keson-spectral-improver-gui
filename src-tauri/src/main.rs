#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use tauri::Manager;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;
use tauri::async_runtime;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use rayon::iter::IntoParallelRefIterator;

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
    is_lossless: Option<bool>,
    note: Option<String>,
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
async fn scan_folder(
    folder: String,
    min_kbps: Option<u32>,
    app: tauri::AppHandle,
) -> Result<Vec<ScanResult>, String> {
    let handle = app.clone();
    async_runtime::spawn_blocking(move || {
        let min = min_kbps.unwrap_or(256);
        let root = Path::new(&folder);
        if !root.exists() {
            return Err("Dossier introuvable".into());
        }

        let mut audio_entries = Vec::new();
        let mut discovered = 0usize;
        let mut tick = 0u32;
        let _ = handle.emit_all("scan_progress", 1u32); // start

        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                discovered += 1;
                if is_audio(entry.path()) {
                    audio_entries.push(entry);
                }
                let pct = 1 + ((discovered as f64).sqrt() as u32 % 12); // gentle movement up to ~13%
                if pct != tick {
                    tick = pct;
                    let _ = handle.emit_all("scan_progress", pct.min(15));
                }
            }
        }

        if audio_entries.is_empty() {
            let _ = handle.emit_all("scan_progress", 100u32);
            return Ok(Vec::new());
        }

        let vendor = vendor_dir(&handle)?;
        let total = audio_entries.len();
        let counter = AtomicUsize::new(0);

        let results: Vec<ScanResult> = audio_entries
            .par_iter()
            .map(|entry| {
                let path = entry.path();
                let analysis = analyze_with_wmb_single(path, &vendor, min);
                let (bitrate, is_lossless, note, status) = match analysis {
                    Ok(res) => res,
                    Err(err) => (None, None, Some(err), "error".to_string()),
                };

                let done = counter.fetch_add(1, Ordering::SeqCst) + 1;
                let percent: f64 = 15.0 + (done as f64 / total as f64) * 85.0;
                let _ = handle.emit_all("scan_progress", percent.round() as u32);

                ScanResult {
                    path: path.display().to_string(),
                    name: entry.file_name().to_string_lossy().into(),
                    bitrate,
                    is_lossless,
                    note,
                    status,
                }
            })
            .collect();

        Ok(results)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn reveal_in_folder(path: String) -> Result<(), String> {
    if !Path::new(&path).exists() {
        return Err("Fichier introuvable".into());
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(&path)
            .status()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg("/select,")
            .arg(path.replace('/', "\\"))
            .status()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(dir) = Path::new(&path).parent() {
            Command::new("xdg-open")
                .arg(dir)
                .status()
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn open_spectrum(path: String) -> Result<String, String> {
    let src = Path::new(&path);
    if !src.exists() {
        return Err("Fichier introuvable".into());
    }

    // Locate vendored whatsmybitrate
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let vendor_candidates = [
        PathBuf::from("vendor/whatsmybitrate"),
        cwd.join("vendor/whatsmybitrate"),
        cwd.join("../vendor/whatsmybitrate"),
    ];
    let vendor_dir = vendor_candidates
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| "Vendor whatsmybitrate introuvable".to_string())?;

    // Ensure python3 is available
    let python = "python3";
    let py_check = Command::new(python)
        .arg("--version")
        .output()
        .map_err(|e| format!("python3 introuvable: {e}"))?;
    if !py_check.status.success() {
        return Err("python3 introuvable (ajoute-le au PATH)".into());
    }

    let script = format!(
        r#"
import sys, tempfile, os
sys.path.insert(0, r"{vendor}")
from wmb_core import AudioFile
path = sys.argv[1]
tmp = tempfile.mkdtemp(prefix="wmb-")
af = AudioFile(path)
af.analyze(generate_spectrogram_flag=True, assets_dir=tmp)
print(af.spectrogram_path or "")
"#,
        vendor = vendor_dir.display()
    );

    let output = Command::new(python)
        .args(["-c", &script, src.to_str().unwrap_or_default()])
        .output()
        .map_err(|e| format!("python3: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("ModuleNotFoundError") || stderr.contains("No module named") {
            return Err(
                "Dépendances Python manquantes (librosa, matplotlib, numpy...). \
Installe-les : pip install -r vendor/whatsmybitrate/requirements.txt"
                    .into(),
            );
        }
        return Err(format!("whatsmybitrate a échoué: {}", stderr));
    }

    let spectro = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if spectro.is_empty() {
        return Err(
            "whatsmybitrate n'a pas produit de spectrogramme (vérifie les dépendances Python ou la taille du fichier)"
                .into(),
        );
    }
    Ok(spectro)
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

fn vendor_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path_resolver()
        .resolve_resource("..")
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let candidates = [
        base.join("vendor/whatsmybitrate"),
        base.join("../vendor/whatsmybitrate"),
        PathBuf::from("vendor/whatsmybitrate"),
    ];
    candidates
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| "Vendor whatsmybitrate introuvable".to_string())
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

fn analyze_with_wmb_single(
    path: &Path,
    vendor_dir: &Path,
    min: u32,
) -> Result<(Option<u32>, Option<bool>, Option<String>, String), String> {
    let python = "python3";
    let py_check = Command::new(python)
        .arg("--version")
        .output()
        .map_err(|e| format!("python3 introuvable: {e}"))?;
    if !py_check.status.success() {
        return Err("python3 introuvable (ajoute-le au PATH)".into());
    }

    let script = format!(
        r#"
import sys, json
sys.path.insert(0, r"{vendor}")
from wmb_core import AudioFile
af = AudioFile(sys.argv[1])
af.analyze(generate_spectrogram_flag=False, assets_dir=None)
print(json.dumps(af.to_dict()))
"#,
        vendor = vendor_dir.display()
    );

    let output = Command::new(python)
        .args(["-c", &script, path.to_str().unwrap_or_default()])
        .output()
        .map_err(|e| format!("python3: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "whatsmybitrate a échoué: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("parse json: {e}"))?;
    let est = parsed
        .get("estimated_bitrate_numeric")
        .and_then(|v| v.as_f64())
        .map(|v| v.round() as u32);
    let lossless = parsed.get("is_lossless").and_then(|v| v.as_bool());
    let err = parsed
        .get("error")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let status = match (err.is_some(), est) {
        (true, _) => "error".to_string(),
        (false, Some(b)) if b < min => "bad".to_string(),
        (false, Some(_)) => "ok".to_string(),
        _ => "error".to_string(),
    };

    Ok((est, lossless, err, status))
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            queue_stats,
            download_link,
            scan_folder,
            reveal_in_folder,
            open_spectrum
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
