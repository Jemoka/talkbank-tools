#![allow(unused_imports)]

//! Property 6: Error Messages Are Helpful
//!
//! Tests BOTH TreeSitterParser and TreeSitterParser to ensure error reporting is consistent.

use super::*;
use proptest::prelude::*;

proptest! {
    #![proptest_config(slow_test_config())]

    /// Verifies parser-emitted diagnostics always contain non-empty messages.
    #[test]
    fn errors_have_non_empty_messages(s in "\\PC*") {
        for parser in parser_suite_for_proptest().unwrap() {
            if let Err(errors) = parser.parse_word(&s) {
                for error in &errors.errors {
                    // Property: error messages should be non-empty
                    prop_assert!(
                        !error.message.is_empty(),
                        "[{}] produced empty error message for '{}'",
                        parser.name(),
                        s
                    );

                }
            }
        }
    }
}

// Regular test (not property-based)
/// Verifies empty input is rejected.
#[test]
fn empty_string_produces_error() -> Result<(), TestError> {
    // Property: truly empty input should produce an error
    for parser in parser_suite()? {
        let result = parser.parse_word("");
        if result.is_ok() {
            return Err(TestError::Failure(format!(
                "[{}] Empty string should error",
                parser.name()
            )));
        }
    }
    Ok(())
}
