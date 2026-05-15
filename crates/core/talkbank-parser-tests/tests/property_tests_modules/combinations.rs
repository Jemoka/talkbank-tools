#![allow(unused_imports)]

//! Property 7: Combining Features
//!
//! Tests BOTH TreeSitterParser and TreeSitterParser to ensure feature combination is consistent.

use super::*;
use proptest::prelude::*;
use talkbank_model::model::{FormType, WordCategory};

proptest! {
    #![proptest_config(slow_test_config())]

    /// Verifies category and form-type markers can be detected together.
    #[test]
    fn category_and_form_type_both_detected(
        category_idx in 0..4usize,
        text in "[a-z]+",
        form_type_idx in 0..5usize
    ) {
        let categories = ["&-", "&~", "&+", "0"];
        let form_types = ["a", "b", "c", "d", "f"];
        let category = categories[category_idx];
        let form_type = form_types[form_type_idx];
        let input = format!("{}{}@{}", category, text, form_type);
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&input) {
                // Property: both category and form type should be present
                prop_assert!(
                    word.category.is_some(),
                    "[{}] category missing for '{}'",
                    parser.name(),
                    input
                );
                prop_assert!(
                    word.form_type.is_some(),
                    "[{}] form_type missing for '{}'",
                    parser.name(),
                    input
                );
            }
        }
    }
}
