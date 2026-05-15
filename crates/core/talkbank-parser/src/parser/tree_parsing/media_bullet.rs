//! Media bullet parsing for tree-sitter parser
//!
//! This module handles parsing of media bullets (timestamp markers).
//! Media bullets mark time ranges in audio/video files: ·start_end·
//!
//! After grammar coarsening, `inline_bullet` and `media_url` are single token nodes
//! (not multi-child sequences). The shared `parse_bullet_text()` helper extracts
//! timestamps from the token text.
//!
//! The grammar rule for `bullet` does not permit any character other
//! than the two timestamps and the `_` separator between them. A
//! trailing `-` (legacy "skip" marker, removed from the grammar
//! 2026-03-31) is treated as a parse error rather than silently
//! stripped — stripping hid real data corruption and defeated the
//! grammar's purpose.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>

use crate::error::{ErrorCode, ErrorContext, ErrorVec, ParseError, Severity, SourceLocation, Span};
use crate::model::Bullet;
use tree_sitter::Node;

/// Bullet delimiter character (U+0015)
const BULLET_CHAR: char = '\u{15}';

/// Parse bullet text content from a token node.
///
/// Input format: `\u{15}START_END\u{15}` — exactly digits on both
/// sides of the underscore, nothing else. Returns `None` for any
/// deviation (including a trailing `-` or any non-digit byte in
/// either timestamp).
pub(crate) fn parse_bullet_text(text: &str) -> Option<(u64, u64)> {
    let inner = text.strip_prefix(BULLET_CHAR)?.strip_suffix(BULLET_CHAR)?;
    let (start_str, end_str) = inner.split_once('_')?;
    let start_ms = start_str.parse::<u64>().ok()?;
    let end_ms = end_str.parse::<u64>().ok()?;
    Some((start_ms, end_ms))
}

/// Extract `(start_ms, end_ms)` from a structured `bullet` CST node.
///
/// The grammar's `bullet` rule has field names `start_time` and `end_time`.
/// Returns `None` if either field is missing, unparseable, OR if the
/// bullet node (or any descendant) carries a tree-sitter ERROR node
/// — that latter case catches ill-formed bullets like the removed
/// `·\d+_\d+-·` skip marker, where the grammar reports ERROR on the
/// trailing `-` but the named fields still resolve. Without the
/// has_error gate we'd silently accept data that violates the
/// grammar.
pub(crate) fn parse_bullet_node_timestamps(node: Node, source: &str) -> Option<(u64, u64)> {
    if node.has_error() {
        return None;
    }
    let start_ms: u64 = node
        .child_by_field_name("start_time")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .and_then(|s| s.parse().ok())?;
    let end_ms: u64 = node
        .child_by_field_name("end_time")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .and_then(|s| s.parse().ok())?;
    Some((start_ms, end_ms))
}

/// Parse media_url node into Bullet
///
/// After grammar coarsening, `media_url` is a single token matching
/// `\u0015\d+_\d+-?\u0015`. We extract the node text and parse it
/// with `parse_bullet_text()`.
///
/// Format: ·start_end· or ·start_end-· (where · represents \u0015)
///
/// Returns: (Option<Bullet>, ErrorVec)
pub fn parse_media_bullet(node: Node, source: &str) -> (Option<Bullet>, ErrorVec) {
    let mut errors = ErrorVec::new();

    let text = match node.utf8_text(source.as_bytes()) {
        Ok(t) => t,
        Err(e) => {
            errors.push(ParseError::new(
                ErrorCode::InvalidMediaBullet,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!("UTF-8 decoding error in media bullet: {e}"),
            ));
            return (None, errors);
        }
    };

    let Some((start_ms, end_ms)) = parse_bullet_text(text) else {
        errors.push(ParseError::new(
            ErrorCode::InvalidMediaBullet,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), text),
            format!("Invalid media bullet: could not parse timestamps from '{text}'"),
        ));
        return (None, errors);
    };

    if start_ms == 0 && end_ms == 0 {
        errors.push(ParseError::new(
            ErrorCode::InvalidMediaBullet,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), text),
            "Invalid media bullet: could not parse timestamps (both start and end are 0)",
        ));
        return (None, errors);
    }

    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let bullet = Bullet::new(start_ms, end_ms).with_span(span);
    (Some(bullet), errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bullet_text_normal() {
        assert_eq!(parse_bullet_text("\u{15}123_456\u{15}"), Some((123, 456)));
    }

    /// Legacy skip dash (deprecated 2026-03-31) is NOT accepted.
    /// The grammar rejects it; `parse_bullet_text` must as well.
    #[test]
    fn test_parse_bullet_text_legacy_skip_rejected() {
        assert_eq!(parse_bullet_text("\u{15}123_456-\u{15}"), None);
    }

    /// Tests parse bullet text invalid.
    #[test]
    fn test_parse_bullet_text_invalid() {
        assert_eq!(parse_bullet_text("not a bullet"), None);
        assert_eq!(parse_bullet_text("\u{15}abc_def\u{15}"), None);
        assert_eq!(parse_bullet_text("\u{15}123\u{15}"), None);
        assert_eq!(parse_bullet_text(""), None);
    }
}
