use hex;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tauri::Manager;

use crate::types::{CacheEntry, ExtractedMetadata};
use crate::cache::enforce_cache_limit;

// Helper to get resource path, checking both root and 'resources' subdir
pub fn get_resource_path(app: &tauri::AppHandle, name: &str) -> Option<PathBuf> {
    let res_dir = app.path().resource_dir().ok()?;
    
    // Check direct path (dev mode often)
    let direct = res_dir.join(name);
    if direct.exists() {
        return Some(direct);
    }

    // Check resources subdir (prod bundle often)
    let nested = res_dir.join("resources").join(name);
    if nested.exists() {
        return Some(nested);
    }
    
    // Check _up_/vendor (bundled ../vendor/whatsmybitrate often ends up here)
    if name == "whatsmybitrate" {
         let up_vendor = res_dir.join("_up_").join("vendor").join(name);
         if up_vendor.exists() {
             return Some(up_vendor);
         }
    }
    
    // Fallback to direct if neither found (or maybe let caller decide)
    Some(direct) 
}

// Helper to get environment with bundled binaries in PATH
pub fn get_env_with_resources(app: &tauri::AppHandle) -> HashMap<String, String> {
    let mut envs = std::env::vars().collect::<HashMap<_, _>>();
    
    if let Ok(resource_dir) = app.path().resource_dir() {
        if let Ok(current_path) = std::env::var("PATH") {
            // Add both resource_dir and resource_dir/resources to PATH to cover all bases
            let nested_res = resource_dir.join("resources");
            
            #[cfg(unix)]
            let new_path = format!("{}:{}:{}", 
                resource_dir.to_string_lossy(), 
                nested_res.to_string_lossy(),
                current_path
            );
            #[cfg(windows)]
            let new_path = format!("{};{};{}", 
                resource_dir.to_string_lossy(), 
                nested_res.to_string_lossy(), 
                current_path
            );
            
            envs.insert("PATH".to_string(), new_path);
        }
    }
    envs
}

/// Check if a file is an audio file based on extension
pub fn is_audio(path: &Path) -> bool {
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

/// Calculate SHA256 hash of a file
pub fn file_hash(path: &Path) -> std::io::Result<String> {
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

/// Extract metadata from an audio file using ffprobe
pub fn extract_metadata_from_file(path: &Path, app: &tauri::AppHandle) -> ExtractedMetadata {
    // Determine ffprobe command (bundled or system)
    let ffprobe_cmd = get_resource_path(app, "ffprobe")
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "ffprobe".to_string());

    let output = Command::new(&ffprobe_cmd)
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
                
                if let Some(dur_str) = format["duration"].as_str() {
                    metadata.duration = dur_str.parse().ok();
                }
                
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
                
                metadata.isrc = tags["isrc"].as_str()
                    .or_else(|| tags["ISRC"].as_str())
                    .or_else(|| tags["TSRC"].as_str())
                    .map(|s| s.to_string());
                
                println!("[GUI] Extracted metadata: artist={:?}, title={:?}, duration={:?}, isrc={:?}", 
                    metadata.artist, metadata.title, metadata.duration, metadata.isrc);
            }
        }
    }

    metadata
}

/// Probe duration of an audio file using ffprobe
pub fn probe_duration(path: &Path, app: &tauri::AppHandle) -> Option<f64> {
    let ffprobe_cmd = get_resource_path(app, "ffprobe")
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "ffprobe".to_string());

    let output = Command::new(&ffprobe_cmd)
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

// New helper function to invoke whatsmybitrate sidecar
pub async fn invoke_whatsmybitrate(
    app: &tauri::AppHandle,
    mode: &str, 
    file_path: &str,
    window: Option<u32>,
    output: Option<&str>,
) -> Result<serde_json::Value, String> {
    
    let args = {
        let mut a = vec![mode.to_string(), file_path.to_string()];
        if let Some(w) = window {
            a.push("--window".to_string());
            a.push(w.to_string());
        }
        if let Some(o) = output {
            a.push("--output".to_string());
            a.push(o.to_string());
        }
        a
    };
    
    // Determine binary name based on platform
    #[cfg(windows)]
    let bin_name = "whatsmybitrate.exe";
    #[cfg(not(windows))]
    let bin_name = "whatsmybitrate";

    // Try to find the bundled onedir executable in resources
    let mut exe_path = None;
    if let Some(dir) = get_resource_path(app, "whatsmybitrate") {
        let candidate = dir.join(bin_name);
        if candidate.exists() {
            exe_path = Some(candidate);
        }
    }

    // Fallback to python3 for development if bundled binary not found
    if exe_path.is_none() {
        let exe_dir = std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("no parent")?.to_path_buf();
        let vendor_dir = exe_dir.join("../vendor/whatsmybitrate");
        let script_path = vendor_dir.join("whatsmybitrate_cli.py");
         
        if script_path.exists() {
             let envs = get_env_with_resources(app);
             let script_path_clone = script_path.clone();

             // Run blocking python command
             return tauri::async_runtime::spawn_blocking(move || {
                 let python = "python3";
                 let mut cmd = Command::new(python);
                 cmd.arg(&script_path_clone);
                 for arg in args {
                     cmd.arg(arg);
                 }
                 
                 let output = cmd
                    .envs(&envs)
                    .output()
                    .map_err(|e| format!("python3 failed: {}", e))?;

                 if !output.status.success() {
                    return Err(String::from_utf8_lossy(&output.stderr).to_string());
                 }

                 serde_json::from_slice(&output.stdout)
                    .map_err(|e| format!("Failed to parse output: {}", e))
            }).await.map_err(|e| e.to_string())?
        }
    }

    let exe_final = exe_path.ok_or("Bundled whatsmybitrate not found and dev script missing")?;
    let exe_clone = exe_final.clone();
    let envs = get_env_with_resources(app);

    // Run bundled executable
    tauri::async_runtime::spawn_blocking(move || {
         let mut cmd = Command::new(&exe_clone);
         cmd.envs(&envs);
         for arg in args {
             cmd.arg(arg);
         }
         
         // On macOS/Linux, we might need to preserve environment or set minimal
         // but onedir should be self-contained. 
         // However, on macOS, adhoc signing might require clean env?
         // Let's inherit env for now.

         let output = cmd
            .output()
            .map_err(|e| format!("whatsmybitrate execution failed: {}", e))?;

         if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
         }

         serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("Failed to parse output: {}", e))
    }).await.map_err(|e| e.to_string())?
}

/// Probe bitrate using whatsmybitrate
pub fn probe_bitrate(path: &Path, app: &tauri::AppHandle) -> Option<u32> {
    let result = tauri::async_runtime::block_on(invoke_whatsmybitrate(
        app, 
        "probe", 
        path.to_str()?, 
        None, 
        None
    )).ok()?;
    
    result.get("bitrate")
        .and_then(|v| v.as_f64())
        .map(|v| v.round() as u32)
}

/// Analyze a single file with whatsmybitrate
pub fn analyze_with_wmb_single(
    path: &Path,
    app: &tauri::AppHandle, // Added app handle
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
                    let status = match (entry.bitrate, entry.is_lossless) {
                        (Some(b), _) if b < min => "bad".to_string(),
                        (Some(_), _) => "ok".to_string(),
                        (None, Some(true)) => "ok".to_string(), // FLAC/lossless = ok
                        (None, _) => {
                            eprintln!("[scan] Cached entry has no bitrate for {:?}, note: {:?}", path, entry.note);
                            "error".to_string()
                        }
                    };
                    return Ok((entry.bitrate, entry.is_lossless, entry.note.clone(), status));
                }
            }
        }
    }


    let parsed = tauri::async_runtime::block_on(invoke_whatsmybitrate(
        app,
        "analyze",
        path.to_str().unwrap_or_default(),
        Some(analysis_window),
        None
    ))?;

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

    let status = match (err.is_some(), est, lossless) {
        (true, _, _) => {
            eprintln!("[scan] Analysis returned error for {:?}: {:?}", path, err);
            "error".to_string()
        }
        (false, Some(b), _) if b < min => "bad".to_string(),
        (false, Some(_), _) => "ok".to_string(),
        (false, None, Some(true)) => "ok".to_string(), // FLAC/lossless = ok
        _ => {
            eprintln!("[scan] No bitrate returned for {:?}, parsed: {:?}", path, parsed);
            "error".to_string()
        }
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

/// Simple quality analysis result for single files
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct QualityAnalysisResult {
    pub bitrate: Option<u32>,
    pub is_lossless: Option<bool>,
    pub quality_string: String,
    pub error: Option<String>,
}

/// Analyze a single file's quality without caching (for downloads)
/// Returns bitrate, lossless flag, and a quality display string
pub fn analyze_file_quality(path: &Path, app: &tauri::AppHandle) -> Result<QualityAnalysisResult, String> {
    // Use a dummy cache since we don't need caching for single downloads
    let dummy_cache = Arc::new(Mutex::new(HashMap::new()));
    
    let (bitrate, is_lossless, error, _status) = analyze_with_wmb_single(
        path,
        app,
        0, // min_kbps - we don't filter, just analyze
        30, // analysis_window seconds
        false, // cache_enabled
        &dummy_cache,
    )?;
    
    // Build quality display string
    let quality_string = match (bitrate, is_lossless) {
        (Some(br), Some(true)) => format!("{} kbps (Lossless)", br),
        (Some(br), _) => format!("{} kbps", br),
        (None, _) => "Unknown".to_string(),
    };
    
    Ok(QualityAnalysisResult {
        bitrate,
        is_lossless,
        quality_string,
        error,
    })
}
