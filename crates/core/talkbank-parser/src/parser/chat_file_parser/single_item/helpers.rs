//! Shared helpers for single-item parse entry points.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use super::TreeSitterParser;
use crate::error::{
    ErrorCode, ErrorContext, ParseError, ParseErrors, ParseResult, Severity, SourceLocation,
};
use crate::parser::tree_parsing::main_tier::structure::find_main_tier_node;

/// Minimal valid CHAT file headers required for parsing isolated items
///
/// A truly minimal valid CHAT file must have:
/// 1. @UTF8
/// 2. @Begin
/// 3. @Languages: (at least one language)
/// 4. @Participants: (at least one participant)
/// 5. @ID: (matching the participant)
/// 6. @End
pub(crate) const MINIMAL_CHAT_PREFIX: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n";
/// Closing `@End` marker that terminates the synthetic wrapper document.
pub(crate) const MINIMAL_CHAT_SUFFIX: &str = "@End\n";

/// Parse a synthetic wrapper document and return its CST.
pub(super) fn _parse_tree(
    parser: &TreeSitterParser,
    input: &str,
    wrapped: &str,
) -> ParseResult<tree_sitter::Tree> {
    parser
        .parser
        .borrow_mut()
        .parse(wrapped, None)
        .ok_or_else(|| {
            let mut errors = ParseErrors::new();
            errors.push(ParseError::new(
                ErrorCode::UnexpectedNode,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                "Tree-sitter parse failed",
            ));
            errors
        })
}

/// Locate the `main_tier` node in a parsed wrapper tree.
pub(super) fn _find_main_tier_node_in_tree<'a>(
    tree: &'a tree_sitter::Tree,
    wrapped: &'a str,
) -> ParseResult<tree_sitter::Node<'a>> {
    find_main_tier_node(tree, wrapped)
}
