use std::path::PathBuf;
use thiserror::Error;

/// Errors for corpus manifest operations.
#[derive(Debug, Error)]
pub enum ManifestError {
    /// Reading the manifest file from disk failed.
    #[error("Failed to read manifest: {path:?}")]
    Read {
        /// Path of the manifest file that could not be read.
        path: PathBuf,
        /// Underlying I/O failure.
        source: std::io::Error,
    },
    /// Writing the manifest file to disk failed.
    #[error("Failed to write manifest: {path:?}")]
    Write {
        /// Path of the manifest file that could not be written.
        path: PathBuf,
        /// Underlying I/O failure.
        source: std::io::Error,
    },
    /// Deserializing manifest JSON failed.
    #[error("Failed to parse manifest: {path:?}")]
    Parse {
        /// Path of the manifest file that contained invalid JSON.
        path: PathBuf,
        /// Underlying JSON parse failure.
        source: serde_json::Error,
    },
    /// Serializing manifest data to JSON failed.
    #[error("Failed to serialize manifest")]
    Serialize {
        /// Underlying JSON serialization failure.
        source: serde_json::Error,
    },
    /// Reading the current system time failed.
    #[error("Invalid system time")]
    Time {
        /// Underlying system time conversion failure.
        source: std::time::SystemTimeError,
    },
    /// A filesystem path could not be represented as UTF-8 text.
    #[error("Invalid corpus path: {path:?}")]
    InvalidPath {
        /// Filesystem path that could not be converted to UTF-8.
        path: PathBuf,
    },
    /// A corpus directory did not have a valid terminal name.
    #[error("Invalid corpus name: {path:?}")]
    InvalidName {
        /// Corpus path whose final component was invalid or missing.
        path: PathBuf,
    },
    /// The requested corpus key is not present in the manifest.
    #[error("Corpus not found: {path}")]
    MissingCorpus {
        /// Canonical corpus key that was requested.
        path: String,
    },
    /// The requested file key is not present in the manifest.
    #[error("File not found: {path}")]
    MissingFile {
        /// Canonical file key that was requested.
        path: String,
    },
    /// Enumerating a directory failed while scanning corpus files.
    #[error("Failed to read directory: {path:?}")]
    ReadDir {
        /// Directory path that could not be enumerated.
        path: PathBuf,
        /// Underlying I/O failure.
        source: std::io::Error,
    },
    /// Reading one directory entry failed while scanning corpus files.
    #[error("Failed to read directory entry: {path:?}")]
    ReadEntry {
        /// Directory whose entry iteration failed.
        path: PathBuf,
        /// Underlying I/O failure.
        source: std::io::Error,
    },
}
