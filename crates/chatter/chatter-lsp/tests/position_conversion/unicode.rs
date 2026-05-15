// Unicode position conversion regression tests.

use crate::position_conversion::{assert_offset_to_position, assert_position_to_offset};

/// Verify conversion behavior around 4-byte emoji.
#[test]
fn test_emoji_offset_to_position() {
    let text = "hello 😀 world";

    for (offset, line, character) in [(0, 0, 0), (6, 0, 6), (10, 0, 7), (11, 0, 8)] {
        assert_offset_to_position(text, offset, line, character);
    }
}

/// Verify reverse conversion around emoji boundaries.
#[test]
fn test_emoji_position_to_offset() {
    let text = "hello 😀 world";

    for (line, character, expected_offset) in [(0, 0, 0), (0, 6, 6), (0, 7, 10), (0, 8, 11)] {
        assert_position_to_offset(text, line, character, expected_offset);
    }
}

/// Verify conversion behavior around CJK characters.
#[test]
fn test_cjk_offset_to_position() {
    let text = "hello 中文 world";

    for (offset, line, character) in [(0, 0, 0), (6, 0, 6), (9, 0, 7), (12, 0, 8)] {
        assert_offset_to_position(text, offset, line, character);
    }
}

/// Verify reverse conversion around CJK boundaries.
#[test]
fn test_cjk_position_to_offset() {
    let text = "hello 中文 world";

    for (line, character, expected_offset) in [(0, 6, 6), (0, 7, 9), (0, 8, 12)] {
        assert_position_to_offset(text, line, character, expected_offset);
    }
}

/// Verify conversion behavior around two-byte accented characters.
#[test]
fn test_accented_offset_to_position() {
    let text = "café";

    for (offset, line, character) in [(0, 0, 0), (3, 0, 3), (5, 0, 4)] {
        assert_offset_to_position(text, offset, line, character);
    }
}

/// Verify reverse conversion around two-byte accented characters.
#[test]
fn test_accented_position_to_offset() {
    let text = "café";

    for (line, character, expected_offset) in [(0, 0, 0), (0, 3, 3), (0, 4, 5)] {
        assert_position_to_offset(text, line, character, expected_offset);
    }
}

/// Verify mixed-width Unicode on a single line.
#[test]
fn test_mixed_multibyte_offset_to_position() {
    let text = "a é 中 😀 b";

    for (offset, line, character) in [(0, 0, 0), (2, 0, 2), (5, 0, 4), (9, 0, 6), (14, 0, 8)] {
        assert_offset_to_position(text, offset, line, character);
    }
}
