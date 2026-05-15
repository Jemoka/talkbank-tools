#![allow(unused_imports)]

//! Property 1: Parser Never Panics
//!
//! The parser should gracefully handle ANY input, even garbage.
//! It may return Err, but should never panic.
//!
//! Tests BOTH TreeSitterParser and TreeSitterParser to ensure both are robust.

use super::*;
use proptest::prelude::*;

proptest! {
    #![proptest_config(slow_test_config())]

    /// Parses word never panics.
    #[test]
    fn parse_word_never_panics(s in "\\PC*") {
        // Property: parse_word accepts any string and doesn't panic
        // Test BOTH parsers to ensure both are robust
        for parser in parser_suite_for_proptest().unwrap() {
            let _ = parser.parse_word(&s);  // May be Ok or Err, but shouldn't panic
        }
    }

    /// Parses word handles unicode.
    #[test]
    fn parse_word_handles_unicode(s in "[\\p{Any}]{0,50}") {
        // Property: parse_word handles unicode characters gracefully
        // Test BOTH parsers to ensure both handle unicode correctly
        for parser in parser_suite_for_proptest().unwrap() {
            let _ = parser.parse_word(&s);
        }
    }

    /// `parse_chat_file` handles any unicode input without panicking.
    /// Covers the 2026-04-22 fuzz crash where error-analysis sliced a
    /// header name by byte index, cutting inside a multi-byte UTF-8
    /// character ("end byte index 7 is not a char boundary; it is
    /// inside '˻'"). The fix migrated to `str::strip_prefix`; this
    /// property keeps similar char-boundary bugs from reappearing in
    /// any other header / text path.
    #[test]
    fn parse_chat_file_never_panics_on_unicode(s in "[\\p{Any}]{0,80}") {
        use talkbank_model::ChatParser;
        let parser = talkbank_parser::TreeSitterParser::new().unwrap();
        let _ = parser.parse_chat_file(&s);
    }
}

/// Specific regression for the 2026-04-22 fuzz-found crash. The input
/// is the exact 10-byte sequence libFuzzer minimized
/// (`@%\x00A\x00*˻V*`); kept here alongside the proptest so bisection
/// is obvious if it ever regresses.
#[test]
fn regression_char_boundary_in_header_prefix_check() {
    use talkbank_model::ChatParser;
    let bytes: &[u8] = &[64, 37, 0, 65, 0, 42, 203, 187, 86, 42];
    let s = std::str::from_utf8(bytes).expect("input is valid UTF-8");
    let parser = talkbank_parser::TreeSitterParser::new().unwrap();
    let _ = parser.parse_chat_file(s);
}
