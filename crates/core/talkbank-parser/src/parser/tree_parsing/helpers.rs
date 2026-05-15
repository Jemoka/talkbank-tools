//! Cross-cutting helper functions for CST error interpretation.
//!
//! These helpers translate generic tree-sitter recovery artifacts (`ERROR`,
//! `MISSING`) into TalkBank-specific parser diagnostics with actionable messages.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

use crate::error::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

/// Analyze ERROR node and provide user-friendly message.
///
/// Inspects the ERROR node's content to determine what went wrong
/// and provides context-specific error messages with suggestions.
///
/// This function is used to transform tree-sitter's internal "ERROR" nodes
/// into actionable, user-friendly error messages that don't expose parser internals.
pub(crate) fn analyze_error_node(node: Node, source: &str, context: &str) -> ParseError {
    let error_text = match node.utf8_text(source.as_bytes()) {
        Ok(text) => text,
        Err(_) => {
            return ParseError::new(
                ErrorCode::UnparsableContent,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!(
                    "Unparsable content in {}: node bytes are not valid UTF-8",
                    context
                ),
            )
            .with_suggestion("Ensure the file is saved as valid UTF-8 encoding");
        }
    };
    let error_text_clean = error_text;
    let is_whitespace_only = error_text.chars().all(|c| c.is_whitespace());

    // Special case: Empty ERROR node (grammar mismatch)
    if error_text.is_empty() || is_whitespace_only {
        return ParseError::new(
            ErrorCode::UnexpectedSyntax,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            format!("Unexpected syntax in {}", context),
        )
        .with_suggestion("Check for missing or malformed elements");
    }

    // Pattern: Unclosed bracket
    if matches!(error_text.chars().next(), Some('['))
        && !matches!(error_text.chars().next_back(), Some(']'))
    {
        return ParseError::new(
            ErrorCode::UnclosedBracket,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            format!("Unclosed bracket in {}", context),
        )
        .with_suggestion("Add closing bracket ']' or check bracket nesting");
    }

    // Pattern: Unclosed parenthesis
    if matches!(error_text.chars().next(), Some('('))
        && !matches!(error_text.chars().next_back(), Some(')'))
    {
        return ParseError::new(
            ErrorCode::UnclosedParenthesis,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            format!("Unclosed parenthesis in {}", context),
        )
        .with_suggestion("Add closing parenthesis ')' to complete the group");
    }

    // Pattern: Incomplete annotation
    if error_text == "[" {
        return ParseError::new(
            ErrorCode::IncompleteAnnotation,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            format!("Incomplete annotation in {}", context),
        )
        .with_suggestion("Complete the annotation like [= comment] or [* error]");
    }

    // Pattern: Invalid characters or Unicode issues
    if error_text.chars().any(|c| c.is_control() && c != '\t') {
        return ParseError::new(
            ErrorCode::InvalidControlCharacter,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            format!("Invalid control characters in {}", context),
        )
        .with_suggestion("Remove or replace control characters (only tabs are allowed)");
    }

    // Redundant terminator in utterance_end (. after .)
    if context == "utterance_end"
        && (error_text.trim() == "." || error_text.trim() == "!" || error_text.trim() == "?")
    {
        return ParseError::new(
            ErrorCode::MissingTerminator,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "Redundant utterance delimiter".to_string(),
        )
        .with_suggestion("Remove the extra terminator — only one is allowed per utterance");
    }

    // Text after terminator in utterance_end
    if context == "utterance_end"
        && !error_text.is_empty()
        && error_text
            .trim()
            .chars()
            .all(|c| c.is_alphanumeric() || c == ' ')
    {
        return ParseError::new(
            ErrorCode::MissingTerminator,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "Text after utterance delimiter is not allowed".to_string(),
        )
        .with_suggestion(
            "Utterance delimiter (. ! ?) must be the last item before any bullet or end of line",
        );
    }

    // Generic fallback: Show what was found
    ParseError::new(
        ErrorCode::UnparsableContent,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(
            error_text_clean,
            0..error_text_clean.len(),
            error_text_clean,
        ),
        format!(
            "Unparsable content in {}: '{}'",
            context,
            if error_text.chars().count() > 30 {
                let truncated: String = error_text.chars().take(30).collect();
                format!("{}...", truncated)
            } else {
                error_text.to_string()
            },
        ),
    )
    .with_suggestion("Check CHAT format specification for valid syntax in this context")
}

/// Create a standardized error for unexpected nodes encountered during parsing.
///
/// This helper detects ERROR nodes (from tree-sitter parse failures) and analyzes
/// them to provide user-friendly messages. For non-ERROR nodes, it reports the
/// unexpected node kind with context.
///
/// **CRITICAL**: This function ensures ERROR nodes are NEVER shown to users directly.
/// Instead, it analyzes the error content to provide actionable, user-friendly messages.
pub(crate) fn unexpected_node_error(node: Node, source: &str, context: &str) -> ParseError {
    // Special handling for ERROR nodes - analyze content for user-friendly message
    if node.is_error() {
        return analyze_error_node(node, source, context);
    }

    // Regular unexpected node (not an ERROR) - report the kind
    ParseError::new(
        ErrorCode::UnexpectedNodeInContext,
        Severity::Error,
        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
        ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
        format!("Unexpected '{}' in {}", node.kind(), context),
    )
    .with_suggestion("This element is not valid in this context")
}
