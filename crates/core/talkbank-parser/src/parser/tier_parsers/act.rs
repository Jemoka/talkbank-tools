//! Parser for `%act` action tiers.
//!
//! `%act` content is modeled as bullet-capable free text and is typically
//! aligned with events around the main tier.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Action_Code>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use talkbank_model::model::{ActTier, BulletContent};
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use tree_sitter::Node;

/// Converts one `%act` tier node into an `ActTier`.
///
/// **Grammar Rule:**
/// ```text
/// act_dependent_tier: seq('%', 'act', colon, tab, text_with_bullets, newline)
/// ```
///
/// **Expected Sequential Order:**
/// 1. '%' (position 0)
/// 2. 'act' (position 1)
/// 3. colon (position 2)
/// 4. tab (position 3)
/// 5. text_with_bullets (position 4) - content with inline bullets
/// 6. newline (position 5)
pub fn parse_act_tier(node: Node, source: &str, errors: &impl ErrorSink) -> ActTier {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

    let mut content = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "text_with_bullets" | "text_with_bullets_and_pics"
        ) {
            content = Some(parse_bullet_content(child, source, errors));
            break;
        }
    }

    let content = match content {
        Some(content) => content,
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(
                    source,
                    node.start_byte()..node.end_byte(),
                    "act_dependent_tier",
                ),
                "Missing content in %act tier".to_string(),
            ));
            BulletContent::from_text("")
        }
    };

    ActTier::new(content).with_span(span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::WriteChat;

    /// Tests act tier construction.
    #[test]
    fn test_act_tier_construction() {
        let tier = ActTier::from_text("picks up toy");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%act:\tpicks up toy");
    }

    /// Tests act tier with timing.
    #[test]
    fn test_act_tier_with_timing() {
        let tier = ActTier::from_text("<1w-2w> holds object out to Amy");
        assert!(!tier.content.is_empty());
        assert_eq!(
            tier.to_chat_string(),
            "%act:\t<1w-2w> holds object out to Amy"
        );
    }

    /// Tests act tier empty.
    #[test]
    fn test_act_tier_empty() {
        let tier = ActTier::from_text("");
        assert!(tier.is_empty());
        assert_eq!(tier.to_chat_string(), "%act:\t");
    }

    /// Tests act tier complex.
    #[test]
    fn test_act_tier_complex() {
        let tier = ActTier::from_text("<aft> manipulates chicken in hands");
        assert!(!tier.content.is_empty());
        assert_eq!(
            tier.to_chat_string(),
            "%act:\t<aft> manipulates chicken in hands"
        );
    }
}
