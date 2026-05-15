//! Parser for `%sin` tier bodies.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Gestures>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Group>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::node_types::{SIN_GROUP, SIN_GROUPS, WHITESPACES};
use talkbank_model::model::SinTier;
use talkbank_model::{ErrorSink, Span};
use tree_sitter::Node;

use super::groups::extract_sin_group_items;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::check_not_missing;

/// Converts one `%sin` tier node into `SinTier`.
///
/// **Grammar Rule:**
/// ```text
/// sin_dependent_tier: seq('%', 'sin', colon, tab, sin_groups, newline)
/// ```
///
/// **Expected Sequential Order:**
/// 1. '%' (position 0)
/// 2. 'sin' (position 1)
/// 3. colon (position 2)
/// 4. tab (position 3)
/// 5. sin_groups (position 4)
/// 6. newline (position 5)
pub fn parse_sin_tier(node: Node, source: &str, errors: &impl ErrorSink) -> SinTier {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

    let mut sin_groups = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == SIN_GROUPS {
            sin_groups = Some(child);
            break;
        }
    }

    let sin_groups = match sin_groups {
        Some(groups) => groups,
        None => {
            return SinTier::new(Vec::new()).with_span(span);
        }
    };

    let child_count = sin_groups.child_count();
    let mut items = Vec::with_capacity(child_count / 2 + 1);
    let mut idx = 0;

    while idx < child_count {
        if let Some(child) = sin_groups.child(idx as u32) {
            // CRITICAL: Check for MISSING nodes before processing
            if !check_not_missing(child, source, errors, "sin_groups") {
                idx += 1;
                continue;
            }

            match child.kind() {
                SIN_GROUP => {
                    let group_items = extract_sin_group_items(child, source, errors);
                    items.extend(group_items);
                }
                WHITESPACES => {}
                _ => {
                    errors.report(unexpected_node_error(child, source, "sin_groups"));
                }
            }
        }
        idx += 1;
    }

    SinTier::new(items).with_span(span)
}
