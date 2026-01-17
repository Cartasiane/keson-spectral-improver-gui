use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct QueueStats {
    pub active: u32,
    pub pending: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DownloadResult {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub bitrate: Option<u32>,
    pub source: Option<String>,
    pub cover_url: Option<String>,
    pub caption: String,
    pub quality: String,
    pub warning: String,
    pub saved_to: String,
}

#[derive(Serialize)]
pub struct RedownloadResult {
    pub original_path: String,
    pub new_path: String,
    pub original_duration: Option<f64>,
    pub new_duration: Option<f64>,
    pub cover_url: Option<String>,
    pub new_bitrate: Option<u32>,
}

#[derive(Serialize)]
pub struct ScanResult {
    pub path: String,
    pub name: String,
    pub bitrate: Option<u32>,
    pub is_lossless: Option<bool>,
    pub note: Option<String>,
    pub status: String, // "ok" | "bad" | "error" | "replaced"
    pub replaced: bool, // true if KESON_REPLACED tag exists
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CacheEntry {
    pub bitrate: Option<u32>,
    pub is_lossless: Option<bool>,
    pub note: Option<String>,
}

/// Metadata extracted from an audio file using ffprobe
#[derive(Serialize, Clone, Debug, Default)]
pub struct ExtractedMetadata {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub isrc: Option<String>,
}
