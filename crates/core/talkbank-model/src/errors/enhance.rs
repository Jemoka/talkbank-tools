//! Post-processing helpers that enrich diagnostics with line/column and snippet context.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::chat_formatting::process_for_plain_display_mapped;
use crate::line_map::LineMap;
use crate::{ErrorCode, ParseError};
use tracing::warn;

/// Check if an error code requires raw source display (no CHAT formatting)
///
/// Errors about formatting characters themselves should show raw control characters
/// so users can see what's wrong. For example:
/// - E356/E357: Unmatched underline markers
/// - E360: Invalid bullet delimiters
fn requires_raw_display(code: &ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::UnmatchedUnderlineBegin
            | ErrorCode::UnmatchedUnderlineEnd
            | ErrorCode::InvalidMediaBullet
    )
}

/// Enhance errors with line/column information and source context from full source.
///
/// Builds a [`LineMap`] internally. If a pre-built `LineMap` is available (e.g.
/// from `ChatFile::line_map`), prefer [`enhance_errors_with_line_map`] to avoid
/// the redundant O(n) construction pass.
///
/// Performance: O(n log m) where n = number of errors, m = number of lines in source.
/// Uses binary search for line lookups instead of linear scanning.
pub fn enhance_errors_with_source(errors: &mut [ParseError], full_source: &str) {
    let line_map = LineMap::new(full_source);
    enhance_errors_with_line_map(errors, full_source, &line_map);
}

/// Enhance errors using a pre-built [`LineMap`].
///
/// This function ensures that all errors have:
/// - `location.line` and `location.column` calculated from byte offsets
/// - `context.line_offset` set for correct miette display
/// - `context.source_text` populated with the source line if empty
///
/// Performance: O(n log m) where n = number of errors, m = number of lines.
pub fn enhance_errors_with_line_map(
    errors: &mut [ParseError],
    full_source: &str,
    line_map: &LineMap,
) {
    let has_content = !full_source.is_empty();
    let source_len = full_source.len() as u32;

    for error in errors {
        if !has_content {
            continue;
        }

        let span_start = error.location.span.start as usize;
        let span_end = error.location.span.end as usize;

        if span_start >= full_source.len() && has_content {
            warn!(
                span_start,
                source_len = full_source.len(),
                code = %error.code,
                "Error span start exceeds source length; clamping to end"
            );
            let clamped = full_source.len().saturating_sub(1) as u32;
            error.location.span.start = clamped;
            error.location.span.end = clamped;
        } else if span_end > full_source.len() {
            warn!(
                span_end,
                source_len = full_source.len(),
                code = %error.code,
                "Error span end exceeds source length; clamping to end"
            );
            error.location.span.end = full_source.len() as u32;
        }

        let span_start = error.location.span.start as usize;
        let span_end = error.location.span.end as usize;

        let needs_span = span_start == 0 && span_end == 0 && has_content;
        let span_start = if needs_span { 0 } else { span_start };
        let span_end = if needs_span {
            1.min(full_source.len())
        } else {
            span_end
        };

        // LineMap.line_col_of works on byte offsets — always succeeds, no
        // mid-character fallback needed (unlike line_index::try_line_col).
        let clamped = |off: usize| off.min(full_source.len().saturating_sub(1));

        // Calculate line/column using binary search - O(log m)
        let (line_0, col_0) = line_map.line_col_of(clamped(span_start) as u32);
        let line_number = line_0 + 1; // 0-indexed → 1-indexed
        let column_number = col_0 + 1;

        // Authoritative line/column for this source buffer.
        // This avoids stale/synthetic positions and guarantees TUI/CLI consistency.
        error.location.line = Some(line_number);
        error.location.column = Some(column_number);

        // Determine the range of lines to extract as context.
        // When labels reference spans on different lines, expand to cover them all.
        let mut min_byte = span_start;
        let mut max_byte = span_end;
        for label in &error.labels {
            let ls = label.span.start as usize;
            let le = label.span.end as usize;
            if ls < full_source.len() {
                min_byte = min_byte.min(ls);
                max_byte = max_byte.max(le.min(full_source.len()));
            }
        }

        // Find the line range covering min_byte..max_byte
        let first_line = line_map.line_of(clamped(min_byte) as u32);
        let last_line = line_map.line_of(clamped(max_byte.saturating_sub(1).max(min_byte)) as u32);

        let context_start = line_map.line_start(first_line) as usize;
        let mut context_end = line_map.line_end(last_line, source_len) as usize;

        // Strip trailing newline if present
        if context_end > context_start
            && full_source.as_bytes().get(context_end - 1) == Some(&b'\n')
        {
            context_end -= 1;
        }

        // Extract multi-line context text
        let context_text = &full_source[context_start..context_end];

        // Calculate primary span relative to the extracted context
        let relative_start = span_start.saturating_sub(context_start);
        let relative_end = span_end
            .saturating_sub(context_start)
            .min(context_text.len());

        // Adjust label spans to be relative to the extracted context
        for label in &mut error.labels {
            let ls = label.span.start as usize;
            let le = label.span.end as usize;
            label.span = crate::Span::from_usize(
                ls.saturating_sub(context_start).min(context_text.len()),
                le.saturating_sub(context_start).min(context_text.len()),
            );
        }

        // Check if this error requires raw display (errors about control characters)
        if requires_raw_display(&error.code) {
            // Don't apply formatting - show raw control characters
            let ctx = error
                .context
                .get_or_insert_with(|| crate::ErrorContext::new("", 0..0, ""));
            ctx.source_text = context_text.to_string();
            ctx.span =
                crate::Span::from_usize(relative_start, relative_end.max(relative_start + 1));
        } else {
            // Single-pass: build display text and position map, then map all spans
            let mapped = process_for_plain_display_mapped(context_text);

            let (display_start, display_end) = mapped.map_span(relative_start, relative_end);

            for label in &mut error.labels {
                let (ls, le) = mapped.map_span(label.span.start as usize, label.span.end as usize);
                label.span = crate::Span::from_usize(ls, le);
            }

            let ctx = error
                .context
                .get_or_insert_with(|| crate::ErrorContext::new("", 0..0, ""));
            ctx.source_text = mapped.text;
            ctx.span = crate::Span::from_usize(display_start, display_end);
        }

        // Always set line_offset for correct miette display
        // Use the first line number in the context range
        let first_line_number = first_line + 1;
        if let Some(ctx) = &mut error.context {
            ctx.line_offset = Some(first_line_number);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorCode, ErrorContext, Severity, SourceLocation, Span};

    #[test]
    fn test_enhance_errors_with_empty_context() {
        let source = "line 1\nline 2 with error\nline 3\n";

        // Error at "error" on line 2 (byte offset 14-19)
        let mut errors = vec![ParseError::new(
            ErrorCode::new("E999"),
            Severity::Error,
            SourceLocation::from_offsets(14, 19),
            ErrorContext::new("", Span::from_usize(0, 0), ""),
            "Test error".to_string(),
        )];

        // Before enhancement
        if let Some(ref ctx) = errors[0].context {
            assert!(ctx.source_text.is_empty());
            assert!(ctx.line_offset.is_none());
        }
        assert!(errors[0].location.line.is_none());

        // Enhance
        enhance_errors_with_source(&mut errors, source);

        // After enhancement
        assert!(
            errors[0].context.is_some(),
            "context should be set after enhancement"
        );
        if let Some(ref ctx) = errors[0].context {
            assert_eq!(ctx.source_text, "line 2 with error");
            assert_eq!(ctx.line_offset, Some(2));
        }
        assert_eq!(errors[0].location.line, Some(2));
        // Column should be calculated from byte offset in full source
        // "line 1\n" = 7 bytes, then "line 2 " = 7 bytes, then "with error" starts at byte 14 (offset from line start = 7)
        assert!(errors[0].location.column.is_some());

        // Span should be relative to the extracted line
        if let Some(ref ctx) = errors[0].context {
            let extracted_line = "line 2 with error";
            if let Some(error_word_start) = extracted_line.find("with") {
                // "with" starts the span
                assert_eq!(ctx.span.start as usize, error_word_start);
            }
        }
    }

    #[test]
    fn test_enhance_errors_replaces_partial_context_with_full_line() {
        let source = "line 1\nline 2 with error\nline 3\n";

        // Find actual byte offset of "error"
        let error_start = source.find("error");
        assert!(error_start.is_some(), "Expected to find 'error' in source");
        let error_start = error_start.unwrap_or(0);
        let error_end = error_start + "error".len();

        let mut errors = vec![ParseError::new(
            ErrorCode::new("E999"),
            Severity::Error,
            SourceLocation::from_offsets(error_start, error_end),
            ErrorContext::new("error", Span::from_usize(0, 5), "error") // partial context
                .with_line_offset(2),
            "Test error".to_string(),
        )];

        // Has partial context
        if let Some(ref ctx) = errors[0].context {
            assert_eq!(ctx.source_text, "error");
        }

        enhance_errors_with_source(&mut errors, source);

        // Should replace with full line
        assert!(
            errors[0].context.is_some(),
            "context should be set after enhancement"
        );
        if let Some(ref ctx) = errors[0].context {
            assert_eq!(ctx.source_text, "line 2 with error");
            // Span should now be relative to full line
            let full_line = "line 2 with error";
            if let Some(error_start_in_line) = full_line.find("error") {
                assert_eq!(ctx.span.start as usize, error_start_in_line);
            }
            assert_eq!(ctx.line_offset, Some(2));
        }
    }

    #[test]
    fn test_enhance_errors_handles_first_line() {
        let source = "first line error\nsecond line\n";

        let mut errors = vec![ParseError::new(
            ErrorCode::new("E999"),
            Severity::Error,
            SourceLocation::from_offsets(11, 16), // "error"
            ErrorContext::new("", Span::from_usize(0, 0), ""),
            "Test error".to_string(),
        )];

        enhance_errors_with_source(&mut errors, source);

        assert!(
            errors[0].context.is_some(),
            "context should be set after enhancement"
        );
        if let Some(ref ctx) = errors[0].context {
            assert_eq!(ctx.source_text, "first line error");
            assert_eq!(ctx.line_offset, Some(1));
        }
        assert_eq!(errors[0].location.line, Some(1));
    }

    #[test]
    fn test_enhance_errors_handles_last_line() {
        let source = "first line\nlast line error";

        let mut errors = vec![ParseError::new(
            ErrorCode::new("E999"),
            Severity::Error,
            SourceLocation::from_offsets(21, 26), // "error" on last line
            ErrorContext::new("", Span::from_usize(0, 0), ""),
            "Test error".to_string(),
        )];

        enhance_errors_with_source(&mut errors, source);

        assert!(
            errors[0].context.is_some(),
            "context should be set after enhancement"
        );
        if let Some(ref ctx) = errors[0].context {
            assert_eq!(ctx.source_text, "last line error");
            assert_eq!(ctx.line_offset, Some(2));
        }
        assert_eq!(errors[0].location.line, Some(2));
    }

    #[test]
    fn test_enhance_preserves_raw_formatting_for_marker_errors() {
        // Source with underline markers
        let source = "hello \u{0002}\u{0001}bad\u{0002}\u{0002} world\n";

        // Error about unmatched underline marker - should show raw control chars
        let mut errors = vec![ParseError::new(
            ErrorCode::UnmatchedUnderlineBegin,
            Severity::Error,
            SourceLocation::from_offsets(6, 8), // Points to \u{0002}\u{0001}
            ErrorContext::new("", Span::from_usize(0, 0), ""),
            "Unmatched underline marker".to_string(),
        )];

        enhance_errors_with_source(&mut errors, source);

        // Should preserve raw control characters
        assert!(
            errors[0].context.is_some(),
            "context should be set after enhancement"
        );
        if let Some(ref ctx) = errors[0].context {
            assert_eq!(
                ctx.source_text,
                "hello \u{0002}\u{0001}bad\u{0002}\u{0002} world"
            );
            // Span should point to actual marker position
            assert_eq!(ctx.span.start, 6);
        }
    }

    #[test]
    fn test_enhance_applies_formatting_for_normal_errors() {
        // Source with underline markers
        let source = "hello \u{0002}\u{0001}bad\u{0002}\u{0002} world\n";

        // Normal error (not about markers) - should apply formatting
        // "world" starts at position 14 in original (after space at 13)
        let mut errors = vec![ParseError::new(
            ErrorCode::new("E999"),
            Severity::Error,
            SourceLocation::from_offsets(14, 19), // Points to "world"
            ErrorContext::new("", Span::from_usize(0, 0), ""),
            "Generic error".to_string(),
        )];

        enhance_errors_with_source(&mut errors, source);

        // Should have markers stripped: "hello bad world"
        assert!(
            errors[0].context.is_some(),
            "context should be set after enhancement"
        );
        if let Some(ref ctx) = errors[0].context {
            assert_eq!(ctx.source_text, "hello bad world");
            // "world" position after removing 4 marker chars: 14 - 4 = 10
            assert_eq!(ctx.span.start, 10);
        }
    }

    #[test]
    fn test_enhance_multi_line_context_with_label() {
        // Simulate main tier on line 2 and %wor tier on line 3
        let source = "line 1\n*CHI:\thello world .\n%wor:\thello world\nline 4\n";

        // Primary error points at the main tier (line 2)
        let main_start = source.find("*CHI:").unwrap();
        let main_end = source[main_start..].find('\n').unwrap() + main_start;

        // Label points at the %wor tier (line 3)
        let wor_start = source.find("%wor:").unwrap();
        let wor_end = source[wor_start..].find('\n').unwrap() + wor_start;

        let mut errors = vec![
            ParseError::new(
                ErrorCode::new("E714"),
                Severity::Error,
                SourceLocation::from_offsets(main_start, main_end),
                ErrorContext::new("", Span::from_usize(0, 0), ""),
                "Alignment mismatch".to_string(),
            )
            .with_label(crate::ErrorLabel::new(
                Span::from_usize(wor_start, wor_end),
                "%wor tier",
            )),
        ];

        enhance_errors_with_source(&mut errors, source);

        let ctx = errors[0].context.as_ref().expect("context should be set");

        // Context should span both lines (main tier + %wor tier)
        assert!(
            ctx.source_text.contains("*CHI:"),
            "context should contain main tier, got: {}",
            ctx.source_text
        );
        assert!(
            ctx.source_text.contains("%wor:"),
            "context should contain wor tier, got: {}",
            ctx.source_text
        );

        // Line offset should be line 2 (first line in context)
        assert_eq!(ctx.line_offset, Some(2));

        // Primary span should point into main tier (relative to context)
        assert!(
            (ctx.span.start as usize) < ctx.source_text.len(),
            "primary span should be within context"
        );

        // Label span should point into %wor tier (relative to context)
        assert_eq!(errors[0].labels.len(), 1);
        let label = &errors[0].labels[0];
        assert!(
            (label.span.start as usize) < ctx.source_text.len(),
            "label span should be within context"
        );
        assert!(
            (label.span.start as usize) > (ctx.span.start as usize),
            "label span should be after primary span (wor is after main)"
        );
    }

    #[test]
    fn test_enhance_label_before_primary() {
        // Label on line 1, primary on line 3
        let source = "alpha\nbeta\ngamma error\ndelta\n";
        let gamma_start = source.find("gamma").unwrap();
        let gamma_end = gamma_start + "gamma error".len();
        let alpha_start = 0;
        let alpha_end = "alpha".len();

        let mut errors = vec![
            ParseError::new(
                ErrorCode::new("E999"),
                Severity::Error,
                SourceLocation::from_offsets(gamma_start, gamma_end),
                ErrorContext::new("", Span::from_usize(0, 0), ""),
                "Error with backward label",
            )
            .with_label(crate::ErrorLabel::new(
                Span::from_usize(alpha_start, alpha_end),
                "related",
            )),
        ];

        enhance_errors_with_source(&mut errors, source);

        let ctx = errors[0].context.as_ref().unwrap();
        // Should span from line 1 to line 3
        assert!(ctx.source_text.contains("alpha"), "should contain alpha");
        assert!(ctx.source_text.contains("gamma"), "should contain gamma");
        assert_eq!(ctx.line_offset, Some(1));

        // Label span should be valid
        let label = &errors[0].labels[0];
        assert_eq!(label.span.start, 0, "label should start at beginning");
    }
}
