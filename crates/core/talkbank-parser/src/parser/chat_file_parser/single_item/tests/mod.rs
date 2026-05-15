//! Test module for mod in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use crate::parser::{ParserInitError, TreeSitterParser};
use talkbank_model::{ErrorCode, ErrorContext, ParseError, ParseErrors, Severity, SourceLocation};

mod utterance;
mod word;

// Helper functions for tests - made pub so test submodules can use them
/// Parses word.
pub fn parse_word(input: &str) -> crate::error::ParseResult<crate::model::Word> {
    let parser = TreeSitterParser::new().map_err(parser_init_errors)?;
    parser.parse_word(input)
}

/// Parses utterance.
pub fn parse_utterance(input: &str) -> crate::error::ParseResult<crate::model::Utterance> {
    let parser = TreeSitterParser::new().map_err(parser_init_errors)?;
    parser.parse_utterance(input)
}

/// Parses main tier.
pub fn parse_main_tier(input: &str) -> crate::error::ParseResult<crate::model::MainTier> {
    let parser = TreeSitterParser::new().map_err(parser_init_errors)?;
    parser.parse_main_tier(input)
}

/// Converts parser-initialization failures into test-local `ParseErrors`.
fn parser_init_errors(err: ParserInitError) -> ParseErrors {
    ParseErrors::from(vec![ParseError::new(
        ErrorCode::TreeParsingError,
        Severity::Error,
        SourceLocation::from_offsets(0, 0),
        ErrorContext::new("", 0..0, "TreeSitterParser::new"),
        err.to_string(),
    )])
}

/// Executes a test closure with local snapshot path/settings overrides.
pub fn with_snapshot_settings<F: FnOnce()>(f: F) {
    let mut settings = insta::Settings::new();
    settings.set_snapshot_path("tests/snapshots");
    settings.set_prepend_module_to_snapshot(false);
    let _guard = settings.bind_to_scope();
    f();
}
