#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use tauri::Manager;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;
use tauri::async_runtime;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use rayon::iter::IntoParallelRefIterator;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::fs;
use std::io::{self, Read};
use sha2::{Sha256, Digest};
use hex;
use rayon::ThreadPoolBuilder;
use num_cpus;
use std::str::FromStr;

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
struct RedownloadResult {
    original_path: String,
    new_path: String,
    original_duration: Option<f64>,
    new_duration: Option<f64>,
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

#[derive(Serialize, Deserialize, Clone)]
struct CacheEntry {
    bitrate: Option<u32>,
    is_lossless: Option<bool>,
    note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Settings {
    min_bitrate: u32,
    analysis_window_seconds: u32,
    rayon_threads: usize, // 0 = default logical cores
    cache_enabled: bool,
    cache_max_entries: usize
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            min_bitrate: 256,
            analysis_window_seconds: 100,
            rayon_threads: 0,
            cache_enabled: true,
            cache_max_entries: 10_000,
        }
    }
}

#[tauri::command]
fn queue_stats() -> QueueStats {
    QueueStats { active: 0, pending: 0 }
}

#[tauri::command]
fn download_link(url: String, output_dir: Option<String>, app: tauri::AppHandle) -> Result<DownloadResult, String> {
    let out_dir = output_dir.unwrap_or_else(|| String::from("./"));
    let _settings = load_settings_path(&app);

    // Check if we should use remote server
    // For now, let's assume if a remote URL is configured in settings (we need to add this to settings struct first), we use it.
    // But since I haven't updated Settings struct yet, I'll stick to local execution for now, 
    // or I can add a quick check for an environment variable or just default to local.
    
    // Let's implement local execution using the new musicdl wrapper in keson-core first.
    
    let script = r#"
      const fs = require('fs');
      const path = require('path');
      const core = require('../keson-spectral-improver-core');
      const url = process.argv[2];
      const destDir = process.argv[3];
      // We can pass token/source via env vars or args if needed. 
      // For now, simple usage.
      
      (async () => {
        const res = await core.downloadTrack(url);
        const src = res.path;
        if (!src) throw new Error('No file path returned by core');
        
        const filename = res.filename || path.basename(src);
        const targetDir = destDir || '.';
        fs.mkdirSync(targetDir, { recursive: true });
        const dest = path.join(targetDir, filename);
        
        // Move file from temp to dest
        if (src !== dest) fs.renameSync(src, dest);
        
        const title = (res.metadata && (res.metadata.title || res.metadata.name)) || filename;
        const quality = res.metadata && (res.metadata.bitrate || res.metadata.quality || '');
        
        console.log(JSON.stringify({ saved_to: dest, title, quality }));
      })().catch((err) => {
        console.error(err && err.stack ? err.stack : String(err));
        process.exit(1);
      });
    "#;

    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&url)
        .arg(&out_dir)
        .current_dir("../")
        .output()
        .map_err(|e| format!("Node exec failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Téléchargement échoué: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Réponse core invalide: {e}: {}", String::from_utf8_lossy(&output.stdout)))?;

    Ok(DownloadResult {
        title: parsed.get("title").and_then(|v| v.as_str()).unwrap_or("Track").to_string(),
        caption: url,
        quality: parsed.get("quality").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        warning: String::new(),
        saved_to: parsed
            .get("saved_to")
            .and_then(|v| v.as_str())
            .unwrap_or(&out_dir)
            .to_string(),
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
        let settings = load_settings_path(&handle);
        init_rayon_pool_with(settings.rayon_threads);
        let min = min_kbps.unwrap_or(settings.min_bitrate);
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
        let cache_path = cache_path(&handle)?;
        let cache = Arc::new(Mutex::new(load_cache(&cache_path, settings.cache_max_entries)));
        let total = audio_entries.len();
        let counter = AtomicUsize::new(0);

        let results: Vec<ScanResult> = audio_entries
            .par_iter()
            .map(|entry| {
                let path = entry.path();
                let analysis = analyze_with_wmb_single(
                    path,
                    &vendor,
                    min,
                    settings.analysis_window_seconds,
                    settings.cache_enabled,
                    &cache,
                );
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

        if let Ok(cache_guard) = cache.lock() {
            let _ = save_cache(&cache_path, &*cache_guard);
        }

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

fn file_hash(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn cache_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path_resolver()
        .app_data_dir()
        .or_else(|| app.path_resolver().app_cache_dir())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let path = base.join("analysis-cache.json");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    Ok(path)
}

fn load_cache(path: &Path, limit: usize) -> HashMap<String, CacheEntry> {
    if let Ok(text) = fs::read_to_string(path) {
        let mut map: HashMap<String, CacheEntry> =
            serde_json::from_str(&text).unwrap_or_default();
        enforce_cache_limit(&mut map, limit);
        map
    } else {
        HashMap::new()
    }
}

fn save_cache(path: &Path, cache: &HashMap<String, CacheEntry>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_string(cache).unwrap_or_default())?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn enforce_cache_limit(cache: &mut HashMap<String, CacheEntry>, limit: usize) {
    if limit == 0 || cache.len() <= limit {
        return;
    }
    while cache.len() > limit {
        if let Some(key) = cache.keys().next().cloned() {
            cache.remove(&key);
        } else {
            break;
        }
    }
}

fn analyze_with_wmb_single(
    path: &Path,
    vendor_dir: &Path,
    min: u32,
    analysis_window: u32,
    cache_enabled: bool,
    cache: &Arc<Mutex<HashMap<String, CacheEntry>>>,
) -> Result<(Option<u32>, Option<bool>, Option<String>, String), String> {
    let hash = if cache_enabled { file_hash(path).ok() } else { None };
    if cache_enabled {
        if let Some(h) = &hash {
            if let Ok(guard) = cache.lock() {
                if let Some(entry) = guard.get(h) {
                    let status = match entry.bitrate {
                        Some(b) if b < min => "bad".to_string(),
                        Some(_) => "ok".to_string(),
                        None => "error".to_string(),
                    };
                    return Ok((
                        entry.bitrate,
                        entry.is_lossless,
                        entry.note.clone(),
                        status,
                    ));
                }
            }
        }
    }

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
import wmb_core
wmb_core.MAX_LOAD_SECONDS = {window}
af = AudioFile(sys.argv[1])
af.analyze(generate_spectrogram_flag=False, assets_dir=None)
print(json.dumps(af.to_dict()))
"#,
        vendor = vendor_dir.display(),
        window = analysis_window
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

    if cache_enabled {
        if let Some(h) = hash {
            if let Ok(mut guard) = cache.lock() {
                guard.insert(
                    h,
                    CacheEntry {
                        bitrate: est,
                        is_lossless: lossless,
                        note: err.clone(),
                    },
                );
                enforce_cache_limit(&mut *guard, 10_000);
            }
        }
    }

    Ok((est, lossless, err, status))
}

fn main() {
    init_rayon_pool();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            queue_stats,
            download_link,
            scan_folder,
            reveal_in_folder,
            open_spectrum,
            get_settings,
            save_settings,
            redownload_bad,
            accept_redownload,
            discard_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn init_rayon_pool() {
    // Allow override via env; else use logical CPUs (safer default)
    let threads = std::env::var("RAYON_NUM_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or_else(|| std::cmp::max(1, num_cpus::get()));

    let _ = ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global();
}

fn init_rayon_pool_with(threads: usize) {
    if rayon::current_num_threads() > 0 {
        return; // already set
    }
    let count = if threads > 0 {
        threads
    } else {
        std::cmp::max(1, num_cpus::get())
    };
    let _ = ThreadPoolBuilder::new()
        .num_threads(count)
        .build_global();
}

fn load_settings_path(app: &tauri::AppHandle) -> Settings {
    let path = settings_path(app);
    if let Ok(text) = fs::read_to_string(&path) {
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        Settings::default()
    }
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> Settings {
    load_settings_path(&app)
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    let path = settings_path(&app);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, serde_json::to_string_pretty(&settings).unwrap_or_default())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn redownload_bad(paths: Vec<String>) -> Result<Vec<RedownloadResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut downloaded = Vec::new();
        for path_str in paths {
            let path = PathBuf::from(&path_str);
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| "Nom de fichier invalide".to_string())?;
            let parent = path
                .parent()
                .map(PathBuf::from)
                .ok_or_else(|| "Chemin sans dossier".to_string())?;

            eprintln!("[musicdl] target='{}' dir='{}'", stem, parent.display());

            let search_url = format!("https://soundcloud.com/search?q={}", stem);
            let dest_dir = parent.to_string_lossy().to_string();
            
            let script = r#"
              const fs = require('fs');
              const path = require('path');
              const core = require('../keson-spectral-improver-core');
              const searchQuery = process.argv[2];
              const destDir = process.argv[3];
              
              (async () => {
                // For redownload, we search on SoundCloud and download the best match
                const res = await core.downloadTrack(searchQuery);
                const src = res.path;
                if (!src) throw new Error('No file path returned by core');
                
                const filename = res.filename || path.basename(src);
                const targetDir = destDir || '.';
                fs.mkdirSync(targetDir, { recursive: true });
                const dest = path.join(targetDir, filename);
                
                if (src !== dest) fs.renameSync(src, dest);
                
                console.log(JSON.stringify({ filepath: dest }));
              })().catch((err) => {
                console.error(err && err.stack ? err.stack : String(err));
                process.exit(1);
              });
            "#;

            let output = Command::new("node")
                .arg("-e")
                .arg(script)
                .arg(search_url)
                .arg(&dest_dir)
                .current_dir("../")
                .output()
                .map_err(|e| format!("Node exec failed: {}", e))?;

            eprintln!(
                "[musicdl] status for '{}': {:?}\nstdout:\n{}\nstderr:\n{}",
                stem,
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("musicdl failed for '{}':\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}", stem, output.status, String::from_utf8_lossy(&output.stdout), stderr);
                return Err(format!("Téléchargement échoué pour {}: {}", stem, stderr.trim()));
            }

            let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
                .map_err(|e| format!("Parse error: {}", e))?;
            
            let new_path = parsed.get("filepath")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .ok_or_else(|| "No filepath in response".to_string())?;

            if !new_path.exists() {
                eprintln!("musicdl filepath missing on disk: {}", new_path.display());
                return Err("Fichier téléchargé introuvable".to_string());
            }

            let original_dur = probe_duration(&path);
            let new_dur = probe_duration(&new_path);

            downloaded.push(RedownloadResult {
                original_path: path.to_string_lossy().to_string(),
                new_path: new_path.to_string_lossy().to_string(),
                original_duration: original_dur,
                new_duration: new_dur,
            });
        }
        Ok(downloaded)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn settings_path(app: &tauri::AppHandle) -> PathBuf {
    app.path_resolver()
        .app_data_dir()
        .or_else(|| app.path_resolver().app_cache_dir())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("settings.json")
}

#[tauri::command]
fn accept_redownload(original: String, new_path: String) -> Result<String, String> {
    let orig = PathBuf::from(&original);
    let newp = PathBuf::from(&new_path);

    if !newp.exists() {
        return Err("Fichier téléchargé introuvable".into());
    }

    if let Some(parent) = orig.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Backup old file
    if orig.exists() {
        let ext = orig.extension().and_then(|e| e.to_str()).unwrap_or("");
        let backup_ext = if ext.is_empty() { "bak".to_string() } else { format!("bak.{}", ext) };
        let backup = orig.with_extension(backup_ext);
        let _ = fs::rename(&orig, &backup);
    }

    fs::rename(&newp, &orig).map_err(|e| e.to_string())?;
    Ok(orig.to_string_lossy().to_string())
}

#[tauri::command]
fn discard_file(path: String) -> Result<(), String> {
    let p = PathBuf::from(path);
    if p.exists() {
        fs::remove_file(p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn probe_duration(path: &Path) -> Option<f64> {
    let output = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            path.to_string_lossy().as_ref(),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?.trim();
    f64::from_str(line).ok()
}

fn find_latest_in_dir(dir: &Path) -> Option<PathBuf> {
    let mut entries: Vec<(std::time::SystemTime, PathBuf)> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let m = meta.modified().ok()?;
            Some((m, e.path()))
        })
        .collect();

    entries.sort_by(|a, b| b.0.cmp(&a.0));
    entries.into_iter().map(|(_, p)| p).find(|p| {
        if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
            matches!(ext, "mp3" | "m4a" | "opus" | "webm" | "m4b" | "ogg" | "flac")
        } else {
            false
        }
    })
}
