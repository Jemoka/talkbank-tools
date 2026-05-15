// Out-of-bounds position conversion regression tests.

use crate::position_conversion::{assert_offset_to_position, assert_position_to_offset};

/// Verify clamping when offsets extend beyond the document.
#[test]
fn test_offset_beyond_text() {
    let text = "hello";

    assert_offset_to_position(text, 5, 0, 5);
    assert_offset_to_position(text, 100, 0, 5);
}

/// Verify clamping when positions extend beyond the document.
#[test]
fn test_position_beyond_document() {
    let text = "line1\nline2";

    assert_position_to_offset(text, 10, 0, 11);
    assert_position_to_offset(text, 0, 100, 5);
}

/// Verify empty-document behavior.
#[test]
fn test_empty_document() {
    let text = "";

    assert_offset_to_position(text, 0, 0, 0);
    assert_position_to_offset(text, 0, 0, 0);
}
