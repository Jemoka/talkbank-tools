//! Shared models for discovering CHAT files before analysis runs.
//!
//! The CLI and LSP both need to turn user-selected files or directories into the
//! flat list of CHAT files consumed by [`AnalysisRunner`](super::AnalysisRunner).
//! Keep that discovery behavior in the library so outer wrappers do not duplicate
//! directory walking or ad hoc skipped-path tracking.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

/// Result of discovering CHAT files from one or more user-selected paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveredChatFiles {
    files: Vec<PathBuf>,
    skipped_paths: Vec<PathBuf>,
}

impl DiscoveredChatFiles {
    /// Discover CHAT files from a single file or directory path.
    pub fn from_path(path: &Path) -> Self {
        let mut discovered = Self::default();
        discovered.extend_from_path(path);
        discovered
    }

    /// Discover CHAT files from a list of file or directory paths.
    pub fn from_paths(paths: &[PathBuf]) -> Self {
        let mut discovered = Self::default();
        for path in paths {
            discovered.extend_from_path(path);
        }
        discovered
    }

    /// Borrow the discovered files in traversal order.
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Consume the discovery result and return the discovered files.
    pub fn into_files(self) -> Vec<PathBuf> {
        self.files
    }

    /// Borrow any user-provided paths that could not be resolved.
    pub fn skipped_paths(&self) -> &[PathBuf] {
        &self.skipped_paths
    }

    /// Whether discovery produced zero files.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    fn extend_from_path(&mut self, path: &Path) {
        if path.is_file() {
            self.files.push(path.to_path_buf());
            return;
        }

        if path.is_dir() {
            for entry in WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(Result::ok)
            {
                let candidate = entry.path();
                if candidate.is_file() && candidate.extension().is_some_and(|ext| ext == "cha") {
                    self.files.push(candidate.to_path_buf());
                }
            }
            return;
        }

        self.skipped_paths.push(path.to_path_buf());
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::DiscoveredChatFiles;

    /// Direct files should be preserved, and directories should contribute nested CHAT files.
    #[test]
    fn discovers_files_from_direct_paths_and_directories() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        let direct_file = root.join("direct.txt");
        let chat_file = root.join("nested").join("sample.cha");
        let ignored_file = root.join("nested").join("sample.txt");

        fs::create_dir_all(chat_file.parent().expect("parent")).expect("create nested dir");
        fs::write(&direct_file, "direct").expect("write direct file");
        fs::write(&chat_file, "@Begin\n@End\n").expect("write chat file");
        fs::write(&ignored_file, "ignore").expect("write ignored file");

        let discovered =
            DiscoveredChatFiles::from_paths(&[direct_file.clone(), root.join("nested")]);

        assert!(discovered.files().contains(&direct_file));
        assert!(discovered.files().contains(&chat_file));
        assert!(!discovered.files().contains(&ignored_file));
        assert!(discovered.skipped_paths().is_empty());
    }

    /// Invalid paths should be tracked so outer wrappers can surface warnings consistently.
    #[test]
    fn tracks_skipped_paths() {
        let missing = PathBuf::from("/definitely/not/a/real/chat/path");
        let discovered = DiscoveredChatFiles::from_paths(std::slice::from_ref(&missing));

        assert!(discovered.files().is_empty());
        assert_eq!(discovered.skipped_paths(), &[missing]);
    }
}
