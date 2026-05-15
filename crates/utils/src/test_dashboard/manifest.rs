//! Manifest mutation and persistence boundary for the test dashboard worker.

use std::path::{Path, PathBuf};

use talkbank_transform::{CorpusFileStatus, CorpusManifest};

use crate::test_dashboard::app::FileTestOutcome;

/// Corpus-level totals produced when a batch of file outcomes is committed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ManifestCommitSummary {
    /// Number of newly passed files in the batch.
    pub newly_passed: usize,
    /// Number of newly failed files in the batch.
    pub newly_failed: usize,
}

/// Owns the dashboard manifest plus its persistence path.
pub struct DashboardManifest {
    manifest: CorpusManifest,
    manifest_path: PathBuf,
}

impl DashboardManifest {
    /// Create a new manifest coordinator around one loaded manifest and save path.
    pub fn new(manifest: CorpusManifest, manifest_path: PathBuf) -> Self {
        Self {
            manifest,
            manifest_path,
        }
    }

    /// Borrow the current manifest snapshot for UI initialization and inspection.
    pub fn manifest(&self) -> &CorpusManifest {
        &self.manifest
    }

    /// Resolve the canonical on-disk corpus path for one manifest key.
    pub fn corpus_path(&self, corpus_path_key: &str) -> Result<PathBuf, String> {
        self.manifest
            .corpora
            .get(corpus_path_key)
            .map(|entry| PathBuf::from(&entry.path))
            .ok_or_else(|| format!("Missing manifest corpus entry: {}", corpus_path_key))
    }

    /// Apply one corpus batch of file outcomes back into the manifest.
    pub fn commit_results(
        &mut self,
        corpus_path_key: &str,
        results: &[FileTestOutcome],
    ) -> Result<ManifestCommitSummary, String> {
        let corpus_path = Path::new(corpus_path_key);
        let mut summary = ManifestCommitSummary::default();

        for result in results {
            let status = if result.passed {
                CorpusFileStatus::Passed
            } else {
                CorpusFileStatus::Failed
            };

            self.manifest
                .update_file_status(
                    corpus_path,
                    &result.path,
                    status,
                    result.failure_reason.clone(),
                    result.error_detail.clone(),
                )
                .map_err(|error| {
                    format!(
                        "Failed to update manifest for {}: {}",
                        result.path.display(),
                        error
                    )
                })?;

            if result.passed {
                summary.newly_passed += 1;
            } else {
                summary.newly_failed += 1;
            }
        }

        Ok(summary)
    }

    /// Persist the current manifest to disk.
    pub fn save(&self) -> Result<(), String> {
        self.manifest
            .save(&self.manifest_path)
            .map_err(|error| format!("Failed to save manifest: {}", error))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use talkbank_transform::CorpusManifest;
    use talkbank_transform::corpus::manifest::{ErrorDetail, FailureReason};

    use super::{DashboardManifest, ManifestCommitSummary};
    use crate::test_dashboard::app::FileTestOutcome;

    #[test]
    fn commit_results_updates_manifest_totals_and_persists()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let corpus_dir = temp_dir.path().join("demo");
        fs::create_dir_all(&corpus_dir)?;

        let passed_file = corpus_dir.join("passed.cha");
        let failed_file = corpus_dir.join("failed.cha");
        fs::write(&passed_file, "@UTF8\n@Begin\n@End\n")?;
        fs::write(&failed_file, "@UTF8\n@Begin\n@End\n")?;

        let mut manifest = CorpusManifest::new()?;
        manifest.add_corpus(corpus_dir.clone())?;

        let manifest_path = temp_dir.path().join("corpus-manifest.json");
        let mut dashboard_manifest = DashboardManifest::new(manifest, manifest_path.clone());

        let summary = dashboard_manifest.commit_results(
            corpus_dir.to_str().expect("utf-8 corpus path"),
            &[
                FileTestOutcome::new(passed_file, true, None, None),
                FileTestOutcome::new(
                    failed_file,
                    false,
                    Some(FailureReason::ParseError),
                    Some(ErrorDetail::new("ParseError", "expected tier marker")),
                ),
            ],
        )?;

        assert_eq!(
            summary,
            ManifestCommitSummary {
                newly_passed: 1,
                newly_failed: 1,
            }
        );
        assert_eq!(dashboard_manifest.manifest().total_passed, 1);
        assert_eq!(dashboard_manifest.manifest().total_failed, 1);
        assert_eq!(dashboard_manifest.manifest().total_not_tested, 0);

        dashboard_manifest.save()?;
        let saved = CorpusManifest::load(&manifest_path)?;
        assert_eq!(saved.total_passed, 1);
        assert_eq!(saved.total_failed, 1);
        assert_eq!(saved.total_not_tested, 0);

        Ok(())
    }
}
