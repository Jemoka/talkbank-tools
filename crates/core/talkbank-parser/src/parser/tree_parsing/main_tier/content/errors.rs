//! Error classification for main-tier word/content parse failures.
//!
//! This module upgrades generic tree-sitter error spans to domain-specific
//! error codes used by TalkBank diagnostics.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

use crate::error::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

/// Classifies a word/content `ERROR` node into a specific `ParseError`.
pub(crate) fn analyze_word_error(error_node: Node, source: &str) -> ParseError {
    let error_text = match error_node.utf8_text(source.as_bytes()) {
        Ok(text) => text,
        Err(_) => {
            return ParseError::new(
                ErrorCode::InvalidControlCharacter,
                Severity::Error,
                SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
                ErrorContext::new(source, error_node.start_byte()..error_node.end_byte(), ""),
                "Could not decode content as valid UTF-8".to_string(),
            )
            .with_suggestion("Re-enter using Unicode standard characters");
        }
    };

    // Recoverable semantic cases: keep parsing and delegate final reporting to validation.
    if error_text.starts_with('"') && !error_text[1..].contains('"') {
        return ParseError::new(
            ErrorCode::UnbalancedQuotation,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "Unbalanced quotation in word content".to_string(),
        )
        .with_suggestion("Close the quotation mark to balance the quoted segment");
    }

    if error_node.start_byte() > 0
        && source.as_bytes()[error_node.start_byte() - 1] == b':'
        && !error_text.is_empty()
    {
        return ParseError::new(
            ErrorCode::LengtheningNotAfterSpokenMaterial,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "Lengthening marker appears before spoken material".to_string(),
        )
        .with_suggestion("Place ':' after spoken material (e.g., bana:nas)");
    }

    // E311: Unclosed replacement bracket (PRIORITY 1)
    if let Some((relative_start, relative_end)) = find_unclosed_replacement_offset(error_text) {
        let absolute_start = error_node.start_byte() + relative_start;
        let absolute_end = error_node.start_byte() + relative_end;
        return ParseError::new(
            ErrorCode::UnexpectedNode,
            Severity::Error,
            SourceLocation::from_offsets(absolute_start, absolute_end),
            ErrorContext::new(source, absolute_start..absolute_end, "[:"),
            "Unclosed replacement bracket - replacements must be in format '[: correct form]' or '[* phonological form]'".to_string(),
        )
        .with_suggestion("Complete the replacement: '[: target]' for word replacements, '[* phonology]' for phonological forms");
    }

    // E350: Quadruple nested brackets (PRIORITY 2)
    if error_text.contains("[[[[") || error_text.contains("]]]]") {
        return ParseError::new(
            ErrorCode::ContentAnnotationParseError,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "Quadruple nested brackets [[[[]]]] are invalid".to_string(),
        )
        .with_suggestion("CHAT supports up to triple nested brackets [[[]]]. Use proper nesting for groups and annotations.");
    }

    // E207: Incomplete word-level annotation
    if matches!(error_text.chars().next(), Some('&')) && error_text.len() == 1 {
        return ParseError::new(
            ErrorCode::UnknownAnnotation,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new("&", 0..1, "&"),
            "Incomplete word-level annotation - '&' must be followed by annotation name"
                .to_string(),
        )
        .with_suggestion("Complete the annotation like '&=laugh' or '&uh' for filler");
    }

    // E207: Unknown scoped annotation marker (PRIORITY 4)
    // Detect [@, which is not a valid scoped annotation
    if error_text.contains("[@")
        || (matches!(error_text.chars().next(), Some('@')) && error_node.start_byte() > 0)
    {
        // Check if this looks like an annotation context (preceded by '[')
        let prev_byte = if error_node.start_byte() > 0 {
            source.as_bytes().get(error_node.start_byte() - 1).copied()
        } else {
            None
        };

        if prev_byte == Some(b'[') || error_text.contains("[@") {
            return ParseError::new(
                ErrorCode::UnknownAnnotation,
                Severity::Error,
                SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
                ErrorContext::new(error_text, 0..error_text.len(), error_text),
                "Unknown scoped annotation marker".to_string(),
            )
            .with_suggestion("Valid annotations: [= explanation], [* error], [+ addition], [//] retracing, [<]/[>] overlap");
        }
    }

    // E208: Empty replacement [:] (PRIORITY 5)
    // Detect replacement with no words
    if error_text.contains("[:]") {
        return ParseError::new(
            ErrorCode::EmptyReplacement,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "Empty replacement - [: ] must contain corrected word(s)".to_string(),
        )
        .with_suggestion("Add replacement text after [: , e.g., word [: corrected]");
    }

    // E202: Missing form type after @ (PRIORITY 5)
    // Detect @ symbol without form type marker (e.g., "hello@", standalone "@", or " @ word")
    if let Some(relative_at) = find_missing_form_type_offset(error_text) {
        let absolute_start = error_node.start_byte() + relative_at;
        let absolute_end = absolute_start + 1;
        return ParseError::new(
            ErrorCode::MissingFormType,
            Severity::Error,
            SourceLocation::from_offsets(absolute_start, absolute_end),
            ErrorContext::new(source, absolute_start..absolute_end, "@"),
            "Missing form type after @".to_string(),
        )
        .with_suggestion("Add a form type after @ (e.g., @b for babbling, @s:eng for L2 English, @n for neologism)");
    }

    // E202: Misplaced question mark or exclamation
    if (error_text == "?" || error_text == "!") && error_node.start_byte() > 0 {
        return ParseError::new(
            ErrorCode::MissingFormType,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            format!(
                "Misplaced '{}' - terminators must appear at end of utterance only",
                error_text
            ),
        )
        .with_suggestion("Move terminator to end of utterance or use [!] for emphasis");
    }

    // E208: Unrecognized freecode or annotation
    if matches!(error_text.chars().next(), Some('‡')) {
        return ParseError::new(
            ErrorCode::EmptyReplacement,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            format!("Unrecognized freecode '{}'", error_text),
        )
        .with_suggestion(
            "Check freecode format. Freecodes should follow standard patterns like ‡code",
        );
    }

    // Repetition count [x N] or broken bracket annotation fragment.
    // Tree-sitter splits ERROR nodes, so we often get just " [" as the fragment
    // when the real issue is an unrecognized [x N] or [/] etc.
    if error_text.contains("[x ") || error_text.contains("[x\t") {
        return ParseError::new(
            ErrorCode::ContentAnnotationParseError,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "Could not parse repetition count annotation".to_string(),
        )
        .with_suggestion("Repetition format: word [x N] or <group> [x N] where N is a number");
    }
    if error_text.trim() == "[" {
        return ParseError::new(
            ErrorCode::ContentAnnotationParseError,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "Could not parse bracket annotation".to_string(),
        )
        .with_suggestion(
            "Valid bracket annotations: [/] [//] [///] [x N] [: replacement] [* error] \
             [= explanation] [+ postcode] [% comment] [<] [>]",
        );
    }

    // Invalid form marker: @ followed by letters but not in the valid set
    if let Some(idx) = find_invalid_form_marker_offset(error_text) {
        let absolute_start = error_node.start_byte() + idx;
        return ParseError::new(
            ErrorCode::InvalidFormType,
            Severity::Error,
            SourceLocation::from_offsets(absolute_start, error_node.end_byte()),
            ErrorContext::new(source, absolute_start..error_node.end_byte(), ""),
            format!(
                "Unknown form marker '{}' - not a recognized CHAT form type",
                &error_text[idx..]
            ),
        )
        .with_suggestion(
            "Valid form markers: @b (babbling), @c (child-invented), @d (dialect), \
             @f (family-specific), @i (interjection), @l (letter), @n (neologism), \
             @o (onomatopoeia), @s:lang (second language), @u (unibet)",
        );
    }

    // Smart/curly quotes that should be straight quotes
    if error_text.contains('\u{2018}') || error_text.contains('\u{2019}') {
        return ParseError::new(
            ErrorCode::InvalidControlCharacter,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "Smart quote \u{2018}/\u{2019} must be replaced by straight quote (')".to_string(),
        )
        .with_suggestion("Replace curly single quotes with straight quote (')");
    }
    if error_text.contains('\u{201C}') || error_text.contains('\u{201D}') {
        return ParseError::new(
            ErrorCode::UnbalancedQuotation,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            format!(
                "Unmatched curly quote {} found on the tier",
                if error_text.contains('\u{201C}') {
                    "\u{201C}"
                } else {
                    "\u{201D}"
                }
            ),
        )
        .with_suggestion("Use straight double quotes (\") for CHAT quotation");
    }

    // Closing bracket fragment "] word" — other half of a broken annotation
    if error_text.trim_start().starts_with(']') {
        return ParseError::new(
            ErrorCode::ContentAnnotationParseError,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "Unexpected ']' — possibly part of a malformed bracket annotation".to_string(),
        )
        .with_suggestion(
            "Check bracket annotation format: [/] [//] [///] [x N] [: replacement] [* error]",
        );
    }

    // CA continuation += is not a valid CHAT construct
    if error_text.trim_start().starts_with("+=") {
        return ParseError::new(
            ErrorCode::MissingTerminator,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "'+=' is not a valid CHAT construct".to_string(),
        )
        .with_suggestion(
            "For linkers use +< (lazy overlap) or +^ (quick uptake). \
             For terminators use . ! ? or CA delimiters (↗ ↘ ↛ ∙ „)",
        );
    }

    // Caret ^ at word start — syllable pause misuse
    if error_text.starts_with('^') {
        return ParseError::new(
            ErrorCode::SyllablePauseNotBetweenSpokenMaterial,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "'^' (syllable pause) cannot appear at word start".to_string(),
        )
        .with_suggestion("Syllable pause '^' must appear between syllables: ba^na^na");
    }

    // Missing space before bracket annotation — "[/]" attached to word
    if error_text.trim_start().starts_with("[/]")
        || error_text.trim_start().starts_with("[//]")
        || error_text.trim_start().starts_with("[///]")
        || error_text.trim_start().starts_with("[*]")
        || error_text.trim_start().starts_with("[=")
        || error_text.trim_start().starts_with("[+")
        || error_text.trim_start().starts_with("[%")
    {
        return ParseError::new(
            ErrorCode::ContentAnnotationParseError,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "Space required before bracket annotation".to_string(),
        )
        .with_suggestion("Add a space before '[' — e.g., 'word [/]' not 'word[/]'");
    }

    // Redundant terminator — "." after the real terminator
    if error_text.trim() == "." || error_text.trim() == "!" || error_text.trim() == "?" {
        return ParseError::new(
            ErrorCode::MissingTerminator,
            Severity::Error,
            SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
            ErrorContext::new(error_text, 0..error_text.len(), error_text),
            "Redundant utterance delimiter".to_string(),
        )
        .with_suggestion("Remove the extra terminator — only one is allowed per utterance");
    }

    // Text after utterance delimiter — content appearing after . ! ? etc.
    // Detect: error text is plain word(s) and preceded by a terminator
    if !error_text.is_empty()
        && error_text
            .chars()
            .all(|c| c.is_alphanumeric() || c == ' ' || c == '\t')
        && error_node.start_byte() > 0
    {
        let prev = source.as_bytes().get(error_node.start_byte() - 1).copied();
        if matches!(prev, Some(b' ' | b'\t')) {
            // Check if there's a terminator before us
            let before = &source[..error_node.start_byte()];
            if before.trim_end().ends_with('.')
                || before.trim_end().ends_with('!')
                || before.trim_end().ends_with('?')
            {
                return ParseError::new(
                    ErrorCode::MissingTerminator,
                    Severity::Error,
                    SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
                    ErrorContext::new(error_text, 0..error_text.len(), error_text),
                    "Text after utterance delimiter is not allowed".to_string(),
                )
                .with_suggestion(
                    "The utterance delimiter (. ! ?) must be the last item before any bullet",
                );
            }
        }
    }

    // Control character immediately before the error position — the parser split
    // the word at the control char, so we see the fragment after it as E316.
    if error_node.start_byte() > 0 {
        let prev_byte = source.as_bytes()[error_node.start_byte() - 1];
        if prev_byte < 0x20 && prev_byte != b'\t' && prev_byte != b'\n' && prev_byte != b'\r' {
            return ParseError::new(
                ErrorCode::InvalidControlCharacter,
                Severity::Error,
                SourceLocation::from_offsets(error_node.start_byte() - 1, error_node.end_byte()),
                ErrorContext::new(
                    source,
                    error_node.start_byte() - 1..error_node.end_byte(),
                    "",
                ),
                format!(
                    "Illegal control character (0x{:02X}) found in word",
                    prev_byte
                ),
            )
            .with_suggestion(
                "Remove or replace the control character. Use Unicode standard characters.",
            );
        }
    }

    // E316: Unparsable content (LOWEST PRIORITY fallback)
    // Use error_text with span 0..len to avoid span/source mismatch that causes OutOfBounds
    // This is safe because error_text is extracted from the ERROR node itself
    ParseError::new(
        ErrorCode::UnparsableContent,
        Severity::Error,
        SourceLocation::from_offsets(error_node.start_byte(), error_node.end_byte()),
        ErrorContext::new(error_text, 0..error_text.len(), error_text),
        format!("Unparsable content on main tier: '{}'", error_text),
    )
    .with_suggestion("Check CHAT format manual for valid syntax at this position")
}

/// Finds missing form type offset.
fn find_missing_form_type_offset(error_text: &str) -> Option<usize> {
    let bytes = error_text.as_bytes();

    for idx in 0..bytes.len() {
        if bytes[idx] != b'@' {
            continue;
        }

        let next = bytes.get(idx + 1).copied();
        let missing = match next {
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

/// Finds an `@` followed by letters that aren't a valid CHAT form marker.
///
/// Valid form markers (from the grammar whitelist):
/// a, b, c, d, f, fp, g, i, k, l, ls, n, o, p, q, sas, si, sl, t, u, wp, x, z
fn find_invalid_form_marker_offset(error_text: &str) -> Option<usize> {
    const VALID_MARKERS: &[&str] = &[
        "a", "b", "c", "d", "f", "fp", "g", "i", "k", "l", "ls", "n", "o", "p", "q", "sas", "si",
        "sl", "t", "u", "wp", "x", "z",
    ];

    for (idx, _) in error_text.match_indices('@') {
        let rest = &error_text[idx + 1..];
        // Must have at least one letter after @
        if rest.is_empty() || !rest.as_bytes()[0].is_ascii_alphabetic() {
            continue;
        }
        // Extract the marker text (letters and hyphens until non-alpha)
        let marker_end = rest
            .find(|c: char| !c.is_ascii_alphabetic() && c != '-')
            .unwrap_or(rest.len());
        let marker = &rest[..marker_end];
        // Skip if it looks like a language suffix (@s:eng) — the base is valid
        let base = marker.split(':').next().unwrap_or(marker);
        if !VALID_MARKERS.contains(&base) {
            return Some(idx);
        }
    }

    None
}

/// Finds unclosed replacement offset.
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

#[cfg(test)]
mod tests {
    use super::{find_missing_form_type_offset, find_unclosed_replacement_offset};

    /// Verifies missing form-type offset detection for lone and trailing `@`.
    #[test]
    fn missing_form_type_offset_detects_lone_and_trailing_at() {
        assert_eq!(find_missing_form_type_offset("@"), Some(0));
        assert_eq!(find_missing_form_type_offset("hello@"), Some(5));
        assert_eq!(find_missing_form_type_offset(" @ world"), Some(1));
    }

    /// Verifies valid form markers are ignored by missing form-type detection.
    #[test]
    fn missing_form_type_offset_ignores_valid_form_markers() {
        assert_eq!(find_missing_form_type_offset("word@s:eng"), None);
        assert_eq!(find_missing_form_type_offset("word@b"), None);
    }

    /// Verifies unclosed replacement offsets are reported relative to input, including leading whitespace.
    #[test]
    fn unclosed_replacement_offset_handles_leading_whitespace() {
        assert_eq!(find_unclosed_replacement_offset(" [: world"), Some((1, 3)));
        assert_eq!(find_unclosed_replacement_offset("[:]"), None);
        assert_eq!(find_unclosed_replacement_offset(" [: fixed]"), None);
    }
}
