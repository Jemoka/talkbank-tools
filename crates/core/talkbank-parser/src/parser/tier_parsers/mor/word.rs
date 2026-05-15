//! Word-level `%mor` parsing.
//!
//! Parses a morphology token into POS, lemma, and optional feature list.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

use talkbank_model::ParseOutcome;
use talkbank_model::model::dependent_tier::{MorFeature, MorWord, PosCategory};
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

use crate::node_types as kind;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::check_not_missing;

/// Converts a `mor_word` CST node into `MorWord`.
///
/// **Grammar Rule:**
/// ```text
/// mor_word: $ => seq(
///     $.mor_pos,
///     $.pipe,
///     $.mor_lemma,
///     repeat($.mor_feature)
/// )
/// ```
pub fn parse_mor_word(node: Node, source: &str, errors: &impl ErrorSink) -> ParseOutcome<MorWord> {
    let mut pos: Option<&str> = None;
    let mut lemma: Option<&str> = None;
    let mut features = Vec::new();

    let child_count = node.child_count();
    let mut idx = 0;

    while idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            if !check_not_missing(child, source, errors, "mor_word") {
                idx += 1;
                continue;
            }

            match child.kind() {
                kind::MOR_POS => match child.utf8_text(source.as_bytes()) {
                    Ok(text) if !text.is_empty() => {
                        pos = Some(text);
                    }
                    Ok(_) => {
                        errors.report(ParseError::new(
                            ErrorCode::MissingRequiredElement,
                            Severity::Error,
                            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                            ErrorContext::new(
                                source,
                                child.start_byte()..child.end_byte(),
                                child.kind(),
                            ),
                            "MOR word has empty POS tag",
                        ));
                    }
                    Err(e) => {
                        errors.report(ParseError::new(
                            ErrorCode::TreeParsingError,
                            Severity::Error,
                            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                            ErrorContext::new(
                                source,
                                child.start_byte()..child.end_byte(),
                                child.kind(),
                            ),
                            format!("Failed to read MOR POS text: {e}"),
                        ));
                    }
                },
                kind::PIPE => {}
                kind::MOR_LEMMA => match child.utf8_text(source.as_bytes()) {
                    Ok(text) if !text.is_empty() => {
                        lemma = Some(text);
                    }
                    Ok(_) => {
                        errors.report(ParseError::new(
                            ErrorCode::MissingRequiredElement,
                            Severity::Error,
                            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                            ErrorContext::new(
                                source,
                                child.start_byte()..child.end_byte(),
                                child.kind(),
                            ),
                            "MOR word has empty lemma",
                        ));
                    }
                    Err(e) => {
                        errors.report(ParseError::new(
                            ErrorCode::TreeParsingError,
                            Severity::Error,
                            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                            ErrorContext::new(
                                source,
                                child.start_byte()..child.end_byte(),
                                child.kind(),
                            ),
                            format!("Failed to read MOR lemma text: {e}"),
                        ));
                    }
                },
                kind::MOR_FEATURE => {
                    if let ParseOutcome::Parsed(feature) = parse_mor_feature(child, source, errors)
                        && let Some(feature) = feature
                    {
                        features.push(feature);
                    }
                }
                _ => {
                    errors.report(unexpected_node_error(child, source, "mor_word"));
                }
            }
        }
        idx += 1;
    }

    let Some(pos) = pos else {
        errors.report(ParseError::new(
            ErrorCode::MissingRequiredElement,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            "MOR word is missing required POS tag",
        ));
        return ParseOutcome::rejected();
    };

    let Some(lemma) = lemma else {
        errors.report(ParseError::new(
            ErrorCode::MissingRequiredElement,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            "MOR word is missing required lemma",
        ));
        return ParseOutcome::rejected();
    };

    ParseOutcome::parsed(MorWord::new(PosCategory::new(pos), lemma).with_features(features))
}

/// Converts one `mor_feature` CST node (`-feature`).
///
/// Returns a [`MorFeature`] wrapping the feature value text (without the leading hyphen).
fn parse_mor_feature(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<MorFeature>> {
    let child_count = node.child_count();
    let mut idx = 0;

    while idx < child_count {
        if let Some(child) = node.child(idx as u32) {
            match child.kind() {
                kind::HYPHEN => {}
                kind::MOR_FEATURE_VALUE => match child.utf8_text(source.as_bytes()) {
                    Ok(text) if !text.is_empty() => {
                        return ParseOutcome::parsed(Some(MorFeature::new(text)));
                    }
                    Ok(_) => {
                        errors.report(unexpected_node_error(
                            child,
                            source,
                            "mor_feature_value empty",
                        ));
                    }
                    Err(_) => {
                        errors.report(unexpected_node_error(
                            child,
                            source,
                            "mor_feature_value utf8 error",
                        ));
                    }
                },
                _ => {
                    errors.report(unexpected_node_error(child, source, "mor_feature"));
                }
            }
        }
        idx += 1;
    }

    ParseOutcome::parsed(None)
}
