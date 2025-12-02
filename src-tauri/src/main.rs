#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use tauri::Manager;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;
use std::io::{BufRead, BufReader};
use tauri::async_runtime;

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
        let paths: Vec<PathBuf> = audio_entries.into_iter().map(|e| e.path().to_path_buf()).collect();
        analyze_batch_wmb_stream(&paths, &vendor, min, &handle)
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

fn analyze_batch_wmb_stream(
    paths: &[PathBuf],
    vendor_dir: &Path,
    min: u32,
    app: &tauri::AppHandle,
) -> Result<Vec<ScanResult>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }

    let python = "python3";
    let py_check = Command::new(python)
        .arg("--version")
        .output()
        .map_err(|e| format!("python3 introuvable: {e}"))?;
    if !py_check.status.success() {
        return Err("python3 introuvable (ajoute-le au PATH)".into());
    }

    let list_json = serde_json::to_string(
        &paths
            .iter()
            .map(|p| p.to_str().unwrap_or_default())
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into());

    let script = format!(
        r#"
import sys, json, concurrent.futures
sys.path.insert(0, r"{vendor}")
from wmb_core import AudioFile

def worker(path):
    try:
        af = AudioFile(path)
        af.analyze(generate_spectrogram_flag=False, assets_dir=None)
        return {{"ok": True, "data": af.to_dict()}}
    except Exception as e:
        return {{"ok": False, "data": {{"file": path, "error": str(e)}}}}

def main():
    paths = json.loads(sys.argv[1])
    with concurrent.futures.ProcessPoolExecutor() as ex:
        for res in ex.map(worker, paths):
            print(json.dumps(res), flush=True)

if __name__ == "__main__":
    main()
"#,
        vendor = vendor_dir.display()
    );

    let mut child = Command::new(python)
        .args(["-u", "-c", &script, &list_json])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("python3 spawn: {e}"))?;

    let total = paths.len();
    let mut results = Vec::with_capacity(total);
    let mut seen = 0usize;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = line.unwrap_or_default();
            if line.trim().is_empty() {
                continue;
            }
            let parsed: serde_json::Value =
                serde_json::from_str(&line).unwrap_or_else(|_| serde_json::json!({}));
            let ok = parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            let data = parsed.get("data").cloned().unwrap_or_default();

            let (path_str, bitrate, is_lossless, note, status) = if ok {
                let est = data
                    .get("estimated_bitrate_numeric")
                    .and_then(|v| v.as_f64())
                    .map(|v| v.round() as u32);
                let lossless = data.get("is_lossless").and_then(|v| v.as_bool());
                let path_str = data
                    .get("file")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let status = match est {
                    Some(b) if b < min => "bad".to_string(),
                    Some(_) => "ok".to_string(),
                    None => "error".to_string(),
                };
                (path_str, est, lossless, None, status)
            } else {
                let path_str = data
                    .get("file")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let err = data
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                (path_str, None, None, Some(err), "error".to_string())
            };

            let name = Path::new(&path_str)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            results.push(ScanResult {
                path: path_str,
                name,
                bitrate,
                is_lossless,
                note,
                status,
            });

            seen += 1;
            let percent = 15.0 + (seen as f64 / total as f64) * 85.0;
            let _ = app.emit_all("scan_progress", percent.round() as u32);
        }
    }

    let status = child.wait().map_err(|e| format!("python3 wait: {e}"))?;
    if !status.success() {
        return Err("whatsmybitrate a échoué (processus)".into());
    }

    Ok(results)
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
