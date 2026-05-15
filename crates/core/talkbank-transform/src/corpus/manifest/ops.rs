use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::errors::ManifestError;
use super::types::{
    CorpusEntry, CorpusManifest, ErrorDetail, FailureReason, FileEntry, FileStatus,
};

impl CorpusManifest {
    /// Create an empty manifest initialized with current timestamps.
    pub fn new() -> Result<Self, ManifestError> {
        let now = current_time_secs()?;

        Ok(Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: now,
            updated_at: now,
            total_corpora: 0,
            total_files: 0,
            total_passed: 0,
            total_failed: 0,
            total_not_tested: 0,
            corpora: BTreeMap::new(),
        })
    }

    /// Load a manifest from a JSON file on disk.
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let content = fs::read_to_string(path).map_err(|source| ManifestError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_str(&content).map_err(|source| ManifestError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Persist the manifest as pretty-printed JSON.
    pub fn save(&self, path: &Path) -> Result<(), ManifestError> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|source| ManifestError::Serialize { source })?;
        fs::write(path, content).map_err(|source| ManifestError::Write {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    }

    /// Discover `.cha` files under a corpus directory and add them to the manifest.
    pub fn add_corpus(&mut self, corpus_path: PathBuf) -> Result<(), ManifestError> {
        let path_str = path_to_string(&corpus_path)?;
        let name = corpus_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .ok_or_else(|| ManifestError::InvalidName {
                path: corpus_path.clone(),
            })?;

        let mut files = BTreeMap::new();
        Self::find_cha_files(&corpus_path, &mut files)?;

        let file_count = files.len();

        let entry = CorpusEntry {
            path: path_str.clone(),
            name,
            file_count,
            passed: 0,
            failed: 0,
            not_tested: file_count,
            files,
        };

        self.corpora.insert(path_str, entry);
        self.recalculate_totals()?;
        Ok(())
    }

    /// Update the latest result metadata for a single tracked file.
    pub fn update_file_status(
        &mut self,
        corpus_path: &Path,
        file_path: &Path,
        status: FileStatus,
        failure_reason: Option<FailureReason>,
        error_detail: Option<ErrorDetail>,
    ) -> Result<(), ManifestError> {
        let corpus_key = path_to_string(corpus_path)?;
        let file_key = path_to_string(file_path)?;
        let corpus =
            self.corpora
                .get_mut(&corpus_key)
                .ok_or_else(|| ManifestError::MissingCorpus {
                    path: corpus_key.clone(),
                })?;

        let file_entry =
            corpus
                .files
                .get_mut(&file_key)
                .ok_or_else(|| ManifestError::MissingFile {
                    path: file_key.clone(),
                })?;

        match file_entry.status {
            FileStatus::NotTested => corpus.not_tested -= 1,
            FileStatus::Passed => corpus.passed -= 1,
            FileStatus::Failed => corpus.failed -= 1,
        }

        file_entry.status = status;
        file_entry.failure_reason = failure_reason;
        file_entry.error_detail = error_detail;
        file_entry.last_tested = Some(current_time_secs()?);

        match status {
            FileStatus::NotTested => corpus.not_tested += 1,
            FileStatus::Passed => corpus.passed += 1,
            FileStatus::Failed => corpus.failed += 1,
        }

        self.recalculate_totals()?;
        Ok(())
    }

    /// Return the pass rate across all tested files.
    pub fn overall_pass_rate(&self) -> f64 {
        let tested = self.total_passed + self.total_failed;
        if tested > 0 {
            (self.total_passed as f64 / tested as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Return percentage progress across all tracked files.
    pub fn overall_progress(&self) -> f64 {
        if self.total_files > 0 {
            ((self.total_passed + self.total_failed) as f64 / self.total_files as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Serialize the manifest to a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, ManifestError> {
        serde_json::to_string_pretty(self).map_err(|source| ManifestError::Serialize { source })
    }

    fn recalculate_totals(&mut self) -> Result<(), ManifestError> {
        self.total_corpora = self.corpora.len();
        self.total_files = 0;
        self.total_passed = 0;
        self.total_failed = 0;
        self.total_not_tested = 0;

        for corpus in self.corpora.values() {
            self.total_files += corpus.file_count;
            self.total_passed += corpus.passed;
            self.total_failed += corpus.failed;
            self.total_not_tested += corpus.not_tested;
        }

        self.updated_at = current_time_secs()?;
        Ok(())
    }

    fn find_cha_files(
        dir: &Path,
        files: &mut BTreeMap<String, FileEntry>,
    ) -> Result<(), ManifestError> {
        Self::find_cha_files_recursive(dir, files)
    }

    fn find_cha_files_recursive(
        dir: &Path,
        files: &mut BTreeMap<String, FileEntry>,
    ) -> Result<(), ManifestError> {
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

            if path.is_dir() {
                Self::find_cha_files_recursive(&path, files)?;
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("cha") {
                let path_str = path_to_string(&path)?;
                files.insert(
                    path_str.clone(),
                    FileEntry {
                        path: path_str,
                        status: FileStatus::NotTested,
                        failure_reason: None,
                        last_tested: None,
                        error_detail: None,
                    },
                );
            }
        }

        Ok(())
    }
}

fn current_time_secs() -> Result<u64, ManifestError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|source| ManifestError::Time { source })
}

fn path_to_string(path: &Path) -> Result<String, ManifestError> {
    path.to_str()
        .map(|value| value.to_string())
        .ok_or_else(|| ManifestError::InvalidPath {
            path: path.to_path_buf(),
        })
}
