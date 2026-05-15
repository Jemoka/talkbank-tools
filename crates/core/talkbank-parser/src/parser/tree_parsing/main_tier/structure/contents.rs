//! Parse `contents` subtrees into `UtteranceContent` sequences.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::UtteranceContent;
use crate::node_types::{
    CA_CONTINUATION_MARKER, CA_NO_BREAK, CA_TECHNICAL_BREAK, COLON, COMMA, CONTENT_ITEM,
    FALLING_TO_LOW, FALLING_TO_MID, LEVEL_PITCH, NON_COLON_SEPARATOR, OVERLAP_POINT,
    RISING_TO_HIGH, RISING_TO_MID, SEMICOLON, SEPARATOR, TAG_MARKER, UNMARKED_ENDING,
    UPTAKE_SYMBOL, VOCATIVE_MARKER, WHITESPACES,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::super::parser_helpers::parse_separator_like;
use super::super::content::{analyze_word_error, parse_overlap_point};
use crate::parser::tree_parsing::helpers::unexpected_node_error;

/// Parse main-tier `contents` nodes into ordered `UtteranceContent` items.
///
/// The `contents` rule collects words, separators, overlap markers, and other inline tokens described
/// in the Main Tier section of the manual. We walk its children, accept either wrapped `content_item`
/// nodes or the granular leaf nodes the serializer sometimes produces, and report structural errors
/// when the tree diverges from the specification. When we encounter parser `ERROR` fragments (common
/// around overlapped markers such as `⌈2`), we attempt to glue them to the preceding word token so the
/// resulting `UtteranceContent` still matches the manual’s lookahead expectations.
pub fn parse_main_tier_contents(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<UtteranceContent> {
    let child_count = node.child_count();
    let mut content = Vec::with_capacity(child_count);

    for idx in 0..child_count {
        let Some(child) = node.child(idx as u32) else {
            continue;
        };

        if child.is_error() {
            if attach_error_suffix_to_previous_word(child, source, &mut content) {
                continue;
            }
            errors.report(analyze_word_error(child, source));
            continue;
        }

        match child.kind() {
            CONTENT_ITEM => {
                if let ParseOutcome::Parsed(item) = parse_content_item(child, source, errors) {
                    content.push(item);
                }
            }
            // Fallback: accept direct overlap/separator nodes
            OVERLAP_POINT
            | SEPARATOR
            | NON_COLON_SEPARATOR
            | COLON
            | COMMA
            | SEMICOLON
            | TAG_MARKER
            | VOCATIVE_MARKER
            | CA_CONTINUATION_MARKER
            | UNMARKED_ENDING
            | UPTAKE_SYMBOL
            | CA_NO_BREAK
            | CA_TECHNICAL_BREAK
            | RISING_TO_HIGH
            | RISING_TO_MID
            | LEVEL_PITCH
            | FALLING_TO_MID
            | FALLING_TO_LOW => {
                if let ParseOutcome::Parsed(item) = parse_content_item(child, source, errors) {
                    content.push(item);
                }
            }
            WHITESPACES => continue,
            _ => {
                errors.report(ParseError::new(
                    ErrorCode::StructuralOrderError,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                    format!("Unexpected '{}' in contents", child.kind()),
                ));
            }
        }
    }

    content
}

/// Attach compact error fragments to the previous word token when the parser emits a split marker.
///
/// Tree-sitter sometimes splits tokens such as `@x` into a word plus a trailing `ERROR` node. When the
/// fragment looks like part of the originating word, we append it so downstream tools reproduce the
/// manual’s tokens exactly and avoid duplicate diagnostics.
fn attach_error_suffix_to_previous_word(
    error_node: Node,
    source: &str,
    content: &mut [UtteranceContent],
) -> bool {
    let Ok(error_text) = error_node.utf8_text(source.as_bytes()) else {
        return false;
    };

    let Some(last) = content.last_mut() else {
        return false;
    };

    match last {
        UtteranceContent::Word(word)
            if should_attach_error_fragment(word.raw_text(), error_text) =>
        {
            let new_raw = format!("{}{}", word.raw_text(), error_text);
            word.set_raw_text(new_raw);
            true
        }
        UtteranceContent::AnnotatedWord(annotated)
            if should_attach_error_fragment(annotated.inner.raw_text(), error_text) =>
        {
            let new_raw = format!("{}{}", annotated.inner.raw_text(), error_text);
            annotated.inner.set_raw_text(new_raw);
            true
        }
        UtteranceContent::ReplacedWord(replaced)
            if should_attach_error_fragment(replaced.word.raw_text(), error_text) =>
        {
            let new_raw = format!("{}{}", replaced.word.raw_text(), error_text);
            replaced.word.set_raw_text(new_raw);
            true
        }
        _ => false,
    }
}

/// Decide whether an `ERROR` fragment should be bound to the preceding word.
///
/// We only attach non-whitespace fragments that either start with `@` or extend an `@`-suffix already
/// present on the word so the parser’s recovery logic stays consistent with CHAT tag notation.
fn should_attach_error_fragment(existing_raw: &str, fragment: &str) -> bool {
    if fragment.is_empty() || fragment.bytes().any(|b| b.is_ascii_whitespace()) {
        return false;
    }

    // Always keep explicit @-suffix fragments attached to the originating word.
    if fragment.starts_with('@') {
        return true;
    }

    // Recovery for split marker tails like hello@x + ERROR("yz").
    existing_raw.contains('@')
        && fragment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b':' | b'+' | b'&' | b'-' | b'_'))
}

/// Parse a `content_item` or compatible leaf node into `UtteranceContent`.
///
/// This helper mirrors the grammar alternatives listed in the Main Tier section: base content, groups,
/// quotations, phonology/syntax groups, separators, and overlap points. When the parser emits bare
/// separators or overlap markers directly (without `content_item` wrappers) we still accept them so the
/// model stays faithful to the grammar’s concrete tokens.
/// Parse a `content_item` or compatible leaf node into `UtteranceContent`.
///
/// Mirrors the main tier grammar described in the CHAT manual by handling base content, groups,
/// quotations, phonology/syntax groups, separators, and overlap points. When the tree-sitter parser
/// emits bare separator/overlap tokens directly (without a `content_item` wrapper) we still consume
/// them to ensure the resulting `UtteranceContent` list matches the concrete syntax the manual defines.
fn parse_content_item(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    use super::super::content::{
        parse_base_content, parse_group_content, parse_pho_group_content, parse_quotation_content,
        parse_sin_group_content,
    };
    use crate::node_types::{
        BASE_CONTENT_ITEM, GROUP_WITH_ANNOTATIONS, MAIN_PHO_GROUP, MAIN_SIN_GROUP, QUOTATION,
    };

    // CRITICAL FIX: Handle the node itself if it's a leaf node (e.g., bare COLON, SEPARATOR)
    // This is needed because the serializer outputs canonical spacing like "⌈2 :" where
    // the colon appears as a bare child of contents, not wrapped in a content_item node.
    match node.kind() {
        SEPARATOR => {
            if let ParseOutcome::Parsed(sep) = parse_separator_like(node, source, errors) {
                return ParseOutcome::parsed(UtteranceContent::Separator(sep));
            }
            return ParseOutcome::rejected();
        }
        NON_COLON_SEPARATOR
        | COLON
        | COMMA
        | SEMICOLON
        | TAG_MARKER
        | VOCATIVE_MARKER
        | CA_CONTINUATION_MARKER
        | UNMARKED_ENDING
        | UPTAKE_SYMBOL
        | CA_NO_BREAK
        | CA_TECHNICAL_BREAK
        | RISING_TO_HIGH
        | RISING_TO_MID
        | LEVEL_PITCH
        | FALLING_TO_MID
        | FALLING_TO_LOW => {
            if let ParseOutcome::Parsed(sep) = parse_separator_like(node, source, errors) {
                return ParseOutcome::parsed(UtteranceContent::Separator(sep));
            }
            return ParseOutcome::rejected();
        }
        OVERLAP_POINT => {
            return parse_overlap_point(node, source, errors);
        }
        BASE_CONTENT_ITEM => return parse_base_content(node, source, errors),
        GROUP_WITH_ANNOTATIONS => return parse_group_content(node, source, errors),
        MAIN_PHO_GROUP => return parse_pho_group_content(node, source, errors),
        MAIN_SIN_GROUP => return parse_sin_group_content(node, source, errors),
        QUOTATION => return parse_quotation_content(node, source, errors),
        // content_item is a supertype wrapper — fall through to iterate its children below
        CONTENT_ITEM => {}
        _ => {
            errors.report(unexpected_node_error(node, source, "content item"));
            return ParseOutcome::rejected();
        }
    }

    // If not a leaf node, iterate over children
    let child_count = node.child_count();

    for idx in 0..child_count {
        let Some(child) = node.child(idx as u32) else {
            continue;
        };

        if child.is_error() {
            errors.report(analyze_word_error(child, source));
            return ParseOutcome::rejected();
        }

        match child.kind() {
            BASE_CONTENT_ITEM => return parse_base_content(child, source, errors),
            GROUP_WITH_ANNOTATIONS => return parse_group_content(child, source, errors),
            MAIN_PHO_GROUP => return parse_pho_group_content(child, source, errors),
            MAIN_SIN_GROUP => return parse_sin_group_content(child, source, errors),
            QUOTATION => return parse_quotation_content(child, source, errors),
            OVERLAP_POINT => {
                return parse_overlap_point(child, source, errors);
            }
            SEPARATOR => {
                if let ParseOutcome::Parsed(sep) = parse_separator_like(child, source, errors) {
                    return ParseOutcome::parsed(UtteranceContent::Separator(sep));
                }
                return ParseOutcome::rejected();
            }
            NON_COLON_SEPARATOR
            | COLON
            | COMMA
            | SEMICOLON
            | TAG_MARKER
            | VOCATIVE_MARKER
            | CA_CONTINUATION_MARKER
            | UNMARKED_ENDING
            | UPTAKE_SYMBOL
            | RISING_TO_HIGH
            | RISING_TO_MID
            | LEVEL_PITCH
            | FALLING_TO_MID
            | FALLING_TO_LOW => {
                if let ParseOutcome::Parsed(sep) = parse_separator_like(child, source, errors) {
                    return ParseOutcome::parsed(UtteranceContent::Separator(sep));
                }
                return ParseOutcome::rejected();
            }
            WHITESPACES => continue,
            _ => {
                errors.report(unexpected_node_error(child, source, "content item child"));
                return ParseOutcome::rejected();
            }
        }
    }

    ParseOutcome::rejected()
}
