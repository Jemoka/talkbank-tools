//! Field-level parser for `%gra` relation tuples.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use talkbank_model::ParseOutcome;
use talkbank_model::model::GrammaticalRelation;
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

/// Converts one `gra_relation` node (`index|head|label`) into `GrammaticalRelation`.
///
/// **Grammar Rule:**
/// ```text
/// gra_relation: seq(gra_index, '|', gra_head, '|', gra_relation_name)
/// ```
///
/// **Expected Sequential Order:**
/// 1. gra_index (position 0) - Word index (1-indexed)
/// 2. '|' (position 1)
/// 3. gra_head (position 2) - Head index (0 = ROOT)
/// 4. '|' (position 3)
/// 5. gra_relation_name (position 4) - Relation type (SUBJ, OBJ, etc.)
pub(super) fn parse_gra_relation(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<GrammaticalRelation> {
    let relation_span = node.start_byte()..node.end_byte();

    let index_text = match node.child(0u32) {
        Some(n) => match n.utf8_text(source.as_bytes()) {
            Ok(text) => text,
            Err(err) => {
                errors.report(ParseError::new(
                    ErrorCode::MalformedGrammarRelation,
                    Severity::Error,
                    SourceLocation::from_offsets(n.start_byte(), n.end_byte()),
                    ErrorContext::new(source, n.start_byte()..n.end_byte(), ""),
                    format!("UTF-8 decoding error in grammatical relation index: {err}"),
                ));
                return ParseOutcome::rejected();
            }
        },
        None => {
            errors.report(ParseError::new(
                ErrorCode::MalformedGrammarRelation,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, relation_span.clone(), ""),
                "Missing index in grammatical relation".to_string(),
            ));
            return ParseOutcome::rejected();
        }
    };

    let index = match index_text.parse::<usize>() {
        Ok(idx) => {
            if idx == 0 {
                errors.report(
                    ParseError::new(
                        ErrorCode::InvalidGrammarIndex,
                        Severity::Error,
                        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                        ErrorContext::new(source, relation_span.clone(), index_text),
                        "Index cannot be 0 (indices are 1-indexed)".to_string(),
                    )
                    .with_suggestion("Index must start at 1 for the first word"),
                );
                return ParseOutcome::rejected();
            } else {
                idx
            }
        }
        Err(_) => {
            errors.report(
                ParseError::new(
                    ErrorCode::MalformedGrammarRelation,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, relation_span.clone(), index_text),
                    format!("Invalid index '{}': must be a positive integer", index_text),
                )
                .with_suggestion("Index must be 1, 2, 3, ... (1-indexed)"),
            );
            return ParseOutcome::rejected();
        }
    };

    let head_text = match node.child(2u32) {
        Some(n) => match n.utf8_text(source.as_bytes()) {
            Ok(text) => text,
            Err(err) => {
                errors.report(ParseError::new(
                    ErrorCode::MalformedGrammarRelation,
                    Severity::Error,
                    SourceLocation::from_offsets(n.start_byte(), n.end_byte()),
                    ErrorContext::new(source, n.start_byte()..n.end_byte(), ""),
                    format!("UTF-8 decoding error in grammatical relation head: {err}"),
                ));
                return ParseOutcome::rejected();
            }
        },
        None => {
            errors.report(ParseError::new(
                ErrorCode::MalformedGrammarRelation,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, relation_span.clone(), ""),
                "Missing head in grammatical relation".to_string(),
            ));
            return ParseOutcome::rejected();
        }
    };

    let head = match head_text.parse::<usize>() {
        Ok(h) => h,
        Err(_) => {
            errors.report(
                ParseError::new(
                    ErrorCode::UnexpectedGrammarNode,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, relation_span.clone(), head_text),
                    format!(
                        "Invalid head '{}': must be a non-negative integer",
                        head_text
                    ),
                )
                .with_suggestion("Head must be 0 (ROOT) or a valid word index"),
            );
            return ParseOutcome::rejected();
        }
    };

    let relation_text = match node.child(4u32) {
        Some(n) => match n.utf8_text(source.as_bytes()) {
            Ok(text) => text,
            Err(err) => {
                errors.report(ParseError::new(
                    ErrorCode::MalformedGrammarRelation,
                    Severity::Error,
                    SourceLocation::from_offsets(n.start_byte(), n.end_byte()),
                    ErrorContext::new(source, n.start_byte()..n.end_byte(), ""),
                    format!("UTF-8 decoding error in grammatical relation label: {err}"),
                ));
                return ParseOutcome::rejected();
            }
        },
        None => {
            errors.report(ParseError::new(
                ErrorCode::MalformedGrammarRelation,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, relation_span.clone(), ""),
                "Missing relation name in grammatical relation".to_string(),
            ));
            return ParseOutcome::rejected();
        }
    };

    if relation_text.is_empty() {
        errors.report(ParseError::new(
            ErrorCode::MalformedGrammarRelation,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, relation_span, relation_text),
            "Missing grammatical relation label".to_string(),
        ));
        return ParseOutcome::rejected();
    }

    ParseOutcome::parsed(GrammaticalRelation::new(index, head, relation_text))
}
