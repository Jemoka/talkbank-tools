#![allow(unused_imports)]

//! Property 8: Raw Text Is Always Preserved
//!
//! Tests BOTH TreeSitterParser and TreeSitterParser to ensure raw_text preservation is consistent.

use super::*;
use proptest::prelude::*;

proptest! {
    #![proptest_config(slow_test_config())]

    /// Verifies `raw_text()` preserves the original lexical token.
    #[test]
    fn raw_text_matches_input(
        s in "[a-zA-Z0-9@&\\-\\(\\)\\[\\]:]{1,50}"
    ) {
        // NOTE: Space removed from pattern - "a b" is two words, not one word
        // parse_word parses ONE word, so input must not contain spaces
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&s) {
                // Property: raw_text should equal original input (trimmed, since words don't include trailing whitespace)
                // In CHAT format, whitespace is a separator between words, not part of the word itself
                prop_assert_eq!(
                    word.raw_text(),
                    s.as_str(),
                    "[{}] raw_text doesn't match input for '{}'",
                    parser.name(),
                    s
                );
            }
        }
    }
}
