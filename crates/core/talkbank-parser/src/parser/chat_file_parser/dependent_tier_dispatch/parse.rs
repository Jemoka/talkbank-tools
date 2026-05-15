//! Dependent-tier dispatch that attaches parsed tiers onto a parent utterance.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::Utterance;
use crate::node_types::DEPENDENT_TIER;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::{parsed, raw, unparsed, user_defined};

/// Parse one dependent tier node and attach it to `utterance`.
pub(crate) fn parse_and_attach_dependent_tier(
    mut utterance: Utterance,
    dep_tier_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> Utterance {
    // With supertypes, we may receive either:
    // 1. A `dependent_tier` wrapper node (look at child(0) for the concrete type)
    // 2. A concrete tier type directly (use node itself)
    let (tier_node, tier_kind) = match resolve_tier_node(dep_tier_node, input, errors) {
        ParseOutcome::Parsed((node, kind)) => (node, kind),
        ParseOutcome::Rejected => return utterance,
    };

    if parsed::apply_parsed_tier(&mut utterance, tier_kind, tier_node, input, errors) {
        return utterance;
    }

    if raw::apply_raw_tier(&mut utterance, tier_kind, tier_node, input, errors) {
        return utterance;
    }

    if user_defined::apply_user_defined_tier(&mut utterance, tier_kind, tier_node, input, errors) {
        return utterance;
    }

    if unparsed::apply_unparsed_tier(&mut utterance, tier_kind, tier_node, input, errors) {
        return utterance;
    }

    errors.report(
        ParseError::new(
            ErrorCode::InvalidDependentTier,
            Severity::Error,
            SourceLocation::from_offsets(tier_node.start_byte(), tier_node.end_byte()),
            ErrorContext::new(
                input,
                tier_node.start_byte()..tier_node.end_byte(),
                tier_kind,
            ),
            format!(
                "Unknown dependent tier type '{}' - parser does not support this tier",
                tier_kind
            ),
        )
        .with_suggestion(
            "This tier type must be added to the parser before this file can be processed",
        ),
    );

    utterance
}

/// Resolve a possibly-wrapped `dependent_tier` choice node into its concrete tier node/kind.
fn resolve_tier_node<'a>(
    dep_tier_node: Node<'a>,
    input: &'a str,
    errors: &impl ErrorSink,
) -> ParseOutcome<(Node<'a>, &'a str)> {
    let node_kind = dep_tier_node.kind();
    if node_kind == DEPENDENT_TIER {
        if let Some(concrete) = dep_tier_node.child(0u32) {
            return ParseOutcome::parsed((concrete, concrete.kind()));
        }

        errors.report(ParseError::new(
            ErrorCode::InvalidDependentTier,
            Severity::Error,
            SourceLocation::from_offsets(dep_tier_node.start_byte(), dep_tier_node.end_byte()),
            ErrorContext::new(
                input,
                dep_tier_node.start_byte()..dep_tier_node.end_byte(),
                "",
            ),
            "dependent_tier choice node has no child",
        ));
        return ParseOutcome::rejected();
    }

    ParseOutcome::parsed((dep_tier_node, node_kind))
}
