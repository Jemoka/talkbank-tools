//! Test module for helpers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use crate::test_utils::parser_suite::{
    ParserImpl, ParserSuiteError, parser_suite as shared_parser_suite,
};
use talkbank_model::ErrorCollector;
use talkbank_parser::ParserInitError;

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
    #[error("Parser {parser} returned None without errors")]
    ParseReturnedNone { parser: &'static str },
}

/// Parses chat file streaming or err.
pub fn parse_chat_file_streaming_or_err(
    parser: &ParserImpl,
    input: &str,
) -> Result<talkbank_model::ChatFile, TestError> {
    let errors = ErrorCollector::new();
    let chat_file =
        parser
            .parse_chat_file_streaming(input, &errors)
            .ok_or(TestError::ParseReturnedNone {
                parser: parser.name(),
            })?;

    let error_vec = errors.into_vec();
    if error_vec.is_empty() {
        Ok(chat_file)
    } else {
        Err(TestError::ParseErrors {
            parser: parser.name(),
            errors: talkbank_model::ParseErrors { errors: error_vec },
        })
    }
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
