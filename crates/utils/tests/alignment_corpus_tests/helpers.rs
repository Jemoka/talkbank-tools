//! Test module for helpers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use crate::test_utils::parser_suite::{
    ParserImpl, ParserSuiteError, parser_suite as shared_parser_suite,
};
use std::fs;
use talkbank_model::model::ChatFile;
use talkbank_model::{ErrorCode, ErrorCollector, ParseError};
use talkbank_parser::{ParserInitError, TreeSitterParser};

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
pub enum TestError {
    #[error("Failed to create TreeSitterParser: {source}")]
    TreeSitterInit { source: ParserInitError },
    #[error("Parse failed for {parser}")]
    ParseErrors {
        parser: &'static str,
        errors: talkbank_model::ParseErrors,
    },
    #[error("Failed to read test file {path}: {source}")]
    ReadError {
        path: String,
        source: std::io::Error,
    },
    #[error("Failed to read directory {path}: {source}")]
    ReadDir {
        path: String,
        source: std::io::Error,
    },
    #[error("Missing filename for path {path}")]
    MissingFileName { path: String },
}

impl ParserImpl {
    /// Parses chat file result.
    pub fn parse_chat_file_result(&self, content: &str) -> Result<ChatFile, TestError> {
        self.0
            .parse_chat_file(content)
            .map_err(|errors| TestError::ParseErrors {
                parser: self.name(),
                errors,
            })
    }
}

/// Returns parser implementations for testing.
pub fn parser_suite() -> Result<Vec<ParserImpl>, TestError> {
    shared_parser_suite().map_err(map_parser_suite_error)
}

fn map_parser_suite_error(error: ParserSuiteError) -> TestError {
    match error {
        ParserSuiteError::TreeSitterInit { source } => TestError::TreeSitterInit { source },
    }
}

/// Validates chat file with alignment.
pub fn validate_chat_file_with_alignment(chat_file: &mut ChatFile) -> Vec<ParseError> {
    let errors = ErrorCollector::new();
    // TEMPORARY: skip %wor alignment - semantics still being worked out
    chat_file.validate_with_alignment(&errors, None);
    errors.into_vec()
}

/// Parses chat file.
pub fn parse_chat_file(content: &str) -> Result<ChatFile, TestError> {
    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    parser
        .parse_chat_file(content)
        .map_err(|errors| TestError::ParseErrors {
            parser: "tree-sitter",
            errors,
        })
}

/// Returns file.
pub fn read_file(path: &str) -> Result<String, TestError> {
    fs::read_to_string(path).map_err(|source| TestError::ReadError {
        path: path.to_string(),
        source,
    })
}

/// Returns whether alignment error.
pub fn is_alignment_error(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::UnexpectedTierNode
            | ErrorCode::TierBeginTimeNotMonotonic
            | ErrorCode::InvalidMorphologyFormat
            | ErrorCode::UnexpectedMorphologyNode
            | ErrorCode::SpeakerSelfOverlap
            | ErrorCode::MorCountMismatchTooFew
            | ErrorCode::MorCountMismatchTooMany
            | ErrorCode::MalformedGrammarRelation
            | ErrorCode::InvalidGrammarIndex
            | ErrorCode::UnexpectedGrammarNode
            | ErrorCode::GraInvalidWordIndex
            | ErrorCode::GraInvalidHeadIndex
            | ErrorCode::PhoCountMismatchTooFew
            | ErrorCode::PhoCountMismatchTooMany
            | ErrorCode::SinCountMismatchTooFew
            | ErrorCode::SinCountMismatchTooMany
            | ErrorCode::MorGraCountMismatch
    )
}
