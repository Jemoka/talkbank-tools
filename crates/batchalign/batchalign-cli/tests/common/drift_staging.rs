//! Filesystem plumbing for the env-gated real-file drift integration tests.
//!
//! Responsibilities of this module — deliberately narrow:
//!
//! - Resolve `BATCHALIGN3_DRIFT_CORPUS_DIR` to a staging root (or return
//!   `None` for SKIP).
//! - Walk one subdirectory (`micase/`, `samtale/`, `biling/`, `rhd/`) and
//!   collect its `.cha` files together with any adjacent media file.
//! - Wrap CHAT filenames in a [`CorpusFileName`] newtype so downstream
//!   outcome types never carry naked strings at this boundary.
//!
//! No batchalign runtime dependencies live here; this is pure filesystem
//! code. The pipeline driver + result aggregator lives in
//! `tests/ml_golden/align/drift_runner.rs`; the test bodies live in
//! `tests/ml_golden/align/drift_integration.rs`.

use std::fmt;
use std::path::{Path, PathBuf};

/// A `.cha` filename from the drift staging corpus. Newtype at the outcome
/// boundary (instead of a naked `String`) so report formatting, sorting, and
/// any future structured-output consumers stay in the typed lane.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CorpusFileName(String);

impl CorpusFileName {
    /// Derive the filename from a CHAT path. Falls back to the full display
    /// string if the path has no `file_name` component — which should not
    /// happen for the files we walk, but we prefer a safe display over a
    /// panic at the seam.
    pub fn from_cha_path(cha_path: &Path) -> Self {
        let raw = cha_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| cha_path.display().to_string());
        Self(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CorpusFileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Return the drift-corpus staging directory if `BATCHALIGN3_DRIFT_CORPUS_DIR`
/// is set and the directory exists. Returns `None` (for SKIP) otherwise.
pub fn require_drift_corpus_dir() -> Option<PathBuf> {
    let raw = std::env::var("BATCHALIGN3_DRIFT_CORPUS_DIR").ok()?;
    let dir = PathBuf::from(raw);
    if !dir.is_dir() {
        eprintln!(
            "SKIP: BATCHALIGN3_DRIFT_CORPUS_DIR={} does not point to an existing directory",
            dir.display()
        );
        return None;
    }
    Some(dir)
}

/// One staged file in the drift corpus. `cha_path` always exists; `media_path`
/// may be absent if the contributor could not stage it — in which case the
/// test logs a one-line SKIP for that file and moves on.
pub struct StagedDriftFile {
    pub cha_path: PathBuf,
    pub media_path: Option<PathBuf>,
}

impl StagedDriftFile {
    pub fn cha_name(&self) -> CorpusFileName {
        CorpusFileName::from_cha_path(&self.cha_path)
    }
}

/// Enumerate the `.cha` files in one subdirectory of the staging root and
/// resolve each file's adjacent media. Filenames listed in `expected_stems`
/// are matched in deterministic order; extra `.cha` files in the directory
/// are also picked up so new additions don't require code changes.
pub fn enumerate_staged_files(subdir: &Path, expected_stems: &[&str]) -> Vec<StagedDriftFile> {
    let mut files: Vec<StagedDriftFile> = Vec::new();
    // Expected stems first, in listed order (deterministic test output).
    for stem in expected_stems {
        let cha = subdir.join(format!("{stem}.cha"));
        if cha.is_file() {
            files.push(StagedDriftFile {
                cha_path: cha.clone(),
                media_path: locate_adjacent_media(&cha),
            });
        }
    }
    // Any additional `.cha` files not already in expected_stems.
    let Ok(entries) = std::fs::read_dir(subdir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("cha") {
            continue;
        }
        if files.iter().any(|f| f.cha_path == path) {
            continue;
        }
        files.push(StagedDriftFile {
            cha_path: path.clone(),
            media_path: locate_adjacent_media(&path),
        });
    }
    files
}

/// Look for an adjacent media file (any supported audio/video extension) next
/// to the given CHA path.
pub fn locate_adjacent_media(cha_path: &Path) -> Option<PathBuf> {
    const EXTS: &[&str] = &["mp3", "wav", "mp4", "m4a", "flac", "ogg", "aac"];
    let stem = cha_path.file_stem()?;
    let parent = cha_path.parent()?;
    for ext in EXTS {
        let candidate = parent.join(format!("{}.{ext}", stem.to_string_lossy()));
        // Both regular files and symlinks-to-existing-files count.
        if std::fs::metadata(&candidate).is_ok() {
            return Some(candidate);
        }
    }
    None
}
