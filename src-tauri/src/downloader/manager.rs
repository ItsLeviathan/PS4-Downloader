//! Download queue: owns all records, starts transfers up to the concurrency
//! limit, retries transient failures, persists state and watches storage.
//!
//! Locking: `inner` is a std mutex that is never held across an `.await` or a
//! slow filesystem call. `persist_lock` is always taken before `inner`.

use super::engine::{self, Transfer};
use super::http;
use super::manifest::MIN_SEGMENTED_SIZE;
use super::speed::SpeedMeter;
use super::{now_ms, DownloadRecord, Integrity, ProgressItem, RetryInfo, Status};
use crate::checksum;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::events::EventSink;
use crate::settings::Settings;
use crate::state::{load_json, save_json};
use crate::storage::{self, FolderUsage, Layout, StorageInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;

const SETTINGS_FILE: &str = "settings.json";
const DOWNLOADS_FILE: &str = "downloads.json";
pub const MAX_ATTEMPTS: u32 = 5;
const TICK: Duration = Duration::from_millis(500);
const PERSIST_EVERY_TICKS: u32 = 10;
const STORAGE_EVERY_TICKS: u32 = 6;
const STORAGE_NOTICE_COOLDOWN: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopIntent {
    None,
    Pause,
    Cancel,
    StorageLost,
    Shutdown,
}

struct Active {
    cancel: CancellationToken,
    intent: StopIntent,
    progress: Arc<AtomicU64>,
    meter: SpeedMeter,
}

struct Inner {
    settings: Settings,
    records: Vec<DownloadRecord>,
    active: HashMap<String, Active>,
    storage: Option<StorageInfo>,
    last_storage_notice: Option<Instant>,
    shutting_down: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StorageFolder {
    Root,
    Completed,
    Incomplete,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub completed: FolderUsage,
    /// Partial data in Downloads/ and Incomplete/.
    pub partial: FolderUsage,
}

pub struct Manager {
    inner: Mutex<Inner>,
    persist_lock: Mutex<()>,
    finalize_lock: Mutex<()>,
    data_dir: PathBuf,
    sink: Arc<dyn EventSink>,
    client: reqwest::Client,
    runtime: Handle,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Manager {
    /// Loads persisted state and starts background work. Must be called from
    /// within a Tokio runtime.
    pub fn load(data_dir: PathBuf, sink: Arc<dyn EventSink>) -> Arc<Self> {
        let settings = load_json::<Settings>(&data_dir.join(SETTINGS_FILE)).unwrap_or_default().sanitized();
        let mut records: Vec<DownloadRecord> = load_json(&data_dir.join(DOWNLOADS_FILE)).unwrap_or_default();

        // Anything that was running when the app closed is now interrupted.
        for rec in &mut records {
            rec.retry = None;
            if rec.status.is_running() || rec.status == Status::Queued {
                rec.status = if settings.auto_start { Status::Queued } else { Status::Paused };
            }
            if rec.status == Status::Paused {
                let _ = rec.layout().move_part(&rec.part_name, false);
            }
        }

        let manager = Arc::new(Self {
            inner: Mutex::new(Inner {
                settings,
                records,
                active: HashMap::new(),
                storage: None,
                last_storage_notice: None,
                shutting_down: false,
            }),
            persist_lock: Mutex::new(()),
            finalize_lock: Mutex::new(()),
            data_dir,
            sink,
            client: http::build_client(),
            runtime: Handle::current(),
        });
        manager.persist();
        manager.runtime.spawn(manager.clone().ticker());
        manager.schedule();
        manager
    }

    fn state(&self) -> MutexGuard<'_, Inner> {
        lock(&self.inner)
    }

    // ---- Queries ---------------------------------------------------------

    pub fn settings(&self) -> Settings {
        self.state().settings.clone()
    }

    pub fn list(&self) -> Vec<DownloadRecord> {
        self.state().records.clone()
    }

    pub fn get(&self, id: &str) -> Option<DownloadRecord> {
        self.state().records.iter().find(|r| r.id == id).cloned()
    }

    pub fn storage_info(&self) -> StorageInfo {
        storage::storage_info(&self.settings().storage_root)
    }

    pub async fn storage_usage(&self) -> StorageUsage {
        let layout = Layout::new(self.settings().storage_root);
        tokio::task::spawn_blocking(move || {
            let mut partial = storage::folder_usage(&layout.downloads());
            let parked = storage::folder_usage(&layout.incomplete());
            partial.files += parked.files;
            partial.bytes += parked.bytes;
            StorageUsage { completed: storage::folder_usage(&layout.completed()), partial }
        })
        .await
        .unwrap_or_else(|_| StorageUsage { completed: FolderUsage::default(), partial: FolderUsage::default() })
    }

    pub fn folder_path(&self, folder: StorageFolder) -> AppResult<PathBuf> {
        let layout = Layout::new(self.settings().storage_root);
        let path = match folder {
            StorageFolder::Root => layout.root().to_path_buf(),
            StorageFolder::Completed => layout.completed(),
            StorageFolder::Incomplete => layout.incomplete(),
        };
        if path.is_dir() {
            Ok(path)
        } else {
            Err(AppError::storage_unavailable(&path))
        }
    }

    /// The file (or partial file) to highlight in Explorer.
    pub fn reveal_path(&self, id: &str) -> AppResult<PathBuf> {
        let rec = self.get(id).ok_or_else(unknown_download)?;
        if let Some(path) = rec.final_path.as_ref().filter(|p| p.is_file()) {
            return Ok(path.clone());
        }
        if rec.status == Status::Completed {
            return Err(AppError::new(
                ErrorCode::Io,
                "The file is no longer in the Completed folder. It may have been moved or deleted.",
            ));
        }
        let layout = rec.layout();
        layout
            .find_part(&rec.part_name)
            .or_else(|| Some(layout.incomplete()).filter(|p| p.is_dir()))
            .ok_or_else(|| AppError::storage_unavailable(layout.root()))
    }

    // ---- Settings & storage ---------------------------------------------

    pub fn update_settings(self: &Arc<Self>, mut new: Settings) -> AppResult<Settings> {
        new.validate()?;
        new.rawg_api_key = new.rawg_api_key.trim().to_string();
        if new.storage_root != self.settings().storage_root {
            storage::prepare_root(&new.storage_root)?;
        }
        save_json(&self.data_dir.join(SETTINGS_FILE), &new).map_err(|e| AppError::io("save settings", &e))?;
        self.state().settings = new.clone();
        self.check_storage();
        self.schedule();
        Ok(new)
    }

    pub fn create_storage_root(&self) -> AppResult<StorageInfo> {
        let root = self.settings().storage_root;
        storage::prepare_root(&root)?;
        self.check_storage();
        Ok(storage::storage_info(&root))
    }

    // ---- Download actions -------------------------------------------------

    pub async fn add(self: &Arc<Self>, url: &str, checksum_text: Option<&str>) -> AppResult<DownloadRecord> {
        let url = http::validate_url(url)?;
        let checksum = checksum::parse(checksum_text.unwrap_or_default())?;
        self.ensure_not_duplicate(url.as_str())?;

        let settings = self.settings();
        let layout = Layout::new(&settings.storage_root);
        if !layout.is_available() {
            return Err(AppError::storage_unavailable(layout.root()));
        }
        storage::prepare_root(layout.root())?;

        let probe = http::probe(&self.client, &url).await?;
        if let Some(total) = probe.total_size {
            let free = storage::available_space(layout.root())?;
            if total > free {
                return Err(storage::insufficient_space(total, free));
            }
        }

        let id = uuid::Uuid::new_v4().simple().to_string();
        let segmentable = probe.supports_range && probe.total_size.is_some_and(|t| t >= MIN_SEGMENTED_SIZE);
        let record = DownloadRecord {
            part_name: format!("{}.{}.part", probe.file_name, &id[..8]),
            id,
            url: url.to_string(),
            file_name: probe.file_name,
            root: settings.storage_root,
            total_size: probe.total_size,
            downloaded: 0,
            supports_range: probe.supports_range,
            connections: if segmentable { settings.connections } else { 1 },
            validator: probe.validator,
            status: Status::Queued,
            error: None,
            checksum,
            integrity: Integrity::NotChecked,
            created_at: now_ms(),
            completed_at: None,
            final_path: None,
            retry: None,
        };

        self.ensure_not_duplicate(&record.url)?;
        self.state().records.push(record.clone());
        self.sink.download_updated(&record);
        self.persist();
        self.schedule();
        Ok(self.get(&record.id).unwrap_or(record))
    }

    fn ensure_not_duplicate(&self, url: &str) -> AppResult<()> {
        let duplicate = self
            .state()
            .records
            .iter()
            .any(|r| r.url == url && !matches!(r.status, Status::Completed | Status::Cancelled));
        if duplicate {
            Err(AppError::new(ErrorCode::Duplicate, "This link is already in your downloads."))
        } else {
            Ok(())
        }
    }

    pub fn pause(&self, id: &str) -> AppResult<()> {
        let updated = {
            let mut s = self.state();
            let status = find(&s.records, id)?.status;
            match status {
                Status::Downloading | Status::Retrying => {
                    stop_active(&mut s.active, id, StopIntent::Pause);
                    None
                }
                Status::Queued => set_status(&mut s.records, id, Status::Paused),
                Status::Paused => None,
                Status::Verifying => return Err(busy_verifying()),
                _ => return Err(invalid_state("This download isn't running.")),
            }
        };
        self.publish(updated);
        Ok(())
    }

    pub fn resume(self: &Arc<Self>, id: &str) -> AppResult<()> {
        let updated = {
            let mut s = self.state();
            match find(&s.records, id)?.status {
                Status::Paused => set_status(&mut s.records, id, Status::Queued),
                Status::Queued | Status::Downloading | Status::Retrying | Status::Verifying => None,
                _ => return Err(invalid_state("Only paused downloads can be resumed.")),
            }
        };
        self.publish(updated);
        self.schedule();
        Ok(())
    }

    pub fn retry(self: &Arc<Self>, id: &str) -> AppResult<()> {
        let rec = self.get(id).ok_or_else(unknown_download)?;
        if !matches!(rec.status, Status::Failed | Status::Cancelled) {
            return Err(invalid_state("Only failed or cancelled downloads can be retried."));
        }
        // Data that failed verification (or was discarded) can't be trusted; start over.
        let fresh = rec.status == Status::Cancelled
            || rec.error.as_ref().is_some_and(|e| matches!(e.code, ErrorCode::ChecksumMismatch | ErrorCode::SizeMismatch));
        if fresh {
            rec.layout().delete_partial(id, &rec.part_name);
        }
        self.update(id, |r| {
            r.status = Status::Queued;
            r.error = None;
            r.retry = None;
            r.integrity = Integrity::NotChecked;
            if fresh {
                r.downloaded = 0;
            }
        });
        self.schedule();
        Ok(())
    }

    pub fn cancel(&self, id: &str) -> AppResult<()> {
        let rec = {
            let mut s = self.state();
            let rec = find(&s.records, id)?.clone();
            match rec.status {
                Status::Downloading | Status::Retrying => {
                    stop_active(&mut s.active, id, StopIntent::Cancel);
                    return Ok(());
                }
                Status::Verifying => return Err(busy_verifying()),
                Status::Queued | Status::Paused | Status::Failed => rec,
                _ => return Err(invalid_state("This download can't be cancelled.")),
            }
        };
        rec.layout().delete_partial(id, &rec.part_name);
        self.update(id, mark_cancelled);
        Ok(())
    }

    /// Removes a record. Completed files are only deleted when `delete_file`
    /// is set; partial data of unfinished downloads is always cleaned up.
    pub fn remove(&self, id: &str, delete_file: bool) -> AppResult<()> {
        let rec = self.get(id).ok_or_else(unknown_download)?;
        if rec.status.is_running() {
            return Err(invalid_state("Pause or cancel the download before removing it."));
        }
        let layout = rec.layout();
        match (&rec.final_path, rec.status) {
            (Some(path), Status::Completed) => {
                if delete_file && layout.owns_completed(path) {
                    match std::fs::remove_file(path) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => return Err(AppError::io("delete the file", &e)),
                    }
                }
            }
            _ => layout.delete_partial(id, &rec.part_name),
        }
        self.state().records.retain(|r| r.id != id);
        self.sink.download_removed(id);
        self.persist();
        Ok(())
    }

    pub fn pause_all(&self) {
        let updated: Vec<_> = {
            let mut s = self.state();
            let Inner { records, active, .. } = &mut *s;
            records
                .iter_mut()
                .filter_map(|r| match r.status {
                    Status::Downloading | Status::Retrying => {
                        stop_active(active, &r.id, StopIntent::Pause);
                        None
                    }
                    Status::Queued => {
                        r.status = Status::Paused;
                        Some(r.clone())
                    }
                    _ => None,
                })
                .collect()
        };
        self.publish_many(updated);
    }

    pub fn resume_all(self: &Arc<Self>) {
        let updated: Vec<_> = {
            let mut s = self.state();
            s.records
                .iter_mut()
                .filter(|r| r.status == Status::Paused)
                .map(|r| {
                    r.status = Status::Queued;
                    r.error = None;
                    r.clone()
                })
                .collect()
        };
        self.publish_many(updated);
        self.schedule();
    }

    /// Stops all transfers so partial data and manifests are saved, waiting up
    /// to `timeout`. Statuses are kept so the next launch sees them as interrupted.
    pub async fn shutdown(&self, timeout: Duration) {
        {
            let mut s = self.state();
            s.shutting_down = true;
            let ids: Vec<String> = s.active.keys().cloned().collect();
            for id in ids {
                stop_active(&mut s.active, &id, StopIntent::Shutdown);
            }
        }
        let deadline = Instant::now() + timeout;
        while !self.state().active.is_empty() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        self.persist();
    }

    // ---- Scheduling and transfer lifecycle --------------------------------

    pub fn schedule(self: &Arc<Self>) {
        let started: Vec<(DownloadRecord, CancellationToken, Arc<AtomicU64>)> = {
            let mut s = self.state();
            if s.shutting_down {
                return;
            }
            let mut slots = usize::from(s.settings.max_simultaneous).saturating_sub(s.active.len());
            let Inner { records, active, .. } = &mut *s;
            let mut started = Vec::new();
            for rec in records.iter_mut() {
                if slots == 0 {
                    break;
                }
                if rec.status != Status::Queued || active.contains_key(&rec.id) {
                    continue;
                }
                rec.status = Status::Downloading;
                rec.error = None;
                rec.retry = None;
                let entry = Active {
                    cancel: CancellationToken::new(),
                    intent: StopIntent::None,
                    progress: Arc::new(AtomicU64::new(rec.downloaded)),
                    meter: SpeedMeter::default(),
                };
                started.push((rec.clone(), entry.cancel.clone(), entry.progress.clone()));
                active.insert(rec.id.clone(), entry);
                slots -= 1;
            }
            started
        };
        if started.is_empty() {
            return;
        }
        for (rec, cancel, progress) in started {
            self.sink.download_updated(&rec);
            self.runtime.spawn(self.clone().drive(rec.id, cancel, progress));
        }
        self.persist();
    }

    async fn drive(self: Arc<Self>, id: String, cancel: CancellationToken, progress: Arc<AtomicU64>) {
        let mut result = self.transfer_with_retries(&id, &cancel, &progress).await;
        if result.is_ok() && !cancel.is_cancelled() {
            result = self.finalize(&id).await;
        }
        let intent = self.state().active.remove(&id).map_or(StopIntent::None, |a| a.intent);
        self.settle(&id, intent, result, progress.load(Ordering::Acquire));
        self.schedule();
    }

    async fn transfer_with_retries(
        &self,
        id: &str,
        cancel: &CancellationToken,
        progress: &Arc<AtomicU64>,
    ) -> AppResult<()> {
        let mut attempt = 0u32;
        let mut best = progress.load(Ordering::Acquire);
        let mut restarted = false;
        loop {
            let result = match self.prepare_transfer(id, cancel, progress) {
                Ok(transfer) => engine::run(&transfer).await,
                Err(e) => Err(e),
            };
            if cancel.is_cancelled() {
                return Ok(());
            }
            let err = match result {
                Ok(()) => return Ok(()),
                Err(e) => self.classify(id, e),
            };

            if err.code == ErrorCode::RangeNotHonored && !restarted {
                // The server stopped honouring ranges or the file changed: start over with fresh metadata.
                restarted = true;
                self.restart_from_scratch(id).await?;
                progress.store(0, Ordering::Release);
                best = 0;
                continue;
            }
            if !err.is_retryable() {
                return Err(err);
            }

            let reached = progress.load(Ordering::Acquire);
            if reached > best {
                // Progress was made since the last failure, so this is a fresh problem.
                best = reached;
                attempt = 0;
            }
            attempt += 1;
            if attempt >= MAX_ATTEMPTS {
                return Err(err);
            }
            let delay = Duration::from_secs((1u64 << attempt).min(30));
            self.update(id, |r| {
                r.status = Status::Retrying;
                r.error = Some(err.clone());
                r.retry = Some(RetryInfo {
                    attempt: attempt + 1,
                    max_attempts: MAX_ATTEMPTS,
                    retry_at: now_ms() + delay.as_millis() as u64,
                });
            });
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                _ = tokio::time::sleep(delay) => {}
            }
            self.update(id, |r| {
                r.status = Status::Downloading;
                r.error = None;
                r.retry = None;
            });
        }
    }

    fn prepare_transfer(
        &self,
        id: &str,
        cancel: &CancellationToken,
        progress: &Arc<AtomicU64>,
    ) -> AppResult<Transfer> {
        let rec = self.get(id).ok_or_else(unknown_download)?;
        let layout = rec.layout();
        if !layout.is_available() {
            return Err(AppError::storage_unavailable(layout.root()));
        }
        layout.ensure().map_err(|e| AppError::io("create the storage folders", &e))?;
        layout.move_part(&rec.part_name, true).map_err(|e| AppError::io("move the partial download", &e))?;

        if let Some(total) = rec.total_size {
            let needed = total.saturating_sub(rec.downloaded);
            let free = storage::available_space(layout.root())?;
            if needed > free {
                return Err(storage::insufficient_space(needed, free));
            }
        }

        Ok(Transfer {
            client: self.client.clone(),
            url: rec.url.clone(),
            part_path: layout.active_part(&rec.part_name),
            manifest_path: layout.manifest(id),
            total: rec.total_size,
            supports_range: rec.supports_range,
            validator: rec.validator.clone(),
            connections: rec.connections,
            progress: progress.clone(),
            cancel: cancel.clone(),
        })
    }

    /// Write failures on a vanished drive become a clear "storage unavailable".
    fn classify(&self, id: &str, err: AppError) -> AppError {
        match self.get(id) {
            Some(rec) if err.code == ErrorCode::Io && !rec.layout().is_available() => {
                let mut e = AppError::storage_unavailable(&rec.root);
                e.details = err.details.or(e.details);
                e
            }
            _ => err,
        }
    }

    async fn restart_from_scratch(&self, id: &str) -> AppResult<()> {
        let rec = self.get(id).ok_or_else(unknown_download)?;
        rec.layout().delete_partial(id, &rec.part_name);
        let url = http::validate_url(&rec.url)?;
        let probe = http::probe(&self.client, &url).await?;
        let connections = self.settings().connections;
        self.update(id, |r| {
            r.total_size = probe.total_size;
            r.supports_range = probe.supports_range;
            r.validator = probe.validator;
            r.downloaded = 0;
            let segmentable = probe.supports_range && probe.total_size.is_some_and(|t| t >= MIN_SEGMENTED_SIZE);
            r.connections = if segmentable { connections } else { 1 };
        });
        Ok(())
    }

    async fn finalize(&self, id: &str) -> AppResult<()> {
        let rec = self.get(id).ok_or_else(unknown_download)?;
        let layout = rec.layout();
        let part = layout.active_part(&rec.part_name);

        let mut integrity = Integrity::NotChecked;
        if let Some(expected) = rec.checksum.clone() {
            self.update(id, |r| r.status = Status::Verifying);
            let path = part.clone();
            let algorithm = expected.algorithm;
            let actual = tokio::task::spawn_blocking(move || checksum::hash_file(&path, algorithm))
                .await
                .map_err(|e| AppError::internal(e.to_string()))?
                .map_err(|e| AppError::io("read the file to verify it", &e))?;
            if actual != expected.value {
                self.update(id, |r| r.integrity = Integrity::Failed);
                return Err(AppError::new(
                    ErrorCode::ChecksumMismatch,
                    "Verification failed: the file's checksum doesn't match the expected value. The file may be corrupted; retrying will download it again.",
                )
                .with_details(format!("expected {}, got {actual}", expected.value)));
            }
            integrity = Integrity::Verified;
        }

        let size = std::fs::metadata(&part).map_err(|e| AppError::io("read the downloaded file", &e))?.len();
        let destination = {
            let _guard = lock(&self.finalize_lock);
            let destination = storage::unique_path(&layout.completed(), &rec.file_name);
            std::fs::rename(&part, &destination)
                .map_err(|e| AppError::io("move the file to the Completed folder", &e))?;
            destination
        };
        let _ = std::fs::remove_file(layout.manifest(id));

        let updated = self.update(id, |r| {
            r.status = Status::Completed;
            r.integrity = integrity;
            r.final_path = Some(destination);
            r.completed_at = Some(now_ms());
            r.total_size = Some(size);
            r.downloaded = size;
            r.error = None;
            r.retry = None;
        });
        if let Some(r) = updated {
            if self.settings().notify_completed {
                self.sink.notify("Download completed", &format!("{}\n{}", r.file_name, storage::format_size(size)));
            }
        }
        Ok(())
    }

    /// Applies the outcome of a finished transfer task.
    fn settle(&self, id: &str, intent: StopIntent, result: AppResult<()>, progress: u64) {
        let Some(rec) = self.get(id) else { return };
        if rec.status == Status::Completed {
            return;
        }
        let layout = rec.layout();
        let park = || {
            if let Err(err) = layout.move_part(&rec.part_name, false) {
                eprintln!("manager: could not move {} to Incomplete: {err}", rec.part_name);
            }
        };

        match (intent, result) {
            (StopIntent::Cancel, _) => {
                layout.delete_partial(id, &rec.part_name);
                self.update(id, mark_cancelled);
            }
            (StopIntent::Shutdown, _) => {
                park();
                if let Some(r) = self.state().records.iter_mut().find(|r| r.id == id) {
                    r.downloaded = progress;
                }
            }
            (StopIntent::Pause, _) | (StopIntent::None, Ok(())) => {
                park();
                self.update(id, |r| {
                    r.status = Status::Paused;
                    r.error = None;
                    r.retry = None;
                    r.downloaded = progress;
                });
            }
            (StopIntent::StorageLost, _) => self.pause_for_storage(id, AppError::storage_unavailable(&rec.root), progress),
            (StopIntent::None, Err(err)) if err.code == ErrorCode::StorageUnavailable => {
                self.pause_for_storage(id, err, progress)
            }
            (StopIntent::None, Err(err)) => {
                park();
                let message = err.message.clone();
                self.update(id, |r| {
                    r.status = Status::Failed;
                    r.error = Some(err);
                    r.retry = None;
                    r.downloaded = progress;
                });
                if self.settings().notify_failed {
                    self.sink.notify("Download failed", &format!("{}\n{}", rec.file_name, first_line(&message)));
                }
            }
        }
    }

    fn pause_for_storage(&self, id: &str, err: AppError, progress: u64) {
        self.update(id, |r| {
            r.status = Status::Paused;
            r.error = Some(err);
            r.retry = None;
            r.downloaded = progress;
        });
        let should_notify = {
            let mut s = self.state();
            let recent = s.last_storage_notice.is_some_and(|t| t.elapsed() < STORAGE_NOTICE_COOLDOWN);
            if !recent {
                s.last_storage_notice = Some(Instant::now());
            }
            s.settings.notify_storage && !recent
        };
        if should_notify {
            self.sink.notify("Download paused", "Storage drive unavailable.\nReconnect the drive to continue.");
        }
    }

    // ---- Background ticker -----------------------------------------------

    async fn ticker(self: Arc<Self>) {
        let mut interval = tokio::time::interval(TICK);
        let mut tick: u32 = 0;
        loop {
            interval.tick().await;
            if self.state().shutting_down {
                break;
            }
            tick = tick.wrapping_add(1);
            let any_active = self.emit_progress();
            if any_active && tick % PERSIST_EVERY_TICKS == 0 {
                self.persist();
            }
            if tick % STORAGE_EVERY_TICKS == 0 {
                self.check_storage();
            }
        }
    }

    fn emit_progress(&self) -> bool {
        let items: Vec<ProgressItem> = {
            let mut s = self.state();
            let now = Instant::now();
            let Inner { records, active, .. } = &mut *s;
            active
                .iter_mut()
                .filter_map(|(id, a)| {
                    let rec = records.iter_mut().find(|r| &r.id == id)?;
                    let downloaded = a.progress.load(Ordering::Acquire);
                    a.meter.record(now, downloaded);
                    let speed = if rec.status == Status::Downloading { a.meter.speed() } else { 0.0 };
                    rec.downloaded = downloaded;
                    let eta = rec
                        .total_size
                        .filter(|_| speed > 1.0)
                        .map(|t| (t.saturating_sub(downloaded) as f64 / speed).ceil() as u64);
                    Some(ProgressItem { id: id.clone(), downloaded, total: rec.total_size, speed, eta })
                })
                .collect()
        };
        if !items.is_empty() {
            self.sink.progress(&items);
        }
        !items.is_empty()
    }

    /// Refreshes drive statistics and pauses transfers whose drive disappeared.
    pub fn check_storage(&self) {
        let root = self.settings().storage_root;
        let info = storage::storage_info(&root);
        let active_roots: Vec<(String, PathBuf)> = {
            let s = self.state();
            s.records
                .iter()
                .filter(|r| s.active.contains_key(&r.id) && r.status != Status::Verifying)
                .map(|r| (r.id.clone(), r.root.clone()))
                .collect()
        };
        let lost: Vec<String> = active_roots
            .into_iter()
            .filter(|(_, root)| !root.is_dir())
            .map(|(id, _)| id)
            .collect();

        let changed = {
            let mut s = self.state();
            for id in &lost {
                stop_active(&mut s.active, id, StopIntent::StorageLost);
            }
            let changed = s.storage.as_ref() != Some(&info);
            s.storage = Some(info.clone());
            changed
        };
        if changed {
            self.sink.storage_changed(&info);
        }
    }

    // ---- Helpers ---------------------------------------------------------

    fn update(&self, id: &str, f: impl FnOnce(&mut DownloadRecord)) -> Option<DownloadRecord> {
        let rec = {
            let mut s = self.state();
            let rec = s.records.iter_mut().find(|r| r.id == id)?;
            f(rec);
            rec.clone()
        };
        self.sink.download_updated(&rec);
        self.persist();
        Some(rec)
    }

    fn publish(&self, record: Option<DownloadRecord>) {
        if let Some(rec) = record {
            self.sink.download_updated(&rec);
            self.persist();
        }
    }

    fn publish_many(&self, records: Vec<DownloadRecord>) {
        if records.is_empty() {
            return;
        }
        for rec in &records {
            self.sink.download_updated(rec);
        }
        self.persist();
    }

    fn persist(&self) {
        let _guard = lock(&self.persist_lock);
        let records = self.state().records.clone();
        if let Err(err) = save_json(&self.data_dir.join(DOWNLOADS_FILE), &records) {
            eprintln!("manager: failed to save downloads: {err}");
        }
    }
}

fn find<'a>(records: &'a [DownloadRecord], id: &str) -> AppResult<&'a DownloadRecord> {
    records.iter().find(|r| r.id == id).ok_or_else(unknown_download)
}

fn set_status(records: &mut [DownloadRecord], id: &str, status: Status) -> Option<DownloadRecord> {
    let rec = records.iter_mut().find(|r| r.id == id)?;
    rec.status = status;
    rec.error = None;
    rec.retry = None;
    Some(rec.clone())
}

fn stop_active(active: &mut HashMap<String, Active>, id: &str, intent: StopIntent) {
    if let Some(a) = active.get_mut(id) {
        // An explicit cancel wins over a pause that is still being processed.
        if a.intent != StopIntent::Cancel {
            a.intent = intent;
        }
        a.cancel.cancel();
    }
}

fn mark_cancelled(r: &mut DownloadRecord) {
    r.status = Status::Cancelled;
    r.downloaded = 0;
    r.error = None;
    r.retry = None;
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}

fn unknown_download() -> AppError {
    AppError::new(ErrorCode::UnknownDownload, "That download no longer exists.")
}

fn invalid_state(message: &str) -> AppError {
    AppError::new(ErrorCode::InvalidState, message)
}

fn busy_verifying() -> AppError {
    invalid_state("The file is being verified. Please wait a moment.")
}
