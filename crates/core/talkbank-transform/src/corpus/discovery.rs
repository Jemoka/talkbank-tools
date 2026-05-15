//! Corpus discovery - find all corpora and build manifest.
//!
//! A corpus is defined as a directory containing a `0metadata.cdc` file.
//! This module discovers all corpora in a root directory and creates a manifest.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use super::manifest::{CorpusManifest, ManifestError};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Discover all corpora in a root directory
pub fn discover_corpora(root: &Path) -> Result<Vec<PathBuf>, ManifestError> {
    let mut corpora = Vec::new();
    discover_corpora_recursive(root, &mut corpora)?;
    corpora.sort();
    Ok(corpora)
}

/// Find corpus directories (those containing 0metadata.cdc)
fn discover_corpora_recursive(dir: &Path, corpora: &mut Vec<PathBuf>) -> Result<(), ManifestError> {
    let entries = fs::read_dir(dir).map_err(|source| ManifestError::ReadDir {
        path: dir.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| ManifestError::ReadEntry {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();

        // Check if this directory is a corpus (contains 0metadata.cdc)
        if path.is_dir() {
            let metadata_file = path.join("0metadata.cdc");
            if metadata_file.exists() {
                corpora.push(path);
            } else {
                // Recursively search subdirectories
                discover_corpora_recursive(&path, corpora)?;
            }
        }
    }

    Ok(())
}

/// Build manifest for all corpora in root directory
pub fn build_manifest(root: &Path) -> Result<CorpusManifest, ManifestError> {
    info!(root = %root.display(), "Discovering corpora");

    let corpora = discover_corpora(root)?;
    info!(count = corpora.len(), "Found corpora");

    let mut manifest = CorpusManifest::new()?;

    for (idx, corpus_path) in corpora.iter().enumerate() {
        if idx % 50 == 0 {
            debug!(
                progress = idx + 1,
                total = corpora.len(),
                "Processing corpus"
            );
        }

        manifest.add_corpus(corpus_path.clone())?;
    }

    Ok(manifest)
}

/// Get human-readable corpus information
pub fn corpus_summary(manifest: &CorpusManifest) -> String {
    let mut summary = String::new();

    summary.push_str(&format!(
        "Manifest Summary (version {})\n",
        manifest.version
    ));
    summary.push_str(&format!("Total corpora: {}\n", manifest.total_corpora));
    summary.push_str(&format!("Total files: {}\n", manifest.total_files));
    summary.push_str(&format!(
        "Status: {} passed, {} failed, {} not tested\n",
        manifest.total_passed, manifest.total_failed, manifest.total_not_tested
    ));
    summary.push_str(&format!(
        "Progress: {:.1}% complete\n",
        manifest.overall_progress()
    ));
    summary.push_str(&format!(
        "Pass rate: {:.1}%\n",
        manifest.overall_pass_rate()
    ));

    summary
}

/// Format manifest in human-readable format, returning a string.
///
/// This function returns the formatted output as a String rather than printing,
/// allowing the caller (typically a CLI binary) to decide how to display it.
pub fn format_manifest(manifest: &CorpusManifest) -> String {
    let mut output = String::new();
    output.push_str(&corpus_summary(manifest));

    output.push_str("\nCorpora by pass rate:\n");
    output.push_str(&format!(
        "{:<70} {:>6} {:>6} {:>8} {:>8}\n",
        "Corpus", "Files", "Pass%", "Passed", "Failed"
    ));
    output.push_str(&"-".repeat(100));
    output.push('\n');

    let mut corpus_list: Vec<_> = manifest.corpora.values().collect();
    corpus_list.sort_by(|a, b| match b.pass_rate().partial_cmp(&a.pass_rate()) {
        Some(ordering) => ordering,
        None => std::cmp::Ordering::Equal,
    });

    for corpus in corpus_list {
        let name = if corpus.name.len() > 70 {
            format!("...{}", &corpus.name[corpus.name.len() - 67..])
        } else {
            corpus.name.clone()
        };

        let tested = corpus.passed + corpus.failed;
        let pass_pct = if tested > 0 {
            (corpus.passed as f64 / tested as f64) * 100.0
        } else {
            0.0
        };

        output.push_str(&format!(
            "{:<70} {:>6} {:>5.1}% {:>8} {:>8}\n",
            name, corpus.file_count, pass_pct, corpus.passed, corpus.failed
        ));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests corpus summary.
    #[test]
    fn test_corpus_summary() -> Result<(), ManifestError> {
        let manifest = CorpusManifest::new()?;
        let summary = corpus_summary(&manifest);
        assert!(summary.contains("Total corpora: 0"));
        Ok(())
    }
}
