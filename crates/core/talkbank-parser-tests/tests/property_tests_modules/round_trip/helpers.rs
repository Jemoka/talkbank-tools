//! Test module for helpers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// Thin wrapper around TreeSitterParser for round-trip testing.
pub struct Parser(pub TreeSitterParser);

impl Parser {
    /// Short backend label for snapshot names and failure messages.
    pub fn name(&self) -> &'static str {
        "tree-sitter"
    }
}

/// Returns the TreeSitterParser backend for round-trip testing.
pub fn parser_suite() -> Result<Vec<Parser>, TestError> {
    let tree_sitter =
        TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    Ok(vec![Parser(tree_sitter)])
}

/// Legacy helper - returns TreeSitterParser only (for backwards compatibility).
pub fn parser() -> Result<TreeSitterParser, TestError> {
    TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))
}
