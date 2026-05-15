#![allow(unused_imports)]

//! Property 5: Shortening Expansion
//!
//! Tests BOTH TreeSitterParser and TreeSitterParser to ensure shortening expansion is consistent.

use super::*;
use proptest::prelude::*;

proptest! {
    #![proptest_config(slow_test_config())]

    /// Verifies shortening payload text is preserved in cleaned text.
    #[test]
    fn shortening_always_expands(
        prefix in "[a-z]{1,3}",
        shortening in "[a-z]{1,3}",
        suffix in "[a-z]{0,3}"
    ) {
        let input = format!("{}({}){}",  prefix, shortening, suffix);
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&input) {
                // Property: cleaned text includes shortening content
                let expected = format!("{}{}{}", prefix, shortening, suffix);
                prop_assert_eq!(
                    word.cleaned_text(),
                    expected,
                    "[{}] shortening expansion failed for '{}'",
                    parser.name(),
                    input
                );
            }
        }
    }
}
