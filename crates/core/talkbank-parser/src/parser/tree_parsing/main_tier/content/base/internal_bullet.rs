//! Parsing for media bullets embedded in main-tier content.

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::UtteranceContent;
use crate::parser::tree_parsing::media_bullet::parse_bullet_node_timestamps;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Converts a structured `bullet` node into `UtteranceContent::InternalBullet`.
pub(crate) fn parse_internal_bullet(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let Some((start_ms, end_ms)) = parse_bullet_node_timestamps(node, source) else {
        errors.report(ParseError::new(
            ErrorCode::InvalidMediaBullet,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            "Invalid bullet: could not extract timestamps",
        ));
        return ParseOutcome::rejected();
    };

    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let bullet = crate::model::Bullet::new(start_ms, end_ms).with_span(span);
    ParseOutcome::parsed(UtteranceContent::InternalBullet(bullet))
}
