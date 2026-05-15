//! Linker decoding for `main_tier` conversion.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::node_types::{
    CA_NO_BREAK_LINKER, CA_TECHNICAL_BREAK_LINKER, LINKER, LINKER_LAZY_OVERLAP,
    LINKER_QUICK_UPTAKE, LINKER_QUICK_UPTAKE_OVERLAP, LINKER_QUOTATION_FOLLOWS,
    LINKER_SELF_COMPLETION, WHITESPACES,
};
use tree_sitter::Node;

use crate::parser::tree_parsing::parser_helpers::is_linker;

/// Parse linker nodes into canonical `Linker` enum values.
pub(super) fn parse_linkers(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<crate::model::Linker> {
    let mut linkers = Vec::new();
    let child_count = node.child_count();
    let make_linker = |kind: &str| -> Option<crate::model::Linker> {
        match kind {
            LINKER_LAZY_OVERLAP => Some(crate::model::Linker::LazyOverlapPrecedes),
            LINKER_QUICK_UPTAKE => Some(crate::model::Linker::OtherCompletion), // ++ is "other completion" not "quick uptake" (+^ is quick uptake)
            LINKER_QUICK_UPTAKE_OVERLAP => Some(crate::model::Linker::QuickUptakeOverlap),
            LINKER_QUOTATION_FOLLOWS => Some(crate::model::Linker::QuotationFollows),
            LINKER_SELF_COMPLETION => Some(crate::model::Linker::SelfCompletion),
            CA_TECHNICAL_BREAK_LINKER => Some(crate::model::Linker::TcuContinuation),
            CA_NO_BREAK_LINKER => Some(crate::model::Linker::NoBreakTcuContinuation),
            _ => None,
        }
    };

    for idx in 0..child_count {
        if let Some(child) = node.child(idx as u32) {
            if child.kind() == LINKER {
                if let Some(linker_child) = child.child(0u32) {
                    if let Some(linker) = make_linker(linker_child.kind()) {
                        linkers.push(linker);
                    } else {
                        errors.report(ParseError::new(
                            ErrorCode::StructuralOrderError,
                            Severity::Error,
                            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                            ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                            format!("Unknown linker kind: {}", linker_child.kind()),
                        ));
                    }
                }
            } else if is_linker(child.kind()) {
                if let Some(linker) = make_linker(child.kind()) {
                    linkers.push(linker);
                } else {
                    errors.report(ParseError::new(
                        ErrorCode::StructuralOrderError,
                        Severity::Error,
                        SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                        ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                        format!("Unknown linker kind: {}", child.kind()),
                    ));
                }
            } else if child.kind() == WHITESPACES {
            } else {
                errors.report(ParseError::new(
                    ErrorCode::StructuralOrderError,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                    format!(
                        "Expected 'linker' or 'whitespaces' in linkers, found '{}'",
                        child.kind()
                    ),
                ));
            }
        }
    }

    linkers
}
