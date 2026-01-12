#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use hex;
use num_cpus;
use rayon::iter::IntoParallelRefIterator;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::path::BaseDirectory;
use tauri::{async_runtime, Emitter};
use tauri::Manager;
use walkdir::WalkDir;

#[derive(Serialize)]
struct QueueStats {
    active: u32,
    pending: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct DownloadResult {
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<f64>,
    bitrate: Option<u32>,
    source: Option<String>,
    cover_url: Option<String>,
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
    cover_url: Option<String>,
    new_bitrate: Option<u32>,
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
    cache_max_entries: usize,
    #[serde(default)]
    core_api_url: Option<String>,
    #[serde(default)]
    core_api_user: Option<String>,
    #[serde(default)]
    core_api_password: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            min_bitrate: 256,
            analysis_window_seconds: 100,
            rayon_threads: 0,
            cache_enabled: true,
            cache_max_entries: 10_000,
            core_api_url: Some("http://localhost:3001".to_string()),
            core_api_user: None,
            core_api_password: None,
        }
    }
}

/// Metadata extracted from an audio file using ffprobe
/// Used to send to Core server for track matching instead of file paths
#[derive(Serialize, Clone, Debug, Default)]
struct ExtractedMetadata {
    artist: Option<String>,
    title: Option<String>,
    album: Option<String>,
    duration: Option<f64>,
    isrc: Option<String>,
}

/// Extract metadata from an audio file using ffprobe
/// Returns artist, title, album, duration, and ISRC if available
fn extract_metadata_from_file(path: &Path) -> ExtractedMetadata {
    let output = Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            path.to_str().unwrap_or_default(),
        ])
        .output();

    let mut metadata = ExtractedMetadata::default();

    if let Ok(output) = output {
        if output.status.success() {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                let format = &json["format"];
                let tags = &format["tags"];
                
                // Extract duration
                if let Some(dur_str) = format["duration"].as_str() {
                    metadata.duration = dur_str.parse().ok();
                }
                
                // Try various tag formats (case-insensitive matching would need more work)
                // ffprobe may return tags in various cases depending on the file format
                metadata.artist = tags["artist"].as_str()
                    .or_else(|| tags["ARTIST"].as_str())
                    .or_else(|| tags["albumartist"].as_str())
                    .or_else(|| tags["ALBUMARTIST"].as_str())
                    .map(|s| s.to_string());
                
                metadata.title = tags["title"].as_str()
                    .or_else(|| tags["TITLE"].as_str())
                    .map(|s| s.to_string());
                
                metadata.album = tags["album"].as_str()
                    .or_else(|| tags["ALBUM"].as_str())
                    .map(|s| s.to_string());
                
                // ISRC (International Standard Recording Code)
                metadata.isrc = tags["isrc"].as_str()
                    .or_else(|| tags["ISRC"].as_str())
                    .or_else(|| tags["TSRC"].as_str()) // ID3v2 tag
                    .map(|s| s.to_string());
                
                println!("[GUI] Extracted metadata: artist={:?}, title={:?}, duration={:?}, isrc={:?}", 
                    metadata.artist, metadata.title, metadata.duration, metadata.isrc);
            }
        }
    }

    metadata
}

#[tauri::command]
fn queue_stats() -> QueueStats {
    QueueStats {
        active: 0,
        pending: 0,
    }
}

#[tauri::command]
async fn download_link(
    url: String,
    output_dir: Option<String>,
    app: tauri::AppHandle,
) -> Result<DownloadResult, String> {
    use tauri::Manager;
    let out_dir = match output_dir.filter(|s| !s.is_empty()) {
        Some(d) => d,
        None => app.path().download_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| String::from("./")),
    };
    let settings = load_settings_path(&app);

    // eprintln!("[GUI] Download request: url={}, dir={}", url, out_dir);

    // 1. Perform Download (API or Local)
    let download_task_result = if let Some(api_url) = &settings.core_api_url {
        if !api_url.trim().is_empty() {
            // eprintln!("[GUI] Using Remote API: {}", api_url);
            let url_clone = url.clone();
            let out_dir_clone = out_dir.clone();
            let settings_clone = settings.clone();
            async_runtime::spawn_blocking(move || {
                download_via_api(&url_clone, &out_dir_clone, &settings_clone)
            })
            .await
            .map_err(|e| format!("Task failed: {e}"))?
        } else {
            download_local(&url, &out_dir)
        }
    } else {
        download_local(&url, &out_dir)
    };

    let mut res = download_task_result?;

    // 2. Analyze with WMB (Local Verification)
    let handle = app.clone();
    let settings_analysis = settings.clone();
    
    // We run analysis in blocking thread
    let analysis_result = async_runtime::spawn_blocking(move || -> Result<DownloadResult, String> {
        let vendor = vendor_dir(&handle).map_err(|e| e.to_string())?;
        let cache_path = cache_path(&handle)?;
        let cache = Arc::new(Mutex::new(load_cache(
            &cache_path,
            settings_analysis.cache_max_entries,
        )));

        let path = Path::new(&res.saved_to);
        if path.exists() {
            let analysis = analyze_with_wmb_single(
                path,
                &vendor,
                settings_analysis.min_bitrate,
                settings_analysis.analysis_window_seconds,
                settings_analysis.cache_enabled,
                &cache,
            );

            if let Ok((est, _lossless, note, _status)) = analysis {
                if let Some(bitrate) = est {
                    res.bitrate = Some(bitrate);
                    res.quality = format!("{} kbps", bitrate);
                }
                if let Some(n) = note {
                    if !res.warning.is_empty() {
                        res.warning.push_str(" | ");
                    }
                    res.warning.push_str(&n);
                }
            }
            
            // Persist cache
            if let Ok(guard) = cache.lock() {
                let _ = save_cache(&cache_path, &*guard);
            }
        }
         Ok(res)
    }).await.map_err(|e| e.to_string())?;

    Ok(analysis_result?)
}

fn download_local(url: &str, out_dir: &str) -> Result<DownloadResult, String> {
    // eprintln!("[GUI] Using Local Execution");
    let script = r#"
      const fs = require('fs');
      const path = require('path');
      const core = require('../keson-spectral-improver-core');
      const url = process.argv[2];
      const destDir = process.argv[3];
      
      (async () => {
        const res = await core.downloadTrack(url);
        const src = res.path;
        if (!src) throw new Error('No file path returned by core');
        
        const filename = res.filename || path.basename(src);
        const targetDir = destDir || '.';
        fs.mkdirSync(targetDir, { recursive: true });
        const dest = path.join(targetDir, filename);
        
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
        .arg(url)
        .arg(out_dir)
        .current_dir("../")
        .output()
        .map_err(|e| format!("Node exec failed: {e}"))?;

    // eprintln!("[GUI] Node stdout: {}", String::from_utf8_lossy(&output.stdout));
    // eprintln!("[GUI] Node stderr: {}", String::from_utf8_lossy(&output.stderr));

    if !output.status.success() {
        return Err(format!(
            "Téléchargement échoué: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| {
        format!(
            "Réponse core invalide: {e}: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })?;

    Ok(DownloadResult {
        title: parsed.get("title").and_then(|v| v.as_str()).unwrap_or("Track").to_string(),
        artist: None,
        album: None,
        duration: None,
        bitrate: parsed.get("quality").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()), // simple parse attempt
        source: None,
        cover_url: None,
        caption: url.to_string(),
        quality: parsed.get("quality").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        warning: String::new(),
        saved_to: parsed.get("saved_to").and_then(|v| v.as_str()).unwrap_or(out_dir).to_string(),
    })
}

// Separate function to keep compatible signature with main.rs flow


fn download_via_api(
    url: &str,
    output_dir: &str,
    settings: &Settings,
) -> Result<DownloadResult, String> {
    let api_url = settings.core_api_url.as_ref().unwrap().trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Client build failed: {e}"))?;
    
    // prepare auth
    let mut req_builder = client.post(format!("{}/download-any", api_url));
    
    if let (Some(u), Some(p)) = (&settings.core_api_user, &settings.core_api_password) {
        if !u.is_empty() {
             req_builder = req_builder.basic_auth(u, Some(p));
        }
    }

    let res = req_builder
        .json(&serde_json::json!({ "url": url }))
        .send()
        .map_err(|e| format!("API request failed: {e}"))?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().unwrap_or_default();
        return Err(format!("API Error ({}): {}", status, text));
    }

    let body: serde_json::Value = res.json().map_err(|e| format!("Invalid JSON: {e}"))?;
    
    if let Some(err) = body.get("error").and_then(|s| s.as_str()) {
         return Err(format!("API returned error: {}", err));
    }

    let download_url = body.get("downloadUrl").and_then(|s| s.as_str())
        .ok_or("No downloadUrl in response")?;
    let filename = body.get("filename").and_then(|s| s.as_str())
        .ok_or("No filename in response")?;

    // Construct full download URL (handling relative paths)
    let full_dl_url = if download_url.starts_with("http") {
        download_url.to_string()
    } else {
        format!("{}{}", api_url, download_url)
    };

    // Download the file
    let mut dl_req = client.get(&full_dl_url);
    if let (Some(u), Some(p)) = (&settings.core_api_user, &settings.core_api_password) {
        if !u.is_empty() {
             dl_req = dl_req.basic_auth(u, Some(p));
        }
    }
    let mut dl_res = dl_req.send().map_err(|e| format!("Download failed: {e}"))?;
    
    if !dl_res.status().is_success() {
         return Err(format!("File download failed: {}", dl_res.status()));
    }

    fs::create_dir_all(output_dir).map_err(|e| format!("Create dir failed: {e}"))?;
    let dest_path = Path::new(output_dir).join(filename);
    let mut file = fs::File::create(&dest_path).map_err(|e| format!("Create file failed: {e}"))?;
    dl_res.copy_to(&mut file).map_err(|e| format!("Save file failed: {e}"))?;

    // Extract metadata for result
    let metadata = body.get("metadata");
    
    let quality = body.get("quality")
        .and_then(|q| q.get("estimated_bitrate"))
        .map(|v| v.to_string())
        .or_else(|| body.get("quality").and_then(|q| q.get("estimated_bitrate_numeric")).map(|v| v.to_string()))
        .unwrap_or_else(|| "Unknown".to_string());

    let title = metadata
        .and_then(|m| m.get("title"))
        .and_then(|s| s.as_str())
        .unwrap_or(filename)
        .to_string();

    let artist = metadata
        .and_then(|m| m.get("artist"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    let album = metadata
        .and_then(|m| m.get("album"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    let duration = metadata
        .and_then(|m| m.get("duration"))
        .and_then(|v| v.as_f64());

    let bitrate = metadata
        .and_then(|m| m.get("bitrate"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let source = metadata
        .and_then(|m| m.get("source"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    let cover_url = metadata
        .and_then(|m| m.get("thumbnail"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());
    
    
    // eprintln!("[GUI] Extracted Metadata - Artist: {:?}, Cover: {:?}", artist, cover_url);


    Ok(DownloadResult {
        title,
        artist,
        album,
        duration,
        bitrate,
        source,
        cover_url,
        caption: url.to_string(),
        quality,
        warning: String::new(),
        saved_to: dest_path.to_string_lossy().to_string(),
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
        let _ = handle.emit("scan_progress", 1u32); // start

        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            // Skip backup-ksi folder
            if entry.file_type().is_dir() && entry.file_name() == "backup-ksi" {
                continue;
            }
            if entry.file_type().is_file() {
                // Skip files inside backup-ksi folders
                if entry.path().components().any(|c| c.as_os_str() == "backup-ksi") {
                    continue;
                }
                discovered += 1;
                if is_audio(entry.path()) {
                    audio_entries.push(entry);
                }
                let pct = 1 + ((discovered as f64).sqrt() as u32 % 12); // gentle movement up to ~13%
                if pct != tick {
                    tick = pct;
                    let _ = handle.emit("scan_progress", pct.min(15));
                }
            }
        }

        if audio_entries.is_empty() {
            let _ = handle.emit("scan_progress", 100u32);
            return Ok(Vec::new());
        }

        let vendor = vendor_dir(&handle)?;
        let cache_path = cache_path(&handle)?;
        let cache = Arc::new(Mutex::new(load_cache(
            &cache_path,
            settings.cache_max_entries,
        )));
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
                let _ = handle.emit("scan_progress", percent.round() as u32);

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
async fn open_spectrum(path: String, app: tauri::AppHandle) -> Result<Vec<u8>, String> {
    let src = Path::new(&path);
    if !src.exists() {
        return Err("Fichier introuvable".into());
    }

    let vendor = vendor_dir(&app)?;

    // Ensure python3 is available
    let python = "python3";
    let py_check = Command::new(python)
        .arg("--version")
        .output()
        .map_err(|e| format!("python3 introuvable: {e}"))?;
    if !py_check.status.success() {
        return Err("python3 introuvable (ajoute-le au PATH)".into());
    }

    let temp_root = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    if !temp_root.exists() {
        std::fs::create_dir_all(&temp_root).map_err(|e| e.to_string())?;
    }
    let temp_root_str = temp_root.to_string_lossy();

    let script = format!(
        r#"
import sys, tempfile, os
sys.path.insert(0, r"{vendor}")
from wmb_core import AudioFile
path = sys.argv[1]
base_tmp = sys.argv[2]
if not os.path.exists(base_tmp):
    try: os.makedirs(base_tmp)
    except: pass
tmp = tempfile.mkdtemp(prefix="wmb-", dir=base_tmp)
af = AudioFile(path)
af.analyze(generate_spectrogram_flag=True, assets_dir=tmp)
print(af.spectrogram_path or "")
"#,
        vendor = vendor.display()
    );

    let output = Command::new(python)
        .args(["-c", &script, src.to_str().unwrap_or_default(), &temp_root_str])
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
    // eprintln!("[GUI] Generated spectrum at: {}", spectro);
    
    // Read the file into bytes to bypass asset protocol/scope issues
    let bytes = std::fs::read(&spectro).map_err(|e| format!("Failed to read generated spectrum: {e}"))?;
    
    // Optional: cleanup temp file immediately? 
    // std::fs::remove_file(&spectro).ok(); 
    // We leave it in cache for now or let OS clean up if it were temp.
    
    Ok(bytes)
}

fn is_audio(path: &Path) -> bool {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
    {
        Some(ext) => matches!(
            ext.as_str(),
            "mp3" | "m4a" | "aac" | "wav" | "flac" | "ogg" | "opus" | "webm"
        ),
        None => false,
    }
}

fn vendor_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .resolve("../vendor/whatsmybitrate", BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    
    // We already resolved fully, so these candidates logic might be redundant if resolve works as expected from bundle.
    // However, if running in dev, resource resolution might differ. 
    // Let's rely on standard resolution first, or keep fallback if resolve fails? 
    // resolve_resource in v1 returned Option<PathBuf>. v2 returns Result<PathBuf>.
    // Let's assume resolve works.
    Ok(base)
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
        .path()
        .app_data_dir()
        .or_else(|_| app.path().app_cache_dir())
        .map_err(|e| e.to_string())?;

    let path = base.join("analysis-cache.json");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    Ok(path)
}

fn load_cache(path: &Path, limit: usize) -> HashMap<String, CacheEntry> {
    if let Ok(text) = fs::read_to_string(path) {
        let mut map: HashMap<String, CacheEntry> = serde_json::from_str(&text).unwrap_or_default();
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
    let hash = if cache_enabled {
        file_hash(path).ok()
    } else {
        None
    };
    if cache_enabled {
        if let Some(h) = &hash {
            if let Ok(guard) = cache.lock() {
                if let Some(entry) = guard.get(h) {
                    let status = match entry.bitrate {
                        Some(b) if b < min => "bad".to_string(),
                        Some(_) => "ok".to_string(),
                        None => "error".to_string(),
                    };
                    return Ok((entry.bitrate, entry.is_lossless, entry.note.clone(), status));
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
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            queue_stats,
            download_link,
            scan_folder,
            reveal_in_folder,
            open_spectrum,
            get_settings,
            save_settings,
            redownload_bad,
            download_with_url,
            accept_redownload,
            discard_file,
            revert_replacement,
            extract_cover
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

    let _ = ThreadPoolBuilder::new().num_threads(threads).build_global();
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
    let _ = ThreadPoolBuilder::new().num_threads(count).build_global();
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
    fs::write(
        &path,
        serde_json::to_string_pretty(&settings).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn redownload_bad(paths: Vec<String>, source: String, backup: bool, app: tauri::AppHandle) -> Result<Vec<RedownloadResult>, String> {
    let settings = load_settings_path(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // 5 minutes for Tidal downloads
            .build()
            .map_err(|e| format!("Client build failed: {e}"))?;
        
        // Use configured Core API URL from settings
        let api_base = settings.core_api_url
            .as_ref()
            .map(|s| s.trim_end_matches('/').to_string())
            .unwrap_or_else(|| "http://localhost:3001".to_string());
        
        println!("[GUI] Using Core API: {}", api_base);
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

            println!("[GUI] Redownload Query for: '{}' (source: {}, backup: {})", stem, source, backup);

            // Extract metadata from the local file using ffprobe
            let file_metadata = extract_metadata_from_file(&path);

            // Clean the query by removing bracketed noise patterns like [ ... ] or ( ... )
            let clean_query = stem
                .split(" - ")
                .take(2) // Keep only "Artist - Title", drop anything after
                .collect::<Vec<_>>()
                .join(" - ");
            let clean_query = if clean_query.is_empty() { stem.to_string() } else { clean_query };
            
            println!("[GUI] Search query (cleaned): '{}'", clean_query);

            // 1. Search using POST with extracted metadata (no file path sent to Core)
            let search_payload = serde_json::json!({
                "query": clean_query,
                "metadata": {
                    "artist": file_metadata.artist,
                    "title": file_metadata.title,
                    "album": file_metadata.album,
                    "duration": file_metadata.duration,
                    "isrc": file_metadata.isrc
                }
            });
            
            let mut download_target: Option<String> = None;
            let mut cover_url: Option<String> = None;
            let mut source_type = "unknown";

            match client.post(format!("{}/search/track", api_base))
                .json(&search_payload)
                .send() {
                Ok(resp) => {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        if json["success"].as_bool().unwrap_or(false) && json["found"].as_bool().unwrap_or(false) {
                            if let Some(url) = json["url"].as_str() {
                                let detected_source = json["source"].as_str().unwrap_or("tidal");
                                let score = json["score"].as_f64().unwrap_or(0.0);
                                println!("[GUI] Found on {}: {} (score: {})", detected_source, url, score);
                                download_target = Some(url.to_string());
                                cover_url = json["cover_url"].as_str().map(|s| s.to_string());
                                println!("[GUI] Found on {}: {} (score: {}, cover: {:?})", detected_source, url, score, cover_url);
                                source_type = if detected_source == "soundcloud" { "soundcloud" } else { "tidal" };
                            }
                        } else {
                            println!("[GUI] No confident match found for: {}", stem);
                            // TODO: Emit event to GUI for manual URL input
                        }
                    }
                }
                Err(e) => eprintln!("[GUI] Search request failed: {}", e),
            }

            // Skip if no match found (manual URL input will be handled by GUI)
            let download_url = match download_target {
                Some(url) => url,
                None => {
                    eprintln!("[GUI] Skipping '{}' - no automatic match", stem);
                    continue;
                }
            };

            // 2. Request Download
            let payload = serde_json::json!({
                "url": download_url,
                "source": source_type
            });

            match client.post(format!("{}/download", api_base))
                .json(&payload)
                .send() {
                    Ok(resp) => {
                        if !resp.status().is_success() {
                            let err_text = resp.text().unwrap_or_default();
                            eprintln!("[GUI] Download request failed: {}", err_text);
                            continue;
                        }

                        if let Ok(json) = resp.json::<serde_json::Value>() {
                            if let Some(rel_url) = json["downloadUrl"].as_str() {
                                // Update cover_url from metadata if not already set
                                if cover_url.is_none() {
                                    cover_url = json["metadata"]["thumbnail"]
                                        .as_str()
                                        .map(|s| s.to_string())
                                        .or_else(|| json["metadata"]["cover_url"].as_str().map(|s| s.to_string()));
                                    if let Some(ref c) = cover_url {
                                         println!("[GUI] Retrieved cover from download metadata: {}", c);
                                    }
                                }

                                // 3. Download File Content from Core
                                let file_url = format!("{}{}", api_base, rel_url);
                                let final_filename = json["filename"].as_str().unwrap_or("downloaded.mp3");
                                let dest_path = parent.join(final_filename);

                                match client.get(&file_url).send() {
                                     Ok(mut file_resp) => {
                                         if let Ok(mut file) = fs::File::create(&dest_path) {
                                             if let Err(e) = file_resp.copy_to(&mut file) {
                                                 eprintln!("[GUI] Failed to write file: {}", e);
                                             } else {
                                                 println!("[GUI] Downloaded to: {:?}", dest_path);
                                                 
                                                 let original_dur = probe_duration(&path);
                                                 let new_dur = probe_duration(&dest_path);

                                                 // Check if durations match (within tolerance)
                                                 let tolerance_sec = 2.0;
                                                 let tolerance_pct = 0.05;
                                                 let diff = (original_dur.unwrap_or(0.0) - new_dur.unwrap_or(0.0)).abs();
                                                 let rel = if original_dur.unwrap_or(0.0) > 0.0 {
                                                     diff / original_dur.unwrap_or(1.0)
                                                 } else {
                                                     1.0
                                                 };
                                                 let is_match = diff <= tolerance_sec || rel <= tolerance_pct;

                                                 // If durations match, auto-replace the original
                                                 if is_match && dest_path != path {
                                                     // Backup original if requested (only after successful download)
                                                     if backup && path.exists() {
                                                         let backup_dir = parent.join("backup-ksi");
                                                         if !backup_dir.exists() {
                                                             let _ = fs::create_dir_all(&backup_dir);
                                                         }
                                                         let backup_path = backup_dir.join(path.file_name().unwrap_or_default());
                                                         if let Err(e) = fs::copy(&path, &backup_path) {
                                                             eprintln!("[GUI] Failed to backup file: {}", e);
                                                         } else {
                                                             println!("[GUI] Backed up to: {:?}", backup_path);
                                                         }
                                                     }
                                                     // Delete original
                                                     if let Err(e) = fs::remove_file(&path) {
                                                         eprintln!("[GUI] Failed to delete original: {}", e);
                                                     } else {
                                                         println!("[GUI] Auto-replaced original file (durations matched)");
                                                     }
                                                 }

                                                 // Probe bitrate of the new file
                                                 let new_bitrate = probe_bitrate(&dest_path);

                                                 downloaded.push(RedownloadResult {
                                                     original_path: path_str.clone(),
                                                     new_path: dest_path.to_string_lossy().to_string(),
                                                     original_duration: original_dur, 
                                                     new_duration: new_dur,
                                                     cover_url: cover_url.clone(),
                                                     new_bitrate,
                                                 });
                                             }
                                         }
                                    },
                                    Err(e) => eprintln!("[GUI] Failed to download file content: {}", e),
                                }
                            }
                        }
                    },
                    Err(e) => eprintln!("[GUI] Download API call failed: {}", e),
            }
        }
        
        Ok(downloaded)
    }).await.map_err(|e| e.to_string())?
}

/// Download a file using a direct URL (for manual fallback when auto-search fails)
#[tauri::command]
async fn download_with_url(original_path: String, url: String, backup: bool, app: tauri::AppHandle) -> Result<RedownloadResult, String> {
    let settings = load_settings_path(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("Client build failed: {e}"))?;
        
        // Use configured Core API URL from settings
        let api_base = settings.core_api_url
            .as_ref()
            .map(|s| s.trim_end_matches('/').to_string())
            .unwrap_or_else(|| "http://localhost:3001".to_string());
        
        println!("[GUI] Using Core API: {}", api_base);
        
        let path = PathBuf::from(&original_path);
        let parent = path
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| "Chemin sans dossier".to_string())?;
        
        let source_type = if url.contains("tidal.com") { "tidal" } else { "soundcloud" };
        
        println!("[GUI] Manual download from {} for: {}", source_type, original_path);
        
        // Request Download
        let payload = serde_json::json!({
            "url": url,
            "source": source_type
        });

        let resp = client.post(format!("{}/download", api_base))
            .json(&payload)
            .send()
            .map_err(|e| format!("Download request failed: {}", e))?;
            
        if !resp.status().is_success() {
            let err_text = resp.text().unwrap_or_default();
            return Err(format!("Download failed: {}", err_text));
        }

        let json: serde_json::Value = resp.json()
            .map_err(|e| format!("JSON parse failed: {}", e))?;
            
        let rel_url = json["downloadUrl"].as_str()
            .ok_or_else(|| "No downloadUrl in response".to_string())?;
        let final_filename = json["filename"].as_str().unwrap_or("downloaded.mp3");
        let dest_path = parent.join(final_filename);

        // Download file content
        let file_url = format!("{}{}", api_base, rel_url);
        let mut file_resp = client.get(&file_url).send()
            .map_err(|e| format!("Failed to fetch file: {}", e))?;
            
        let mut file = fs::File::create(&dest_path)
            .map_err(|e| format!("Failed to create file: {}", e))?;
            
        file_resp.copy_to(&mut file)
            .map_err(|e| format!("Failed to write file: {}", e))?;

        println!("[GUI] Downloaded to: {:?}", dest_path);
        
        println!("[GUI] Probing original duration for: {:?}", path);
        let original_dur = probe_duration(&path).unwrap_or(0.0);
        println!("[GUI] Original duration: {}", original_dur);

        println!("[GUI] Probing new duration for: {:?}", dest_path);
        let new_dur = probe_duration(&dest_path).unwrap_or(0.0);
        println!("[GUI] New duration: {}", new_dur);

        // Auto-replace logic if backup enabled
        if backup {
             let backup_dir = parent.join("backup-ksi");
             if !backup_dir.exists() {
                  let _ = fs::create_dir_all(&backup_dir);
             }
             let backup_path = backup_dir.join(path.file_name().unwrap_or_default());
             // Copy original to backup
             if let Err(e) = fs::copy(&path, &backup_path) {
                  eprintln!("[GUI] Failed to backup file: {}", e);
             } else {
                  println!("[GUI] Backed up to: {:?}", backup_path);
             }
             
             // Replace original
             if let Err(e) = fs::remove_file(&path) {
                 eprintln!("[GUI] Failed to delete original: {}", e);
             } else if let Err(e) = fs::rename(&dest_path, &path) {
                 eprintln!("[GUI] Failed to move new file to original: {}", e);
             } else {
                 println!("[GUI] Replaced original file");
                 // Ghostbuster: Ensure source is gone
                 if dest_path.exists() && dest_path != path {
                     println!("[GUI] Source file persisted after rename. Force deleting: {:?}", dest_path);
                     let _ = fs::remove_file(&dest_path);
                 }
             }
        }

        let new_file_path = if backup { &path } else { &dest_path };
        let new_bitrate = probe_bitrate(new_file_path);

        Ok(RedownloadResult {
            original_path: original_path,
            new_path: new_file_path.to_string_lossy().to_string(),
            original_duration: Some(original_dur),
            new_duration: Some(new_dur),
            cover_url: json["metadata"]["thumbnail"].as_str().map(|s| s.to_string().replace("url(\"", "").replace("\")", "")),
            new_bitrate,
        })
    }).await.map_err(|e| e.to_string())?
}

/// Revert a replacement by restoring the backup file
#[tauri::command]
async fn revert_replacement(original_path: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(&original_path);
        let parent = path.parent().ok_or("Invalid path")?;
        let filename = path.file_name().ok_or("Invalid filename")?;
        let backup_path = parent.join("backup-ksi").join(filename);

        println!("[GUI] Attempting to revert: {:?} from {:?}", path, backup_path);

        if !backup_path.exists() {
            return Err("Backup file not found".to_string());
        }

        // Delete current file (the new one)
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("Failed to remove current file: {}", e))?;
        }

        // Restore backup
        fs::rename(&backup_path, &path).map_err(|e| format!("Failed to restore backup: {}", e))?;
        
        // Aggressive Cleanup: Remove potential ghost files with different extensions (e.g. .m4a)
        if let Some(_) = path.file_stem() {
             let ghosts = ["m4a", "flac", "wav", "mp3", "aac", "ogg"];
             for ext in ghosts {
                  let ghost_path = path.with_extension(ext);
                  // Don't delete the path we just restored!
                  if ghost_path == path { continue; }
                  
                  if ghost_path.exists() {
                       println!("[GUI] Revert cleanup: Removing ghost file {:?}", ghost_path);
                       let _ = fs::remove_file(ghost_path);
                  }
             }
        }

        println!("[GUI] Reverted successfully");
        Ok(true)
    }).await.map_err(|e| e.to_string())?
}



fn settings_path(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .or_else(|_| app.path().app_cache_dir())
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
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
        let backup_ext = if ext.is_empty() {
            "bak".to_string()
        } else {
            format!("bak.{}", ext)
        };
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

/// Extract embedded cover art from an audio file using ffmpeg
/// Returns the path to the extracted image file, or None if no cover found
#[tauri::command]
fn extract_cover(audio_path: String) -> Result<Option<String>, String> {
    let path = PathBuf::from(&audio_path);
    if !path.exists() {
        return Err(format!("File not found: {}", audio_path));
    }

    // Create temp output path
    let temp_dir = std::env::temp_dir();
    let hash = format!("{:x}", md5::compute(&audio_path));
    let output_path = temp_dir.join(format!("keson-cover-{}.jpg", hash));

    // If already extracted, return cached
    if output_path.exists() {
        return Ok(Some(output_path.to_string_lossy().to_string()));
    }

    // Run ffmpeg to extract cover
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-i", &audio_path,
            "-an",
            "-vcodec", "copy",
            output_path.to_string_lossy().as_ref(),
        ])
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if !status.status.success() {
        // No cover or error
        return Ok(None);
    }

    if output_path.exists() {
        Ok(Some(output_path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

fn probe_duration(path: &Path) -> Option<f64> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
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

fn probe_bitrate(path: &Path) -> Option<u32> {
    // Use whatsmybitrate for real spectral analysis
    // Find vendor directory relative to executable
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    
    // Try several possible locations for vendor
    let vendor_candidates = [
        exe_dir.join("../vendor/whatsmybitrate"),
        exe_dir.join("vendor/whatsmybitrate"),
        PathBuf::from("vendor/whatsmybitrate"),
        PathBuf::from("../vendor/whatsmybitrate"),
    ];
    
    let vendor_dir = vendor_candidates.iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("vendor/whatsmybitrate"));

    let script = format!(
        r#"
import sys, json
sys.path.insert(0, r"{vendor}")
from wmb_core import AudioFile
import wmb_core
wmb_core.MAX_LOAD_SECONDS = 30
af = AudioFile(sys.argv[1])
af.analyze(generate_spectrogram_flag=False, assets_dir=None)
print(json.dumps({{"bitrate": af.to_dict().get("estimated_bitrate_numeric")}}))
"#,
        vendor = vendor_dir.display()
    );

    let output = Command::new("python3")
        .args(["-c", &script, path.to_str()?])
        .output()
        .ok()?;

    if !output.status.success() {
        eprintln!("[probe_bitrate] whatsmybitrate failed: {}", String::from_utf8_lossy(&output.stderr));
        return None;
    }

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    parsed.get("bitrate")
        .and_then(|v| v.as_f64())
        .map(|v| v.round() as u32)
}

#[allow(dead_code)]
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
            matches!(
                ext,
                "mp3" | "m4a" | "opus" | "webm" | "m4b" | "ogg" | "flac"
            )
        } else {
            false
        }
    })
}
