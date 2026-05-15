//! Shared error type for root-workspace integration tests.
//!
//! Mirrors `talkbank-parser-tests::TestError` but with a smaller variant set,
//! covering only IO, parsing, and assertion failures.

use thiserror::Error;

/// Enum variants for TestError.
#[derive(Debug, Error)]
pub enum TestError {
    /// File system operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// CHAT parsing produced errors.
    #[error("Parse error: {0}")]
    Parse(#[from] talkbank_model::ParseErrors),
    /// General test assertion failure with message.
    #[error("Failure: {0}")]
    Failure(String),
}
