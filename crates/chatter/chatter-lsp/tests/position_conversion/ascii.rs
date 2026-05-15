// ASCII-only position conversion regression tests.

use crate::position_conversion::{assert_offset_to_position, assert_position_to_offset};

/// Verify byte-to-position conversion on a single ASCII line.
#[test]
fn test_ascii_single_line_offset_to_position() {
    let text = "hello world";

    for (offset, line, character) in [(0, 0, 0), (5, 0, 5), (6, 0, 6)] {
        assert_offset_to_position(text, offset, line, character);
    }
}

/// Verify byte-to-position conversion across ASCII newlines.
#[test]
fn test_ascii_multiline_offset_to_position() {
    let text = "line1\nline2\nline3";

    for (offset, line, character) in [(0, 0, 0), (5, 0, 5), (6, 1, 0), (9, 1, 3), (12, 2, 0)]
    {
        assert_offset_to_position(text, offset, line, character);
    }
}

/// Verify position-to-byte conversion across ASCII-only content.
#[test]
fn test_ascii_position_to_offset() {
    let text = "line1\nline2\nline3";

    for (line, character, expected_offset) in
        [(0, 0, 0), (0, 5, 5), (1, 0, 6), (1, 3, 9), (2, 0, 12)]
    {
        assert_position_to_offset(text, line, character, expected_offset);
    }
}
