//! Test that TUI correctly expands tabs for aligned caret display\n
/// Tests tab expansion simple.
#[test]
fn test_tab_expansion_simple() {
    // Simulate the expand_tabs_for_display function
    let text = "*CHI:\thello world";
    let span_start = 6; // Start of "hello" after tab
    let span_end = 11; // End of "hello"

    let (display, display_start, display_length) =
        expand_tabs_for_display(text, span_start, span_end);

    // "*CHI:" = 5 chars, then tab expands to column 8 (next 8-boundary)
    // So tab becomes 3 spaces: "*CHI:   hello world"
    assert_eq!(display, "*CHI:   hello world");

    // "hello" starts at display column 8
    assert_eq!(display_start, 8);

    // "hello" is 5 characters
    assert_eq!(display_length, 5);
}

/// Tests tab expansion chat format.
#[test]
fn test_tab_expansion_chat_format() {
    // CHAT format: *CHI:\tword .
    let text = "*CHI:\tword .";
    let span_start = 6; // Start of "word"
    let span_end = 10; // End of "word"

    let (display, display_start, display_length) =
        expand_tabs_for_display(text, span_start, span_end);

    // Tab should expand to align at column 8
    assert!(display.contains("word"));
    assert!(!display.contains('\t'), "Tabs should be expanded");
    assert_eq!(
        display.chars().filter(|c| *c == '\t').count(),
        0,
        "No tabs should remain"
    );

    // Display should have spaces instead of tab
    assert_eq!(display, "*CHI:   word .");

    // "word" starts at column 8
    assert_eq!(display_start, 8);
    assert_eq!(display_length, 4);
}

/// Tests tab expansion multiple tabs.
#[test]
fn test_tab_expansion_multiple_tabs() {
    // Text with multiple tabs
    let text = "a\tb\tc";
    // a = 0, tab, b = 2, tab, c = 4
    let span_start = 2;
    let span_end = 3;

    let (_display, display_start, display_length) =
        expand_tabs_for_display(text, span_start, span_end);

    // "a" + tab (7 spaces to col 8) + "b" + tab + "c"
    // a is at 0, tab expands to 8, b is at 8, tab expands to 16, c is at 16
    assert_eq!(display_start, 8, "b should start at display column 8");
    assert_eq!(display_length, 1, "b is 1 character");
}

/// Tests tab expansion no tabs.
#[test]
fn test_tab_expansion_no_tabs() {
    // No tabs - should be identity
    let text = "hello world";
    let span_start = 0;
    let span_end = 5;

    let (display, display_start, display_length) =
        expand_tabs_for_display(text, span_start, span_end);

    assert_eq!(display, "hello world");
    assert_eq!(display_start, 0);
    assert_eq!(display_length, 5);
}

/// Helper function (copied from TUI code for testing)
fn expand_tabs_for_display(
    text: &str,
    span_start: usize,
    span_end: usize,
) -> (String, usize, usize) {
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

        if ch == '\t' {
            // Expand tab to next 8-column boundary
            let spaces_to_add = 8 - (display_pos % 8);
            for _ in 0..spaces_to_add {
                display.push(' ');
                display_pos += 1;
            }
        } else {
            display.push(ch);
            display_pos += 1;
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
