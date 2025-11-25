#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;

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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![queue_stats, download_link])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
