//! `%gra` tier-level parsing logic.
//!
//! Converts one `%gra` line into a `GraTier` by decoding each whitespace-
//! separated `index|head|relation` triple.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use crate::node_types as kind;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{GraTier, GraTierType, GrammaticalRelation};
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use tree_sitter::Node;

use super::relation::parse_gra_relation;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::check_not_missing;

/// Converts one `%gra` tier node into `GraTier`.
///
/// **Grammar Rule:**
/// ```text
/// gra_dependent_tier: seq('%', 'gra', colon, tab, gra_contents, newline)
/// ```
///
/// **Expected Sequential Order:**
/// 1. '%' (position 0)
/// 2. 'gra' (position 1)
/// 3. colon (position 2)
/// 4. tab (position 3)
/// 5. gra_contents (position 4)
/// 6. newline (position 5)
pub fn parse_gra_tier(node: Node, source: &str, errors: &impl ErrorSink) -> GraTier {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

    let mut gra_contents = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind::GRA_CONTENTS {
            gra_contents = Some(child);
            break;
        }
    }

    let gra_contents = match gra_contents {
        Some(contents) => contents,
        None => {
            errors.report(ParseError::new(
                ErrorCode::MalformedGrammarRelation,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                "Missing gra_contents node in %gra tier".to_string(),
            ));
            return GraTier::new(GraTierType::Gra, Vec::new()).with_span(span);
        }
    };

    let child_count = gra_contents.child_count();
    let mut relations: Vec<GrammaticalRelation> = Vec::with_capacity(child_count / 2 + 1);
    let mut idx = 0;

    while idx < child_count {
        if let Some(child) = gra_contents.child(idx as u32) {
            // CRITICAL: Check for MISSING nodes before processing
            if !check_not_missing(child, source, errors, "gra_contents") {
                idx += 1;
                continue;
            }

            match child.kind() {
                kind::GRA_RELATION => {
                    if let ParseOutcome::Parsed(relation) =
                        parse_gra_relation(child, source, errors)
                    {
                        relations.push(relation);
                    }
                }
                kind::WHITESPACES => {}
                _ => {
                    errors.report(unexpected_node_error(child, source, "gra_contents"));
                }
            }
        }
        idx += 1;
    }

    GraTier::new(GraTierType::Gra, relations).with_span(span)
}
