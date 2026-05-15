//! Test module for helpers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

pub use crate::test_utils::parser_suite::ParserImpl;
use crate::test_utils::parser_suite::{ParserSuiteError, parser_suite as shared_parser_suite};
use talkbank_parser::ParserInitError;

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
pub enum TestError {
    #[error("Missing environment variable {name}: {source}")]
    #[allow(dead_code)]
    MissingEnvVar {
        name: &'static str,
        source: std::env::VarError,
    },
    #[error("Failed to read file {path}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },
    #[error("Failed to create TreeSitterParser: {source}")]
    TreeSitterInit { source: ParserInitError },
    #[error("Parse failed for {parser}")]
    ParseErrors {
        parser: &'static str,
        errors: talkbank_model::ParseErrors,
    },
    #[error("Missing participant {participant} for parser {parser}")]
    MissingParticipant {
        parser: &'static str,
        participant: &'static str,
    },
}

/// Returns both parser implementations for testing
pub fn parser_suite() -> Result<Vec<ParserImpl>, TestError> {
    shared_parser_suite().map_err(map_parser_suite_error)
}

fn map_parser_suite_error(error: ParserSuiteError) -> TestError {
    match error {
        ParserSuiteError::TreeSitterInit { source } => TestError::TreeSitterInit { source },
    }
}
