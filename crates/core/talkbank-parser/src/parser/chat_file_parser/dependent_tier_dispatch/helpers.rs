//! Shared helper routines for dependent-tier dispatch.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::NonEmptyString;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Extract content from a simple unparsed tier node using CST navigation.
///
/// Grammar for simple unparsed tiers: seq('%', tier_code, ':', '\t', 'free_text', '\n')
/// - Position 0: %
/// - Position 1: tier_code
/// - Position 2: :
/// - Position 3: \t
/// - Position 4: free_text (content)
/// - Position 5: \n
///
/// Returns `Some(NonEmptyString)` if content exists, is non-empty, and is valid UTF-8.
/// Returns `None` and reports error if content is missing, empty, or UTF-8 extraction fails.
///
/// The `NonEmptyString` type enforces that tier content cannot be empty (correct by construction).
pub(crate) fn extract_unparsed_tier_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<NonEmptyString> {
    let mut content_node = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "free_text" | "text_with_bullets" | "text_with_bullets_and_pics"
        ) {
            content_node = Some(child);
            break;
        }
    }

    let content_node = match content_node {
        Some(n) => n,
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "tier"),
                "Tier is missing content node",
            ));
            return ParseOutcome::rejected();
        }
    };

    let text = match content_node.utf8_text(source.as_bytes()) {
        Ok(t) => t,
        Err(e) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(content_node.start_byte(), content_node.end_byte()),
                ErrorContext::new(
                    source,
                    content_node.start_byte()..content_node.end_byte(),
                    "tier_content",
                ),
                format!("Failed to extract UTF-8 text from tier content: {}", e),
            ));
            return ParseOutcome::rejected();
        }
    };

    // Use NonEmptyString::new() which returns None for empty content
    match NonEmptyString::new(text) {
        Some(content) => ParseOutcome::parsed(content),
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "tier"),
                "Tier has empty content",
            ));
            ParseOutcome::rejected()
        }
    }
}
