//! Corpus manifest for tracking roundtrip test status across multiple corpora.

mod errors;
mod ops;
mod types;

pub use errors::ManifestError;
pub use types::{
    CorpusEntry, CorpusManifest, ErrorDetail, ErrorLocation, FailureReason, FileEntry, FileStatus,
};

#[cfg(test)]
mod tests {
    use super::*;
    use thiserror::Error;

    /// Test-only failure cases for manifest fixture assertions.
    #[derive(Debug, Error)]
    enum TestError {
        #[error("Manifest error")]
        Manifest(#[from] ManifestError),
        #[error("Serde error")]
        Serde(#[from] serde_json::Error),
        #[error("Missing location")]
        MissingLocation,
        #[error("Missing context")]
        MissingContext,
        #[error("Missing diff summary")]
        MissingDiffSummary,
    }

    #[test]
    fn test_manifest_creation() -> Result<(), TestError> {
        let manifest = CorpusManifest::new()?;
        assert_eq!(manifest.total_corpora, 0);
        assert_eq!(manifest.total_files, 0);
        Ok(())
    }

    #[test]
    fn test_file_status_display() -> Result<(), TestError> {
        assert_eq!(FileStatus::NotTested.to_string(), "NotTested");
        assert_eq!(FileStatus::Passed.to_string(), "Passed");
        assert_eq!(FileStatus::Failed.to_string(), "Failed");
        Ok(())
    }

    #[test]
    fn test_error_detail_builder() -> Result<(), TestError> {
        let detail = ErrorDetail::new("ParseError", "Unexpected token")
            .with_location(10, 5)
            .with_diff_summary("Missing closing bracket");

        assert_eq!(detail.error_type, "ParseError");
        assert_eq!(detail.message, "Unexpected token");
        let location = detail.location.as_ref().ok_or(TestError::MissingLocation)?;
        assert_eq!(location.line, 10);
        assert_eq!(location.column, 5);
        let diff_summary = detail
            .diff_summary
            .as_ref()
            .ok_or(TestError::MissingDiffSummary)?;
        assert_eq!(diff_summary, "Missing closing bracket");
        Ok(())
    }

    #[test]
    fn test_error_detail_with_context() -> Result<(), TestError> {
        let detail = ErrorDetail::new("ValidationError", "Invalid speaker code")
            .with_location_and_context(15, 3, "  *CHI:\thello\n  *XXX:\tworld");

        let location = detail.location.as_ref().ok_or(TestError::MissingLocation)?;
        assert_eq!(location.line, 15);
        assert_eq!(location.column, 3);
        let context = location.context.as_ref().ok_or(TestError::MissingContext)?;
        assert!(context.contains("*CHI"));
        Ok(())
    }

    #[test]
    fn test_error_detail_serialization() -> Result<(), TestError> {
        let detail = ErrorDetail::new("ChatMismatch", "Roundtrip mismatch")
            .with_location(5, 10)
            .with_diff_summary("Expected 'hello' but got 'helo'");

        let value = serde_json::to_value(&detail)?;
        assert_eq!(
            value.get("error_type").and_then(|value| value.as_str()),
            Some("ChatMismatch")
        );
        assert_eq!(
            value.get("message").and_then(|value| value.as_str()),
            Some("Roundtrip mismatch")
        );

        let deserialized: ErrorDetail = serde_json::from_value(value)?;
        assert_eq!(deserialized.error_type, "ChatMismatch");
        assert_eq!(deserialized.message, "Roundtrip mismatch");
        Ok(())
    }
}
