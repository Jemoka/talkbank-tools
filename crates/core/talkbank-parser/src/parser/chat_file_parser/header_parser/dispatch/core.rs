//! Core header-node resolution and structural header decoding.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::Header;
use crate::node_types::*;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Resolve wrapper/supertype headers to the concrete header node and kind.
pub(super) fn resolve_header_node<'a>(
    header_node: Node<'a>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<(Node<'a>, &'a str)> {
    let node_kind = header_node.kind();
    if node_kind == HEADER {
        // Legacy wrapper mode - the concrete header is at child(0)
        if let Some(c) = header_node.child(0u32) {
            ParseOutcome::parsed((c, c.kind()))
        } else {
            errors.report(ParseError::new(
                ErrorCode::UnknownHeader,
                Severity::Error,
                SourceLocation::from_offsets(header_node.start_byte(), header_node.end_byte()),
                ErrorContext::new(input, header_node.start_byte()..header_node.end_byte(), ""),
                "header choice node has no child",
            ));
            ParseOutcome::rejected()
        }
    } else {
        // Supertypes mode - the node itself is the concrete header type
        ParseOutcome::parsed((header_node, node_kind))
    }
}

/// Parse headers that are represented as marker-only variants.
pub(super) fn parse_core_header(header_kind: &str) -> Option<Header> {
    match header_kind {
        UTF8_HEADER => Some(Header::Utf8),
        BEGIN_HEADER => Some(Header::Begin),
        END_HEADER => Some(Header::End),
        NEW_EPISODE_HEADER => Some(Header::NewEpisode),
        BLANK_HEADER => Some(Header::Blank),
        _ => None,
    }
}
