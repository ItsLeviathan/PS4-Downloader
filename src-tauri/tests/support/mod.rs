//! A minimal HTTP/1.1 server with configurable range support, plus a harness
//! that runs a `Manager` against a throwaway storage folder.

use ps4_downloader_lib::downloader::manager::Manager;
use ps4_downloader_lib::downloader::{DownloadRecord, ProgressItem, Status};
use ps4_downloader_lib::events::EventSink;
use ps4_downloader_lib::storage::{Layout, StorageInfo};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Deterministic file contents so downloads can be verified byte for byte.
pub fn byte_at(i: u64) -> u8 {
    (i % 251) as u8 ^ ((i / 251) % 256) as u8
}

pub fn body(start: u64, end_exclusive: u64) -> Vec<u8> {
    (start..end_exclusive).map(byte_at).collect()
}

pub fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

#[derive(Default)]
pub struct ServerState {
    pub max_range_start: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub etag_version: AtomicU64,
    pub requests: AtomicU64,
    flaky_tripped: AtomicBool,
}

pub struct TestServer {
    addr: SocketAddr,
    pub state: Arc<ServerState>,
}

impl TestServer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(ServerState::default());
        let shared = state.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(handle(stream, shared.clone()));
            }
        });
        Self { addr, state }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }
}

struct Request {
    path: String,
    headers: HashMap<String, String>,
}

impl Request {
    /// `Range: bytes=a-b` or `bytes=a-`.
    fn range(&self) -> Option<(u64, Option<u64>)> {
        let spec = self.headers.get("range")?.strip_prefix("bytes=")?;
        let (a, b) = spec.split_once('-')?;
        Some((a.parse().ok()?, b.parse().ok()))
    }

    fn is_probe(&self) -> bool {
        self.range() == Some((0, Some(0)))
    }
}

async fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    while !data.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut buf).await.ok()?;
        if n == 0 {
            return None;
        }
        data.extend_from_slice(&buf[..n]);
    }
    let text = String::from_utf8_lossy(&data);
    let mut lines = text.split("\r\n");
    let path = lines.next()?.split_whitespace().nth(1)?.to_string();
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    Some(Request { path, headers })
}

#[derive(Default)]
struct Serve {
    ranges: bool,
    delay_ms: u64,
    disposition: Option<&'static str>,
    /// Stop after this many body bytes (simulates a dropped connection).
    cut_after: Option<u64>,
}

async fn handle(mut stream: TcpStream, state: Arc<ServerState>) {
    let Some(req) = read_request(&mut stream).await else { return };
    state.requests.fetch_add(1, Ordering::SeqCst);
    let path = req.path.split('?').next().unwrap_or_default().to_string();
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let num = |i: usize| parts.get(i).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);

    match parts[0] {
        "range" => serve(stream, &state, &req, num(1), Serve { ranges: true, ..Default::default() }).await,
        "norange" => serve(stream, &state, &req, num(1), Serve::default()).await,
        "slow" => {
            let opts = Serve { ranges: true, delay_ms: num(2), ..Default::default() };
            serve(stream, &state, &req, num(1), opts).await
        }
        "cd" => {
            let opts = Serve {
                ranges: true,
                disposition: Some("attachment; filename=\"..\\..\\Windows\\evil?.exe\""),
                ..Default::default()
            };
            serve(stream, &state, &req, num(1), opts).await
        }
        "flaky" => {
            let size = num(1);
            let cut = !req.is_probe() && !state.flaky_tripped.swap(true, Ordering::SeqCst);
            let opts = Serve { ranges: true, cut_after: cut.then_some(size / 2), ..Default::default() };
            serve(stream, &state, &req, size, opts).await
        }
        "probe-only" if req.is_probe() => {
            serve(stream, &state, &req, num(1), Serve { ranges: true, ..Default::default() }).await
        }
        "probe-only" => respond(stream, "503 Service Unavailable", &[], b"busy").await,
        "redirect" => {
            let location = format!("Location: /{}", parts[1..].join("/"));
            respond(stream, "302 Found", &[&location], b"").await
        }
        "redirect-ftp" => respond(stream, "302 Found", &["Location: ftp://127.0.0.1/file.bin"], b"").await,
        "status" => {
            let line = match num(1) {
                401 => "401 Unauthorized",
                403 => "403 Forbidden",
                _ => "404 Not Found",
            };
            respond(stream, line, &[], b"no").await
        }
        "html" => respond(stream, "200 OK", &["Content-Type: text/html; charset=utf-8"], b"<html></html>").await,
        _ => respond(stream, "404 Not Found", &[], b"").await,
    }
}

async fn respond(mut stream: TcpStream, status: &str, headers: &[&str], body: &[u8]) {
    let mut head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
    for h in headers {
        head.push_str(h);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(body).await;
    let _ = stream.shutdown().await;
}

async fn serve(mut stream: TcpStream, state: &ServerState, req: &Request, size: u64, opts: Serve) {
    let etag = format!("\"v{}\"", state.etag_version.load(Ordering::SeqCst));
    let if_range_ok = req.headers.get("if-range").is_none_or(|v| *v == etag);
    let range = req.range().filter(|_| opts.ranges && if_range_ok);

    let (status, start, end_exclusive) = match range {
        Some((a, _)) if a >= size => {
            let cr = format!("Content-Range: bytes */{size}");
            return respond(stream, "416 Range Not Satisfiable", &[&cr], b"").await;
        }
        Some((a, b)) => {
            state.max_range_start.fetch_max(a, Ordering::SeqCst);
            ("206 Partial Content", a, b.map_or(size, |b| (b + 1).min(size)))
        }
        None => ("200 OK", 0, size),
    };

    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n",
        end_exclusive - start
    );
    if opts.ranges {
        head.push_str(&format!("Accept-Ranges: bytes\r\nETag: {etag}\r\n"));
    }
    if status.starts_with("206") {
        head.push_str(&format!("Content-Range: bytes {start}-{}/{size}\r\n", end_exclusive - 1));
    }
    if let Some(cd) = opts.disposition {
        head.push_str(&format!("Content-Disposition: {cd}\r\n"));
    }
    head.push_str("\r\n");
    if stream.write_all(head.as_bytes()).await.is_err() {
        return;
    }

    let stop_at = opts.cut_after.map_or(end_exclusive, |c| (start + c).min(end_exclusive));
    let mut pos = start;
    while pos < stop_at {
        let next = (pos + 64 * 1024).min(stop_at);
        if stream.write_all(&body(pos, next)).await.is_err() {
            return;
        }
        state.bytes_sent.fetch_add(next - pos, Ordering::SeqCst);
        pos = next;
        if opts.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(opts.delay_ms)).await;
        }
    }
    let _ = stream.shutdown().await;
}

#[derive(Default)]
pub struct TestSink {
    statuses: Mutex<Vec<(String, Status)>>,
}

impl TestSink {
    pub fn saw_status(&self, id: &str, status: Status) -> bool {
        self.statuses.lock().unwrap().iter().any(|(i, s)| i == id && *s == status)
    }
}

impl EventSink for TestSink {
    fn download_updated(&self, record: &DownloadRecord) {
        self.statuses.lock().unwrap().push((record.id.clone(), record.status));
    }
    fn download_removed(&self, _id: &str) {}
    fn progress(&self, _items: &[ProgressItem]) {}
    fn storage_changed(&self, _info: &StorageInfo) {}
    fn notify(&self, _title: &str, _body: &str) {}
}

pub struct Harness {
    pub manager: Arc<Manager>,
    pub sink: Arc<TestSink>,
    pub data_dir: PathBuf,
    pub root: PathBuf,
}

impl Harness {
    pub async fn new(connections: u8) -> Self {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("it-{}", uuid::Uuid::new_v4().simple()));
        let data_dir = base.join("app-data");
        let root = base.join("storage");
        std::fs::create_dir_all(&root).unwrap();
        let sink = Arc::new(TestSink::default());
        let manager = Manager::load(data_dir.clone(), sink.clone());
        let mut settings = manager.settings();
        settings.storage_root = root.clone();
        settings.connections = connections;
        settings.max_simultaneous = 3;
        manager.update_settings(settings).unwrap();
        Self { manager, sink, data_dir, root }
    }

    pub fn with_manager(&self, manager: Arc<Manager>) -> Self {
        Self { manager, sink: self.sink.clone(), data_dir: self.data_dir.clone(), root: self.root.clone() }
    }

    pub fn layout(&self) -> Layout {
        Layout::new(&self.root)
    }

    pub fn set_max_simultaneous(&self, n: u8) {
        let mut s = self.manager.settings();
        s.max_simultaneous = n;
        self.manager.update_settings(s).unwrap();
    }

    pub async fn wait_until(&self, id: &str, secs: u64, pred: impl Fn(&DownloadRecord) -> bool) -> DownloadRecord {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            let rec = self.manager.get(id).expect("record exists");
            if pred(&rec) {
                return rec;
            }
            if Instant::now() > deadline {
                panic!("timed out waiting; record: {rec:#?}");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    pub async fn wait_for(&self, id: &str, status: Status, secs: u64) -> DownloadRecord {
        self.wait_until(id, secs, |r| {
            if r.status == Status::Failed && status != Status::Failed {
                panic!("download failed unexpectedly: {:#?}", r.error);
            }
            r.status == status
        })
        .await
    }

    pub fn assert_completed_file(&self, rec: &DownloadRecord, size: u64) {
        let path = rec.final_path.clone().expect("completed download has a path");
        assert_eq!(path.parent().unwrap(), self.layout().completed());
        let data = std::fs::read(&path).unwrap();
        assert_eq!(data.len() as u64, size, "file size");
        assert!(data == body(0, size), "file contents match the source byte for byte");
        assert_eq!(rec.downloaded, size);
        assert!(self.layout().find_part(&rec.part_name).is_none(), "no leftover partial file");
    }
}
