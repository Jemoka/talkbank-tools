//! Test module for utterance roundtrip in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::ErrorCollector;
use talkbank_model::validation::{Validate, ValidationContext};

use super::roundtrip::{
    RoundtripError, parse_utterance_direct, true_roundtrip_tier_with_validation,
};

/// Validates utterance.
fn validate_utterance(utterance: &talkbank_model::Utterance) -> Vec<talkbank_model::ParseError> {
    let ctx = ValidationContext::default();
    let errors = ErrorCollector::new();
    utterance.validate(&ctx, &errors);
    errors.into_vec()
}

// Composite Type Roundtrip Tests (Utterances)

/// Runs roundtrip utterance simple.
#[test]
fn roundtrip_utterance_simple() -> Result<(), RoundtripError> {
    let input = "*CHI:\thello .";
    true_roundtrip_tier_with_validation(input, parse_utterance_direct, validate_utterance)
}

/// Runs roundtrip utterance with pause.
#[test]
fn roundtrip_utterance_with_pause() -> Result<(), RoundtripError> {
    let input = "*MOT:\thello (.) there .";
    true_roundtrip_tier_with_validation(input, parse_utterance_direct, validate_utterance)
}

/// Runs roundtrip utterance with replacement.
#[test]
fn roundtrip_utterance_with_replacement() -> Result<(), RoundtripError> {
    let input = "*CHI:\tgoed [: went] .";
    true_roundtrip_tier_with_validation(input, parse_utterance_direct, validate_utterance)
}

/// Runs roundtrip utterance complex.
#[test]
fn roundtrip_utterance_complex() -> Result<(), RoundtripError> {
    let input = "*CHI:\tI goed [: went] home .";
    true_roundtrip_tier_with_validation(input, parse_utterance_direct, validate_utterance)
}

/// Runs roundtrip utterance question.
#[test]
fn roundtrip_utterance_question() -> Result<(), RoundtripError> {
    let input = "*MOT:\twhere are you ?";
    true_roundtrip_tier_with_validation(input, parse_utterance_direct, validate_utterance)
}
