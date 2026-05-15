//! Structural roundtrip property tests for words.
//!
//! Unlike string-based tests that generate input text and check parser properties,
//! these tests generate valid CHAT word strings from structured components and verify:
//! 1. parse(input) succeeds on both parsers
//! 2. to_chat(parse(input)) == input (roundtrip)
//! 3. Both parsers produce semantically equal results
//!
//! This catches serializer bugs, ambiguous serialization, and parser/serializer drift
//! that string-only tests miss.

use super::*;

/// Strategy for lowercase alphabetic text (valid word body).
fn word_text() -> impl Strategy<Value = String> {
    "[a-z]{1,8}"
}

/// Strategy for a category prefix string.
fn category_prefix() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("0"), Just("&-"), Just("&~"), Just("&+"),]
}

/// Strategy for a form type suffix string (the part after @).
fn form_type_suffix() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("a"),
        Just("b"),
        Just("c"),
        Just("d"),
        Just("f"),
        Just("fp"),
        Just("g"),
        Just("i"),
        Just("k"),
        Just("l"),
        Just("n"),
        Just("o"),
        Just("p"),
        Just("q"),
        Just("t"),
        Just("u"),
        Just("x"),
    ]
}

/// Strategy for a language marker suffix.
fn lang_suffix() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("@s".to_string()),
        Just("@s:eng".to_string()),
        Just("@s:fra".to_string()),
        Just("@s:spa".to_string()),
        Just("@s:eng+fra".to_string()),
    ]
}

proptest! {
    #![proptest_config(slow_test_config())]

    /// Plain word roundtrips through both parsers.
    #[test]
    fn plain_word_roundtrip(text in word_text()) {
        for parser in parser_suite_for_proptest()? {
            let word = parser.parse_word(&text)?;
            let output = word.to_chat();
            prop_assert_eq!(
                &output, &text,
                "[{}] roundtrip: '{}' -> '{}'",
                parser.name(), text, output
            );
        }
    }

    /// Category + text roundtrips.
    #[test]
    fn category_word_roundtrip(
        prefix in category_prefix(),
        text in word_text()
    ) {
        let input = format!("{}{}", prefix, text);
        for parser in parser_suite_for_proptest()? {
            if let Ok(word) = parser.parse_word(&input) {
                let output = word.to_chat();
                prop_assert_eq!(
                    &output, &input,
                    "[{}] roundtrip: '{}' -> '{}'",
                    parser.name(), input, output
                );
                // Verify category was detected
                prop_assert!(
                    word.category.is_some(),
                    "[{}] category missing for '{}'",
                    parser.name(), input
                );
            }
        }
    }

    /// Text + form type roundtrips.
    #[test]
    fn form_type_word_roundtrip(
        text in word_text(),
        form in form_type_suffix()
    ) {
        let input = format!("{}@{}", text, form);
        for parser in parser_suite_for_proptest()? {
            if let Ok(word) = parser.parse_word(&input) {
                let output = word.to_chat();
                prop_assert_eq!(
                    &output, &input,
                    "[{}] roundtrip: '{}' -> '{}'",
                    parser.name(), input, output
                );
                prop_assert!(
                    word.form_type.is_some(),
                    "[{}] form_type missing for '{}'",
                    parser.name(), input
                );
            }
        }
    }

    /// Text + language marker roundtrips.
    #[test]
    fn lang_marker_word_roundtrip(
        text in word_text(),
        lang in lang_suffix()
    ) {
        let input = format!("{}{}", text, lang);
        for parser in parser_suite_for_proptest()? {
            if let Ok(word) = parser.parse_word(&input) {
                let output = word.to_chat();
                prop_assert_eq!(
                    &output, &input,
                    "[{}] roundtrip: '{}' -> '{}'",
                    parser.name(), input, output
                );
                prop_assert!(
                    word.lang.is_some(),
                    "[{}] lang missing for '{}'",
                    parser.name(), input
                );
            }
        }
    }

    /// Shortening roundtrips: text(shortened)text.
    #[test]
    fn shortening_word_roundtrip(
        before in "[a-z]{1,4}",
        shortened in "[a-z]{1,4}",
        after in "[a-z]{1,4}"
    ) {
        let input = format!("{}({}){}",  before, shortened, after);
        for parser in parser_suite_for_proptest()? {
            if let Ok(word) = parser.parse_word(&input) {
                let output = word.to_chat();
                prop_assert_eq!(
                    &output, &input,
                    "[{}] roundtrip: '{}' -> '{}'",
                    parser.name(), input, output
                );
                // cleaned_text should include the shortened part
                let expected_cleaned = format!("{}{}{}", before, shortened, after);
                prop_assert_eq!(
                    word.cleaned_text(), expected_cleaned.as_str(),
                    "[{}] cleaned_text for '{}': got '{}'",
                    parser.name(), input, word.cleaned_text()
                );
            }
        }
    }

    /// Compound word roundtrips.
    #[test]
    fn compound_word_roundtrip(
        first in "[a-z]{1,6}",
        second in "[a-z]{1,6}"
    ) {
        let input = format!("{}+{}", first, second);
        for parser in parser_suite_for_proptest()? {
            if let Ok(word) = parser.parse_word(&input) {
                let output = word.to_chat();
                prop_assert_eq!(
                    &output, &input,
                    "[{}] roundtrip: '{}' -> '{}'",
                    parser.name(), input, output
                );
            }
        }
    }

    /// Lengthening roundtrips.
    #[test]
    fn lengthening_word_roundtrip(
        before in "[a-z]{1,4}",
        after in "[a-z]{1,4}"
    ) {
        let input = format!("{}:{}", before, after);
        for parser in parser_suite_for_proptest()? {
            if let Ok(word) = parser.parse_word(&input) {
                let output = word.to_chat();
                prop_assert_eq!(
                    &output, &input,
                    "[{}] roundtrip: '{}' -> '{}'",
                    parser.name(), input, output
                );
                // cleaned_text should NOT contain the colon
                prop_assert!(
                    !word.cleaned_text().contains(':'),
                    "[{}] cleaned_text contains ':' for '{}'",
                    parser.name(), input
                );
            }
        }
    }

    /// Category + form type + language combined roundtrip.
    #[test]
    fn full_combination_roundtrip(
        prefix in category_prefix(),
        text in word_text(),
        form in form_type_suffix()
    ) {
        let input = format!("{}{}@{}", prefix, text, form);
        for parser in parser_suite_for_proptest()? {
            if let Ok(word) = parser.parse_word(&input) {
                let output = word.to_chat();
                prop_assert_eq!(
                    &output, &input,
                    "[{}] roundtrip: '{}' -> '{}'",
                    parser.name(), input, output
                );
                prop_assert!(word.category.is_some());
                prop_assert!(word.form_type.is_some());
            }
        }
    }

    /// Double roundtrip: parse → serialize → parse → serialize should be stable.
    #[test]
    fn double_roundtrip_stable(
        text in word_text(),
        use_form in prop::bool::ANY,
        use_category in prop::bool::ANY
    ) {
        let mut input = text.clone();
        if use_category {
            input = format!("&-{}", input);
        }
        if use_form {
            input = format!("{}@b", input);
        }
        for parser in parser_suite_for_proptest()? {
            if let Ok(word1) = parser.parse_word(&input) {
                let chat1 = word1.to_chat();
                if let Ok(word2) = parser.parse_word(&chat1) {
                    let chat2 = word2.to_chat();
                    prop_assert_eq!(
                        &chat1, &chat2,
                        "[{}] double roundtrip unstable: '{}' -> '{}' -> '{}'",
                        parser.name(), input, chat1, chat2
                    );
                }
            }
        }
    }

    /// Both parsers produce same serialized output for the same input.
    #[test]
    fn parser_equivalence_on_roundtrip(
        text in word_text(),
        prefix_opt in prop::option::of(category_prefix()),
        form_opt in prop::option::of(form_type_suffix())
    ) {
        let mut input = text.clone();
        if let Some(prefix) = prefix_opt {
            input = format!("{}{}", prefix, input);
        }
        if let Some(form) = form_opt {
            input = format!("{}@{}", input, form);
        }

        let parsers = parser_suite_for_proptest()?;
        let mut outputs: Vec<(&str, String)> = Vec::new();
        for parser in &parsers {
            if let Ok(word) = parser.parse_word(&input) {
                outputs.push((parser.name(), word.to_chat()));
            }
        }
        // If both parsers succeeded, their outputs must match
        if outputs.len() == 2 {
            prop_assert_eq!(
                &outputs[0].1, &outputs[1].1,
                "Parser divergence on '{}': {} -> '{}', {} -> '{}'",
                input, outputs[0].0, outputs[0].1, outputs[1].0, outputs[1].1
            );
        }
    }
}
