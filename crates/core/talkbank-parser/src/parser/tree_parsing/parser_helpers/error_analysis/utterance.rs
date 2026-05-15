//! Utterance-context error analysis for malformed main-tier lines.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use tree_sitter::Node;

/// Classifies an `ERROR` node in utterance context.
pub(crate) fn analyze_utterance_error(
    error_node: Node,
    line_node: Node,
    source: &str,
    errors: &impl ErrorSink,
) {
    let _line_text = extract_utf8_text(line_node, source, errors, "utterance_line", "");
    let error_text = extract_utf8_text(error_node, source, errors, "utterance_error", "");
    let start = error_node.start_byte();
    let end = error_node.end_byte();

    // Check for @ without form type (e.g., "hello@")
    let ends_with_at = error_text
        .rfind('@')
        .is_some_and(|idx| idx + 1 == error_text.len());
    if ends_with_at
        || (error_text.contains('@') && !error_text.contains("@b") && !error_text.contains("@s"))
    {
        errors.report(
            ParseError::new(
                ErrorCode::MissingFormType,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, error_text),
                "Missing form type after @",
            )
            .with_suggestion("Add a form type after @ (e.g., @b, @s:eng, @n)"),
        );
        return;
    }

    // Check for empty replacement (e.g., "[:]")
    if error_text.contains("[:]")
        || (error_text.contains("[") && error_text.contains(":") && error_text.contains("]"))
    {
        errors.report(
            ParseError::new(
                ErrorCode::EmptyReplacement,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, error_text),
                "Replacement text cannot be empty",
            )
            .with_suggestion("Add replacement text after [: , e.g., word [: corrected]"),
        );
        return;
    }

    // Check for invalid scoped annotation (e.g., "[@ xyz]")
    if error_text.contains("[@") {
        errors.report(ParseError::new(
            ErrorCode::UnknownAnnotation,
            Severity::Error,
            SourceLocation::from_offsets(start, end),
            ErrorContext::new(source, start..end, error_text),
            "Unknown scoped annotation marker",
        ));
        return;
    }

    // Generic utterance error
    errors.report(ParseError::new(
        ErrorCode::UnparsableUtterance,
        Severity::Error,
        SourceLocation::from_offsets(start, end),
        ErrorContext::new(source, start..end, error_text),
        "Syntax error in utterance",
    ));
}
