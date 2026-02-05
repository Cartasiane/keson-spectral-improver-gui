#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod cache;
mod settings;
mod tagging;
mod types;

use num_cpus;
use rayon::iter::IntoParallelRefIterator;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{async_runtime, Emitter, Manager};
use walkdir::WalkDir;

use audio::{analyze_with_wmb_single, analyze_file_quality, extract_metadata_from_file, is_audio, probe_bitrate, probe_duration};
use cache::{cache_path, load_cache, save_cache};
pub use settings::{get_settings, load_settings, save_settings};
use types::{DownloadResult, QueueStats, RedownloadResult, ScanResult, SearchResult};

/// Core API URL - always uses production server
const CORE_API_URL: &str = "https://keson.api.acab.love";

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
    let out_dir = match output_dir.filter(|s| !s.is_empty()) {
        Some(d) => d,
        None => app.path().download_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| String::from("./")),
    };
    let settings = load_settings(&app);
    
    // Check if client is registered
    let client_token = settings.client_token.as_ref()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "Non enregistré. Veuillez entrer votre code d'invitation.".to_string())?;

    let url_clone = url.clone();
    let out_dir_clone = out_dir.clone();
    let token_clone = client_token.clone();
    let app_handle = app.clone(); // Clone app for thread
    
    let download_task_result = async_runtime::spawn_blocking(move || {
        download_via_api(&url_clone, &out_dir_clone, &token_clone, &app_handle)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?;

    let mut res = download_task_result?;

    let handle = app.clone();
    let settings_analysis = settings.clone();
    
    let analysis_result = async_runtime::spawn_blocking(move || -> Result<DownloadResult, String> {
        let cache_path = cache_path(&handle)?;
        let cache = Arc::new(Mutex::new(load_cache(
            &cache_path,
            settings_analysis.cache_max_entries,
        )));

        let path = Path::new(&res.saved_to);
        if path.exists() {
            // Skip analysis for FLAC files - they are always lossless
            let ext = path.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());
            
            if ext.as_deref() == Some("flac") {
                res.quality = "Lossless".to_string();
                res.bitrate = None; // Don't show bitrate for lossless
            } else {
                // Analyze lossy formats (m4a, mp3, etc.)
                let analysis = analyze_with_wmb_single(
                    path,
                    &handle, // Pass AppHandle
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
            }
            
            if let Ok(guard) = cache.lock() {
                let _ = save_cache(&cache_path, &*guard);
            }
        }
         Ok(res)
    }).await.map_err(|e| e.to_string())?;

    Ok(analysis_result?)
}



fn download_via_api(
    url: &str,
    output_dir: &str,
    client_token: &str,
    app: &tauri::AppHandle,
) -> Result<DownloadResult, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Client build failed: {e}"))?;
    
    let res = client
        .post(format!("{}/download-any", CORE_API_URL))
        .header("X-Client-Token", client_token)
        .json(&serde_json::json!({ "url": url }))
        .send()
        .map_err(|e| format!("API request failed: {e}"))?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().unwrap_or_default();
        
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("AUTH_REQUIRED: Session expirée ou invalide. Veuillez vous réenregistrer.".to_string());
        }
        
        if status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if json.get("code").and_then(|c| c.as_str()) == Some("QUEUE_FULL") {
                    return Err("QUEUE_FULL: Le serveur est saturé, réessayez dans un instant.".to_string());
                }
            }
        }
        
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

    let full_dl_url = if download_url.starts_with("http") {
        download_url.to_string()
    } else {
        format!("{}{}", CORE_API_URL, download_url)
    };

    let mut dl_res = client
        .get(&full_dl_url)
        .header("X-Client-Token", client_token)
        .send()
        .map_err(|e| format!("Download failed: {e}"))?;
    
    if !dl_res.status().is_success() {
         return Err(format!("File download failed: {}", dl_res.status()));
    }

    fs::create_dir_all(output_dir).map_err(|e| format!("Create dir failed: {e}"))?;
    let dest_path = Path::new(output_dir).join(filename);
    let mut file = fs::File::create(&dest_path).map_err(|e| format!("Create file failed: {e}"))?;
    dl_res.copy_to(&mut file).map_err(|e| format!("Save file failed: {e}"))?;

    let metadata = body.get("metadata");
    
    // Analyze quality locally using whatsmybitrate (same as Quality tab)
    let quality_result = analyze_file_quality(&dest_path, app);
    let (quality, analyzed_bitrate) = match quality_result {
        Ok(result) => (result.quality_string, result.bitrate),
        Err(e) => {
            log::info!("[download] Quality analysis failed: {}", e);
            ("Unknown".to_string(), None)
        }
    };

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

    let bitrate = analyzed_bitrate.or_else(|| {
        metadata
            .and_then(|m| m.get("bitrate"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
    });

    let source = metadata
        .and_then(|m| m.get("source"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    // Try to extract embedded cover from the downloaded file
    let cover_url = extract_embedded_cover(&dest_path.to_string_lossy(), app)
        .ok()
        .flatten()
        .or_else(|| {
            // Fallback to metadata thumbnail if embedded extraction fails
            metadata
                .and_then(|m| m.get("thumbnail"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_string())
        });

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

/// Extract embedded cover from audio file using Lofty (native Rust, no ffmpeg)
fn extract_embedded_cover(audio_path: &str, _app: &tauri::AppHandle) -> Result<Option<String>, String> {
    use lofty::prelude::*;
    use lofty::probe::Probe;
    use std::fs::File;
    use std::io::Write;

    let path = PathBuf::from(audio_path);
    if !path.exists() {
        return Err(format!("File not found: {}", audio_path));
    }

    // Check if we already have a cached version
    let temp_dir = std::env::temp_dir();
    let hash = format!("{:x}", md5::compute(audio_path));
    
    // We don't know the extension yet, so we check specifically for our known formats
    let jpg_path = temp_dir.join(format!("keson-cover-{}.jpg", hash));
    let png_path = temp_dir.join(format!("keson-cover-{}.png", hash));

    if jpg_path.exists() {
        return Ok(Some(jpg_path.to_string_lossy().to_string()));
    }
    if png_path.exists() {
        return Ok(Some(png_path.to_string_lossy().to_string()));
    }

    // Read the file with Lofty
    log::info!("[cover] Probing file: {}", audio_path);
    let tagged_file = match Probe::open(&path) {
        Ok(probe) => match probe.read() {
            Ok(tf) => tf,
            Err(e) => {
                log::error!("[cover] Failed to read file: {}", e);
                return Err(format!("Failed to read file: {}", e));
            }
        },
        Err(e) => {
            log::error!("[cover] Failed to open file: {}", e);
            return Err(format!("Failed to open file: {}", e));
        }
    };

    // Find the first picture
    let tag = match tagged_file.primary_tag() {
        Some(t) => t,
        None => {
            log::warn!("[cover] No primary tag found, trying first_tag");
            match tagged_file.first_tag() {
                Some(t) => t,
                None => {
                    log::warn!("[cover] No tags found at all");
                    return Ok(None);
                }
            }
        },
    };

    let pictures = tag.pictures();
    log::info!("[cover] Found {} pictures", pictures.len());

    let picture = match pictures.first() {
        Some(p) => p,
        None => {
            log::warn!("[cover] No pictures in tag");
            return Ok(None);
        }
    };

    // Determine extension based on mime type
    log::info!("[cover] Mime type: {:?}", picture.mime_type());
    let extension = match picture.mime_type() {
        Some(lofty::picture::MimeType::Png) => "png",
        Some(lofty::picture::MimeType::Jpeg) => "jpg",
        Some(lofty::picture::MimeType::Tiff) => "tiff",
        Some(lofty::picture::MimeType::Bmp) => "bmp",
        Some(lofty::picture::MimeType::Gif) => "gif",
        _ => {
            log::warn!("[cover] Unknown or unhandled mime type, defaulting to jpg");
            "jpg"
        }, 
    };

    let output_path = temp_dir.join(format!("keson-cover-{}.{}", hash, extension));
    log::info!("[cover] Writing to: {:?}", output_path);
    
    // Write data to temp file
    let mut file = match File::create(&output_path) {
        Ok(f) => f,
        Err(e) => {
            log::error!("[cover] Failed to create temp file: {}", e);
            return Err(format!("Failed to create temp file: {}", e));
        }
    };

    if let Err(e) = file.write_all(picture.data()) {
        log::error!("[cover] Failed to write data: {}", e);
        return Err(format!("Failed to write cover data: {}", e));
    }

    let result = output_path.to_string_lossy().to_string();
    log::info!("[cover] Success: {}", result);
    Ok(Some(result))
}

#[tauri::command]
async fn scan_folder(
    folder: String,
    min_kbps: Option<u32>,
    app: tauri::AppHandle,
) -> Result<Vec<ScanResult>, String> {
    let handle = app.clone();
    async_runtime::spawn_blocking(move || {
        let settings = load_settings(&handle);
        init_rayon_pool_with(settings.rayon_threads);
        let min = min_kbps.unwrap_or(settings.min_bitrate);
        let root = Path::new(&folder);
        if !root.exists() {
            return Err("Dossier introuvable".into());
        }

        let mut audio_entries = Vec::new();
        let mut discovered = 0usize;
        let mut tick = 0u32;
        let _ = handle.emit("scan_progress", 1u32);

        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_dir() && entry.file_name() == "backup-ksi" {
                continue;
            }
            if entry.file_type().is_file() {
                if entry.path().components().any(|c| c.as_os_str() == "backup-ksi") {
                    continue;
                }
                discovered += 1;
                if is_audio(entry.path()) {
                    audio_entries.push(entry);
                }
                let pct = 1 + ((discovered as f64).sqrt() as u32 % 12);
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
                    &handle, // Pass AppHandle
                    min,
                    settings.analysis_window_seconds,
                    settings.cache_enabled,
                    &cache,
                );
                let (bitrate, is_lossless, note, status) = match analysis {
                    Ok(res) => res,
                    Err(err) => {
                        log::error!("[scan] Analysis FAILED for {:?}: {}", path, err);
                        (None, None, Some(err), "error".to_string())
                    }
                };

                // Check if file has been replaced (has KESON_REPLACED tag)
                let replaced = tagging::has_replaced_tag(path);
                
                // If file was replaced, mark status as "replaced" instead of "bad"
                let final_status = if replaced && status == "bad" {
                    "replaced".to_string()
                } else {
                    status
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
                    status: final_status,
                    replaced,
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
async fn open_file(path: String) -> Result<(), String> {
    if !Path::new(&path).exists() {
        return Err("Fichier introuvable".into());
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .status()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/c", "start", "", &path.replace('/', "\\")])
            .status()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&path)
            .status()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn open_logs_folder(app: tauri::AppHandle) -> Result<(), String> {
    let log_dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    
    if !log_dir.exists() {
        return Err("Dossier de logs introuvable".into());
    }

    let path = log_dir.to_string_lossy().to_string();

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .status()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&path.replace('/', "\\"))
            .status()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&path)
            .status()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn open_spectrum(path: String, app: tauri::AppHandle) -> Result<Vec<u8>, String> {
    let src = Path::new(&path);
    if !src.exists() {
        return Err("Fichier introuvable".into());
    }

    let temp_root = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    if !temp_root.exists() {
        std::fs::create_dir_all(&temp_root).map_err(|e| e.to_string())?;
    }
    let temp_root_str = temp_root.to_string_lossy();

    let result = audio::invoke_whatsmybitrate(
        &app,
        "spectrum",
        src.to_str().unwrap_or_default(),
        None,
        Some(&temp_root_str),
    ).await;

    match result {
        Ok(json) => {
             // Check if "error" key is present in the JSON response
            if let Some(err) = json.get("error").and_then(|s| s.as_str()) {
                return Err(format!("whatsmybitrate failed: {}", err));
            }
             
            let spectro_path = json.get("spectrogram_path").and_then(|s| s.as_str());
            if let Some(p) = spectro_path {
                 let bytes = std::fs::read(p).map_err(|e| format!("Failed to read generated spectrum: {e}"))?;
                 // Clean up the file
                 let _ = std::fs::remove_file(p); 
                 Ok(bytes)
            } else {
                 Err("whatsmybitrate did not return a spectrogram path".into())
            }
        },
        Err(e) => Err(format!("whatsmybitrate execution failed: {}", e))
    }
}



/// Response from auth status check
#[derive(serde::Serialize)]
struct AuthStatus {
    registered: bool,
    invite_required: bool,
    slots_remaining: Option<u32>,
}

/// Register client with invite code
#[tauri::command]
async fn register_client(invite_code: String, app: tauri::AppHandle) -> Result<(), String> {
    let device_name = tauri_plugin_os::hostname();
    
    let result = tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Client build failed: {e}"))?;
        
        let resp = client
            .post(format!("{}/register", CORE_API_URL))
            .json(&serde_json::json!({
                "invite_code": invite_code,
                "device_name": device_name
            }))
            .send()
            .map_err(|e| format!("Registration request failed: {e}"))?;
        
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err("Code d'invitation invalide.".to_string());
            }
            
            return Err(format!("Échec de l'enregistrement: {}", text));
        }
        
        let body: serde_json::Value = resp.json()
            .map_err(|e| format!("Invalid JSON: {e}"))?;
        
        let token = body.get("client_token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| "No client_token in response".to_string())?;
        
        Ok(token.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    
    // Save token to settings
    let mut settings = load_settings(&app);
    settings.client_token = Some(result);
    save_settings(app, settings)?;
    
    Ok(())
}

/// Check if client is registered and get auth status
/// Validates token with server if present
#[tauri::command]
async fn check_auth_status(app: tauri::AppHandle) -> Result<AuthStatus, String> {
    let settings = load_settings(&app);
    
    // If we have a token, validate it with the server
    if let Some(token) = settings.client_token.as_ref().filter(|t| !t.is_empty()) {
        let token_clone = token.clone();
        
        // Try to validate - returns Some(true) if valid, Some(false) if explicitly rejected (401), None if unreachable
        let validation_result = tauri::async_runtime::spawn_blocking(move || {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .ok()?;
            
            // Use /auth/validate endpoint to check if token is valid
            match client
                .get(format!("{}/auth/validate", CORE_API_URL))
                .header("X-Client-Token", &token_clone)
                .send() 
            {
                Ok(resp) => {
                    if resp.status().is_success() {
                        Some(true)  // Token is valid
                    } else if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                        Some(false)  // Token explicitly rejected
                    } else {
                        None  // Other error, treat as unreachable
                    }
                }
                Err(_) => None  // Network error, server unreachable
            }
        })
        .await
        .ok()
        .flatten();
        
        match validation_result {
            Some(true) => {
                // Token is valid
                return Ok(AuthStatus {
                    registered: true,
                    invite_required: false,
                    slots_remaining: None,
                });
            }
            Some(false) => {
                // Token explicitly invalid (401) - clear it
                log::info!("[auth] Token rejected by server (401), clearing token");
                let mut new_settings = settings.clone();
                new_settings.client_token = None;
                let _ = save_settings(app.clone(), new_settings);
            }
            None => {
                // Server unreachable - assume token is still valid, don't clear it
                log::info!("[auth] Server unreachable, assuming token is valid");
                return Ok(AuthStatus {
                    registered: true,
                    invite_required: false,
                    slots_remaining: None,
                });
            }
        }
    }
    
    // No token or invalid token - check with server for invite status
    let result = tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Client build failed: {e}"))?;
        
        let resp = client
            .get(format!("{}/auth/status", CORE_API_URL))
            .send()
            .map_err(|e| format!("Auth status request failed: {e}"))?;
        
        if !resp.status().is_success() {
            return Err("Failed to get auth status".to_string());
        }
        
        let body: serde_json::Value = resp.json()
            .map_err(|e| format!("Invalid JSON: {e}"))?;
        
        let slots = body.get("slots_remaining")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        
        Ok(AuthStatus {
            registered: false,
            invite_required: true,
            slots_remaining: slots,
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    
    Ok(result)
}

/// Search for tracks on Tidal and SoundCloud
#[tauri::command]
async fn search_tracks(query: String, app: tauri::AppHandle) -> Result<Vec<SearchResult>, String> {
    let settings = load_settings(&app);
    let client_token = settings.client_token.clone()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "Non enregistré. Veuillez entrer votre code d'invitation.".to_string())?;
    
    if query.trim().len() < 2 {
        return Err("La recherche doit contenir au moins 2 caractères".to_string());
    }
    
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Client build failed: {e}"))?;
        
        log::info!("[GUI] Search query: '{}'", query);
        
        let payload = serde_json::json!({
            "query": query
        });
        
        let resp = client.post(format!("{}/search/multi", CORE_API_URL))
            .header("X-Client-Token", &client_token)
            .json(&payload)
            .send()
            .map_err(|e| format!("Search request failed: {e}"))?;
        
        if !resp.status().is_success() {
            let err_text = resp.text().unwrap_or_default();
            log::error!("[GUI] Search failed: {}", err_text);
            return Err(format!("Search failed: {}", err_text));
        }
        
        let json: serde_json::Value = resp.json()
            .map_err(|e| format!("JSON parse failed: {e}"))?;
        
        let results: Vec<SearchResult> = json["results"]
            .as_array()
            .map(|arr| {
                arr.iter().filter_map(|v| {
                    Some(SearchResult {
                        source: v["source"].as_str()?.to_string(),
                        url: v["url"].as_str()?.to_string(),
                        title: v["title"].as_str().unwrap_or("Unknown").to_string(),
                        artist: v["artist"].as_str().unwrap_or("Unknown").to_string(),
                        duration: v["duration"].as_f64(),
                        cover_url: v["cover_url"].as_str().map(|s| s.to_string()),
                        score: v["score"].as_f64().unwrap_or(0.0),
                    })
                }).collect()
            })
            .unwrap_or_default();
        
        log::info!("[GUI] Search returned {} results", results.len());
        Ok(results)
    }).await.map_err(|e| e.to_string())?
}

fn main() {
    log_panics::init();
    init_rayon_pool();
    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_log::Builder::default()
            .targets([
                tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: None }),
                tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
            ])
            .level(log::LevelFilter::Info)
            .filter(|metadata| {
                // Silence reqwest and hyper trace/debug logs
                if metadata.target().starts_with("reqwest") || metadata.target().starts_with("hyper") {
                    return false;
                }
                true
            })
            .build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|_app| {
            // Only register updater plugin if with-updater feature is enabled
            #[cfg(feature = "with-updater")]
            {
                _app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            }

            // Windows-specific: Disable decorations for custom title bar
            #[cfg(target_os = "windows")]
            {
                use tauri::Manager;
                if let Some(window) = _app.get_webview_window("main") {
                    let _ = window.set_decorations(false);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            queue_stats,
            download_link,
            scan_folder,
            reveal_in_folder,
            open_file,
            open_spectrum,
            get_settings,
            save_settings,
            redownload_bad,
            download_with_url,
            accept_redownload,
            discard_file,
            revert_replacement,
            extract_cover,
            register_client,
            check_auth_status,
            open_logs_folder,
            search_tracks
        ])

        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn init_rayon_pool() {
    let threads = std::env::var("RAYON_NUM_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or_else(|| std::cmp::max(1, num_cpus::get()));

    let _ = ThreadPoolBuilder::new().num_threads(threads).build_global();
}

fn init_rayon_pool_with(threads: usize) {
    if rayon::current_num_threads() > 0 {
        return;
    }
    let count = if threads > 0 {
        threads
    } else {
        std::cmp::max(1, num_cpus::get())
    };
    let _ = ThreadPoolBuilder::new().num_threads(count).build_global();
}

#[tauri::command]
async fn redownload_bad(paths: Vec<String>, source: String, backup: bool, app: tauri::AppHandle) -> Result<Vec<RedownloadResult>, String> {
    let settings = load_settings(&app);
    let client_token = settings.client_token.clone()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "Non enregistré. Veuillez entrer votre code d'invitation.".to_string())?;
    
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("Client build failed: {e}"))?;
        
        log::info!("[GUI] Using Core API: {}", CORE_API_URL);
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

            log::info!("[GUI] Redownload Query for: '{}' (source: {}, backup: {})", stem, source, backup);

            let file_metadata = extract_metadata_from_file(&path, &app);

            let clean_query = stem
                .split(" - ")
                .take(2)
                .collect::<Vec<_>>()
                .join(" - ");
            let clean_query = if clean_query.is_empty() { stem.to_string() } else { clean_query };
            
            log::info!("[GUI] Search query (cleaned): '{}'", clean_query);

            let search_payload = serde_json::json!({
                "query": clean_query,
                "metadata": {
                    "artist": file_metadata.artist,
                    "title": file_metadata.title,
                    "album": file_metadata.album,
                    "duration": file_metadata.duration,
                    "isrc": file_metadata.isrc
                },
                "source": source
            });
            
            let mut download_target: Option<String> = None;
            let mut cover_url: Option<String> = None;
            let mut source_type = "unknown";

            match client.post(format!("{}/search/track", CORE_API_URL))
                .header("X-Client-Token", &client_token)
                .json(&search_payload)
                .send() {
                Ok(resp) => {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        if json["success"].as_bool().unwrap_or(false) && json["found"].as_bool().unwrap_or(false) {
                            if let Some(url) = json["url"].as_str() {
                                let detected_source = json["source"].as_str().unwrap_or("tidal");
                                let score = json["score"].as_f64().unwrap_or(0.0);
                                log::info!("[GUI] Found on {}: {} (score: {})", detected_source, url, score);
                                download_target = Some(url.to_string());
                                cover_url = json["cover_url"].as_str().map(|s| s.to_string());
                                log::info!("[GUI] Found on {}: {} (score: {}, cover: {:?})", detected_source, url, score, cover_url);
                                source_type = if detected_source == "soundcloud" { "soundcloud" } else { "tidal" };
                            }
                        } else {
                            log::info!("[GUI] No confident match found for: {}", stem);
                        }
                    }
                }
                Err(e) => log::error!("[GUI] Search request failed: {}", e),
            }

            let download_url = match download_target {
                Some(url) => url,
                None => {
                    log::error!("[GUI] Skipping '{}' - no automatic match", stem);
                    continue;
                }
            };

            let payload = serde_json::json!({
                "url": download_url,
                "source": source_type
            });

            match client.post(format!("{}/download", CORE_API_URL))
                .header("X-Client-Token", &client_token)
                .json(&payload)
                .send() {
                    Ok(resp) => {
                        if !resp.status().is_success() {
                            let err_text = resp.text().unwrap_or_default();
                            log::error!("[GUI] Download request failed: {}", err_text);
                            continue;
                        }

                        if let Ok(json) = resp.json::<serde_json::Value>() {
                            if let Some(rel_url) = json["downloadUrl"].as_str() {
                                if cover_url.is_none() {
                                    cover_url = json["metadata"]["thumbnail"]
                                        .as_str()
                                        .map(|s| s.to_string())
                                        .or_else(|| json["metadata"]["cover_url"].as_str().map(|s| s.to_string()));
                                    if let Some(ref c) = cover_url {
                                         log::info!("[GUI] Retrieved cover from download metadata: {}", c);
                                    }
                                }

                                let file_url = format!("{}{}", CORE_API_URL, rel_url);
                                let final_filename = json["filename"].as_str().unwrap_or("downloaded.mp3");
                                let dest_path = parent.join(final_filename);

                                match client.get(&file_url)
                                     .header("X-Client-Token", &client_token)
                                     .send() {
                                     Ok(mut file_resp) => {
                                         if let Ok(mut file) = fs::File::create(&dest_path) {
                                             if let Err(e) = file_resp.copy_to(&mut file) {
                                                 log::error!("[GUI] Failed to write file: {}", e);
                                             } else {
                                                 // Explicitly sync file to disk before probing (fixes macOS race condition)
                                                 let _ = file.sync_all();
                                                 drop(file); // Ensure file handle is closed
                                                 log::info!("[GUI] Downloaded to: {:?}", dest_path);
                                                 
                                                 let original_dur = probe_duration(&path, &app);
                                                 let new_dur = probe_duration(&dest_path, &app);

                                                 let tolerance_sec = 2.0;
                                                 let tolerance_pct = 0.05;
                                                 let diff = (original_dur.unwrap_or(0.0) - new_dur.unwrap_or(0.0)).abs();
                                                 let rel = if original_dur.unwrap_or(0.0) > 0.0 {
                                                     diff / original_dur.unwrap_or(1.0)
                                                 } else {
                                                     1.0
                                                 };
                                                 let is_match = diff <= tolerance_sec || rel <= tolerance_pct;

                                                 if is_match && dest_path != path {
                                                     if backup && path.exists() {
                                                         let backup_dir = parent.join("backup-ksi");
                                                         if !backup_dir.exists() {
                                                             let _ = fs::create_dir_all(&backup_dir);
                                                         }
                                                         let backup_path = backup_dir.join(path.file_name().unwrap_or_default());
                                                         if let Err(e) = fs::copy(&path, &backup_path) {
                                                             log::error!("[GUI] Failed to backup file: {}", e);
                                                         } else {
                                                             log::info!("[GUI] Backed up to: {:?}", backup_path);
                                                         }
                                                     }
                                                     if let Err(e) = fs::remove_file(&path) {
                                                         log::error!("[GUI] Failed to delete original: {}", e);
                                                     } else {
                                                         log::info!("[GUI] Auto-replaced original file (durations matched)");
                                                     }
                                                 }

                                                 let new_bitrate = probe_bitrate(&dest_path, &app);

                                                 // Write KESON_REPLACED tag to mark file as replaced
                                                 if let Err(e) = tagging::write_replaced_tag(&dest_path) {
                                                     log::error!("[GUI] Failed to write replaced tag: {}", e);
                                                 }

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
                                    Err(e) => log::error!("[GUI] Failed to download file content: {}", e),
                                }
                            }
                        }
                    },
                    Err(e) => log::error!("[GUI] Download API call failed: {}", e),
            }
        }
        
        Ok(downloaded)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn download_with_url(original_path: String, url: String, backup: bool, app: tauri::AppHandle) -> Result<RedownloadResult, String> {
    let settings = load_settings(&app);
    let client_token = settings.client_token.clone()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "Non enregistré. Veuillez entrer votre code d'invitation.".to_string())?;
    
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("Client build failed: {e}"))?;
        
        log::info!("[GUI] Using Core API: {}", CORE_API_URL);
        
        let path = PathBuf::from(&original_path);
        let parent = path
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| "Chemin sans dossier".to_string())?;
        
        let source_type = if url.contains("tidal.com") { "tidal" } else { "soundcloud" };
        
        log::info!("[GUI] Manual download from {} for: {}", source_type, original_path);
        
        let payload = serde_json::json!({
            "url": url,
            "source": source_type
        });

        let resp = client.post(format!("{}/download", CORE_API_URL))
            .header("X-Client-Token", &client_token)
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

        let file_url = format!("{}{}", CORE_API_URL, rel_url);
        let mut file_resp = client.get(&file_url)
            .header("X-Client-Token", &client_token)
            .send()
            .map_err(|e| format!("Failed to fetch file: {}", e))?;
            
        let mut file = fs::File::create(&dest_path)
            .map_err(|e| format!("Failed to create file: {}", e))?;
            
        file_resp.copy_to(&mut file)
            .map_err(|e| format!("Failed to write file: {}", e))?;

        // Explicitly sync file to disk before probing (fixes macOS race condition)
        file.sync_all().map_err(|e| format!("Failed to sync file: {}", e))?;
        drop(file); // Ensure file handle is closed
        
        log::info!("[GUI] Downloaded to: {:?}", dest_path);
        
        log::info!("[GUI] Probing original duration for: {:?}", path);
        let original_dur = probe_duration(&path, &app).unwrap_or(0.0);
        log::info!("[GUI] Original duration: {}", original_dur);

        log::info!("[GUI] Probing new duration for: {:?}", dest_path);
        let new_dur = probe_duration(&dest_path, &app).unwrap_or(0.0);
        log::info!("[GUI] New duration: {}", new_dur);

        if backup {
             let backup_dir = parent.join("backup-ksi");
             if !backup_dir.exists() {
                  let _ = fs::create_dir_all(&backup_dir);
             }
             let backup_path = backup_dir.join(path.file_name().unwrap_or_default());
             if let Err(e) = fs::copy(&path, &backup_path) {
                  log::error!("[GUI] Failed to backup file: {}", e);
             } else {
                  log::info!("[GUI] Backed up to: {:?}", backup_path);
             }
             
             if let Err(e) = fs::remove_file(&path) {
                 log::error!("[GUI] Failed to delete original: {}", e);
             } else if let Err(e) = fs::rename(&dest_path, &path) {
                 log::error!("[GUI] Failed to move new file to original: {}", e);
             } else {
                 log::info!("[GUI] Replaced original file");
                 if dest_path.exists() && dest_path != path {
                     log::info!("[GUI] Source file persisted after rename. Force deleting: {:?}", dest_path);
                     let _ = fs::remove_file(&dest_path);
                 }
             }
        }

        let new_file_path = if backup { &path } else { &dest_path };
        let new_bitrate = probe_bitrate(new_file_path, &app);

        // Write KESON_REPLACED tag to mark file as replaced
        if let Err(e) = tagging::write_replaced_tag(new_file_path) {
            log::error!("[GUI] Failed to write replaced tag: {}", e);
        }

        Ok(RedownloadResult {
            original_path,
            new_path: new_file_path.to_string_lossy().to_string(),
            original_duration: Some(original_dur),
            new_duration: Some(new_dur),
            cover_url: json["metadata"]["thumbnail"].as_str().map(|s| s.to_string().replace("url(\"", "").replace("\")", "")),
            new_bitrate,
        })
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn revert_replacement(original_path: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(&original_path);
        let parent = path.parent().ok_or("Invalid path")?;
        let filename = path.file_name().ok_or("Invalid filename")?;
        let backup_path = parent.join("backup-ksi").join(filename);

        log::info!("[GUI] Attempting to revert: {:?} from {:?}", path, backup_path);

        if !backup_path.exists() {
            return Err("Backup file not found".to_string());
        }

        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("Failed to remove current file: {}", e))?;
        }

        fs::rename(&backup_path, &path).map_err(|e| format!("Failed to restore backup: {}", e))?;
        
        if let Some(_) = path.file_stem() {
             let ghosts = ["m4a", "flac", "wav", "mp3", "aac", "ogg"];
             for ext in ghosts {
                  let ghost_path = path.with_extension(ext);
                  if ghost_path == path { continue; }
                  
                  if ghost_path.exists() {
                       log::info!("[GUI] Revert cleanup: Removing ghost file {:?}", ghost_path);
                       let _ = fs::remove_file(ghost_path);
                  }
             }
        }

        log::info!("[GUI] Reverted successfully");
        Ok(true)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn accept_redownload(app: tauri::AppHandle, original: String, new_path: String) -> Result<String, String> {
    log::error!("[accept_redownload] Request to replace '{}' with '{}'", original, new_path);
    let orig = PathBuf::from(&original);
    let newp = PathBuf::from(&new_path);

    if !newp.exists() {
        log::error!("[accept_redownload] New file not found: {:?}", newp);
        return Err("Fichier téléchargé introuvable".into());
    }

    if let Some(parent) = orig.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            log::error!("[accept_redownload] Failed to create parent dir: {}", e);
            return Err(e.to_string());
        }
    }

    if orig.exists() {
        if let Some(parent) = orig.parent() {
            let backup_dir = parent.join("backup-ksi");
            if !backup_dir.exists() {
                if let Err(e) = fs::create_dir_all(&backup_dir) {
                    log::error!("[accept_redownload] Failed to create backup dir: {}", e);
                }
            }
            
            if backup_dir.exists() {
                let filename = orig.file_name().unwrap_or_default();
                let backup_path = backup_dir.join(filename);
                
                log::error!("[accept_redownload] Backing up original to: {:?}", backup_path);
                
                // If backup already exists, maybe overwrite or rename? 
                // For now, let's just overwrite backup (standard behavior for simple bak)
                if let Err(e) = fs::rename(&orig, &backup_path) {
                     log::error!("[accept_redownload] Backup failed: {}", e);
                }
            }
        }
    }

    log::error!("[accept_redownload] Renaming new file to original...");
    match fs::rename(&newp, &orig) {
        Ok(_) => {
             log::error!("[accept_redownload] Success");
             
             // Invalidate cache for this file
             let settings = load_settings(&app); // pass reference to app
             if let Ok(path) = cache_path(&app) {
                  let mut cache = load_cache(&path, settings.cache_max_entries);
                  if cache.remove(&orig.to_string_lossy().to_string()).is_some() {
                      log::error!("[accept_redownload] Invalidated cache for: {:?}", orig);
                      let _ = save_cache(&path, &cache);
                  }
             }

             Ok(orig.to_string_lossy().to_string())
        },
        Err(e) => {
             log::error!("[accept_redownload] Rename failed: {}", e);
             Err(e.to_string())
        }
    }
}

#[tauri::command]
fn discard_file(path: String) -> Result<(), String> {
    let p = PathBuf::from(path);
    if p.exists() {
        fs::remove_file(p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn extract_cover(audio_path: String, app: tauri::AppHandle) -> Result<Option<String>, String> {
    extract_embedded_cover(&audio_path, &app)
}
