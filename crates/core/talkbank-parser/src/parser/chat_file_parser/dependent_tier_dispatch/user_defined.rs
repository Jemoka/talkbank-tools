//! Dispatch for user-defined and unsupported dependent tiers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#User_Defined_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::dependent_tier::DependentTier;
use crate::model::{NonEmptyString, Utterance};
use crate::node_types::{
    TEXT_WITH_BULLETS, UNSUPPORTED_DEPENDENT_TIER, UNSUPPORTED_TIER_PREFIX, X_DEPENDENT_TIER,
    X_TIER_PREFIX,
};
use talkbank_model::model::dependent_tier::{
    PhoalnTier, SylTier, SylTierType, parse_phoaln_content, parse_syl_content,
};
use tree_sitter::Node;

/// Parse and attach user-defined/unsupported dependent tiers.
pub(super) fn apply_user_defined_tier(
    utterance: &mut Utterance,
    tier_kind: &str,
    tier_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> bool {
    match tier_kind {
        X_DEPENDENT_TIER => apply_x_tier(utterance, tier_node, input, errors),
        UNSUPPORTED_DEPENDENT_TIER => apply_unsupported_tier(utterance, tier_node, input, errors),
        _ => false,
    }
}

/// Handle user-defined %x* tiers (%xfoo, %xpho, %xmod, etc.).
/// The grammar now uses a single greedy token for the full prefix (e.g. "%xfoo"),
/// so the label is extracted by stripping the "%x" prefix from the token text.
fn apply_x_tier(
    utterance: &mut Utterance,
    tier_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> bool {
    // Grammar: x_dependent_tier = x_tier_prefix, tier_sep, text_with_bullets, newline
    // x_tier_prefix is a single token matching /%x[a-zA-Z][a-zA-Z0-9]*/
    let full_prefix = match find_child_by_kind(tier_node, X_TIER_PREFIX) {
        Some(n) => match n.utf8_text(input.as_bytes()) {
            Ok(text) => text,
            Err(_) => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(n.start_byte(), n.end_byte()),
                    ErrorContext::new(input, n.start_byte()..n.end_byte(), X_TIER_PREFIX),
                    "User-defined tier prefix is not valid UTF-8",
                ));
                return true;
            }
        },
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                "Missing tier prefix in user-defined tier",
            ));
            return true;
        }
    };

    // Extract label by stripping "%x" prefix (e.g. "%xfoo" → "foo")
    let tier_label = full_prefix.strip_prefix("%x").unwrap_or(full_prefix);

    let content_text = match find_child_by_kind(tier_node, TEXT_WITH_BULLETS) {
        Some(n) => match n.utf8_text(input.as_bytes()) {
            Ok(text) => text,
            Err(_) => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(n.start_byte(), n.end_byte()),
                    ErrorContext::new(input, n.start_byte()..n.end_byte(), TEXT_WITH_BULLETS),
                    "User-defined tier content is not valid UTF-8",
                ));
                return true;
            }
        },
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                format!("Missing content in user-defined tier %x{}", tier_label),
            ));
            return true;
        }
    };

    // Content must be non-empty
    let content = match NonEmptyString::new(content_text) {
        Some(c) => c,
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                format!("Empty content in user-defined tier %x{}", tier_label),
            ));
            return true;
        }
    };

    let span = crate::error::Span::new(tier_node.start_byte() as u32, tier_node.end_byte() as u32);

    // Intercept Phon project tiers (%xmodsyl, %xphosyl, %xphoaln) and route
    // them to structured types. When the 'x' prefix is eventually dropped
    // (global replace %xmodsyl → %modsyl), the grammar rules in raw.rs
    // take over seamlessly — both paths produce the same model types.
    match tier_label {
        "modsyl" => {
            let words = parse_syl_content(content.as_str());
            utterance.dependent_tiers.push(DependentTier::Modsyl(
                SylTier::new(SylTierType::Modsyl, words).with_span(span),
            ));
            return true;
        }
        "phosyl" => {
            let words = parse_syl_content(content.as_str());
            utterance.dependent_tiers.push(DependentTier::Phosyl(
                SylTier::new(SylTierType::Phosyl, words).with_span(span),
            ));
            return true;
        }
        "phoaln" => {
            match parse_phoaln_content(content.as_str()) {
                Ok(words) => {
                    utterance.dependent_tiers.push(DependentTier::Phoaln(
                        PhoalnTier::new(words).with_span(span),
                    ));
                }
                Err(e) => {
                    errors.report(ParseError::new(
                        ErrorCode::InvalidDependentTier,
                        Severity::Error,
                        SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                        ErrorContext::new(
                            input,
                            tier_node.start_byte()..tier_node.end_byte(),
                            "%xphoaln",
                        ),
                        format!("malformed %xphoaln content: {}", e),
                    ));
                }
            }
            return true;
        }
        _ => {}
    }

    // For UserDefined tiers, prepend 'x' to the label to avoid collision with built-in tiers
    // e.g., %xmor stores label="xmor" not "mor" to avoid collision with %mor
    let label = NonEmptyString::new_unchecked(format!("x{}", tier_label));

    let tier = DependentTier::UserDefined(crate::model::UserDefinedDependentTier {
        label,
        content,
        span,
    });

    utterance.dependent_tiers.push(tier);
    true
}

/// Handle unsupported dependent tiers (%custom, %foo, etc.) caught by the grammar catch-all.
/// These are stored as UserDefined tiers so the file can still be parsed.
fn apply_unsupported_tier(
    utterance: &mut Utterance,
    tier_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> bool {
    // Extract the tier prefix (e.g. "%custom") from unsupported_tier_prefix child
    let label_text = match find_child_by_kind(tier_node, UNSUPPORTED_TIER_PREFIX) {
        Some(n) => match n.utf8_text(input.as_bytes()) {
            Ok(text) => {
                // Strip the leading '%'
                text.strip_prefix('%').unwrap_or(text)
            }
            Err(_) => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(n.start_byte(), n.end_byte()),
                    ErrorContext::new(input, n.start_byte()..n.end_byte(), UNSUPPORTED_TIER_PREFIX),
                    "Unsupported tier prefix is not valid UTF-8",
                ));
                return true;
            }
        },
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                "Missing tier prefix in unsupported dependent tier",
            ));
            return true;
        }
    };

    // Get the raw content (everything after colon+tab to newline)
    let content_text = match tier_node.utf8_text(input.as_bytes()) {
        Ok(text) => text.trim().to_string(),
        Err(_) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                format!(
                    "Unsupported tier %{} content is not valid UTF-8",
                    label_text
                ),
            ));
            return true;
        }
    };

    // Extract just the content part after the ":\t"
    let content_str = match content_text.split_once(":\t") {
        Some((_, c)) => c.trim(),
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                format!(
                    "Missing colon-tab separator in unsupported tier %{}",
                    label_text
                ),
            ));
            return true;
        }
    };

    let content = match NonEmptyString::new(content_str) {
        Some(c) => c,
        None => {
            // Empty unsupported tier — skip it
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
                ErrorContext::new(input, tier_node.start_byte()..tier_node.end_byte(), ""),
                format!("Empty unsupported dependent tier %{}", label_text),
            ));
            return true;
        }
    };

    let label = NonEmptyString::new_unchecked(label_text);
    let span = crate::error::Span::new(tier_node.start_byte() as u32, tier_node.end_byte() as u32);
    let tier = DependentTier::Unsupported(crate::model::UserDefinedDependentTier {
        label,
        content,
        span,
    });

    utterance.dependent_tiers.push(tier);
    true
}

/// Find first direct child matching `kind`.
fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}
