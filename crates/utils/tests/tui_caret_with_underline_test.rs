//! Test that TUI caret alignment works correctly when source line contains underline markers
//!
//! The issue: underline markers (\u0002\u0001...\u0002\u0002) are control characters that
//! don't display but affect positions. We need to strip them before calculating positions.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use unicode_width::UnicodeWidthChar;

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Missing span: {label}")]
    SpanNotFound { label: &'static str },
    #[error("Missing display start for span")]
    DisplayStartMissing,
    #[error("Missing display end for span")]
    DisplayEndMissing,
}

/// Tests caret alignment with underline markers.
#[test]
fn test_caret_alignment_with_underline_markers() -> Result<(), TestError> {
    // Text: "hello \u0002\u0001world\u0002\u0002 bad"
    // Display: "hello world bad" with "world" underlined
    // Error span points to "bad" (position 16-19 in original, 12-15 in display)
    let text = "hello \u{0002}\u{0001}world\u{0002}\u{0002} bad";
    let span_start = 16; // "bad" starts at position 16 in original
    let span_end = 19;

    let (spans, display_offset, display_length) =
        process_source_line_for_display(text, span_start, span_end)?;

    // Display text should be "hello world bad"
    let display_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(display_text, "hello world bad");

    // "hello world " = 12 display positions
    assert_eq!(
        display_offset, 12,
        "Error should start at display position 12"
    );
    assert_eq!(display_length, 3, "Error span should be 3 characters wide");

    // Check that "world" is underlined
    let world_span = spans
        .iter()
        .find(|s| s.content == "world")
        .ok_or(TestError::SpanNotFound { label: "world" })?;
    assert!(world_span.style.add_modifier.contains(Modifier::UNDERLINED));

    Ok(())
}

/// Tests caret alignment error in underlined region.
#[test]
fn test_caret_alignment_error_in_underlined_region() -> Result<(), TestError> {
    // Text: "hello \u0002\u0001bad word\u0002\u0002"
    // Display: "hello bad word" with "bad word" underlined
    // Error span points to "bad" (position 8-11 in original, 6-9 in display)
    let text = "hello \u{0002}\u{0001}bad word\u{0002}\u{0002}";
    let span_start = 8; // "bad" starts after "hello " + markers
    let span_end = 11;

    let (spans, display_offset, display_length) =
        process_source_line_for_display(text, span_start, span_end)?;

    let display_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(display_text, "hello bad word");

    // "hello " = 6 display positions
    assert_eq!(display_offset, 6);
    assert_eq!(display_length, 3); // "bad"

    Ok(())
}

/// Tests caret with tab and underline.
#[test]
fn test_caret_with_tab_and_underline() -> Result<(), TestError> {
    // Text: "word\t\u0002\u0001underlined\u0002\u0002"
    // Display: "word    underlined" with "underlined" underlined
    // Error on "underlined" (position 7-17 in original, 8-18 in display)
    let text = "word\t\u{0002}\u{0001}underlined\u{0002}\u{0002}";
    let span_start = 7; // After tab and markers
    let span_end = 17;

    let (spans, display_offset, display_length) =
        process_source_line_for_display(text, span_start, span_end)?;

    let _display_text: String = spans.iter().map(|s| s.content.as_ref()).collect();

    // "word" (4) + tab expands to column 8 (4 spaces) = 8 positions
    assert_eq!(display_offset, 8);
    assert_eq!(display_length, 10); // "underlined"

    // Check underline styling
    let underlined =
        spans
            .iter()
            .find(|s| s.content == "underlined")
            .ok_or(TestError::SpanNotFound {
                label: "underlined",
            })?;
    assert!(underlined.style.add_modifier.contains(Modifier::UNDERLINED));

    Ok(())
}

/// Tests multiple underline sections caret.
#[test]
fn test_multiple_underline_sections_caret() -> Result<(), TestError> {
    // Text: "\u0002\u0001first\u0002\u0002 bad \u0002\u0001second\u0002\u0002"
    // Display: "first bad second" with "first" and "second" underlined
    // Error on "bad" (position 10-13 in original, 6-9 in display)
    let text = "\u{0002}\u{0001}first\u{0002}\u{0002} bad \u{0002}\u{0001}second\u{0002}\u{0002}";
    let span_start = 10; // "bad" position in original
    let span_end = 13;

    let (spans, display_offset, display_length) =
        process_source_line_for_display(text, span_start, span_end)?;

    let display_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(display_text, "first bad second");

    // "first " = 6 display positions
    assert_eq!(display_offset, 6);
    assert_eq!(display_length, 3); // "bad"

    Ok(())
}

/// Helper function (from TUI code)
fn process_source_line_for_display(
    text: &str,
    span_start: usize,
    span_end: usize,
) -> Result<(Vec<Span<'static>>, usize, usize), TestError> {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    let mut is_underlined = false;

    let mut char_pos: usize = 0; // Position in original text
    let mut display_pos: usize = 0; // Position in display
    let mut display_start: Option<usize> = None;
    let mut display_end: Option<usize> = None;

    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        // Track display positions for error span
        if char_pos == span_start && display_start.is_none() {
            display_start = Some(display_pos);
        }
        if char_pos == span_end && display_end.is_none() {
            display_end = Some(display_pos);
        }

        // Handle underline markers
        if ch == '\u{0002}'
            && let Some(&next_ch) = chars.peek()
        {
            if next_ch == '\u{0001}' {
                // UNDERLINE_BEGIN
                chars.next();
                if !current_text.is_empty() {
                    spans.push(if is_underlined {
                        Span::styled(
                            current_text.clone(),
                            Style::default().add_modifier(Modifier::UNDERLINED),
                        )
                    } else {
                        Span::raw(current_text.clone())
                    });
                    current_text.clear();
                }
                is_underlined = true;
                char_pos += 2;
                continue;
            } else if next_ch == '\u{0002}' {
                // UNDERLINE_END
                chars.next();
                if !current_text.is_empty() {
                    spans.push(if is_underlined {
                        Span::styled(
                            current_text.clone(),
                            Style::default().add_modifier(Modifier::UNDERLINED),
                        )
                    } else {
                        Span::raw(current_text.clone())
                    });
                    current_text.clear();
                }
                is_underlined = false;
                char_pos += 2;
                continue;
            }
        }

        // Handle regular characters
        match ch {
            '\t' => {
                let spaces_to_add = 8 - (display_pos % 8);
                for _ in 0..spaces_to_add {
                    current_text.push(' ');
                    display_pos += 1;
                }
            }
            '\u{0015}' => {
                current_text.push('•');
                display_pos += 1;
            }
            _ => {
                current_text.push(ch);
                let width = ch.width().unwrap_or(1);
                display_pos += width;
            }
        }

        char_pos += 1;
    }

    // Handle span_end at the very end
    if char_pos == span_end && display_end.is_none() {
        display_end = Some(display_pos);
    }

    // Flush any remaining text
    if !current_text.is_empty() {
        spans.push(if is_underlined {
            Span::styled(
                current_text,
                Style::default().add_modifier(Modifier::UNDERLINED),
            )
        } else {
            Span::raw(current_text)
        });
    }

    if spans.is_empty() {
        spans.push(Span::raw(String::new()));
    }

    let offset = display_start.ok_or(TestError::DisplayStartMissing)?;
    let end = display_end.ok_or(TestError::DisplayEndMissing)?;
    let length = end.saturating_sub(offset).max(1);

    Ok((spans, offset, length))
}
