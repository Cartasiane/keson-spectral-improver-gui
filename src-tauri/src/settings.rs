use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub min_bitrate: u32,
    pub analysis_window_seconds: u32,
    pub rayon_threads: usize,
    pub cache_enabled: bool,
    pub cache_max_entries: usize,
    /// Client token received after registration with the Core server
    #[serde(default)]
    pub client_token: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            min_bitrate: 256,
            analysis_window_seconds: 100,
            rayon_threads: 0,
            cache_enabled: true,
            cache_max_entries: 10_000,
            client_token: None,
        }
    }
}

pub fn settings_path(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .or_else(|_| app.path().app_cache_dir())
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("settings.json")
}

pub fn load_settings(app: &tauri::AppHandle) -> Settings {
    let path = settings_path(app);
    if let Ok(text) = fs::read_to_string(&path) {
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        Settings::default()
    }
}

#[tauri::command]
pub fn get_settings(app: tauri::AppHandle) -> Settings {
    load_settings(&app)
}

#[tauri::command]
pub fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
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
