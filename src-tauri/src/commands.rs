//! Tauri command handlers — thin wrappers over `Manager`.

use crate::downloader::manager::{Manager, StorageFolder, StorageUsage};
use crate::downloader::DownloadRecord;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::games::{self, GameDb, GameDetails, GameSearchPage};
use crate::settings::Settings;
use crate::storage::StorageInfo;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

type Mgr<'a> = State<'a, Arc<Manager>>;

#[tauri::command]
pub fn get_settings(manager: Mgr<'_>) -> Settings {
    manager.settings()
}

#[tauri::command]
pub fn update_settings(manager: Mgr<'_>, settings: Settings) -> AppResult<Settings> {
    manager.update_settings(settings)
}

#[tauri::command]
pub fn get_storage_info(manager: Mgr<'_>) -> StorageInfo {
    manager.storage_info()
}

#[tauri::command]
pub async fn get_storage_usage(manager: Mgr<'_>) -> AppResult<StorageUsage> {
    Ok(manager.storage_usage().await)
}

#[tauri::command]
pub fn create_storage_folder(manager: Mgr<'_>) -> AppResult<StorageInfo> {
    manager.create_storage_root()
}

#[tauri::command]
pub fn list_downloads(manager: Mgr<'_>) -> Vec<DownloadRecord> {
    manager.list()
}

#[tauri::command]
pub async fn add_download(manager: Mgr<'_>, url: String, checksum: Option<String>) -> AppResult<DownloadRecord> {
    let manager = manager.inner().clone();
    manager.add(&url, checksum.as_deref()).await
}

#[tauri::command]
pub fn pause_download(manager: Mgr<'_>, id: String) -> AppResult<()> {
    manager.pause(&id)
}

#[tauri::command]
pub fn resume_download(manager: Mgr<'_>, id: String) -> AppResult<()> {
    manager.resume(&id)
}

#[tauri::command]
pub fn retry_download(manager: Mgr<'_>, id: String) -> AppResult<()> {
    manager.retry(&id)
}

#[tauri::command]
pub fn cancel_download(manager: Mgr<'_>, id: String) -> AppResult<()> {
    manager.cancel(&id)
}

#[tauri::command]
pub fn remove_download(manager: Mgr<'_>, id: String, delete_file: bool) -> AppResult<()> {
    manager.remove(&id, delete_file)
}

#[tauri::command]
pub fn pause_all(manager: Mgr<'_>) {
    manager.pause_all()
}

#[tauri::command]
pub fn resume_all(manager: Mgr<'_>) {
    manager.resume_all()
}

/// Highlights the file in Explorer. Never opens or executes the file itself.
#[tauri::command]
pub fn reveal_download(app: AppHandle, manager: Mgr<'_>, id: String) -> AppResult<()> {
    let path = manager.reveal_path(&id)?;
    let result = if path.is_dir() {
        app.opener().open_path(path.display().to_string(), None::<&str>)
    } else {
        app.opener().reveal_item_in_dir(&path)
    };
    result.map_err(open_failed)
}

/// Opens one of the storage folders in Explorer.
#[tauri::command]
pub fn open_storage_folder(app: AppHandle, manager: Mgr<'_>, folder: StorageFolder) -> AppResult<()> {
    let path = manager.folder_path(folder)?;
    app.opener().open_path(path.display().to_string(), None::<&str>).map_err(open_failed)
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlatformFilter {
    Ps4,
    Ps5,
    All,
}

#[tauri::command]
pub async fn search_games(
    manager: Mgr<'_>,
    db: State<'_, GameDb>,
    query: String,
    page: u32,
    platform: PlatformFilter,
) -> AppResult<GameSearchPage> {
    let platforms: &[u32] = match platform {
        PlatformFilter::Ps4 => &[games::PLATFORM_PS4],
        PlatformFilter::Ps5 => &[games::PLATFORM_PS5],
        PlatformFilter::All => &[],
    };
    db.search(&manager.settings().rawg_api_key, &query, page, platforms).await
}

#[tauri::command]
pub async fn get_game(manager: Mgr<'_>, db: State<'_, GameDb>, id: u64) -> AppResult<GameDetails> {
    db.details(&manager.settings().rawg_api_key, id).await
}

/// Opens an https:// page (a game's website or its RAWG page) in the default browser.
#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> AppResult<()> {
    let parsed = crate::downloader::http::validate_url(&url)?;
    if parsed.scheme() != "https" {
        return Err(AppError::new(ErrorCode::UnsupportedProtocol, "Only https:// pages can be opened."));
    }
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|e| AppError::new(ErrorCode::Io, "The web browser could not be opened.").with_details(e.to_string()))
}

fn open_failed(err: tauri_plugin_opener::Error) -> AppError {
    AppError::new(ErrorCode::Io, "Windows Explorer could not be opened.").with_details(err.to_string())
}
