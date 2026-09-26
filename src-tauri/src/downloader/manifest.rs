//! Resume manifest: which byte ranges of the partial file are already on disk.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Files smaller than this are always downloaded over one connection.
pub const MIN_SEGMENTED_SIZE: u64 = 16 * 1024 * 1024;
const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub start: u64,
    /// Inclusive end offset; `None` when the file size is unknown.
    pub end: Option<u64>,
    /// Bytes of this segment that have been written to disk.
    pub done: u64,
}

impl Segment {
    pub fn len(&self) -> Option<u64> {
        self.end.map(|e| e - self.start + 1)
    }

    /// Only meaningful for segments with a known end.
    pub fn is_complete(&self) -> bool {
        self.len().is_some_and(|len| self.done >= len)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: u32,
    pub total: Option<u64>,
    pub validator: Option<String>,
    pub segments: Vec<Segment>,
}

impl Manifest {
    /// Splits the file into `connections` equal segments when that is safe,
    /// otherwise returns a single sequential segment.
    pub fn plan(total: Option<u64>, supports_range: bool, connections: u8, validator: Option<String>) -> Self {
        let segments = match total {
            Some(total) if supports_range && connections > 1 && total >= MIN_SEGMENTED_SIZE => {
                let n = u64::from(connections);
                let size = total / n;
                (0..n)
                    .map(|i| Segment {
                        start: i * size,
                        end: Some(if i == n - 1 { total - 1 } else { (i + 1) * size - 1 }),
                        done: 0,
                    })
                    .collect()
            }
            _ => vec![Segment { start: 0, end: total.filter(|&t| t > 0).map(|t| t - 1), done: 0 }],
        };
        Self { version: VERSION, total, validator, segments }
    }

    pub fn single(total: Option<u64>, validator: Option<String>) -> Self {
        Self::plan(total, false, 1, validator)
    }

    pub fn downloaded(&self) -> u64 {
        self.segments.iter().map(|s| s.done).sum()
    }

    pub fn is_segmented(&self) -> bool {
        self.segments.len() > 1
    }

    pub fn load(path: &Path) -> Option<Self> {
        let manifest: Self = crate::state::load_json(path)?;
        manifest.is_consistent().then_some(manifest)
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        crate::state::save_json(path, self)
    }

    /// Rejects manifests that were corrupted or hand-edited into nonsense.
    fn is_consistent(&self) -> bool {
        if self.version != VERSION || self.segments.is_empty() {
            return false;
        }
        let mut expected_start = 0;
        for seg in &self.segments {
            if seg.start != expected_start {
                return false;
            }
            match (seg.end, seg.len()) {
                (Some(end), Some(len)) => {
                    if end < seg.start || seg.done > len {
                        return false;
                    }
                    expected_start = end + 1;
                }
                _ => {
                    if self.segments.len() != 1 {
                        return false;
                    }
                }
            }
        }
        match (self.total, self.segments.last().and_then(|s| s.end)) {
            (Some(total), Some(end)) => end + 1 == total,
            (Some(0), None) | (None, None) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_segments_covering_the_file() {
        let total = 100 * 1024 * 1024 + 3;
        let m = Manifest::plan(Some(total), true, 4, None);
        assert_eq!(m.segments.len(), 4);
        assert_eq!(m.segments[0].start, 0);
        assert_eq!(m.segments[3].end, Some(total - 1));
        let covered: u64 = m.segments.iter().map(|s| s.len().unwrap()).sum();
        assert_eq!(covered, total);
        assert!(m.is_consistent());
    }

    #[test]
    fn small_or_unranged_files_use_one_segment() {
        assert_eq!(Manifest::plan(Some(1024), true, 8, None).segments.len(), 1);
        assert_eq!(Manifest::plan(Some(1 << 40), false, 8, None).segments.len(), 1);
        assert_eq!(Manifest::plan(None, true, 8, None).segments.len(), 1);
        assert_eq!(Manifest::plan(Some(1 << 30), true, 1, None).segments.len(), 1);
    }

    #[test]
    fn huge_sizes_do_not_overflow() {
        let total = 200 * 1024 * 1024 * 1024u64; // 200 GiB
        let m = Manifest::plan(Some(total), true, 8, None);
        assert_eq!(m.segments.last().unwrap().end, Some(total - 1));
        assert!(m.is_consistent());
    }

    #[test]
    fn rejects_inconsistent_manifests() {
        let mut m = Manifest::plan(Some(64 * 1024 * 1024), true, 4, None);
        m.segments[1].done = u64::MAX;
        assert!(!m.is_consistent());
        let mut m = Manifest::plan(Some(64 * 1024 * 1024), true, 4, None);
        m.segments[2].start += 1;
        assert!(!m.is_consistent());
        let mut m = Manifest::plan(Some(10), true, 1, None);
        m.total = Some(11);
        assert!(!m.is_consistent());
    }
}
