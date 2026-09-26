pub mod engine;
pub mod http;
pub mod manager;
pub mod manifest;
pub mod speed;

use crate::checksum::ExpectedChecksum;
use crate::error::AppError;
use crate::storage::Layout;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Queued,
    Downloading,
    Retrying,
    Verifying,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl Status {
    /// States in which a transfer task owns the download.
    pub fn is_running(self) -> bool {
        matches!(self, Status::Downloading | Status::Retrying | Status::Verifying)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Integrity {
    NotChecked,
    Verified,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryInfo {
    pub attempt: u32,
    pub max_attempts: u32,
    /// Unix time in milliseconds.
    pub retry_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRecord {
    pub id: String,
    pub url: String,
    pub file_name: String,
    /// Name of the partial file inside Downloads/ or Incomplete/.
    pub part_name: String,
    /// Storage root this download belongs to (kept even if the setting changes later).
    pub root: PathBuf,
    pub total_size: Option<u64>,
    pub downloaded: u64,
    pub supports_range: bool,
    pub connections: u8,
    pub validator: Option<String>,
    pub status: Status,
    pub error: Option<AppError>,
    pub checksum: Option<ExpectedChecksum>,
    pub integrity: Integrity,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub final_path: Option<PathBuf>,
    #[serde(default)]
    pub retry: Option<RetryInfo>,
}

impl DownloadRecord {
    pub fn layout(&self) -> Layout {
        Layout::new(&self.root)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressItem {
    pub id: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    /// Bytes per second over a rolling window.
    pub speed: f64,
    /// Seconds remaining, when both size and speed are known.
    pub eta: Option<u64>,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
