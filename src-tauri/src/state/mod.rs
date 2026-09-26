//! Small JSON persistence helpers. Files are written atomically (temp file +
//! rename) so a crash or power loss never leaves a half-written state file.

use serde::{de::DeserializeOwned, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Loads a JSON file. A missing file yields `None`. A corrupted file is moved
/// aside to `<name>.corrupt` so the app can start fresh without losing it.
pub fn load_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let bytes = std::fs::read(path).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(value) => Some(value),
        Err(err) => {
            eprintln!("state: {} is corrupted ({err}); moving it aside", path.display());
            let _ = std::fs::rename(path, with_suffix(path, "corrupt"));
            None
        }
    }
}

pub fn save_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    let tmp = with_suffix(path, "tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(suffix);
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_corruption_recovery() {
        let dir = std::env::temp_dir().join(format!("ps4dl-state-{}", uuid::Uuid::new_v4()));
        let path = dir.join("value.json");
        save_json(&path, &vec![1u64, 2, 3]).unwrap();
        assert_eq!(load_json::<Vec<u64>>(&path), Some(vec![1, 2, 3]));

        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(load_json::<Vec<u64>>(&path), None);
        assert!(dir.join("value.json.corrupt").exists());
        assert!(!path.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
