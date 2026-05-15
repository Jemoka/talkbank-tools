//! Test module for validation word in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::ErrorCollector;
use talkbank_model::LanguageCode;
use talkbank_model::validation::{Validate, ValidationContext};

use super::roundtrip::{RoundtripError, parse_word_direct, true_roundtrip_tier_with_validation};

/// Validates word with language.
fn validate_word_with_language(
    word: &talkbank_model::Word,
    lang: &str,
) -> Vec<talkbank_model::ParseError> {
    let ctx = ValidationContext::new()
        .with_default_language(LanguageCode::new(lang))
        .with_declared_languages(vec![LanguageCode::new(lang)]);
    let errors = ErrorCollector::new();
    word.validate(&ctx, &errors);
    errors.into_vec()
}

// Word-Level Validation Errors (E2xx)

/// Runs validation error e220 word with digits in english.
#[test]
fn validation_error_e220_word_with_digits_in_english() -> Result<(), RoundtripError> {
    let input = "hello123";

    let result = true_roundtrip_tier_with_validation(input, parse_word_direct, |w| {
        validate_word_with_language(w, "eng")
    });
    match result {
        Err(RoundtripError::Validation { .. }) => Ok(()),
        Err(err) => Err(err),
        Ok(()) => Err(RoundtripError::Validation {
            count: 0,
            messages: Vec::new(),
        }),
    }
}

/// Runs validation success word with digits in chinese.
#[test]
fn validation_success_word_with_digits_in_chinese() -> Result<(), RoundtripError> {
    let input = "你好123";

    true_roundtrip_tier_with_validation(input, parse_word_direct, |w| {
        validate_word_with_language(w, "zho")
    })
}
