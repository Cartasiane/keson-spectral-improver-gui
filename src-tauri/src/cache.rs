use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tauri::Manager;

use crate::types::CacheEntry;

pub fn cache_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
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

pub fn load_cache(path: &Path, limit: usize) -> HashMap<String, CacheEntry> {
    if let Ok(text) = fs::read_to_string(path) {
        let mut map: HashMap<String, CacheEntry> = serde_json::from_str(&text).unwrap_or_default();
        enforce_cache_limit(&mut map, limit);
        map
    } else {
        HashMap::new()
    }
}

pub fn save_cache(path: &Path, cache: &HashMap<String, CacheEntry>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_string(cache).unwrap_or_default())?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn enforce_cache_limit(cache: &mut HashMap<String, CacheEntry>, limit: usize) {
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
