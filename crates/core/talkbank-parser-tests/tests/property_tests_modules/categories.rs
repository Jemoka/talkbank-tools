#![allow(unused_imports)]

//! Property 2: Category Prefixes Are Always Detected
//!
//! Tests BOTH TreeSitterParser and TreeSitterParser to ensure category detection is consistent.

use super::*;
use proptest::prelude::*;
use talkbank_model::model::WordCategory;

proptest! {
    #![proptest_config(slow_test_config())]

    /// Verifies the filler category prefix is detected.
    #[test]
    fn category_filler_always_detected(text in "[a-z]+") {
        let input = format!("&-{}", text);
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&input) {
                prop_assert_eq!(
                    word.category,
                    Some(WordCategory::Filler),
                    "[{}] failed to detect filler category in '{}'",
                    parser.name(),
                    input
                );
            }
        }
    }

    /// Verifies the omission category prefix is detected.
    #[test]
    fn category_omission_always_detected(text in "[a-z]+") {
        // 0 prefix = omitted word (per CHAT/JFlex spec)
        let input = format!("0{}", text);
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&input) {
                prop_assert_eq!(
                    word.category,
                    Some(WordCategory::Omission),
                    "[{}] failed to detect omission category in '{}'",
                    parser.name(),
                    input
                );
            }
        }
    }

    /// Verifies the nonword category prefix is detected.
    #[test]
    fn category_nonword_always_detected(text in "[a-z]+") {
        // &~ prefix = nonword/babbling (per CHAT/JFlex spec)
        let input = format!("&~{}", text);
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&input) {
                prop_assert_eq!(
                    word.category,
                    Some(WordCategory::Nonword),
                    "[{}] failed to detect nonword category in '{}'",
                    parser.name(),
                    input
                );
            }
        }
    }

    /// Verifies the phonological-fragment category prefix is detected.
    #[test]
    fn category_phonological_fragment_always_detected(
        text in "[a-z]+"
    ) {
        // &+ prefix = phonological fragment (per CHAT/JFlex spec)
        let input = format!("&+{}", text);
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&input) {
                prop_assert_eq!(
                    word.category,
                    Some(WordCategory::PhonologicalFragment),
                    "[{}] failed to detect phonological fragment category in '{}'",
                    parser.name(),
                    input
                );
            }
        }
    }
}
