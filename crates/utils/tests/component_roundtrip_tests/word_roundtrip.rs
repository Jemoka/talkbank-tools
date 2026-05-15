//! Test module for word roundtrip in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::ErrorCollector;
use talkbank_model::validation::{Validate, ValidationContext};

use super::roundtrip::{RoundtripError, parse_word_direct, true_roundtrip_tier_with_validation};

/// Validates word.
fn validate_word(word: &talkbank_model::Word) -> Vec<talkbank_model::ParseError> {
    let ctx = ValidationContext::default();
    let errors = ErrorCollector::new();
    word.validate(&ctx, &errors);
    errors.into_vec()
}

// Composite Type Roundtrip Tests (Words)

/// Runs roundtrip word simple.
#[test]
fn roundtrip_word_simple() -> Result<(), RoundtripError> {
    let input = "hello";
    true_roundtrip_tier_with_validation(input, parse_word_direct, validate_word)
}

/// Runs roundtrip word with form type.
#[test]
fn roundtrip_word_with_form_type() -> Result<(), RoundtripError> {
    let input = "hello@b";
    true_roundtrip_tier_with_validation(input, parse_word_direct, validate_word)
}

/// Runs roundtrip word with shortening.
#[test]
fn roundtrip_word_with_shortening() -> Result<(), RoundtripError> {
    let input = "hel(lo)";
    true_roundtrip_tier_with_validation(input, parse_word_direct, validate_word)
}

/// Runs roundtrip word with category.
#[test]
fn roundtrip_word_with_category() -> Result<(), RoundtripError> {
    let input = "&-uh";
    true_roundtrip_tier_with_validation(input, parse_word_direct, validate_word)
}

/// Runs roundtrip word omission.
#[test]
fn roundtrip_word_omission() -> Result<(), RoundtripError> {
    let input = "0he";
    true_roundtrip_tier_with_validation(input, parse_word_direct, validate_word)
}
