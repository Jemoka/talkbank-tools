//! Error analysis specialized for dependent-tier failures.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use crate::error::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

/// Classifies one dependent-tier error node with optional tier context.
pub(crate) fn analyze_dependent_tier_error_with_context(
    error_node: Node,
    source: &str,
    tier_type: Option<&str>,
) -> ParseError {
    let start = error_node.start_byte();
    let end = error_node.end_byte();
    let error_text = match error_node.utf8_text(source.as_bytes()) {
        Ok(text) => text,
        Err(_) => {
            return ParseError::new(
                ErrorCode::InvalidControlCharacter,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, ""),
                "Could not decode dependent tier content as valid UTF-8",
            )
            .with_suggestion("Re-enter using Unicode standard characters");
        }
    };

    // E710: Invalid %gra - non-numeric index (entire tier is ERROR)
    // Pattern: ERROR node starts with %gra:
    if error_text.contains("%gra:") {
        return ParseError::new(
            ErrorCode::UnexpectedGrammarNode,
            Severity::Error,
            SourceLocation::from_offsets(start, end),
            ErrorContext::new(source, start..end, error_text),
            "Invalid GRA relation - non-numeric index",
        )
        .with_suggestion("GRA relation indices must be numbers (e.g., 1|2|SUBJ, not one|2|SUBJ)");
    }

    // E702: Invalid %mor format - missing pipe (ERROR within mor_word)
    // Pattern: space + letter(s) when in mor tier context
    // Example: ERROR(" n") within mor_word means "hello n|world" instead of "hello|x n|world"
    // Check if error node has actual content (not just whitespace) by checking byte length
    if tier_type == Some("mor") && !error_text.is_empty() && end > start {
        // ERROR node in mor tier with non-empty content = missing pipe
        return ParseError::new(
            ErrorCode::InvalidMorphologyFormat,
            Severity::Error,
            SourceLocation::from_offsets(start, end),
            ErrorContext::new(source, start..end, error_text),
            "Invalid MOR chunk format - missing pipe separator",
        )
        .with_suggestion("MOR chunks must have format: pos|stem (e.g., v|hello, n|world)");
    }

    // Double comma in dependent tier
    if error_text.contains(",,") {
        return ParseError::new(
            ErrorCode::ConsecutiveCommas,
            Severity::Error,
            SourceLocation::from_offsets(start, end),
            ErrorContext::new(source, start..end, error_text),
            "Double comma found in dependent tier",
        )
        .with_suggestion("Use single comma or replace ,, with special character");
    }

    // Generic dependent tier error
    ParseError::new(
        ErrorCode::UnparsableContent,
        Severity::Error,
        SourceLocation::from_offsets(start, end),
        ErrorContext::new(source, start..end, error_text),
        format!(
            "Unparsable content on dependent tier: '{}'",
            match error_text.lines().next() {
                Some(line) => line,
                None => error_text,
            }
        ),
    )
    .with_suggestion(
        "Check dependent tier format — each entry must follow the tier-specific syntax",
    )
}

/// Backward-compatible wrapper without explicit tier context.
pub(crate) fn analyze_dependent_tier_error(error_node: Node, source: &str) -> ParseError {
    analyze_dependent_tier_error_with_context(error_node, source, None)
}
