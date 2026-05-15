//! Parse utterance CST nodes into model utterances with parse-health tainting.
//!
//! This file is the bridge between raw utterance CST and `model::Utterance`.
//! It performs three critical tasks:
//! 1. Builds the main tier from CST.
//! 2. Dispatches each dependent tier to typed parsers.
//! 3. Marks `ParseHealth` taint when dependency-bearing tiers are malformed.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use super::dependent_tier_dispatch::parse_and_attach_dependent_tier;
use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation,
};
use crate::model::{ParseHealth, ParseHealthTier, Utterance};
use crate::node_types::{
    DEPENDENT_TIER, GRA_DEPENDENT_TIER, MAIN_TIER, MOD_DEPENDENT_TIER, MOR_DEPENDENT_TIER,
    PHO_DEPENDENT_TIER, SIN_DEPENDENT_TIER, WOR_DEPENDENT_TIER, X_DEPENDENT_TIER,
};
use crate::parser::TreeSitterParser;
use crate::parser::tree_parsing::main_tier::structure::convert_main_tier_node;
use crate::parser::tree_parsing::parser_helpers::{
    analyze_dependent_tier_error, check_for_errors_recursive, is_dependent_tier,
};
use talkbank_model::ParseOutcome;

impl TreeSitterParser {
    /// Parse a CST utterance node into a model Utterance, streaming errors.
    pub fn parse_utterance_cst(
        &self,
        utt_node: tree_sitter::Node,
        input: &str,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Utterance> {
        parse_utterance_node(utt_node, input, errors)
    }
}

/// Builds one `Utterance` from a CST utterance subtree and attaches dependent tiers.
///
/// The parser keeps going after local tier failures, reports every error through
/// `errors`, and records taint on `ParseHealth` so downstream alignment logic can
/// treat this utterance conservatively.
pub fn parse_utterance_node(
    utt_node: tree_sitter::Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Utterance> {
    let mut utterance_builder: Option<Utterance> = None;
    let mut parse_health = ParseHealth::default();

    let mut utt_cursor = utt_node.walk();
    for utt_child in utt_node.children(&mut utt_cursor) {
        if utt_child.is_error() {
            let error_start = utt_child.start_byte();
            let error_end = utt_child.end_byte();
            let error_text = &input[error_start..error_end];

            if let Some(relative_at) = find_missing_form_type_offset(error_text) {
                let at_start = error_start + relative_at;
                let at_end = at_start + 1;
                errors.report(
                    ParseError::new(
                        ErrorCode::MissingFormType,
                        Severity::Error,
                        SourceLocation::from_offsets(at_start, at_end),
                        ErrorContext::new(input, at_start..at_end, "@"),
                        "Missing form type after @",
                    )
                    .with_suggestion("Add a form type after @ (e.g., @b for babbling)"),
                );
            } else if let Some((relative_start, relative_end)) =
                find_unclosed_replacement_offset(error_text)
            {
                let bracket_start = error_start + relative_start;
                let bracket_end = error_start + relative_end;
                errors.report(
                    ParseError::new(
                        ErrorCode::UnexpectedNode,
                        Severity::Error,
                        SourceLocation::from_offsets(bracket_start, bracket_end),
                        ErrorContext::new(
                            input,
                            bracket_start..bracket_end,
                            &input[bracket_start..bracket_end],
                        ),
                        "Unclosed replacement bracket",
                    )
                    .with_suggestion("Close replacement brackets and provide replacement text"),
                );
            } else if error_text.contains("[:]") {
                if let Some(relative) = error_text.find("[:]") {
                    let start = error_start + relative;
                    let end = start + 3;
                    errors.report(
                        ParseError::new(
                            ErrorCode::EmptyReplacement,
                            Severity::Error,
                            SourceLocation::from_offsets(start, end),
                            ErrorContext::new(input, start..end, "[:]"),
                            "Empty replacement in [: ] construct",
                        )
                        .with_suggestion(
                            "Add replacement text after [: , e.g., word [: corrected]",
                        ),
                    );
                } else {
                    errors.report(
                        ParseError::new(
                            ErrorCode::EmptyReplacement,
                            Severity::Error,
                            SourceLocation::from_offsets(error_start, error_end),
                            ErrorContext::new(input, error_start..error_end, error_text),
                            "Empty replacement in [: ] construct",
                        )
                        .with_suggestion(
                            "Add replacement text after [: , e.g., word [: corrected]",
                        ),
                    );
                }
            } else if matches!(error_text.chars().next(), Some('%')) {
                errors.report(analyze_dependent_tier_error(utt_child, input));
                match classify_percent_error_text(error_text) {
                    Some(tier) => parse_health.taint(tier),
                    None => parse_health.taint_all_alignment_dependents(),
                }
            } else {
                errors.report(ParseError::new(
                    ErrorCode::UnrecognizedUtteranceError,
                    Severity::Error,
                    SourceLocation::from_offsets(error_start, error_end),
                    ErrorContext::new(input, error_start..error_end, error_text),
                    format!(
                        "Unrecognized ERROR node in utterance: {}",
                        match error_text.lines().next() {
                            Some(line) => line,
                            None => error_text,
                        }
                    ),
                ));
                parse_health.taint(ParseHealthTier::Main);
            }
            continue;
        }

        if utt_child.kind() == MAIN_TIER {
            let line = &input[utt_child.start_byte()..utt_child.end_byte()];
            let main_tier_errors = ErrorCollector::new();
            let main_tier =
                convert_main_tier_node(utt_child, input, line, &main_tier_errors).into_option();
            let main_tier_error_vec = main_tier_errors.into_vec();
            if has_actual_errors(&main_tier_error_vec) {
                parse_health.taint(ParseHealthTier::Main);
            }
            errors.report_all(main_tier_error_vec);
            if let Some(main_tier) = main_tier {
                utterance_builder = Some(Utterance::new(main_tier));
            } else {
                parse_health.taint(ParseHealthTier::Main);
            }
        } else if is_dependent_tier(utt_child.kind()) {
            let mut tier_had_parse_errors = false;
            let dependent_tier = classify_dependent_tier_node(utt_child, input);

            let mut dep_cursor = utt_child.walk();
            for dep_child in utt_child.children(&mut dep_cursor) {
                if dep_child.is_error() {
                    errors.report(analyze_dependent_tier_error(dep_child, input));
                    tier_had_parse_errors = true;
                } else {
                    // check_for_errors_recursive needs to be converted to use ErrorSink
                    let mut temp_errors = Vec::new();
                    check_for_errors_recursive(dep_child, input, &mut temp_errors);
                    if has_actual_errors(&temp_errors) {
                        tier_had_parse_errors = true;
                    }
                    errors.report_all(temp_errors);
                }
            }

            if let Some(mut utt) = utterance_builder.take() {
                let tier_errors = ErrorCollector::new();
                utt = parse_and_attach_dependent_tier(utt, utt_child, input, &tier_errors);
                let tier_error_vec = tier_errors.into_vec();
                if has_actual_errors(&tier_error_vec) {
                    tier_had_parse_errors = true;
                }
                errors.report_all(tier_error_vec);
                utterance_builder = Some(utt);
            }

            if tier_had_parse_errors {
                match dependent_tier {
                    Some(tier) => parse_health.taint(tier),
                    None => parse_health.taint_all_alignment_dependents(),
                }
            }
        } else {
            errors.report(ParseError::new(
                ErrorCode::UnexpectedUtteranceChild,
                Severity::Error,
                SourceLocation::from_offsets(utt_child.start_byte(), utt_child.end_byte()),
                ErrorContext::new(
                    input,
                    utt_child.start_byte()..utt_child.end_byte(),
                    utt_child.kind(),
                ),
                format!("Unexpected child '{}' in utterance", utt_child.kind()),
            ));
            parse_health.taint(ParseHealthTier::Main);
        }
    }

    if let Some(mut utterance) = utterance_builder {
        utterance.parse_health = parse_health.into_state();
        ParseOutcome::parsed(utterance)
    } else {
        ParseOutcome::rejected()
    }
}

/// Return `true` when at least one diagnostic has `Severity::Error`.
fn has_actual_errors(errors: &[ParseError]) -> bool {
    errors
        .iter()
        .any(|error| matches!(error.severity, Severity::Error))
}

/// Best-effort tier classification for malformed `%tier` text from `ERROR` nodes.
pub(super) fn classify_percent_error_text(text: &str) -> Option<ParseHealthTier> {
    match dependent_tier_label_bytes(text)? {
        b"mor" => Some(ParseHealthTier::Mor),
        b"gra" => Some(ParseHealthTier::Gra),
        b"pho" => Some(ParseHealthTier::Pho),
        b"mod" | b"xmod" => Some(ParseHealthTier::Mod),
        b"wor" => Some(ParseHealthTier::Wor),
        b"sin" => Some(ParseHealthTier::Sin),
        _ => None,
    }
}

/// Extract the raw tier label bytes after `%` (e.g. `%mor` -> `b"mor"`).
fn dependent_tier_label_bytes(text: &str) -> Option<&[u8]> {
    let bytes = text.as_bytes();
    if bytes.first().copied() != Some(b'%') {
        return None;
    }

    let mut end = 1usize;
    while end < bytes.len() {
        match bytes[end] {
            b':' | b'\t' | b' ' | b'\r' | b'\n' => break,
            _ => end += 1,
        }
    }

    if end == 1 {
        return None;
    }

    Some(&bytes[1..end])
}

/// Find the byte offset of an `@` marker that is missing its form-type suffix.
fn find_missing_form_type_offset(error_text: &str) -> Option<usize> {
    let bytes = error_text.as_bytes();

    for idx in 0..bytes.len() {
        if bytes[idx] != b'@' {
            continue;
        }

        let missing = match bytes.get(idx + 1).copied() {
            None => true,
            Some(next) if next.is_ascii_whitespace() => true,
            Some(b'.' | b',' | b';' | b'!' | b'?' | b')' | b']') => true,
            _ => false,
        };

        if missing {
            return Some(idx);
        }
    }

    None
}

/// Find the span of an unclosed `[:` replacement marker.
fn find_unclosed_replacement_offset(error_text: &str) -> Option<(usize, usize)> {
    let bytes = error_text.as_bytes();
    let mut idx = 0usize;

    while idx + 1 < bytes.len() {
        if bytes[idx] == b'[' && bytes[idx + 1] == b':' {
            let has_closing = bytes[idx + 2..].contains(&b']');
            if !has_closing {
                return Some((idx, idx + 2));
            }
        }
        idx += 1;
    }

    None
}

/// Map a concrete dependent-tier CST node to its parse-health tier category.
fn classify_dependent_tier_node(node: tree_sitter::Node, input: &str) -> Option<ParseHealthTier> {
    let concrete = if node.kind() == DEPENDENT_TIER {
        node.child(0u32)?
    } else {
        node
    };

    match concrete.kind() {
        MOR_DEPENDENT_TIER => Some(ParseHealthTier::Mor),
        GRA_DEPENDENT_TIER => Some(ParseHealthTier::Gra),
        PHO_DEPENDENT_TIER => Some(ParseHealthTier::Pho),
        MOD_DEPENDENT_TIER => Some(ParseHealthTier::Mod),
        WOR_DEPENDENT_TIER => Some(ParseHealthTier::Wor),
        SIN_DEPENDENT_TIER => Some(ParseHealthTier::Sin),
        X_DEPENDENT_TIER => classify_x_tier_label(concrete, input),
        _ => None,
    }
}

/// Classify `%x...` tiers that map onto known alignment tiers (currently `%xmod`).
fn classify_x_tier_label(node: tree_sitter::Node, input: &str) -> Option<ParseHealthTier> {
    // x_tier_prefix is a single token like "%xmod" — extract label by stripping "%x"
    let prefix_node = node.child(0u32)?;
    let prefix_text = prefix_node.utf8_text(input.as_bytes()).ok()?;
    let label = prefix_text.strip_prefix("%x")?;
    if label == "mod" {
        Some(ParseHealthTier::Mod)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_percent_error_text, find_missing_form_type_offset,
        find_unclosed_replacement_offset,
    };
    use crate::model::ParseHealthTier;

    #[test]
    fn missing_form_type_offset_detects_lone_at() {
        assert_eq!(find_missing_form_type_offset("hello @ world"), Some(6));
        assert_eq!(find_missing_form_type_offset("@"), Some(0));
        assert_eq!(find_missing_form_type_offset("hello@"), Some(5));
    }

    #[test]
    fn missing_form_type_offset_skips_valid_marker_prefixes() {
        assert_eq!(find_missing_form_type_offset("hello@s:eng"), None);
        assert_eq!(find_missing_form_type_offset("word@b"), None);
    }

    #[test]
    fn unclosed_replacement_offset_detects_open_bracket_without_close() {
        assert_eq!(
            find_unclosed_replacement_offset("hello [: world"),
            Some((6, 8))
        );
        assert_eq!(find_unclosed_replacement_offset("[:]"), None);
        assert_eq!(find_unclosed_replacement_offset("hello [: fixed]"), None);
    }

    #[test]
    fn classify_percent_error_text_accepts_malformed_labels_without_colon() {
        assert_eq!(
            classify_percent_error_text("%mor no_tab_separator"),
            Some(ParseHealthTier::Mor)
        );
        assert_eq!(
            classify_percent_error_text("%gra no_tab_separator"),
            Some(ParseHealthTier::Gra)
        );
        assert_eq!(
            classify_percent_error_text("%pho no_tab_separator"),
            Some(ParseHealthTier::Pho)
        );
        assert_eq!(
            classify_percent_error_text("%wor no_tab_separator"),
            Some(ParseHealthTier::Wor)
        );
        assert_eq!(
            classify_percent_error_text("%xmod no_tab_separator"),
            Some(ParseHealthTier::Mod)
        );
        assert_eq!(classify_percent_error_text("%xfoo no_tab_separator"), None);
    }
}
