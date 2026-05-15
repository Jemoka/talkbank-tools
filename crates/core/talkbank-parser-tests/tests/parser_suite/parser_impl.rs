//! Parser implementation wrapper for integration tests.
//!
//! After the Chumsky direct parser removal (fa9623b), only TreeSitterParser
//! remains. The dual-parser comparison infrastructure has been collapsed to
//! a single-parser suite. ParserImpl is kept as a type alias for minimal
//! disruption to existing test code.

use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

pub const SAMPLE_WORD_COUNT: usize = 3;
pub const MOR_TIER_INPUT: &str = "pro|I v|want n|cookie-PL .";
pub const UTTERANCE_INPUT: &str = "*CHI:\tI want cookie .\n%mor:\tpro|I v|want n|cookie-PL .";

/// Type alias — was an enum wrapping both TreeSitter and Direct parsers.
/// Now just TreeSitterParser since the direct parser was removed.
pub type ParserImpl = TreeSitterParser;

/// Single-parser suite. Historically returned two parsers for cross-backend
/// comparison. Now returns one TreeSitterParser.
pub fn parser_suite() -> Result<Vec<ParserImpl>, TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    Ok(vec![parser])
}
