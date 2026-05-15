//! Locates the first `main_tier` node in a parsed CHAT tree.
//!
//! The single-item APIs wrap user input into synthetic files; this helper walks
//! line/utterance wrappers so callers can project directly to the main-tier node.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use crate::error::{
    ErrorCode, ErrorContext, ParseError, ParseErrors, ParseResult, Severity, SourceLocation,
};
use crate::node_types::{LINE, MAIN_TIER, UTTERANCE};
use tree_sitter::{Node, Tree};

/// Returns the first `main_tier` node reachable under line/utterance wrappers.
#[allow(dead_code)] // Public API for single-item parse entry points
pub fn find_main_tier_node<'a>(tree: &'a Tree, _source: &str) -> ParseResult<Node<'a>> {
    let root = tree.root_node();
    let mut cursor = root.walk();

    for line_node in root.children(&mut cursor) {
        if line_node.kind() == LINE {
            let mut line_cursor = line_node.walk();
            for utterance_node in line_node.children(&mut line_cursor) {
                if utterance_node.kind() == UTTERANCE {
                    let mut utt_cursor = utterance_node.walk();
                    for main_tier_node in utterance_node.children(&mut utt_cursor) {
                        if main_tier_node.kind() == MAIN_TIER {
                            return Ok(main_tier_node);
                        }
                    }
                }
            }
        }
    }

    let mut errors = ParseErrors::new();
    errors.push(ParseError::new(
        ErrorCode::MissingMainTier,
        Severity::Error,
        SourceLocation::from_offsets(0, 0),
        ErrorContext::new("", 0..0, ""),
        "Could not find main_tier node in parse tree",
    ));
    Err(errors)
}
