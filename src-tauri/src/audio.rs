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

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Helper to resolve the absolute path of a bundled sidecar binary
/// logic:
/// 1. Check same directory as current executable (standard for Tauri bundled apps)
/// 2. Check resource_dir/binaries/ (dev mode or alternative config)
fn resolve_sidecar_path(app: &tauri::AppHandle, name: &str) -> Option<PathBuf> {
    // 1. Check relative to executable (Contents/MacOS/ on Mac, or root of portable exe)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let path = exe_dir.join(name);
            log::error!("[sidecar] Checking exe_dir: {:?}", path);
            if path.exists() {
                return Some(path);
            }
        }
    }

    // 2. Check resource directory (standard dev layout or Windows sometimes)
    if let Ok(resource_dir) = app.path().resource_dir() {
        let path = resource_dir.join("binaries").join(name);
        log::error!("[sidecar] Checking resource_dir: {:?}", path);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Run ffprobe sidecar with given arguments, returns stdout as bytes
/// Uses synchronous execution to avoid tokio runtime deadlocks
pub fn run_ffprobe_sidecar(app: &tauri::AppHandle, args: Vec<&str>) -> Result<Vec<u8>, String> {
    // Determine the bundled ffprobe path based on platform
    #[cfg(target_os = "macos")]
    let binary_name = "ffprobe";
    #[cfg(target_os = "windows")]
    let binary_name = "ffprobe.exe";
    #[cfg(target_os = "linux")]
    let binary_name = "ffprobe";
    
    // Try to find the bundled binary
    if let Some(bundled_path) = resolve_sidecar_path(app, binary_name) {
        log::error!("[ffprobe] Found bundled binary at {:?}, executing synchronously...", bundled_path);
        
        let mut cmd = Command::new(&bundled_path);
        cmd.args(&args);
        
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        
        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                    log::error!("[ffprobe] Bundled ffprobe succeeded, stdout len: {}", output.stdout.len());
                    return Ok(output.stdout);
                } else {
                    log::error!("[ffprobe] Bundled ffprobe failed: {}", String::from_utf8_lossy(&output.stderr));
                    // Proceed to fallback
                }
            },
            Err(e) => {
                 log::error!("[ffprobe] Failed to execute bundled binary: {}", e);
                 // Proceed to fallback
            }
        }
    } else {
        log::error!("[ffprobe] Bundled binary '{}' not found in standard locations", binary_name);
    }
    
    // Fallback to system ffprobe (dev mode or if bundled binary not found/failed)
    log::error!("[ffprobe] Falling back to system ffprobe");
    
    let mut cmd = Command::new("ffprobe");
    cmd.args(&args);
    
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    
    let output = cmd.output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;
    
    if output.status.success() {
        log::error!("[ffprobe] System ffprobe succeeded, stdout len: {}", output.stdout.len());
        Ok(output.stdout)
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        log::error!("[ffprobe] System ffprobe failed: {}", err);
        Err(err)
    }
}

/// Run ffmpeg sidecar with given arguments, returns success status
/// Uses synchronous execution to avoid tokio runtime deadlocks
pub fn run_ffmpeg_sidecar(app: &tauri::AppHandle, args: Vec<&str>) -> Result<bool, String> {
    // Determine the bundled ffmpeg path based on platform
    #[cfg(target_os = "macos")]
    let binary_name = "ffmpeg";
    #[cfg(target_os = "windows")]
    let binary_name = "ffmpeg.exe";
    #[cfg(target_os = "linux")]
    let binary_name = "ffmpeg";
    
    // Try to find the bundled binary
    if let Some(bundled_path) = resolve_sidecar_path(app, binary_name) {
         let mut cmd = Command::new(&bundled_path);
         cmd.args(&args);
         
         #[cfg(target_os = "windows")]
         cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
         
         match cmd.output() {
             Ok(output) => {
                 return Ok(output.status.success());
             },
             Err(_) => {
                 // proceed to fallback
             }
         }
    }
    
    // Fallback to system ffmpeg (dev mode or if bundled binary not found)
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&args);
    
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    
    let output = cmd.output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;
    
    Ok(output.status.success())
}

// Helper to get resource path, checking both root and 'resources' subdir
pub fn get_resource_path(app: &tauri::AppHandle, name: &str) -> Option<PathBuf> {
    let res_dir = app.path().resource_dir().ok()?;
    
    // Check direct path (dev mode often)
    let direct = res_dir.join(name);
    if direct.exists() {
        // For whatsmybitrate, we explicitly want the directory, not a stray binary file in target root
        if name == "whatsmybitrate" {
            if direct.is_dir() {
                return Some(direct);
            }
        } else {
            return Some(direct);
        }
    }

    // Check resources subdir (prod bundle often, and correctly placed in debug/resources)
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
    
    // Get the directory where the current executable is running
    // In dev: target/debug/
    // In prod: Bundle content directory
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    if let Ok(resource_dir) = app.path().resource_dir() {
        if let Ok(current_path) = std::env::var("PATH") {
            // Add resource_dir, resource_dir/resources, and exe_dir to PATH
            let nested_res = resource_dir.join("resources");
            
            let mut paths = vec![
                resource_dir.to_path_buf(),
                nested_res,
            ];
            
            if let Some(ed) = exe_dir {
                paths.push(ed);
            }
            
            // Re-construct PATH safely
             #[cfg(unix)]
            let separator = ":";
            #[cfg(windows)]
            let separator = ";";

            let extra_paths = paths.iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(separator);
                
            let new_path = format!("{}{}{}", extra_paths, separator, current_path);
            
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

/// Extract metadata from an audio file using ffprobe (sidecar)
pub fn extract_metadata_from_file(path: &Path, app: &tauri::AppHandle) -> ExtractedMetadata {
    let mut metadata = ExtractedMetadata::default();
    
    let path_str = path.to_str().unwrap_or_default();
    let args = vec!["-v", "quiet", "-print_format", "json", "-show_format", path_str];
    
    if let Ok(stdout) = run_ffprobe_sidecar(app, args) {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&stdout) {
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
            
            log::info!("[GUI] Extracted metadata: artist={:?}, title={:?}, duration={:?}, isrc={:?}", 
                metadata.artist, metadata.title, metadata.duration, metadata.isrc);
        }
    }

    metadata
}

/// Probe duration of an audio file using ffprobe (sidecar)
pub fn probe_duration(path: &Path, app: &tauri::AppHandle) -> Option<f64> {
    log::error!("[probe_duration] Probing: {:?}", path);
    let path_str = path.to_string_lossy();
    let args = vec![
        "-v", "error",
        "-show_entries", "format=duration",
        "-of", "default=noprint_wrappers=1:nokey=1",
        &path_str,
    ];
    
    match run_ffprobe_sidecar(app, args) {
        Ok(stdout) => {
            let text = String::from_utf8_lossy(&stdout);
            log::error!("[probe_duration] Raw output: '{}'", text.trim());
            let line = text.lines().next()?.trim();
            let duration = f64::from_str(line).ok();
            log::error!("[probe_duration] Parsed duration: {:?}", duration);
            duration
        }
        Err(e) => {
            log::error!("[probe_duration] ffprobe failed: {}", e);
            None
        }
    }
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
                 
                 #[cfg(windows)]
                 {
                     use std::os::windows::process::CommandExt;
                     const CREATE_NO_WINDOW: u32 = 0x08000000;
                     cmd.creation_flags(CREATE_NO_WINDOW);
                 }

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
    
    // Explicitly add FFPROBE_PATH to envs if we can find the resource
    let mut envs = get_env_with_resources(app);
    #[cfg(target_os = "windows")]
    let ffprobe_name = "ffprobe.exe";
    #[cfg(not(target_os = "windows"))]
    let ffprobe_name = "ffprobe";

    // Use the robust sidecar resolution to find ffprobe (handles Contents/MacOS/ on bundle)
    if let Some(ffprobe_path) = resolve_sidecar_path(app, ffprobe_name) {
        envs.insert("FFPROBE_PATH".to_string(), ffprobe_path.to_string_lossy().to_string());
        log::info!("[whatsmybitrate] Injected FFPROBE_PATH: {:?}", ffprobe_path);
    } else {
        log::info!("[whatsmybitrate] WARNING: Could not resolve ffprobe path for injection");
    }

    // Run bundled executable
    tauri::async_runtime::spawn_blocking(move || {
         let mut cmd = Command::new(&exe_clone);
         cmd.envs(&envs);

         #[cfg(target_os = "windows")]
         {
             let _ = cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
         }

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
                    // Check if entry is valid (has bitrate OR is lossless)
                    let is_valid_entry = entry.bitrate.is_some() || entry.is_lossless.unwrap_or(false);
                    
                    if is_valid_entry {
                        let status = match (entry.bitrate, entry.is_lossless) {
                            (Some(b), _) if b < min => "bad".to_string(),
                            (Some(_), _) => "ok".to_string(), 
                            (None, Some(true)) => "ok".to_string(), // Lossless
                            _ => "ok".to_string(), // Should be covered by is_valid_entry
                        };
                        return Ok((entry.bitrate, entry.is_lossless, entry.note.clone(), status));
                    } else {
                        // Entry exists but is incomplete (failed analysis) - ignore it and re-scan
                        // log::info!("[scan] Ignoring incomplete cache entry for {:?}", path);
                    }
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
            log::error!("[scan] Analysis returned error for {:?}: {:?}", path, err);
            "error".to_string()
        }
        (false, Some(b), _) if b < min => "bad".to_string(),
        (false, Some(_), _) => "ok".to_string(),
        (false, None, Some(true)) => "ok".to_string(), // FLAC/lossless = ok
        _ => {
            log::error!("[scan] No bitrate returned for {:?}, parsed: {:?}", path, parsed);
            "error".to_string()
        }
    };

    // Only cache if analysis was successful (has bitrate OR is lossless)
    // AND there was no error
    let analysis_successful = (est.is_some() || lossless.unwrap_or(false)) && err.is_none();

    if cache_enabled && analysis_successful {
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
