//! Shared parser-suite utilities for root integration tests.

use talkbank_model::{ChatFile, ErrorSink};
use talkbank_parser::ParserInitError;
use talkbank_parser::TreeSitterParser;

/// Shared initialization failure for test parser suites.
#[derive(Debug, thiserror::Error)]
pub enum ParserSuiteError {
    #[error("Failed to create TreeSitterParser: {source}")]
    TreeSitterInit { source: ParserInitError },
}

/// Wrapper around `TreeSitterParser` for test suites.
pub struct ParserImpl(pub TreeSitterParser);

impl ParserImpl {
    /// Returns the display name used in assertions.
    pub fn name(&self) -> &'static str {
        "tree-sitter"
    }

    /// Parse a CHAT file through the streaming API.
    pub fn parse_chat_file_streaming(
        &self,
        input: &str,
        errors: &impl ErrorSink,
    ) -> Option<ChatFile> {
        self.0.parse_chat_file_fragment(input, 0, errors).into()
    }
}

/// Build the standard root-test parser suite.
pub fn parser_suite() -> Result<Vec<ParserImpl>, ParserSuiteError> {
    let parser =
        TreeSitterParser::new().map_err(|source| ParserSuiteError::TreeSitterInit { source })?;
    Ok(vec![ParserImpl(parser)])
}
