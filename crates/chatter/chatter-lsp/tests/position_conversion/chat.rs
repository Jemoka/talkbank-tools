// CHAT-specific position conversion regression tests.

use crate::position_conversion::assert_offset_to_position;

/// Verify offsets inside a CHAT line containing emoji.
#[test]
fn test_chat_file_with_emoji() {
    let text = "@UTF8\n@Begin\n*CHI:\thello 😀 world .\n@End\n";

    for (offset, line, character) in [(13, 2, 0), (19, 2, 6), (25, 2, 12), (29, 2, 13)] {
        assert_offset_to_position(text, offset, line, character);
    }
}

/// Verify offsets inside a CHAT line containing CJK characters.
#[test]
fn test_chat_file_with_cjk() {
    let text = "@UTF8\n*CHI:\t中文 .\n";

    for (offset, line, character) in [(6, 1, 0), (12, 1, 6), (15, 1, 7)] {
        assert_offset_to_position(text, offset, line, character);
    }
}
