//! File-level `ERROR` analysis and fallback diagnostic routing.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use tree_sitter::Node;

/// Classifies a top-level `ERROR` node into a specific parse error.
pub(crate) fn analyze_error_node(node: Node, source: &str, errors: &impl ErrorSink) {
    let error_text = extract_utf8_text(node, source, errors, "file_error", "");
    let start = node.start_byte();
    let end = node.end_byte();

    // Check if this is a dependent tier error (starts with %)
    if matches!(error_text.chars().next(), Some('%')) {
        // E710: Invalid %gra - non-numeric index
        if error_text.contains("%gra:") {
            errors.report(
                ParseError::new(
                    ErrorCode::UnexpectedGrammarNode,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "Invalid GRA relation - non-numeric index",
                )
                .with_suggestion(
                    "GRA relation indices must be numbers (e.g., 1|2|SUBJ, not one|2|SUBJ)",
                ),
            );
            return;
        }

        // Recoverable dependent-tier parse failures:
        // keep file parsing alive and let downstream validation report semantic issues.
        let (code, message) = if error_text.contains(":\t") {
            (
                ErrorCode::InvalidDependentTier,
                format!(
                    "Could not fully parse dependent tier: {}",
                    match error_text.lines().next() {
                        Some(line) => line,
                        None => error_text,
                    }
                ),
            )
        } else {
            (
                ErrorCode::MalformedTierHeader,
                format!(
                    "Malformed dependent tier header: {}",
                    match error_text.lines().next() {
                        Some(line) => line,
                        None => error_text,
                    }
                ),
            )
        };

        errors.report(
            ParseError::new(
                code,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, error_text),
                message,
            )
            .with_suggestion(
                "Check dependent tier syntax (%tier:\\tcontent) and tier-specific format",
            ),
        );
        return;
    }

    // Check if this is a main tier error (starts with *)
    if matches!(error_text.chars().next(), Some('*')) {
        // E301: Check for empty speaker (*: with no code between * and :)
        if error_text.contains("*:") || error_text.contains("*\t") {
            errors.report(
                ParseError::new(
                    ErrorCode::MissingMainTier,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "Empty speaker code in main tier",
                )
                .with_suggestion("Add a speaker code between * and : (e.g., *CHI:)"),
            );
            return;
        }

        // E306: `*SPEAKER:` with nothing after — empty utterance.
        // Detected when the error text ends at (or just after) the colon.
        if let Some(last_colon) = error_text.rfind(':') {
            let trailing_ws = error_text
                .bytes()
                .rev()
                .take_while(|&b| b == b'\n' || b == b'\r' || b == b'\t' || b == b' ')
                .count();
            if trailing_ws + 1 >= error_text.len() - last_colon {
                errors.report(
                    ParseError::new(
                        ErrorCode::EmptyUtterance,
                        Severity::Error,
                        SourceLocation::from_offsets(start, end),
                        ErrorContext::new(source, start..end, error_text),
                        "Main tier missing content after speaker",
                    )
                    .with_suggestion(
                        "Add utterance content after the colon-tab (e.g., *CHI:\thello world .)",
                    ),
                );
                return;
            }
        }
    }

    // Check if this is a header error by looking at the content
    if matches!(error_text.chars().next(), Some('@')) {
        // Empty-header checks via `strip_prefix` so we never slice at
        // a byte index that might fall inside a multi-byte UTF-8 char.
        // (A previous `error_text[..N] == "@Name:"` pattern panicked on
        // fuzz input like `@%…˻…` where byte index 7 sat inside `˻`.)
        let empty_header_check: Option<(&str, ErrorCode, &'static str)> = None
            .or_else(|| {
                error_text
                    .strip_prefix("@Languages:")
                    .map(|rest| (rest, ErrorCode::EmptyLanguagesHeader, "@Languages"))
            })
            .or_else(|| {
                error_text
                    .strip_prefix("@Date:")
                    .map(|rest| (rest, ErrorCode::EmptyDateHeader, "@Date"))
            })
            .or_else(|| {
                error_text
                    .strip_prefix("@Media:")
                    .map(|rest| (rest, ErrorCode::EmptyMediaHeader, "@Media"))
            });
        if let Some((after_colon, code, name)) = empty_header_check
            && after_colon
                .bytes()
                .all(|b| b == b'\t' || b == b' ' || b == b'\n' || b == b'\r')
        {
            errors.report(ParseError::new(
                code,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, error_text),
                format!("{name} header cannot be empty"),
            ));
            return;
        }

        // @Page header (not a standard CHAT header but used in some files)
        if error_text.starts_with("@Page") {
            errors.report(
                ParseError::new(
                    ErrorCode::UnknownHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "@Page header is not a standard CHAT header",
                )
                .with_suggestion("@Page is a legacy header. Consider removing it."),
            );
            return;
        }

        // @Comment with spaces instead of tab after colon
        if error_text.starts_with("@Comment:") && !error_text.contains(":\t") {
            errors.report(
                ParseError::new(
                    ErrorCode::SyntaxError,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "Space character instead of TAB after header colon",
                )
                .with_suggestion("Replace spaces after ':' with a single TAB character"),
            );
            return;
        }

        // Check for @ID errors
        // ERROR node with @ID means tree-sitter failed to parse the structure
        // Don't try to manually parse it - just report it's malformed
        if error_text.contains("@ID:") {
            errors.report(
                ParseError::new(
                    ErrorCode::InvalidIDFormat,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "Invalid @ID header format: structure could not be parsed",
                )
                .with_suggestion(
                    "@ID requires exactly 10 pipe-separated fields: @ID:\tlang|corpus|speaker|age|sex|group|SES|role|education|custom|",
                ),
            );
            return;
        }
    }

    // Duplicate @Begin
    if error_text.starts_with("@Begin") {
        errors.report(
            ParseError::new(
                ErrorCode::DuplicateHeader,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, error_text),
                "Duplicate @Begin header: only one @Begin is allowed per file",
            )
            .with_suggestion("Remove the extra @Begin header"),
        );
        return;
    }

    // Content after @End
    if error_text.starts_with("@End") {
        errors.report(
            ParseError::new(
                ErrorCode::DuplicateHeader,
                Severity::Error,
                SourceLocation::from_offsets(start, end),
                ErrorContext::new(source, start..end, error_text),
                "Duplicate @End header or content after @End: nothing may follow the @End line",
            )
            .with_suggestion("Remove all content after @End — only one @End is allowed per file"),
        );
        return;
    }

    // Main tier containing caret prefix (^word), inline annotations ([%add:]),
    // or other content that causes the entire line to be an ERROR node
    if error_text.starts_with('*') && error_text.contains(":\t") {
        let content_start = error_text.find(":\t").unwrap_or(0) + 2;
        let content = error_text[content_start..].trim();

        // ^word — caret/blocking prefix (obsolete CHAT construct)
        if content.starts_with('^') {
            errors.report(
                ParseError::new(
                    ErrorCode::SyllablePauseNotBetweenSpokenMaterial,
                    Severity::Error,
                    SourceLocation::from_offsets(start + content_start, end),
                    ErrorContext::new(source, start..end, error_text),
                    format!(
                        "'^' cannot appear at utterance start — '{}' is not valid CHAT",
                        content.split_whitespace().next().unwrap_or(content)
                    ),
                )
                .with_suggestion(
                    "'^' is a syllable pause marker (ba^na^na). It cannot be used as a word prefix.",
                ),
            );
            return;
        }

        // [%add: ...] or similar inline dependent tier annotation
        if content.starts_with("[%") {
            errors.report(
                ParseError::new(
                    ErrorCode::ContentAnnotationParseError,
                    Severity::Error,
                    SourceLocation::from_offsets(start + content_start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "Inline dependent tier annotation cannot appear at utterance start".to_string(),
                )
                .with_suggestion(
                    "Place [%add: ...] after the word it modifies, not at utterance start",
                ),
            );
            return;
        }

        // <group> [x N] — repetition that fails to parse at file level
        if content.contains("[x ") || content.contains("[x\t") {
            errors.report(
                ParseError::new(
                    ErrorCode::ContentAnnotationParseError,
                    Severity::Error,
                    SourceLocation::from_offsets(start, end),
                    ErrorContext::new(source, start..end, error_text),
                    "Could not parse utterance containing repetition count [x N]".to_string(),
                )
                .with_suggestion(
                    "Check repetition format: word [x N] or <group> [x N]. \
                     The number must follow [x with a space.",
                ),
            );
            return;
        }
    }

    // Main tier with non-ASCII speaker name (e.g., *CHIé:)
    if error_text.starts_with('*')
        && let Some(colon_pos) = error_text.find(':')
    {
        let speaker = &error_text[1..colon_pos];
        if !speaker.is_ascii() {
            errors.report(
                ParseError::new(
                    ErrorCode::SpeakerNotDefined,
                    Severity::Error,
                    SourceLocation::from_offsets(start, start + 1 + colon_pos),
                    ErrorContext::new(source, start..start + 1 + colon_pos, ""),
                    format!("Speaker '{}' contains non-ASCII characters and cannot be resolved", speaker),
                )
                .with_suggestion(
                    "Speaker codes must use only uppercase ASCII letters and digits (e.g., CHI, MOT, SP1)",
                ),
            );
            return;
        }
    }

    // Generic file-level error
    errors.report(
        ParseError::new(
            ErrorCode::UnparsableContent,
            Severity::Error,
            SourceLocation::from_offsets(start, end),
            ErrorContext::new(source, start..end, error_text),
            format!(
                "Unparsable content at file level: '{}'",
                match error_text.lines().next() {
                    Some(line) => line,
                    None => error_text,
                }
            ),
        )
        .with_suggestion("Check CHAT format specification for valid syntax at this position"),
    );
}
