//! Test module for io in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use std::fs;
use std::path::{Path, PathBuf};

/// Resolves test file.
pub fn resolve_test_file(corpus_path: &Path, test_file: &str) -> PathBuf {
    corpus_path.join(test_file)
}

/// Returns file.
pub fn read_file(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("Failed to read file: {}", e))
}
