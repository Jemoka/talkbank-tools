//! Parsing for GEM-style headers (`@Bg`, `@Eg`, `@G`).
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#G_Header>

use crate::error::ErrorSink;
use crate::model::{self, Header};
use crate::node_types::*;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::helpers::parse_optional_gem_label;

/// Parse optional GEM label payload from known child nodes or raw fallback text.
fn parse_gem_label(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<model::GemLabel>> {
    let parsed =
        match parse_optional_gem_label(find_child_by_kind(header_actual, FREE_TEXT), input, errors)
        {
            ParseOutcome::Parsed(label) => label,
            ParseOutcome::Rejected => return ParseOutcome::rejected(),
        };

    if parsed.is_some() {
        return ParseOutcome::parsed(parsed);
    }

    let fallback = header_actual
        .utf8_text(input.as_bytes())
        .ok()
        .and_then(|raw| {
            let colon = raw.find(':')?;
            let label = raw[colon + 1..].trim_matches(|c| matches!(c, '\r' | '\n' | '\t' | ' '));
            if label.is_empty() {
                None
            } else {
                Some(model::GemLabel::new(label.to_string()))
            }
        });
    ParseOutcome::parsed(fallback)
}

/// Return whether raw header text contains a `:` separator.
fn header_contains_colon(header_actual: Node, input: &str) -> bool {
    header_actual
        .utf8_text(input.as_bytes())
        .ok()
        .is_some_and(|raw| raw.contains(':'))
}

/// Parse one GEM header into `BeginGem`, `EndGem`, or `LazyGem`.
pub(super) fn parse_gem_header(
    header_kind: &str,
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    match header_kind {
        BG_HEADER => {
            let label = match parse_gem_label(header_actual, input, errors) {
                ParseOutcome::Parsed(label) => label,
                ParseOutcome::Rejected => return ParseOutcome::rejected(),
            };
            if label.is_none() && header_contains_colon(header_actual, input) {
                // Recover @Bg: (empty label) as lazy gem semantics for validation (E530).
                ParseOutcome::parsed(Header::LazyGem { label: None })
            } else {
                ParseOutcome::parsed(Header::BeginGem { label })
            }
        }
        EG_HEADER => {
            let label = match parse_gem_label(header_actual, input, errors) {
                ParseOutcome::Parsed(label) => label,
                ParseOutcome::Rejected => return ParseOutcome::rejected(),
            };
            ParseOutcome::parsed(Header::EndGem { label })
        }
        G_HEADER => {
            let label = match parse_gem_label(header_actual, input, errors) {
                ParseOutcome::Parsed(label) => label,
                ParseOutcome::Rejected => return ParseOutcome::rejected(),
            };
            ParseOutcome::parsed(Header::LazyGem { label })
        }
        _ => ParseOutcome::rejected(),
    }
}

/// Find first direct child matching `kind`.
fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}
