#![allow(unused_imports)]

//! Property 4: Cleaned Text Has Expected Properties
//!
//! Tests BOTH TreeSitterParser and TreeSitterParser to ensure cleaned_text processing is consistent.

use super::*;
use proptest::prelude::*;

proptest! {
    #![proptest_config(slow_test_config())]

    /// Verifies cleaned text removes category prefixes.
    #[test]
    fn cleaned_text_no_category_prefix(
        prefix_idx in 0..4usize,
        text in "[a-z]+"
    ) {
        let prefixes = ["&-", "&~", "&+", "0"];
        let prefix = prefixes[prefix_idx];
        let input = format!("{}{}", prefix, text);
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&input) {
                // Property: cleaned text should parse to a word without category markers
                match parser.parse_word(&word.cleaned_text()) {
                    Ok(cleaned_word) => {
                        prop_assert!(
                            cleaned_word.category.is_none(),
                            "[{}] cleaned_text retains category for '{}'",
                            parser.name(),
                            input
                        );
                    }
                    Err(_) => {
                        prop_assert!(
                            false,
                            "[{}] cleaned_text did not parse for '{}'",
                            parser.name(),
                            input
                        );
                    }
                }
            }
        }
    }

    /// Verifies cleaned text removes form-type markers.
    #[test]
    fn cleaned_text_no_form_marker(text in "[a-z]+") {
        let input = format!("{}@b", text);
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&input) {
                // Property: cleaned text should not contain @marker
                prop_assert!(
                    !word.cleaned_text().contains('@'),
                    "[{}] cleaned_text contains '@' for '{}'",
                    parser.name(),
                    input
                );
            }
        }
    }

    // TODO(parser-tests): Refactor annotation testing after parse_word() API stabilizes
    // Status: Blocked - parse_word() API changed; test needs to use parse_annotated_word()
    // Context: parse_word() returns bare Word; annotations are parsed at utterance level
    // Fix: Either remove this test or create parse_annotated_word() in ChatParser trait
    /// Verifies cleaned text excludes bracketed annotation syntax.
    #[test]
    #[ignore = "Needs refactoring - parse_word() now returns bare Word without annotations"]
    fn cleaned_text_no_brackets(text in "[a-z]+") {
        let input = format!("{} [: annotation]", text);
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&input) {
                // Property: cleaned text should not contain annotations
                prop_assert!(!word.cleaned_text().contains('['));
                prop_assert!(!word.cleaned_text().contains(']'));
            }
        }
    }
}
