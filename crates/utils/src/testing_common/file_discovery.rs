//! File and corpus discovery utilities for corpus hierarchies.

use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Enum variants for DiscoveryError.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Recursively find all .cha files in a directory tree.
pub fn find_cha_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), DiscoveryError> {
    find_cha_files_recursive(dir, files)
}

/// Finds cha files recursive.
fn find_cha_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), DiscoveryError> {
    let entries = fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            find_cha_files_recursive(&path, files)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("cha") {
            files.push(path);
        }
    }

    Ok(())
}

/// Discover all CHAT corpora (directories with 0metadata.cdc) in a root directory.
pub fn discover_corpora(root: &Path) -> Result<Vec<PathBuf>, DiscoveryError> {
    let mut corpora = Vec::new();
    discover_corpora_recursive(root, &mut corpora)?;
    corpora.sort();
    Ok(corpora)
}

/// Discovers corpora recursive.
fn discover_corpora_recursive(
    dir: &Path,
    corpora: &mut Vec<PathBuf>,
) -> Result<(), DiscoveryError> {
    if !dir.is_dir() {
        return Ok(());
    }

    // Check if this is a corpus (has 0metadata.cdc)
    if dir.join("0metadata.cdc").exists() {
        corpora.push(dir.to_path_buf());
        return Ok(()); // Don't recurse into subdirectories of a corpus
    }

    // Recurse into subdirectories
    let entries = fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        if entry.path().is_dir() {
            discover_corpora_recursive(&entry.path(), corpora)?;
        }
    }
    Ok(())
}

/// Count .cha files in a directory tree.
pub fn count_cha_files(dir: &Path) -> Result<usize, DiscoveryError> {
    let mut count = 0;
    count_cha_files_recursive(dir, &mut count)?;
    Ok(count)
}

/// Counts cha files recursive.
fn count_cha_files_recursive(dir: &Path, count: &mut usize) -> Result<(), DiscoveryError> {
    let entries = fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            count_cha_files_recursive(&path, count)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("cha") {
            *count += 1;
        }
    }
    Ok(())
}
