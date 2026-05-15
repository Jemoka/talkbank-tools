//! Error Scenario Tests
//!
//! Tests for malformed word input - verifies error handling
//!
//! Tests BOTH TreeSitterParser and TreeSitterParser to ensure error handling is consistent.

use super::*;

/// Verifies empty input is rejected by both parser implementations.
#[test]
fn error_empty_word() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let result = parser.parse_word("");
        if result.is_ok() {
            return Err(TestError::Failure(format!(
                "[{}] Empty input should error",
                parser.name()
            )));
        }
    }
    Ok(())
}

/// Verifies unclosed shortening spans are rejected.
#[test]
fn error_unclosed_shortening() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let result = parser.parse_word("hel(lo");
        if result.is_ok() {
            return Err(TestError::Failure(format!(
                "[{}] Unclosed shortening should error",
                parser.name()
            )));
        }
    }
    Ok(())
}

/// Verifies dangling form-type markers are rejected.
#[test]
fn error_missing_form_type() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let result = parser.parse_word("hello@");
        if result.is_ok() {
            return Err(TestError::Failure(format!(
                "[{}] Missing form type should error",
                parser.name()
            )));
        }
    }
    Ok(())
}

/// Verifies unknown form-type markers are rejected.
/// NOTE: The structured word grammar accepts all @X form markers at parse time.
/// Validation of form type values (e.g., rejecting @z) is a validation-layer concern.
#[test]
#[ignore = "Form type validation moved to validation layer with structured word grammar"]
fn error_invalid_form_type() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let result = parser.parse_word("hello@z");
        if result.is_ok() {
            return Err(TestError::Failure(format!(
                "[{}] Invalid form type should error",
                parser.name()
            )));
        }
    }
    Ok(())
}
