//! Test module for discovery in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use std::fs;
use std::path::{Path, PathBuf};

/// Enum variants for DiscoveryError.
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("Failed to read corpus directory {path}: {source}")]
    ReadDir {
        path: String,
        source: std::io::Error,
    },
}

/// Runs list chat files.
pub fn list_chat_files(corpus_dir: &Path) -> Result<Vec<PathBuf>, DiscoveryError> {
    let mut entries: Vec<PathBuf> = fs::read_dir(corpus_dir)
        .map_err(|source| DiscoveryError::ReadDir {
            path: corpus_dir.display().to_string(),
            source,
        })?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("cha"))
        .collect();

    entries.sort();
    Ok(entries)
}
