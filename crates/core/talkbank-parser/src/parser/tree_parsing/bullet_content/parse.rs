//! Converts `text_with_bullets` CST nodes into structured `BulletContent`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::node_types::*;
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use smallvec::SmallVec;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{BulletContent, BulletContentSegment};
use tree_sitter::Node;

use super::inline_bullet::parse_inline_bullet;
use super::inline_pic::parse_inline_pic;

/// Converts a bullet-capable text node into `BulletContent`.
///
/// **Grammar Rules:**
/// ```text
/// text_with_bullets: $ => repeat1(choice(
///   $.text_segment,
///   $.inline_bullet,
///   $.continuation
/// )),
///
/// text_with_bullets_and_pics: $ => repeat1(choice(
///   $.text_segment,
///   $.inline_bullet,
///   $.inline_pic,
///   $.continuation
/// ))
/// ```
///
/// **Expected Sequential Order:**
/// - `repeat1(choice(...))` means 1+ segments in any order
/// - Each child is one of: text_segment, inline_bullet, inline_pic, continuation
///
/// **Returns:** BulletContent (errors streamed via ErrorSink)
///
/// **Error Recovery:**
/// - Invalid bullet timestamps → Report E515, skip bullet, continue
/// - Invalid picture filename → Report E999, skip picture, continue
/// - Unexpected node types → Report E999, skip node, continue
/// - Missing content → Return empty BulletContent with error
pub fn parse_bullet_content(node: Node, source: &str, errors: &impl ErrorSink) -> BulletContent {
    let mut segments = SmallVec::<[BulletContentSegment; 4]>::new();

    // Verify node type
    let node_kind = node.kind();
    if node_kind != TEXT_WITH_BULLETS && node_kind != TEXT_WITH_BULLETS_AND_PICS {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node_kind),
            format!(
                "Expected text_with_bullets or text_with_bullets_and_pics, got: {}",
                node_kind
            ),
        ));
        return BulletContent::from_text("");
    }

    let child_count = node.child_count();
    let mut idx = 0;

    while idx < child_count {
        let child = match node.child(idx as u32) {
            Some(c) => c,
            None => {
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                    format!(
                        "Failed to access child at position {} in bullet_content",
                        idx
                    ),
                ));
                idx += 1;
                continue;
            }
        };
        let child_kind = child.kind();

        match child_kind {
            TEXT_SEGMENT => {
                // Extract plain text from text_segment node
                let text = extract_utf8_text(child, source, errors, "text_segment", "");
                if !text.is_empty() {
                    segments.push(BulletContentSegment::text(text));
                }
                idx += 1;
            }

            BULLET => {
                // Parse structured bullet: seq(bullet_start, start_time, '_', end_time, bullet_end)
                if let ParseOutcome::Parsed((start_ms, end_ms)) =
                    parse_inline_bullet(child, source, errors)
                {
                    segments.push(BulletContentSegment::bullet(start_ms, end_ms));
                }
                idx += 1;
            }

            INLINE_PIC => {
                // Parse inline pic: seq(bullet_end, pic_marker, '"', pic_filename, '"', bullet_end)
                if let ParseOutcome::Parsed(filename) = parse_inline_pic(child, source, errors) {
                    segments.push(BulletContentSegment::picture(filename));
                }
                idx += 1;
            }

            CONTINUATION => {
                // Preserve continuation markers for roundtrip fidelity
                segments.push(BulletContentSegment::continuation());
                idx += 1;
            }

            _ => {
                // Unexpected node type - report error and skip
                errors.report(ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), child_kind),
                    format!(
                        "Unexpected node type '{}' at position {} in bullet content",
                        child_kind, idx
                    ),
                ));
                idx += 1;
            }
        }
    }

    BulletContent::new(segments.into_vec())
}
