//! Outbound notifications from the backend. The download manager only talks to
//! this trait, which keeps it independent of Tauri and testable.

use crate::downloader::{DownloadRecord, ProgressItem};
use crate::storage::StorageInfo;

pub const DOWNLOAD_UPDATED: &str = "download-updated";
pub const DOWNLOAD_REMOVED: &str = "download-removed";
pub const DOWNLOAD_PROGRESS: &str = "download-progress";
pub const STORAGE_CHANGED: &str = "storage-changed";

pub trait EventSink: Send + Sync + 'static {
    fn download_updated(&self, record: &DownloadRecord);
    fn download_removed(&self, id: &str);
    fn progress(&self, items: &[ProgressItem]);
    fn storage_changed(&self, info: &StorageInfo);
    fn notify(&self, title: &str, body: &str);
}

pub struct TauriSink {
    app: tauri::AppHandle,
}

impl TauriSink {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }

    fn emit<T: serde::Serialize + Clone>(&self, event: &str, payload: T) {
        use tauri::Emitter;
        if let Err(err) = self.app.emit(event, payload) {
            eprintln!("events: failed to emit {event}: {err}");
        }
    }
}

impl EventSink for TauriSink {
    fn download_updated(&self, record: &DownloadRecord) {
        self.emit(DOWNLOAD_UPDATED, record.clone());
    }

    fn download_removed(&self, id: &str) {
        self.emit(DOWNLOAD_REMOVED, id.to_string());
    }

    fn progress(&self, items: &[ProgressItem]) {
        self.emit(DOWNLOAD_PROGRESS, items.to_vec());
    }

    fn storage_changed(&self, info: &StorageInfo) {
        self.emit(STORAGE_CHANGED, info.clone());
    }

    fn notify(&self, title: &str, body: &str) {
        use tauri_plugin_notification::NotificationExt;
        if let Err(err) = self.app.notification().builder().title(title).body(body).show() {
            eprintln!("events: notification failed: {err}");
        }
    }
}
