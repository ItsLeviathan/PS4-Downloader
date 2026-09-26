//! Checksum parsing and streaming file hashing.

use crate::error::{AppError, AppResult, ErrorCode};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Algorithm {
    Sha256,
    Sha1,
    Md5,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedChecksum {
    pub algorithm: Algorithm,
    /// Lowercase hex.
    pub value: String,
}

/// Accepts `sha256:<hex>`, `sha1:<hex>`, `md5:<hex>`, a bare hex digest (the
/// algorithm is inferred from its length) or a `sha256sum`-style line
/// (`<hex>  filename`). Empty input means "no checksum".
pub fn parse(input: &str) -> AppResult<Option<ExpectedChecksum>> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(None);
    }
    let invalid = || {
        AppError::new(
            ErrorCode::InvalidChecksum,
            "The checksum doesn't look valid. Paste a SHA-256, SHA-1 or MD5 value in hexadecimal.",
        )
    };

    let (prefix, rest) = match input.split_once(':') {
        Some((p, r)) => (Some(p.trim().to_ascii_lowercase().replace('-', "")), r),
        None => (None, input),
    };
    let hex_value = rest.split_whitespace().next().ok_or_else(invalid)?.to_ascii_lowercase();
    if !hex_value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid());
    }

    let algorithm = match (prefix.as_deref(), hex_value.len()) {
        (Some("sha256") | None, 64) => Algorithm::Sha256,
        (Some("sha1") | None, 40) => Algorithm::Sha1,
        (Some("md5") | None, 32) => Algorithm::Md5,
        _ => return Err(invalid()),
    };
    Ok(Some(ExpectedChecksum { algorithm, value: hex_value }))
}

/// Hashes a file with a fixed-size buffer. Blocking — run it off the async runtime.
pub fn hash_file(path: &Path, algorithm: Algorithm) -> std::io::Result<String> {
    match algorithm {
        Algorithm::Sha256 => hash_with::<sha2::Sha256>(path),
        Algorithm::Sha1 => hash_with::<sha1::Sha1>(path),
        Algorithm::Md5 => hash_with::<md5::Md5>(path),
    }
}

fn hash_with<D: Digest>(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = D::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA256_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn parses_formats() {
        assert_eq!(parse("  ").unwrap(), None);
        let c = parse(SHA256_ABC).unwrap().unwrap();
        assert_eq!(c.algorithm, Algorithm::Sha256);
        let c = parse(&format!("SHA-256:{}", SHA256_ABC.to_uppercase())).unwrap().unwrap();
        assert_eq!(c.value, SHA256_ABC);
        let c = parse(&format!("{SHA256_ABC}  game.pkg")).unwrap().unwrap();
        assert_eq!(c.algorithm, Algorithm::Sha256);
        let c = parse("900150983cd24fb0d6963f7d28e17f72").unwrap().unwrap();
        assert_eq!(c.algorithm, Algorithm::Md5);
        let c = parse("a9993e364706816aba3e25717850c26c9cd0d89d").unwrap().unwrap();
        assert_eq!(c.algorithm, Algorithm::Sha1);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("xyz").is_err());
        assert!(parse(&format!("md5:{SHA256_ABC}")).is_err());
        assert!(parse("123").is_err());
    }

    #[test]
    fn hashes_files() {
        let path = std::env::temp_dir().join(format!("ps4dl-hash-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(hash_file(&path, Algorithm::Sha256).unwrap(), SHA256_ABC);
        assert_eq!(hash_file(&path, Algorithm::Md5).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
        std::fs::remove_file(path).unwrap();
    }
}
