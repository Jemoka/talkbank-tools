//! Parser for `%cod` coding tiers.
//!
//! `%cod` carries analyst-defined coding content and reuses the same
//! bullet-capable free-text structure as `%act`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Coding_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use talkbank_model::model::{BulletContent, CodTier};
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use tree_sitter::Node;

/// Converts one `%cod` tier node into a `CodTier`.
///
/// **Grammar Rule:**
/// ```text
/// cod_dependent_tier: seq('%', 'cod', colon, tab, text_with_bullets, newline)
/// ```
///
/// **Expected Sequential Order:**
/// 1. '%' (position 0)
/// 2. 'cod' (position 1)
/// 3. colon (position 2)
/// 4. tab (position 3)
/// 5. text_with_bullets (position 4) - content with inline bullets
/// 6. newline (position 5)
pub fn parse_cod_tier(node: Node, source: &str, errors: &impl ErrorSink) -> CodTier {
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
                    "cod_dependent_tier",
                ),
                "Missing content in %cod tier".to_string(),
            ));
            BulletContent::from_text("")
        }
    };

    CodTier::new(content).with_span(span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::WriteChat;

    /// Tests cod tier construction.
    #[test]
    fn test_cod_tier_construction() {
        let tier = CodTier::from_text("general coding");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\tgeneral coding");
    }

    /// Tests cod tier single index.
    #[test]
    fn test_cod_tier_single_index() {
        let tier = CodTier::from_text("<1> atul");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<1> atul");
    }

    /// Tests cod tier compound index.
    #[test]
    fn test_cod_tier_compound_index() {
        let tier = CodTier::from_text("<1+2> eje");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<1+2> eje");
    }

    /// Tests cod tier multiple indices.
    #[test]
    fn test_cod_tier_multiple_indices() {
        let tier = CodTier::from_text("<1 , 3> atul");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<1 , 3> atul");
    }

    /// Tests cod tier complex.
    #[test]
    fn test_cod_tier_complex() {
        let tier = CodTier::from_text("<2 , 7> ledet <8> Itamar");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<2 , 7> ledet <8> Itamar");
    }

    /// Tests cod tier empty.
    #[test]
    fn test_cod_tier_empty() {
        let tier = CodTier::from_text("");
        assert!(tier.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t");
    }
}
