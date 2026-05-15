use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// File roundtrip test status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatus {
    /// The file has not been tested yet.
    #[serde(rename = "not_tested")]
    NotTested,
    /// The file passed the most recent test run.
    #[serde(rename = "passed")]
    Passed,
    /// The file failed the most recent test run.
    #[serde(rename = "failed")]
    Failed,
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileStatus::NotTested => write!(f, "NotTested"),
            FileStatus::Passed => write!(f, "Passed"),
            FileStatus::Failed => write!(f, "Failed"),
        }
    }
}

/// Failure reason for a roundtrip test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureReason {
    /// The source file could not be read.
    #[serde(rename = "read_error")]
    ReadError,
    /// Parsing the source file failed.
    #[serde(rename = "parse_error")]
    ParseError,
    /// Validation failed after parsing.
    #[serde(rename = "validation_error")]
    ValidationError,
    /// JSON output differed from the expected roundtrip output.
    #[serde(rename = "json_mismatch")]
    JsonMismatch,
    /// CHAT output differed from the expected roundtrip output.
    #[serde(rename = "chat_mismatch")]
    ChatMismatch,
}

impl std::fmt::Display for FailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailureReason::ReadError => write!(f, "ReadError"),
            FailureReason::ParseError => write!(f, "ParseError"),
            FailureReason::ValidationError => write!(f, "ValidationError"),
            FailureReason::JsonMismatch => write!(f, "JsonMismatch"),
            FailureReason::ChatMismatch => write!(f, "ChatMismatch"),
        }
    }
}

/// Error location information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorLocation {
    /// One-based line number of the failure location.
    pub line: usize,
    /// One-based column number of the failure location.
    pub column: usize,
    /// Optional source excerpt near the failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

/// Detailed error information for failed files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    /// Machine-readable error category.
    pub error_type: String,
    /// Human-readable error message.
    pub message: String,
    /// Optional source location for the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<ErrorLocation>,
    /// Optional short diff summary for mismatch failures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_summary: Option<String>,
}

impl ErrorDetail {
    /// Build a new error detail with type and message only.
    pub fn new(error_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error_type: error_type.into(),
            message: message.into(),
            location: None,
            diff_summary: None,
        }
    }

    /// Attach a line and column location to the error detail.
    pub fn with_location(mut self, line: usize, column: usize) -> Self {
        self.location = Some(ErrorLocation {
            line,
            column,
            context: None,
        });
        self
    }

    /// Attach a line, column, and short source context to the error detail.
    pub fn with_location_and_context(
        mut self,
        line: usize,
        column: usize,
        context: impl Into<String>,
    ) -> Self {
        self.location = Some(ErrorLocation {
            line,
            column,
            context: Some(context.into()),
        });
        self
    }

    /// Attach a short diff summary to the error detail.
    pub fn with_diff_summary(mut self, diff: impl Into<String>) -> Self {
        self.diff_summary = Some(diff.into());
        self
    }
}

/// Per-file manifest entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// Canonical file path key.
    pub path: String,
    /// Most recent test status.
    pub status: FileStatus,
    /// Optional structured failure reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<FailureReason>,
    /// Unix timestamp of the last test run.
    pub last_tested: Option<u64>,
    /// Optional detailed error payload from the last failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_detail: Option<ErrorDetail>,
}

/// Corpus entry containing aggregate metadata and tracked files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusEntry {
    /// Canonical corpus path key.
    pub path: String,
    /// Human-readable corpus directory name.
    pub name: String,
    /// Number of tracked `.cha` files in the corpus.
    pub file_count: usize,
    /// Number of files whose latest run passed.
    pub passed: usize,
    /// Number of files whose latest run failed.
    pub failed: usize,
    /// Number of files that have not been tested.
    pub not_tested: usize,
    /// Map of tracked files keyed by canonical path.
    pub files: BTreeMap<String, FileEntry>,
}

impl CorpusEntry {
    /// Return the pass rate among files that have been tested.
    pub fn pass_rate(&self) -> f64 {
        let tested = self.passed + self.failed;
        if tested > 0 {
            (self.passed as f64 / tested as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Return percentage progress across all tracked files.
    pub fn progress(&self) -> f64 {
        let tested = self.passed + self.failed;
        if self.file_count > 0 {
            (tested as f64 / self.file_count as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// Complete corpus manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusManifest {
    /// Manifest schema or package version string.
    pub version: String,
    /// Unix timestamp when the manifest was created.
    pub created_at: u64,
    /// Unix timestamp when the manifest was last updated.
    pub updated_at: u64,
    /// Number of tracked corpora.
    pub total_corpora: usize,
    /// Number of tracked files across all corpora.
    pub total_files: usize,
    /// Number of files whose latest run passed.
    pub total_passed: usize,
    /// Number of files whose latest run failed.
    pub total_failed: usize,
    /// Number of files not yet tested.
    pub total_not_tested: usize,
    /// All tracked corpora keyed by canonical path.
    pub corpora: BTreeMap<String, CorpusEntry>,
}
