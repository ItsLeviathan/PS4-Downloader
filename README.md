# PS4 Downloader

A Windows desktop download manager for large files, built to put big game files
on a dedicated storage drive (`F:\PS4 Games` by default). It downloads **files you
are authorized to download** from direct HTTP/HTTPS links. It resumes interrupted
transfers, uses several connections when the server allows it, and verifies
checksums when you provide one.

## Features

- **Resumable downloads.** Pause and resume, and survive app restarts and crashes, using HTTP range requests.
- **Segmented downloads.** Up to 1/2/4/8 connections per file (default 4), used only when the server supports ranges and the file is at least 16 MB.
- **Download queue.** Configurable number of simultaneous downloads (default 2). You can pause, resume, cancel, retry, and remove.
- **Live progress.** Speed averaged over a rolling 5-second window, plus downloaded/total, percentage, and ETA.
- **Automatic retries.** Temporary network and server errors are retried with exponential backoff (2 s, 4 s, 8 s, 16 s), up to 5 attempts, and partial data is kept.
- **Filename detection** from `Content-Disposition`, the URL, or the redirect target. Names are sanitized for Windows.
- **Storage management.**
  - Real drive capacity, free space, and used space.
  - A disk-space check before starting.
  - A warning when the drive or folder is missing, with options to create or choose a folder.
  - Downloads pause automatically if the drive disappears.
- **Checksum verification** (SHA-256, SHA-1, MD5). A file is shown as **Verified** only after a real comparison. Files that fail verification are never moved to Completed.
- **History** of completed, failed, and cancelled downloads. Removing an entry never deletes a completed file unless you tick "Also delete the file from disk".
- **Desktop notifications** for completed downloads, failed downloads, and storage problems. Each type can be turned off.
- **Themes:** dark (default), light, or follow the system.

## Tech stack

| Layer    | Technology                                                                  |
| -------- | --------------------------------------------------------------------------- |
| Shell    | Tauri 2                                                                     |
| Backend  | Rust: tokio, reqwest (Windows native TLS), sha2/sha1/md-5, fs2, windows-sys |
| Frontend | React 19 + TypeScript (strict), Vite, zustand, plain CSS                    |
| Plugins  | dialog (folder picker), notification, opener (Explorer)                     |

## Development setup

Prerequisites (Windows 10/11):

- Node.js 20.19+ (tested with 25)
- Rust stable, MSVC toolchain (`rustup default stable-x86_64-pc-windows-msvc`)
- Visual Studio Build Tools with "Desktop development with C++"
- WebView2 runtime (preinstalled on Windows 11)

```powershell
npm install
```

On this machine Rust is installed on the F: drive. If `cargo` isn't on `PATH` in
a new terminal, sign out and back in, or run:

```powershell
$env:RUSTUP_HOME = 'F:\tools\rust\rustup'; $env:CARGO_HOME = 'F:\tools\rust\cargo'; $env:Path = "F:\tools\rust\cargo\bin;$env:Path"
```

## Running

```powershell
npm run tauri dev
```

## Building

```powershell
npm run tauri build
```

Outputs:

- `src-tauri\target\release\ps4-downloader.exe`: the standalone app
- `src-tauri\target\release\bundle\nsis\PS4 Downloader_0.1.0_x64-setup.exe`: the installer

## Testing

```powershell
cd src-tauri
cargo test          # unit tests + end-to-end download tests
cd ..
npm run typecheck
```

The integration tests (`src-tauri/tests`) run the real download manager against a
local HTTP server that supports or ignores range requests, redirects, returns
errors, drops connections and throttles transfers.

## Storage structure

```text
F:\PS4 Games\
├── Downloads\    active transfers  (<name>.<id>.part)
├── Incomplete\   paused/interrupted transfers, ready to resume
├── Completed\    finished files
└── Metadata\     resume manifests  (<id>.json)
```

App state lives in `%APPDATA%\com.ps4downloader.desktop\`: `settings.json`
holds settings and `downloads.json` holds the queue and history. It stays there
so the app still works when the storage drive is missing. All writes are atomic
(temp file + rename). A corrupted state file is set aside as `*.corrupt`, and
the app starts fresh.

## Download architecture

```text
React UI ──invoke──▶ commands.rs ──▶ Manager (queue, retries, persistence, storage watch)
   ▲                                     │
   └──── events: download-updated,       ├── http.rs      probe (Range: bytes=0-0), client, errors
         download-progress (2×/s),       ├── engine.rs    per-segment streaming writers
         download-removed,               ├── manifest.rs  which byte ranges are on disk
         storage-changed                 ├── storage/     layout, disk info, filenames, sparse files
                                         └── checksum/    streaming hashes (off the async runtime)
```

1. **Probe.** A single `GET` with `Range: bytes=0-0`. A `206` reply with `Content-Range` confirms range support and gives the total size. The same reply gives the filename, and an `ETag`/`Last-Modified` validator.
2. **Plan.** If ranges are supported and the file is at least 16 MB, the file is split into N equal segments. The partial file is marked **sparse**, so segments can be written at far offsets without NTFS zero-filling the gaps. On filesystems without sparse support (FAT32/exFAT), the download falls back to one connection.
3. **Transfer.** Each segment streams into a 1 MB buffer that is flushed to disk at least once per second. A segment's `done` counter only advances after its bytes are written. The manifest is saved every 2 seconds and on every stop, so it never claims data that isn't in the file.
4. **Resume.** The app checks that the manifest and the partial file agree, then requests the remaining ranges with `If-Range`. If the file changed on the server, or the server stops honoring ranges, the download starts again cleanly with fresh metadata.
5. **Finish.** The file is synced and its exact size checked, and the checksum is verified if one was given. The file is then renamed into `Completed\`, where a name collision gets a ` (1)` suffix.

## Security considerations

- Only `http://` and `https://` links are accepted. Redirects are limited to 10, must stay on http/https, and may not downgrade from HTTPS to HTTP.
- Server-provided names are reduced to a single sanitized path component: invalid characters, control characters, reserved device names (`CON`, `NUL`, …), and trailing dots are handled. Downloads can't escape the storage folder.
- Downloaded files are never opened or executed. "Open folder" only highlights the file in Explorer.
- The app doesn't try to get past logins, CAPTCHAs, DRM, paywalls, or rate limits. Links that need a login (401/403) or lead to a web page (`text/html`) are reported as not directly downloadable.
- The frontend has no direct file or network access. It can only call the app's commands, and the only plugin permission granted to it is the folder picker.
- No telemetry, accounts or remote services. URLs are only sent to the server they point at.

## Known limitations

- **No browser-session downloads.** Links that only work with browser cookies or after clicking through a web page aren't supported.
- **Sparse partial files.** Segmented downloads use sparse files, so disk space is consumed as data arrives rather than reserved up front. Free space is checked before each download starts, but other programs could still fill the drive in the meantime.
- **Checksums are entered by hand.** You paste the checksum from the download page; server digest headers aren't read.
- **No verification progress bar.** While a checksum is being computed the card shows "Verifying" without a percentage, which can take a few minutes for very large files.
- **No run-at-startup option** (intentionally left out rather than half-implemented).
- **Drive unplug not tested on real hardware.** Removing the drive mid-download is handled (the transfer pauses and partial data is kept), but it was verified by code path, not by physically unplugging a drive.
