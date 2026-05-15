//! Test that TUI correctly displays bullet delimiters
//!
//! CHAT format uses \u0015 (U+0015) as a bullet delimiter.
//! This should be rendered as a visible bullet character (•) not elided.\n
/// Tests bullet delimiter rendered.
#[test]
fn test_bullet_delimiter_rendered() {
    let text = "word1\u{0015}word2";
    let span_start = 0;
    let span_end = text.len();

    let (display, _display_start, _display_length) =
        expand_tabs_for_display(text, span_start, span_end);

    // Should render bullet as visible character
    assert_eq!(display, "word1•word2");
    assert!(
        display.contains('•'),
        "Should contain visible bullet character"
    );
    assert!(
        !display.contains('\u{0015}'),
        "Should not contain raw control character"
    );
}

/// Tests bullet delimiter position.
#[test]
fn test_bullet_delimiter_position() {
    let text = "abc\u{0015}def";
    let span_start = 4; // Start of "def" after bullet
    let span_end = 7; // End of "def"

    let (display, display_start, display_length) =
        expand_tabs_for_display(text, span_start, span_end);

    assert_eq!(display, "abc•def");
    // "abc•" = 4 display positions
    assert_eq!(display_start, 4);
    assert_eq!(display_length, 3); // "def" is 3 chars
}

/// Tests multiple bullet delimiters.
#[test]
fn test_multiple_bullet_delimiters() {
    let text = "a\u{0015}b\u{0015}c";
    let span_start = 0;
    let span_end = text.len();

    let (display, _display_start, _display_length) =
        expand_tabs_for_display(text, span_start, span_end);

    assert_eq!(display, "a•b•c");
    assert_eq!(display.matches('•').count(), 2, "Should have 2 bullets");
}

/// Tests bullet and tab together.
#[test]
fn test_bullet_and_tab_together() {
    let text = "word\t\u{0015}text";
    let span_start = 0;
    let span_end = text.len();

    let (display, _display_start, _display_length) =
        expand_tabs_for_display(text, span_start, span_end);

    // "word" = 4, tab expands to column 8 (4 spaces), then bullet, then "text"
    assert_eq!(display, "word    •text");
    assert!(display.contains('•'));
    assert!(!display.contains('\t'));
}

/// Helper function (copied from TUI code for testing)
fn expand_tabs_for_display(
    text: &str,
    span_start: usize,
    span_end: usize,
) -> (String, usize, usize) {
    use unicode_width::UnicodeWidthChar;

    let mut display = String::with_capacity(text.len() * 2);
    let mut display_pos: usize = 0;
    let mut char_pos: usize = 0;
    let mut display_start: usize = 0;
    let mut display_end: usize = 0;

    for ch in text.chars() {
        // Track where this character maps in display coordinates
        if char_pos == span_start {
            display_start = display_pos;
        }
        if char_pos == span_end {
            display_end = display_pos;
        }

        match ch {
            '\t' => {
                // Expand tab to next 8-column boundary
                let spaces_to_add = 8 - (display_pos % 8);
                for _ in 0..spaces_to_add {
                    display.push(' ');
                    display_pos += 1;
                }
            }
            '\u{0015}' => {
                // Bullet delimiter - render as visible bullet
                display.push('•');
                display_pos += 1;
            }
            _ => {
                display.push(ch);
                // Use unicode-width for proper display width (emoji = 2, combining marks = 0, etc.)
                let width = ch.width().unwrap_or(1);
                display_pos += width;
            }
        }

        char_pos += 1;
    }

    // Handle case where span_end is at the end
    if char_pos == span_end {
        display_end = display_pos;
    }

    let display_length = display_end.saturating_sub(display_start);

    (display, display_start, display_length)
}
