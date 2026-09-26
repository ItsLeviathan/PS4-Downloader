//! Storage layout, drive statistics and safe filesystem helpers.
//!
//! ```text
//! <root>\
//! ├── Downloads\   active transfers (<name>.<id>.part)
//! ├── Incomplete\  paused / interrupted transfers, ready to resume
//! ├── Completed\   finished, verified files
//! └── Metadata\    resume manifests (<id>.json)
//! ```

pub mod filename;

use crate::error::{AppError, AppResult, ErrorCode};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn downloads(&self) -> PathBuf {
        self.root.join("Downloads")
    }
    pub fn incomplete(&self) -> PathBuf {
        self.root.join("Incomplete")
    }
    pub fn completed(&self) -> PathBuf {
        self.root.join("Completed")
    }
    pub fn metadata(&self) -> PathBuf {
        self.root.join("Metadata")
    }

    pub fn active_part(&self, part_name: &str) -> PathBuf {
        self.downloads().join(part_name)
    }
    pub fn parked_part(&self, part_name: &str) -> PathBuf {
        self.incomplete().join(part_name)
    }
    pub fn manifest(&self, id: &str) -> PathBuf {
        self.metadata().join(format!("{id}.json"))
    }

    pub fn is_available(&self) -> bool {
        self.root.is_dir()
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        for dir in [self.downloads(), self.incomplete(), self.completed(), self.metadata()] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// Where the partial file for a download currently lives, if anywhere.
    pub fn find_part(&self, part_name: &str) -> Option<PathBuf> {
        [self.active_part(part_name), self.parked_part(part_name)].into_iter().find(|p| p.is_file())
    }

    /// Moves a partial file between Downloads and Incomplete. Renames stay on
    /// the same volume, so they are atomic and never copy data.
    pub fn move_part(&self, part_name: &str, to_active: bool) -> std::io::Result<()> {
        let (from, to) = if to_active {
            (self.parked_part(part_name), self.active_part(part_name))
        } else {
            (self.active_part(part_name), self.parked_part(part_name))
        };
        if !from.is_file() {
            return Ok(());
        }
        // Antivirus scanners can hold a just-written file open for a moment.
        let mut attempt = 0;
        loop {
            match std::fs::rename(&from, &to) {
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && attempt < 5 => {
                    attempt += 1;
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                result => return result,
            }
        }
    }

    /// Deletes the partial file and resume manifest. Missing files are fine.
    pub fn delete_partial(&self, id: &str, part_name: &str) {
        for path in [self.active_part(part_name), self.parked_part(part_name), self.manifest(id)] {
            if let Err(err) = std::fs::remove_file(&path) {
                if err.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("storage: could not delete {}: {err}", path.display());
                }
            }
        }
    }

    /// True if `path` is inside this layout's Completed folder.
    pub fn owns_completed(&self, path: &Path) -> bool {
        path.parent() == Some(self.completed().as_path())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub root: String,
    /// The storage folder exists and can be used.
    pub available: bool,
    /// The drive (e.g. `F:\`) is present, even if the folder is missing.
    pub drive_available: bool,
    pub free: u64,
    pub used: u64,
    pub total: u64,
}

pub fn storage_info(root: &Path) -> StorageInfo {
    let available = root.is_dir();
    let drive = root.ancestors().last().filter(|d| !d.as_os_str().is_empty());
    let drive_available = available || drive.is_some_and(|d| d.is_dir());
    let probe = if available { Some(root) } else if drive_available { drive } else { None };

    let (free, total) = probe
        .and_then(|p| Some((fs2::available_space(p).ok()?, fs2::total_space(p).ok()?)))
        .unwrap_or((0, 0));

    StorageInfo {
        root: root.display().to_string(),
        available,
        drive_available,
        free,
        used: total.saturating_sub(free),
        total,
    }
}

/// Creates the folder structure and checks that it is writable.
pub fn prepare_root(root: &Path) -> AppResult<()> {
    let layout = Layout::new(root);
    layout.ensure().map_err(|e| {
        AppError::new(
            ErrorCode::StorageUnavailable,
            format!("Could not create the storage folders in {}.", root.display()),
        )
        .with_details(e.to_string())
    })?;
    let probe = layout.metadata().join(".write-test");
    std::fs::write(&probe, b"ok")
        .and_then(|_| std::fs::remove_file(&probe))
        .map_err(|e| {
            AppError::new(
                ErrorCode::StorageUnavailable,
                format!("PS4 Downloader cannot write to {}. Choose a different folder.", root.display()),
            )
            .with_details(e.to_string())
        })
}

pub fn available_space(root: &Path) -> AppResult<u64> {
    fs2::available_space(root).map_err(|e| {
        AppError::storage_unavailable(root).with_details(e.to_string())
    })
}

pub fn insufficient_space(required: u64, available: u64) -> AppError {
    AppError::new(
        ErrorCode::InsufficientSpace,
        format!(
            "Not enough storage space.\nRequired: {}\nAvailable: {}\nChoose another storage location or free up space.",
            format_size(required),
            format_size(available)
        ),
    )
}

/// Picks `name`, or `name (1).ext`, `name (2).ext`, … if it already exists.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = filename::split_extension(name);
    (1u32..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("an unused file name exists")
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderUsage {
    pub files: u64,
    pub bytes: u64,
}

/// Recursively totals a folder. Unreadable entries are skipped.
pub fn folder_usage(dir: &Path) -> FolderUsage {
    let mut usage = FolderUsage::default();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else { continue };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                usage.files += 1;
                usage.bytes += meta.len();
            }
        }
    }
    usage
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Marks a file as sparse so segments can be written at far offsets without
/// NTFS zero-filling the gap first (which would stall for minutes on large files).
#[cfg(windows)]
pub fn make_sparse(file: &std::fs::File) -> std::io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Ioctl::FSCTL_SET_SPARSE;
    use windows_sys::Win32::System::IO::DeviceIoControl;

    let mut returned = 0u32;
    // SAFETY: the handle is valid for the lifetime of `file`; FSCTL_SET_SPARSE
    // with no input buffer sets the sparse flag and returns no data.
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            FSCTL_SET_SPARSE,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn make_sparse(_file: &std::fs::File) -> std::io::Result<()> {
    // Most Unix filesystems create sparse files implicitly.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_path_appends_counter() {
        let dir = std::env::temp_dir().join(format!("ps4dl-uniq-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(unique_path(&dir, "a.zip"), dir.join("a.zip"));
        std::fs::write(dir.join("a.zip"), b"x").unwrap();
        assert_eq!(unique_path(&dir, "a.zip"), dir.join("a (1).zip"));
        std::fs::write(dir.join("a (1).zip"), b"x").unwrap();
        assert_eq!(unique_path(&dir, "a.zip"), dir.join("a (2).zip"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_drive_is_reported_not_panicking() {
        let info = storage_info(Path::new(r"Q:\definitely\missing"));
        assert!(!info.available);
        assert_eq!(info.total, 0);
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(86_400_000_000), "80.5 GB");
    }
}
