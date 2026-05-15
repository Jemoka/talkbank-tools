//! Test that TUI vertical bar alignment is correct
//!
//! The issue: │ (U+2502) is a 3-byte UTF-8 character, but .len() returns
//! byte length not display width, causing misalignment.\n
/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Missing bar character in {label}")]
    MissingBar { label: &'static str },
}

/// Finds bar pos.
fn find_bar_pos(input: &str, label: &'static str) -> Result<usize, TestError> {
    input
        .chars()
        .position(|c| c == '│')
        .ok_or(TestError::MissingBar { label })
}

/// Tests vertical bar byte length vs char count.
#[test]
fn test_vertical_bar_byte_length_vs_char_count() {
    let line_num_prefix = "  5 ";
    let full_with_bar = format!("{}│ ", line_num_prefix);

    // Byte length includes the 3 bytes for │
    let byte_length = full_with_bar.len();

    // Character count treats │ as one character
    let char_count = full_with_bar.chars().count();

    // These should be different!
    assert_eq!(
        byte_length, 8,
        "│ is 3 bytes: '  5 ' (4 bytes) + '│' (3 bytes) + ' ' (1 byte)"
    );
    assert_eq!(
        char_count, 6,
        "Display width: '  5 ' (4 chars) + '│' (1 char) + ' ' (1 char)"
    );
    assert_ne!(
        byte_length, char_count,
        "Byte length != char count due to │"
    );
}

/// Tests caret line alignment.
#[test]
fn test_caret_line_alignment() -> Result<(), TestError> {
    // Simulate creating source line and caret line
    let context_line = 5;
    let line_num_prefix = format!("  {} ", context_line);

    // Source line: "  5 │ source text"
    let source_line_prefix = format!("{}│ ", line_num_prefix);

    // Caret line: should align the │ at the same position
    // WRONG: let spaces_before_bar = source_line_prefix.len() - 2;  // Uses byte length!
    // RIGHT: Use char count for display width
    let spaces_before_bar = line_num_prefix.chars().count();

    let caret_line_prefix = format!("{}│ ", " ".repeat(spaces_before_bar));

    // Both should have the same character count
    assert_eq!(
        source_line_prefix.chars().count(),
        caret_line_prefix.chars().count(),
        "Source and caret line prefixes should have same display width"
    );

    // Verify the bars are at the same position
    let source_bar_pos = find_bar_pos(&source_line_prefix, "source line")?;
    let caret_bar_pos = find_bar_pos(&caret_line_prefix, "caret line")?;

    assert_eq!(
        source_bar_pos, caret_bar_pos,
        "Vertical bars should be at same position"
    );
    assert_eq!(
        source_bar_pos, 4,
        "Bar should be at position 4 (after '  5 ')"
    );

    Ok(())
}

/// Tests different line numbers.
#[test]
fn test_different_line_numbers() -> Result<(), TestError> {
    // Test that alignment works for different line number widths
    for line_num in [5, 50, 500] {
        let line_num_prefix = format!("  {} ", line_num);
        let source_line_prefix = format!("{}│ ", line_num_prefix);
        let spaces_before_bar = line_num_prefix.chars().count();
        let caret_line_prefix = format!("{}│ ", " ".repeat(spaces_before_bar));

        let source_bar_pos = find_bar_pos(&source_line_prefix, "source line")?;
        let caret_bar_pos = find_bar_pos(&caret_line_prefix, "caret line")?;

        assert_eq!(
            source_bar_pos, caret_bar_pos,
            "Bars should align for line {}",
            line_num
        );
    }

    Ok(())
}
