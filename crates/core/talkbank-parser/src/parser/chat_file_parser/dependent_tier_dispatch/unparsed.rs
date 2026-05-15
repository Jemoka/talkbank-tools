//! Dispatch for `%x...` tiers that remain unparsed or semi-parsed.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#User_Defined_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::dependent_tier::DependentTier;
use crate::model::{NonEmptyString, Utterance};
use crate::node_types::{FREE_TEXT, X_DEPENDENT_TIER};
use crate::parser::tier_parsers::pho::parse_mod_tier_from_unparsed;
use tree_sitter::Node;

/// Parse and attach `%x...` tiers, including `%xmod` fallback to `%mod`.
pub(super) fn apply_unparsed_tier(
    utterance: &mut Utterance,
    tier_kind: &str,
    tier_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> bool {
    if tier_kind != X_DEPENDENT_TIER {
        return false;
    }

    let tier_name = if let Some(name_node) = tier_node.child(1u32) {
        match name_node.utf8_text(input.as_bytes()) {
            Ok(text) => text,
            Err(_) => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(name_node.start_byte(), name_node.end_byte()),
                    ErrorContext::new(input, name_node.start_byte()..name_node.end_byte(), "tier"),
                    "Unparsed tier label is not valid UTF-8",
                ));
                return true;
            }
        }
    } else {
        ""
    };

    match tier_name {
        "mod" => {
            let tier = parse_mod_tier_from_unparsed(tier_node, input, errors);
            utterance.dependent_tiers.push(DependentTier::Mod(tier));
        }
        _ => {
            // Extract label - must be non-empty
            let label = match NonEmptyString::new(tier_name) {
                Some(l) => l,
                None => {
                    errors.report(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                        ErrorContext::new(
                            input,
                            tier_node.start_byte()..tier_node.end_byte(),
                            "tier",
                        ),
                        "Unparsed tier has empty label",
                    ));
                    return true;
                }
            };

            // Extract content - must be non-empty
            let mut content_text = "";
            let mut cursor = tier_node.walk();
            for child in tier_node.children(&mut cursor) {
                if child.kind() == FREE_TEXT {
                    content_text = match child.utf8_text(input.as_bytes()) {
                        Ok(text) => text,
                        Err(_) => {
                            errors.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                                ErrorContext::new(
                                    input,
                                    child.start_byte()..child.end_byte(),
                                    "tier",
                                ),
                                "Unparsed tier content is not valid UTF-8",
                            ));
                            return true;
                        }
                    };
                    break;
                }
            }

            let content = match NonEmptyString::new(content_text) {
                Some(c) => c,
                None => {
                    errors.report(ParseError::new(
                        ErrorCode::TreeParsingError,
                        Severity::Error,
                        SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                        ErrorContext::new(
                            input,
                            tier_node.start_byte()..tier_node.end_byte(),
                            "tier",
                        ),
                        format!("Unparsed tier %{} has empty content", tier_name),
                    ));
                    return true;
                }
            };

            let span =
                crate::error::Span::new(tier_node.start_byte() as u32, tier_node.end_byte() as u32);
            utterance.dependent_tiers.push(DependentTier::UserDefined(
                crate::model::UserDefinedDependentTier {
                    label,
                    content,
                    span,
                },
            ));
        }
    }

    true
}
