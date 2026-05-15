//! Shared imports for position-conversion integration tests.

pub use chatter_lsp::backend::utils::{offset_to_position, position_to_offset};
pub use tower_lsp::lsp_types::Position;

/// Assert one byte-offset to LSP-position conversion.
pub fn assert_offset_to_position(text: &str, offset: usize, line: u32, character: u32) {
    assert_eq!(
        offset_to_position(
            text,
            u32::try_from(offset).expect("position-conversion offset fits in u32"),
        ),
        Position { line, character }
    );
}

/// Assert one LSP-position to byte-offset conversion.
pub fn assert_position_to_offset(text: &str, line: u32, character: u32, expected_offset: usize) {
    assert_eq!(
        position_to_offset(text, Position { line, character }),
        expected_offset
    );
}

/// Assert that byte offsets roundtrip through LSP positions.
pub fn assert_roundtrip_offsets(text: &str, offsets: &[usize]) {
    for &byte_offset in offsets {
        let pos = offset_to_position(
            text,
            u32::try_from(byte_offset).expect("position-conversion offset fits in u32"),
        );
        let roundtrip_offset = position_to_offset(text, pos);
        assert_eq!(byte_offset, roundtrip_offset);
    }
}
