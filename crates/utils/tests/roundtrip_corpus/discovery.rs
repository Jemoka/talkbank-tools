//! Test module for discovery in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use std::fs;
use std::path::PathBuf;

/// Finds cha files.
pub fn find_cha_files(dir: &PathBuf, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_cha_files(&path, files);
            } else if path.extension().and_then(|s| s.to_str()) == Some("cha") {
                files.push(path);
            }
        }
    }
}
