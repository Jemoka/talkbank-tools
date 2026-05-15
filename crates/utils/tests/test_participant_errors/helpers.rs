//! Test module for helpers in `talkbank-tools`.
//!
//! These tests document expected behavior and regressions.

use crate::test_utils::parser_suite::ParserImpl;
use talkbank_parser::{ParserInitError, TreeSitterParser};

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
pub enum TestError {
    #[error("Failed to read error corpus file {path}: {source}")]
    ReadError {
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
    #[error("Expected parse to fail for parser {parser}, but it succeeded")]
    UnexpectedParseSuccess { parser: &'static str },
}

/// Returns error corpus.
pub fn load_error_corpus(path: &str) -> Result<String, TestError> {
    std::fs::read_to_string(path).map_err(|source| TestError::ReadError {
        path: path.to_string(),
        source,
    })
}

impl ParserImpl {
    /// Parses chat file result.
    pub fn parse_chat_file_result(
        &self,
        content: &str,
    ) -> Result<talkbank_model::ChatFile, TestError> {
        self.0
            .parse_chat_file(content)
            .map_err(|errors| TestError::ParseErrors {
                parser: self.name(),
                errors,
            })
    }
}

/// Returns the parser for validation tests.
pub fn validation_parser_suite() -> Result<Vec<ParserImpl>, TestError> {
    TreeSitterParser::new()
        .map(|parser| vec![ParserImpl(parser)])
        .map_err(|source| TestError::TreeSitterInit { source })
}
