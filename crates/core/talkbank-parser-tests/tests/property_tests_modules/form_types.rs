#![allow(unused_imports)]

//! Property 3: Form Types Are Always Detected
//!
//! Tests BOTH TreeSitterParser and TreeSitterParser to ensure form type detection is consistent.

use super::*;
use proptest::prelude::*;
use talkbank_model::model::FormType;

proptest! {
    #![proptest_config(slow_test_config())]

    /// Verifies `@b` form types are detected.
    #[test]
    fn form_type_b_always_detected(text in "[a-z]+") {
        let input = format!("{}@b", text);
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&input) {
                prop_assert_eq!(
                    word.form_type,
                    Some(FormType::B),
                    "[{}] failed to detect form type B in '{}'",
                    parser.name(),
                    input
                );
            }
        }
    }

    /// Verifies `@c` form types are detected.
    #[test]
    fn form_type_c_always_detected(text in "[a-z]+") {
        let input = format!("{}@c", text);
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&input) {
                prop_assert_eq!(
                    word.form_type,
                    Some(FormType::C),
                    "[{}] failed to detect form type C in '{}'",
                    parser.name(),
                    input
                );
            }
        }
    }

    /// Verifies `@d` form types are detected.
    #[test]
    fn form_type_d_always_detected(text in "[a-z]+") {
        let input = format!("{}@d", text);
        for parser in parser_suite_for_proptest().unwrap() {
            if let Ok(word) = parser.parse_word(&input) {
                prop_assert_eq!(
                    word.form_type,
                    Some(FormType::D),
                    "[{}] failed to detect form type D in '{}'",
                    parser.name(),
                    input
                );
            }
        }
    }
}
