// Roundtrip byte-offset and position conversion tests.

use crate::position_conversion::assert_roundtrip_offsets;

/// Verify roundtrip conversion on ASCII text.
#[test]
fn test_roundtrip_ascii() {
    let text = "hello world\nline two\nline three";
    assert_roundtrip_offsets(text, &[0, 5, 6, 11, 12, 20, 21, 31]);
}

/// Verify roundtrip conversion around emoji boundaries.
#[test]
fn test_roundtrip_emoji() {
    let text = "hello 😀 world\n😀😀😀\nend";
    assert_roundtrip_offsets(text, &[0, 6, 10, 11, 16, 17, 21, 25, 29, 30, 33]);
}

/// Verify roundtrip conversion around CJK boundaries.
#[test]
fn test_roundtrip_cjk() {
    let text = "中文测试\nhello\n世界";
    assert_roundtrip_offsets(text, &[0, 3, 6, 9, 12, 13, 18, 19, 22, 25]);
}
