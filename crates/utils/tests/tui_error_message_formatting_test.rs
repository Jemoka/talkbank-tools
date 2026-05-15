//! Test that TUI correctly formats error messages
//!
//! Error messages may contain CHAT formatting like tabs, bullets, underline markers.
//! These should be rendered properly, not elided.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use unicode_width::UnicodeWidthChar;

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Missing span: {label}")]
    MissingSpan { label: &'static str },
}

/// Tests error message with tabs.
#[test]
fn test_error_message_with_tabs() {
    let message = "Syntax error in file:\t@Code:junk";
    let spans = process_text_for_display(message);

    // Should have expanded tab to spaces
    let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(!text.contains('\t'), "Tabs should be expanded");
    assert!(text.contains("Syntax error in file:"));
    assert!(text.contains("@Code:junk"));
}

/// Tests error message with bullet delimiter.
#[test]
fn test_error_message_with_bullet_delimiter() {
    let message = "Error: @Code:junk|foo|bar";
    let message_with_bullets = message.replace('|', "\u{0015}");
    let spans = process_text_for_display(&message_with_bullets);

    // Should render bullets as visible character
    let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.contains('•'),
        "Should contain visible bullet character"
    );
    assert_eq!(text, "Error: @Code:junk•foo•bar");
    assert!(
        !text.contains('\u{0015}'),
        "Should not contain raw control char"
    );
}

/// Tests error message with underline.
#[test]
fn test_error_message_with_underline() -> Result<(), TestError> {
    let message = "Error: \u{0002}\u{0001}underlined\u{0002}\u{0002} text";
    let spans = process_text_for_display(message);

    // Should have multiple spans with underline styling
    assert!(spans.len() > 1, "Should have multiple spans");

    // Find the underlined span
    let underlined =
        spans
            .iter()
            .find(|s| s.content == "underlined")
            .ok_or(TestError::MissingSpan {
                label: "underlined",
            })?;
    assert!(
        underlined.style.add_modifier.contains(Modifier::UNDERLINED),
        "Should be underlined"
    );

    // Other spans should not be underlined
    let normal = spans
        .iter()
        .find(|s| s.content.contains("Error:"))
        .ok_or(TestError::MissingSpan { label: "Error" })?;
    assert!(
        !normal.style.add_modifier.contains(Modifier::UNDERLINED),
        "Normal text should not be underlined"
    );

    Ok(())
}

/// Tests error message with multiple formatting.
#[test]
fn test_error_message_with_multiple_formatting() -> Result<(), TestError> {
    // Tabs + bullets + underline
    let message = "Error:\t\u{0015}\u{0002}\u{0001}bad\u{0002}\u{0002}";
    let spans = process_text_for_display(message);

    let text: String = spans.iter().map(|s| s.content.as_ref()).collect();

    // Tab expanded
    assert!(!text.contains('\t'));

    // Bullet rendered
    assert!(text.contains('•'));

    // Underline applied to correct span
    let bad_span = spans
        .iter()
        .find(|s| s.content == "bad")
        .ok_or(TestError::MissingSpan { label: "bad" })?;
    assert!(bad_span.style.add_modifier.contains(Modifier::UNDERLINED));

    Ok(())
}

/// Helper function (from TUI code)
fn process_text_for_display(text: &str) -> Vec<Span<'static>> {
    // First, expand tabs and render control characters
    let mut processed = String::with_capacity(text.len() * 2);
    let mut display_pos: usize = 0;

    for ch in text.chars() {
        match ch {
            '\t' => {
                // Expand tab to next 8-column boundary
                let spaces_to_add = 8 - (display_pos % 8);
                for _ in 0..spaces_to_add {
                    processed.push(' ');
                    display_pos += 1;
                }
            }
            '\u{0015}' => {
                // Bullet delimiter - render as visible bullet
                processed.push('•');
                display_pos += 1;
            }
            _ => {
                processed.push(ch);
                // Use unicode-width for proper display width
                let width = ch.width().unwrap_or(1);
                display_pos += width;
            }
        }
    }

    // Then, parse underline markers
    parse_underline_markers(&processed)
}

/// Parses underline markers.
fn parse_underline_markers(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    let mut is_underlined = false;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{0002}' {
            // Check if this is a marker
            if let Some(&next_ch) = chars.peek() {
                if next_ch == '\u{0001}' {
                    // UNDERLINE_BEGIN
                    chars.next(); // Consume the \u{0001}

                    // Flush current text as normal span (owned)
                    if !current_text.is_empty() {
                        spans.push(Span::raw(current_text.clone()));
                        current_text.clear();
                    }
                    is_underlined = true;
                    continue;
                } else if next_ch == '\u{0002}' {
                    // UNDERLINE_END
                    chars.next(); // Consume the second \u{0002}

                    // Flush current text as underlined span (owned)
                    if !current_text.is_empty() {
                        spans.push(Span::styled(
                            current_text.clone(),
                            Style::default().add_modifier(Modifier::UNDERLINED),
                        ));
                        current_text.clear();
                    }
                    is_underlined = false;
                    continue;
                }
            }
        }

        // Regular character - add to current text
        current_text.push(ch);
    }

    // Flush any remaining text (owned)
    if !current_text.is_empty() {
        if is_underlined {
            spans.push(Span::styled(
                current_text,
                Style::default().add_modifier(Modifier::UNDERLINED),
            ));
        } else {
            spans.push(Span::raw(current_text));
        }
    }

    // Return at least one span
    if spans.is_empty() {
        vec![Span::raw(String::new())]
    } else {
        spans
    }
}
