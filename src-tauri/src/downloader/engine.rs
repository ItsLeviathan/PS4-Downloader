//! Transfers one file: prepares/validates the partial file, runs one task per
//! segment, and keeps the resume manifest up to date.
//!
//! Invariant: a segment's `done` counter only advances after its bytes were
//! handed to the OS, so the manifest never claims data that isn't in the file.

use super::http::{self, parse_content_range};
use super::manifest::Manifest;
use crate::error::{AppError, AppResult, ErrorCode};
use futures_util::StreamExt;
use reqwest::header::{CONTENT_RANGE, IF_RANGE, RANGE};
use reqwest::{Client, StatusCode};
use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

/// Received bytes are batched before writing to keep syscalls large on HDDs.
const WRITE_BUFFER: usize = 1024 * 1024;
const MAX_BUFFER_AGE: Duration = Duration::from_secs(1);
const MANIFEST_INTERVAL: Duration = Duration::from_secs(2);

pub struct Transfer {
    pub client: Client,
    pub url: String,
    pub part_path: PathBuf,
    pub manifest_path: PathBuf,
    pub total: Option<u64>,
    pub supports_range: bool,
    pub validator: Option<String>,
    pub connections: u8,
    /// Bytes received so far (including data still in write buffers). Read by the UI ticker.
    pub progress: Arc<AtomicU64>,
    pub cancel: CancellationToken,
}

/// Runs the transfer until it completes, fails, or `cancel` fires. Returns
/// `Ok(())` both on completion and on cancellation; callers check the token.
pub async fn run(t: &Transfer) -> AppResult<()> {
    let mut manifest = prepare(t).await?;
    t.progress.store(manifest.downloaded(), Ordering::Release);

    let counters: Vec<Arc<AtomicU64>> =
        manifest.segments.iter().map(|s| Arc::new(AtomicU64::new(s.done))).collect();
    let stop = t.cancel.child_token();
    let whole_file = !manifest.is_segmented();

    let mut tasks = JoinSet::new();
    for (seg, done) in manifest.segments.iter().zip(&counters) {
        if seg.is_complete() {
            continue;
        }
        tasks.spawn(fetch_segment(SegmentJob {
            client: t.client.clone(),
            url: t.url.clone(),
            path: t.part_path.clone(),
            start: seg.start,
            end: seg.end,
            done: done.clone(),
            progress: t.progress.clone(),
            ranged: t.supports_range,
            validator: t.validator.clone(),
            whole_file,
            cancel: stop.clone(),
        }));
    }

    let mut first_error = None;
    let mut ticker = tokio::time::interval(MANIFEST_INTERVAL);
    ticker.tick().await;
    loop {
        tokio::select! {
            joined = tasks.join_next() => {
                let result = match joined {
                    None => break,
                    Some(Ok(result)) => result,
                    Some(Err(join_err)) => Err(AppError::internal(join_err.to_string())),
                };
                if let Err(err) = result {
                    if first_error.is_none() {
                        first_error = Some(err);
                        stop.cancel();
                    }
                }
            }
            _ = ticker.tick() => {
                sync_counters(&mut manifest, &counters);
                // A failed save is not fatal here; the next one or the final save will report it.
                let _ = manifest.save(&t.manifest_path);
            }
        }
    }

    sync_counters(&mut manifest, &counters);
    let saved = manifest.save(&t.manifest_path).map_err(|e| AppError::io("save download progress", &e));
    if let Some(err) = first_error {
        return Err(err);
    }
    saved?;
    if t.cancel.is_cancelled() {
        return Ok(());
    }

    finish(t, &manifest).await
}

/// Verifies the file is complete and flushes it to disk.
async fn finish(t: &Transfer, manifest: &Manifest) -> AppResult<()> {
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&t.part_path)
        .await
        .map_err(|e| AppError::io("open the downloaded file", &e))?;
    file.sync_all().await.map_err(|e| AppError::io("flush the downloaded file to disk", &e))?;
    let len = file.metadata().await.map_err(|e| AppError::io("read the downloaded file", &e))?.len();

    let incomplete = manifest.segments.iter().any(|s| s.end.is_some() && !s.is_complete());
    let expected = manifest.total.unwrap_or_else(|| manifest.downloaded());
    if incomplete || len != expected {
        return Err(AppError::new(
            ErrorCode::SizeMismatch,
            "The downloaded file is not the size the server reported, so it was not marked as complete.",
        )
        .with_details(format!("expected {expected} bytes, file has {len} bytes")));
    }
    Ok(())
}

fn sync_counters(manifest: &mut Manifest, counters: &[Arc<AtomicU64>]) {
    for (seg, done) in manifest.segments.iter_mut().zip(counters) {
        seg.done = done.load(Ordering::Acquire);
    }
}

/// Loads and validates existing partial state, or starts a fresh file.
async fn prepare(t: &Transfer) -> AppResult<Manifest> {
    let existing = Manifest::load(&t.manifest_path);
    let file_len = tokio::fs::metadata(&t.part_path).await.ok().map(|m| m.len());

    if let (Some(mut manifest), Some(len)) = (existing, file_len) {
        let same_file = manifest.total == t.total && manifest.validator == t.validator;
        if same_file && !manifest.is_segmented() {
            // Sequential download: keep what's on disk only if we can ask for the rest.
            let seg = &mut manifest.segments[0];
            seg.done = if t.supports_range { seg.done.min(len) } else { 0 };
            set_file_len(t, seg.done).await?;
            return Ok(manifest);
        }
        if same_file && Some(len) == manifest.total {
            return Ok(manifest);
        }
    }

    let mut manifest = Manifest::plan(t.total, t.supports_range, t.connections, t.validator.clone());
    let file = tokio::fs::File::create(&t.part_path)
        .await
        .map_err(|e| AppError::io("create the download file", &e))?
        .into_std()
        .await;
    if let Some(total) = manifest.total.filter(|_| manifest.is_segmented()) {
        let prepared = tokio::task::spawn_blocking(move || {
            crate::storage::make_sparse(&file)?;
            file.set_len(total)
        })
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
        if let Err(err) = prepared {
            // Sparse files are unsupported on e.g. FAT32/exFAT; fall back to one sequential stream.
            eprintln!("engine: segmented download unavailable ({err}); using one connection");
            manifest = Manifest::single(t.total, t.validator.clone());
            set_file_len(t, 0).await?;
        }
    }
    manifest.save(&t.manifest_path).map_err(|e| AppError::io("save download progress", &e))?;
    Ok(manifest)
}

async fn set_file_len(t: &Transfer, len: u64) -> AppResult<()> {
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&t.part_path)
        .await
        .map_err(|e| AppError::io("open the partial download", &e))?;
    file.set_len(len).await.map_err(|e| AppError::io("prepare the partial download", &e))
}

struct SegmentJob {
    client: Client,
    url: String,
    path: PathBuf,
    start: u64,
    end: Option<u64>,
    done: Arc<AtomicU64>,
    progress: Arc<AtomicU64>,
    ranged: bool,
    validator: Option<String>,
    /// This single segment spans the whole file, so a full `200` response is acceptable.
    whole_file: bool,
    cancel: CancellationToken,
}

pub fn range_not_honored() -> AppError {
    AppError::new(
        ErrorCode::RangeNotHonored,
        "The server no longer supports resuming this file, so it has to be downloaded from the start.",
    )
}

async fn fetch_segment(job: SegmentJob) -> AppResult<()> {
    let offset = job.start + job.done.load(Ordering::Acquire);
    let limit = job.end.map(|e| e + 1);
    if limit.is_some_and(|l| offset >= l) {
        return Ok(());
    }

    let mut request = job.client.get(&job.url);
    let send_range = job.ranged && (offset > 0 || !job.whole_file);
    if send_range {
        let range = match job.end {
            Some(end) => format!("bytes={offset}-{end}"),
            None => format!("bytes={offset}-"),
        };
        request = request.header(RANGE, range);
        if let Some(v) = job.validator.as_deref().filter(|_| offset > 0) {
            // If the file changed on the server, it answers 200 with the full new file.
            request = request.header(IF_RANGE, v);
        }
    }

    let response = tokio::select! {
        _ = job.cancel.cancelled() => return Ok(()),
        r = request.send() => r.map_err(|e| http::from_reqwest(&e))?,
    };
    let status = response.status();
    match status {
        StatusCode::PARTIAL_CONTENT if send_range => {
            let start = response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(parse_content_range)
                .map(|r| r.start);
            if start != Some(offset) {
                return Err(range_not_honored());
            }
        }
        StatusCode::OK if !send_range || (offset == 0 && job.whole_file) => {}
        StatusCode::OK => return Err(range_not_honored()),
        s if !s.is_success() => return Err(http::status_error(s)),
        s => {
            return Err(AppError::new(ErrorCode::HttpStatus, "The server sent an unexpected response.")
                .with_details(format!("HTTP {s}")))
        }
    }

    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&job.path)
        .await
        .map_err(|e| AppError::io("open the download file", &e))?;
    file.seek(SeekFrom::Start(offset)).await.map_err(|e| AppError::io("write to the download file", &e))?;

    let mut writer = SegmentWriter { file, buf: Vec::with_capacity(WRITE_BUFFER), done: job.done.clone(), position: offset };
    let mut stream = response.bytes_stream();
    let mut last_flush = Instant::now();

    let outcome: AppResult<()> = loop {
        let next = tokio::select! {
            biased;
            _ = job.cancel.cancelled() => break Ok(()),
            next = stream.next() => next,
        };
        match next {
            None => {
                break match limit {
                    Some(l) if writer.pending_end() < l => Err(AppError::new(
                        ErrorCode::Network,
                        "The connection closed before the download finished.",
                    )
                    .with_details(format!("segment stopped at byte {} of {}", writer.pending_end(), l))),
                    _ => Ok(()),
                };
            }
            Some(Err(err)) => break Err(http::from_reqwest(&err)),
            Some(Ok(chunk)) => {
                let mut data = &chunk[..];
                if let Some(l) = limit {
                    // Never write past this segment, even if the server sends extra bytes.
                    let room = l.saturating_sub(writer.pending_end());
                    data = &data[..data.len().min(usize::try_from(room).unwrap_or(usize::MAX))];
                }
                writer.buf.extend_from_slice(data);
                job.progress.fetch_add(data.len() as u64, Ordering::AcqRel);

                if writer.buf.len() >= WRITE_BUFFER || last_flush.elapsed() >= MAX_BUFFER_AGE {
                    if let Err(e) = writer.flush().await {
                        break Err(e);
                    }
                    last_flush = Instant::now();
                }
                if limit.is_some_and(|l| writer.pending_end() >= l) {
                    break Ok(());
                }
            }
        }
    };

    // Persist whatever arrived before stopping; a write failure (e.g. the drive
    // was unplugged) is the more useful error to report.
    let flushed = writer.flush().await;
    flushed.and(outcome)
}

struct SegmentWriter {
    file: tokio::fs::File,
    buf: Vec<u8>,
    done: Arc<AtomicU64>,
    /// File offset just past the last byte written to the OS.
    position: u64,
}

impl SegmentWriter {
    fn pending_end(&self) -> u64 {
        self.position + self.buf.len() as u64
    }

    async fn flush(&mut self) -> AppResult<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let write = async {
            self.file.write_all(&self.buf).await?;
            // tokio's File completes writes in the background; flush waits for them.
            self.file.flush().await
        };
        write.await.map_err(|e| AppError::io("write to the download file", &e))?;
        let n = self.buf.len() as u64;
        self.position += n;
        self.done.fetch_add(n, Ordering::AcqRel);
        self.buf.clear();
        Ok(())
    }
}
