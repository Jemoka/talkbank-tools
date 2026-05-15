//! Test that TUI correctly parses and displays underline markers
//!
//! CHAT format uses:
//! - UNDERLINE_BEGIN = "\u0002\u0001"
//! - UNDERLINE_END = "\u0002\u0002"

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

/// Helper function (copied from TUI code for testing)
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

/// Tests no underline markers.
#[test]
fn test_no_underline_markers() {
    let text = "hello world";
    let spans = parse_underline_markers(text);

    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "hello world");
    // No underline modifier
    assert!(!spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
}

/// Tests simple underline.
#[test]
fn test_simple_underline() {
    let text = "hello \u{0002}\u{0001}underlined\u{0002}\u{0002} world";
    let spans = parse_underline_markers(text);

    assert_eq!(
        spans.len(),
        3,
        "Should have 3 spans: before, underlined, after"
    );

    assert_eq!(spans[0].content, "hello ");
    assert!(!spans[0].style.add_modifier.contains(Modifier::UNDERLINED));

    assert_eq!(spans[1].content, "underlined");
    assert!(
        spans[1].style.add_modifier.contains(Modifier::UNDERLINED),
        "Middle span should be underlined"
    );

    assert_eq!(spans[2].content, " world");
    assert!(!spans[2].style.add_modifier.contains(Modifier::UNDERLINED));
}

/// Tests multiple underline sections.
#[test]
fn test_multiple_underline_sections() {
    let text =
        "\u{0002}\u{0001}first\u{0002}\u{0002} normal \u{0002}\u{0001}second\u{0002}\u{0002}";
    let spans = parse_underline_markers(text);

    assert_eq!(spans.len(), 3);

    assert_eq!(spans[0].content, "first");
    assert!(spans[0].style.add_modifier.contains(Modifier::UNDERLINED));

    assert_eq!(spans[1].content, " normal ");
    assert!(!spans[1].style.add_modifier.contains(Modifier::UNDERLINED));

    assert_eq!(spans[2].content, "second");
    assert!(spans[2].style.add_modifier.contains(Modifier::UNDERLINED));
}

/// Tests entire text underlined.
#[test]
fn test_entire_text_underlined() {
    let text = "\u{0002}\u{0001}all underlined\u{0002}\u{0002}";
    let spans = parse_underline_markers(text);

    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "all underlined");
    assert!(spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
}

/// Tests unclosed underline.
#[test]
fn test_unclosed_underline() {
    // If underline is not closed, it should still be underlined
    let text = "normal \u{0002}\u{0001}unclosed";
    let spans = parse_underline_markers(text);

    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].content, "normal ");
    assert!(!spans[0].style.add_modifier.contains(Modifier::UNDERLINED));

    assert_eq!(spans[1].content, "unclosed");
    assert!(
        spans[1].style.add_modifier.contains(Modifier::UNDERLINED),
        "Unclosed underline should still apply"
    );
}

/// Tests empty string.
#[test]
fn test_empty_string() {
    let text = "";
    let spans = parse_underline_markers(text);

    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "");
}
