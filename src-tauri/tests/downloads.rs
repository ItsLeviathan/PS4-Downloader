//! End-to-end tests of the download manager against a local HTTP server.

mod support;

use ps4_downloader_lib::downloader::manager::Manager;
use ps4_downloader_lib::downloader::{Integrity, Status};
use ps4_downloader_lib::error::ErrorCode;
use sha2::Digest;
use std::sync::atomic::Ordering;
use std::time::Duration;
use support::{body, Harness, TestServer};

const MB: u64 = 1024 * 1024;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn segmented_download_with_range_support() {
    let server = TestServer::start().await;
    let h = Harness::new(4).await;
    let size = 40 * MB + 123;
    let rec = h.manager.add(&server.url(&format!("/range/{size}/game.pkg")), None).await.unwrap();
    assert!(rec.supports_range);
    assert_eq!(rec.connections, 4);
    assert_eq!(rec.total_size, Some(size));

    let done = h.wait_for(&rec.id, Status::Completed, 60).await;
    h.assert_completed_file(&done, size);
    assert!(server.state.max_range_start.load(Ordering::SeqCst) > 0, "segments used ranges");
    assert!(!h.layout().manifest(&rec.id).exists(), "manifest cleaned up");
    assert_eq!(std::fs::read_dir(h.layout().downloads()).unwrap().count(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sequential_download_without_range_support() {
    let server = TestServer::start().await;
    let h = Harness::new(4).await;
    let size = 20 * MB;
    let rec = h.manager.add(&server.url(&format!("/norange/{size}/plain.zip")), None).await.unwrap();
    assert!(!rec.supports_range);
    assert_eq!(rec.connections, 1);
    let done = h.wait_for(&rec.id, Status::Completed, 60).await;
    h.assert_completed_file(&done, size);
    assert_eq!(done.file_name, "plain.zip");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn follows_redirects_and_sanitizes_names() {
    let server = TestServer::start().await;
    let h = Harness::new(2).await;

    let rec = h.manager.add(&server.url("/redirect/range/5000/final-name.bin"), None).await.unwrap();
    assert_eq!(rec.file_name, "final-name.bin");
    let done = h.wait_for(&rec.id, Status::Completed, 30).await;
    h.assert_completed_file(&done, 5000);

    let rec = h.manager.add(&server.url("/cd/3000"), None).await.unwrap();
    assert_eq!(rec.file_name, "evil_.exe", "path parts and invalid characters removed");
    let done = h.wait_for(&rec.id, Status::Completed, 30).await;
    let path = done.final_path.clone().unwrap();
    assert_eq!(path.parent().unwrap(), h.layout().completed());
    h.assert_completed_file(&done, 3000);

    // Same name again lands next to the first file instead of overwriting it.
    let rec = h.manager.add(&server.url("/range/10/final-name.bin?again"), None).await.unwrap();
    let done = h.wait_for(&rec.id, Status::Completed, 30).await;
    assert_eq!(done.final_path.unwrap().file_name().unwrap(), "final-name (1).bin");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reports_http_and_url_errors() {
    let server = TestServer::start().await;
    let h = Harness::new(2).await;
    let cases = [
        ("/status/404", ErrorCode::NotFound),
        ("/status/403", ErrorCode::Forbidden),
        ("/status/401", ErrorCode::AuthRequired),
        ("/html", ErrorCode::NotAFile),
    ];
    for (path, code) in cases {
        let err = h.manager.add(&server.url(path), None).await.unwrap_err();
        assert_eq!(err.code, code, "{path}");
        assert!(!err.message.contains("reqwest"), "no raw library errors in messages");
    }
    let err = h.manager.add("ftp://example.com/file.zip", None).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::UnsupportedProtocol);
    let err = h.manager.add("file:///C:/Windows/win.ini", None).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::UnsupportedProtocol);
    let err = h.manager.add("not a url", None).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidUrl);

    // Nothing listens on this port.
    let err = h.manager.add(&format!("http://127.0.0.1:{}/x.bin", support::closed_port()), None).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Network);

    // A 5 EiB file cannot fit on any disk.
    let err = h.manager.add(&server.url("/range/5764607523034234880/huge.bin"), None).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::InsufficientSpace);

    // An unsafe redirect target is refused.
    let err = h.manager.add(&server.url("/redirect-ftp"), None).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Redirect);

    assert!(h.manager.list().is_empty(), "failed probes don't create downloads");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn duplicate_links_are_rejected() {
    let server = TestServer::start().await;
    let h = Harness::new(1).await;
    let url = server.url("/slow/4000000/1/dup.bin");
    h.manager.add(&url, None).await.unwrap();
    let err = h.manager.add(&url, None).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Duplicate);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn verifies_checksums() {
    let server = TestServer::start().await;
    let h = Harness::new(2).await;
    let size = 2 * MB;
    let good = hex::encode(sha2::Sha256::digest(body(0, size)));

    let rec = h.manager.add(&server.url(&format!("/range/{size}/ok.bin")), Some(&good)).await.unwrap();
    let done = h.wait_for(&rec.id, Status::Completed, 30).await;
    assert_eq!(done.integrity, Integrity::Verified);

    let bad = "0".repeat(64);
    let rec = h.manager.add(&server.url(&format!("/range/{size}/bad.bin")), Some(&bad)).await.unwrap();
    let failed = h.wait_for(&rec.id, Status::Failed, 30).await;
    assert_eq!(failed.integrity, Integrity::Failed);
    assert_eq!(failed.error.unwrap().code, ErrorCode::ChecksumMismatch);
    assert!(failed.final_path.is_none());
    assert!(!h.layout().completed().join("bad.bin").exists(), "corrupt file never marked complete");

    let err = h.manager.add(&server.url("/range/10/x.bin"), Some("not-a-hash")).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidChecksum);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pause_and_resume_continue_from_partial_data() {
    for (connections, size) in [(1u8, 16 * MB), (4, 48 * MB)] {
        let server = TestServer::start().await;
        let h = Harness::new(connections).await;
        let rec = h.manager.add(&server.url(&format!("/slow/{size}/2/p.bin")), None).await.unwrap();

        // Pause early so the transfer can't finish first on a busy machine.
        h.wait_until(&rec.id, 30, |r| r.downloaded > size / 10).await;
        h.manager.pause(&rec.id).unwrap();
        let paused = h.wait_for(&rec.id, Status::Paused, 10).await;
        assert!(h.layout().parked_part(&rec.part_name).exists(), "partial file kept in Incomplete");
        assert!(!h.layout().active_part(&rec.part_name).exists());
        let saved = paused.downloaded;
        assert!(saved > 0 && saved < size);

        server.state.max_range_start.store(0, Ordering::SeqCst);
        h.manager.resume(&rec.id).unwrap();
        let done = h.wait_for(&rec.id, Status::Completed, 60).await;
        h.assert_completed_file(&done, size);
        assert!(
            server.state.max_range_start.load(Ordering::SeqCst) > 0,
            "resume used HTTP ranges ({connections} connections)"
        );
        assert!(
            server.state.bytes_sent.load(Ordering::SeqCst) < 2 * size,
            "resume did not re-download everything"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn survives_application_restart() {
    let server = TestServer::start().await;
    let h = Harness::new(4).await;
    let size = 32 * MB;
    let rec = h.manager.add(&server.url(&format!("/slow/{size}/2/restart.bin")), None).await.unwrap();
    h.wait_until(&rec.id, 30, |r| r.downloaded > size / 4).await;

    // Simulate closing the app.
    h.manager.shutdown(Duration::from_secs(5)).await;

    let reopened = Manager::load(h.data_dir.clone(), h.sink.clone());
    let restored = reopened.get(&rec.id).unwrap();
    assert_eq!(restored.status, Status::Paused, "interrupted download is offered for resume");
    assert!(restored.downloaded > 0);
    assert!(h.layout().parked_part(&rec.part_name).exists());

    reopened.resume_all();
    let h2 = h.with_manager(reopened);
    let done = h2.wait_for(&rec.id, Status::Completed, 60).await;
    h2.assert_completed_file(&done, size);
    assert!(server.state.bytes_sent.load(Ordering::SeqCst) < 2 * size);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn corrupted_resume_state_restarts_cleanly() {
    let server = TestServer::start().await;
    let h = Harness::new(4).await;
    let size = 24 * MB;
    let rec = h.manager.add(&server.url(&format!("/slow/{size}/2/corrupt.bin")), None).await.unwrap();
    h.wait_until(&rec.id, 30, |r| r.downloaded > size / 4).await;
    h.manager.pause(&rec.id).unwrap();
    h.wait_for(&rec.id, Status::Paused, 10).await;

    std::fs::write(h.layout().manifest(&rec.id), b"{ garbage").unwrap();
    // Also truncate the partial file, as a crash might.
    let part = h.layout().parked_part(&rec.part_name);
    std::fs::OpenOptions::new().write(true).open(&part).unwrap().set_len(1000).unwrap();

    h.manager.resume(&rec.id).unwrap();
    let done = h.wait_for(&rec.id, Status::Completed, 60).await;
    h.assert_completed_file(&done, size);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn changed_file_on_server_restarts_download() {
    let server = TestServer::start().await;
    let h = Harness::new(1).await;
    let size = 6 * MB;
    let rec = h.manager.add(&server.url(&format!("/slow/{size}/2/changing.bin")), None).await.unwrap();
    h.wait_until(&rec.id, 30, |r| r.downloaded > size / 4).await;
    h.manager.pause(&rec.id).unwrap();
    h.wait_for(&rec.id, Status::Paused, 10).await;

    server.state.etag_version.fetch_add(1, Ordering::SeqCst);
    h.manager.resume(&rec.id).unwrap();
    let done = h.wait_for(&rec.id, Status::Completed, 60).await;
    h.assert_completed_file(&done, size);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn retries_after_connection_drop() {
    let server = TestServer::start().await;
    let h = Harness::new(1).await;
    let size = 3 * MB;
    let rec = h.manager.add(&server.url(&format!("/flaky/{size}/flaky.bin")), None).await.unwrap();
    let done = h.wait_for(&rec.id, Status::Completed, 30).await;
    h.assert_completed_file(&done, size);
    assert!(h.sink.saw_status(&rec.id, Status::Retrying), "a retry was shown to the user");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn gives_up_after_limited_retries_on_server_errors() {
    let server = TestServer::start().await;
    let h = Harness::new(1).await;
    // Probe succeeds, every download request then fails with 503.
    let rec = h.manager.add(&server.url("/probe-only/1000/x.bin"), None).await.unwrap();
    let failed = h.wait_for(&rec.id, Status::Failed, 60).await;
    assert_eq!(failed.error.unwrap().code, ErrorCode::ServerError);
    let attempts = server.state.requests.load(Ordering::SeqCst);
    assert_eq!(attempts, 1 + ps4_downloader_lib::downloader::manager::MAX_ATTEMPTS as u64);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancel_deletes_partial_data_and_queue_respects_limit() {
    let server = TestServer::start().await;
    let h = Harness::new(1).await;
    h.set_max_simultaneous(1);
    let a = h.manager.add(&server.url("/slow/8000000/5/a.bin"), None).await.unwrap();
    let b = h.manager.add(&server.url("/slow/8000000/5/b.bin"), None).await.unwrap();
    h.wait_until(&a.id, 10, |r| r.downloaded > 0).await;
    assert_eq!(h.manager.get(&b.id).unwrap().status, Status::Queued, "second download waits its turn");

    h.manager.cancel(&a.id).unwrap();
    let cancelled = h.wait_for(&a.id, Status::Cancelled, 10).await;
    assert_eq!(cancelled.downloaded, 0);
    assert!(h.layout().find_part(&a.part_name).is_none());
    assert!(!h.layout().manifest(&a.id).exists());

    // The queued download starts once a slot frees up.
    h.wait_until(&b.id, 10, |r| r.status == Status::Downloading).await;
    h.manager.cancel(&b.id).unwrap();
    h.wait_for(&b.id, Status::Cancelled, 10).await;

    // Removing a cancelled entry leaves no files behind.
    h.manager.remove(&a.id, false).unwrap();
    assert!(h.manager.get(&a.id).is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn removing_history_keeps_files_unless_asked() {
    let server = TestServer::start().await;
    let h = Harness::new(2).await;
    let a = h.manager.add(&server.url("/range/1000/keep.bin"), None).await.unwrap();
    let a = h.wait_for(&a.id, Status::Completed, 30).await;
    h.manager.remove(&a.id, false).unwrap();
    assert!(a.final_path.as_ref().unwrap().exists(), "file kept");

    let b = h.manager.add(&server.url("/range/1000/delete.bin"), None).await.unwrap();
    let b = h.wait_for(&b.id, Status::Completed, 30).await;
    h.manager.remove(&b.id, true).unwrap();
    assert!(!b.final_path.as_ref().unwrap().exists(), "file deleted on explicit request");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn missing_storage_is_reported() {
    let server = TestServer::start().await;
    let h = Harness::new(2).await;
    let mut settings = h.manager.settings();
    settings.storage_root = h.root.join("does-not-exist-yet");
    // Selecting a folder creates it, so point the setting at it and then remove it.
    h.manager.update_settings(settings.clone()).unwrap();
    std::fs::remove_dir_all(&settings.storage_root).unwrap();

    let info = h.manager.storage_info();
    assert!(!info.available);
    assert!(info.drive_available);
    let err = h.manager.add(&server.url("/range/10/x.bin"), None).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::StorageUnavailable);

    let info = h.manager.create_storage_root().unwrap();
    assert!(info.available);
    assert!(h.manager.add(&server.url("/range/10/x.bin"), None).await.is_ok());
}
