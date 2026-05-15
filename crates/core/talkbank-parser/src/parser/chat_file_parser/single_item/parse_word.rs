//! Parse isolated CHAT words via multi-root grammar.
//!
//! With `standalone_word` in the source_file union, a bare word like `hello`
//! or `&-uh` is parsed directly — no synthetic wrapper needed.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>

use super::TreeSitterParser;
use crate::error::{
    ErrorCode, ErrorContext, ParseError, ParseErrors, ParseResult, Severity, SourceLocation,
};
use crate::model::Word;
use crate::node_types::STANDALONE_WORD;
use crate::parser::tree_parsing::main_tier::word::convert_word_node;

/// Parse a single word token directly via the multi-root grammar.
///
/// The input (e.g., `hello`, `&-uh`, `hel(lo)`) is parsed directly as a
/// `standalone_word` fragment. No synthetic `*CHI:\t... .` wrapper is needed.
pub(super) fn parse_word(parser: &TreeSitterParser, input: &str) -> ParseResult<Word> {
    if input.is_empty() {
        let mut errors = ParseErrors::new();
        errors.push(ParseError::new(
            ErrorCode::InvalidWordFormat,
            Severity::Error,
            SourceLocation::from_offsets(0, 0),
            ErrorContext::new(input, 0..0, input),
            "Empty word input",
        ));
        return Err(errors);
    }

    let tree = {
        let mut ts_parser = parser.parser.borrow_mut();
        ts_parser.parse(input.as_bytes(), None).ok_or_else(|| {
            let mut errors = ParseErrors::new();
            errors.push(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                "Tree-sitter parse returned None",
            ));
            errors
        })?
    };

    // Navigate: source_file → standalone_word
    let root = tree.root_node();
    if root.kind() != "source_file" {
        let mut errors = ParseErrors::new();
        errors.push(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(0, input.len()),
            ErrorContext::new(input, 0..input.len(), input),
            format!("Expected source_file root, got '{}'", root.kind()),
        ));
        return Err(errors);
    }

    let word_node = root
        .child(0)
        .filter(|c| c.kind() == STANDALONE_WORD)
        .ok_or_else(|| {
            let mut errors = ParseErrors::new();
            let actual = root
                .child(0)
                .map(|c| c.kind().to_string())
                .unwrap_or_default();
            errors.push(ParseError::new(
                ErrorCode::InvalidWordFormat,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                format!("Expected standalone_word fragment, got '{actual}'"),
            ));
            errors
        })?;

    // Verify the parse consumed the entire input (no trailing ERROR nodes)
    if root.child_count() > 1 || word_node.has_error() {
        let mut errors = ParseErrors::new();
        errors.push(ParseError::new(
            ErrorCode::InvalidWordFormat,
            Severity::Error,
            SourceLocation::from_offsets(0, input.len()),
            ErrorContext::new(input, 0..input.len(), input),
            "Word input contains unparsable content",
        ));
        return Err(errors);
    }

    // Convert the standalone_word CST node directly to Word model
    let errors_sink = crate::error::ErrorCollector::new();
    let outcome = convert_word_node(word_node, input, &errors_sink);

    let tier_errors = errors_sink.into_vec();
    let has_actual_errors = tier_errors
        .iter()
        .any(|e| matches!(e.severity, Severity::Error));

    let word = outcome.into_option();

    if has_actual_errors || word.is_none() {
        let mut errors = ParseErrors::new();
        errors.errors.extend(tier_errors);
        if word.is_none() && errors.is_empty() {
            errors.push(ParseError::new(
                ErrorCode::InvalidWordFormat,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                "Failed to build word from parse tree",
            ));
        }
        return Err(errors);
    }

    word.ok_or_else(|| {
        let mut errors = ParseErrors::new();
        errors.push(ParseError::new(
            ErrorCode::InvalidWordFormat,
            Severity::Error,
            SourceLocation::from_offsets(0, input.len()),
            ErrorContext::new(input, 0..input.len(), input),
            "Failed to build word from parse tree",
        ));
        errors
    })
}
