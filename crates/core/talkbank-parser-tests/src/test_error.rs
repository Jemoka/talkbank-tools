//! Shared error type for parser-test binaries and integration tests.
//!
//! Unifies IO, parsing, serialisation, and assertion failures into a single
//! `Result` type so test functions can use `?` throughout.

use thiserror::Error;

/// Shared failure modes for parser-test binaries and integration suites.
#[derive(Debug, Error)]
pub enum TestError {
    /// File system operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// String formatting failed.
    #[error("Format error: {0}")]
    Fmt(#[from] std::fmt::Error),
    /// Required environment variable missing or invalid.
    #[error("Env var error: {0}")]
    EnvVar(#[from] std::env::VarError),
    /// CHAT parsing produced errors.
    #[error("Parse error: {0}")]
    Parse(#[from] talkbank_model::ParseErrors),
    /// Tree-sitter parser failed to initialise.
    #[error("Parser init error: {0}")]
    ParserInit(String),
    /// Snapshot serialization or deserialization failed.
    #[error("Snapshot serialization error: {0}")]
    Snapshot(#[from] serde_json::Error),
    /// General test assertion failure with message.
    #[error("Test failure: {0}")]
    Failure(String),
}

impl From<talkbank_parser::ParserInitError> for TestError {
    /// Convert parser initialization failures into `TestError`.
    fn from(err: talkbank_parser::ParserInitError) -> Self {
        TestError::ParserInit(err.to_string())
    }
}
