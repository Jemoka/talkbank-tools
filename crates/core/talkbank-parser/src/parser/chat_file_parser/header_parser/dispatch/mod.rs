//! Header-node dispatch pipeline.
//!
//! Routes each header CST node through core/structured/special/GEM/simple
//! handlers until one produces a `Header`.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>

mod core;
mod gem;
mod simple;
mod special;
mod structured;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::model::Header;
use talkbank_model::ParseOutcome;

/// Parse a header CST node into a typed `Header`.
pub fn parse_header_node(
    header_node: tree_sitter::Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let (header_actual, header_kind) = match core::resolve_header_node(header_node, input, errors) {
        ParseOutcome::Parsed(resolved) => resolved,
        ParseOutcome::Rejected => return ParseOutcome::rejected(),
    };

    if let Some(header) = core::parse_core_header(header_kind) {
        return ParseOutcome::parsed(header);
    }

    if let ParseOutcome::Parsed(header) =
        structured::parse_structured_header(header_kind, header_actual, input, errors)
    {
        return ParseOutcome::parsed(header);
    }

    if let ParseOutcome::Parsed(header) =
        special::parse_special_header(header_kind, header_actual, input, errors)
    {
        return ParseOutcome::parsed(header);
    }

    if let ParseOutcome::Parsed(header) =
        gem::parse_gem_header(header_kind, header_actual, input, errors)
    {
        return ParseOutcome::parsed(header);
    }

    if let ParseOutcome::Parsed(header) =
        simple::parse_simple_header(header_kind, header_actual, input, errors)
    {
        return ParseOutcome::parsed(header);
    }

    if header_kind.ends_with("_header") {
        errors.report(ParseError::new(
            ErrorCode::UnknownHeader,
            Severity::Error,
            SourceLocation::from_offsets(header_actual.start_byte(), header_actual.end_byte()),
            ErrorContext::new(
                input,
                header_actual.start_byte()..header_actual.end_byte(),
                header_kind,
            ),
            format!("Unrecognized header type '{}'", header_kind),
        ));
        return ParseOutcome::rejected();
    }

    ParseOutcome::rejected()
}
