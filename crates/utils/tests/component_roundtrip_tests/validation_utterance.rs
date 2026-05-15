//! Test module for validation utterance in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::ErrorCollector;
use talkbank_model::SpeakerCode;
use talkbank_model::validation::{Validate, ValidationContext};

use super::roundtrip::{
    RoundtripError, parse_utterance_direct, true_roundtrip_tier_with_validation,
};

/// Validates utterance with participants.
fn validate_utterance_with_participants(
    utterance: &talkbank_model::Utterance,
    participants: Vec<&str>,
) -> Vec<talkbank_model::ParseError> {
    let ctx = ValidationContext::new()
        .with_participant_ids(participants.iter().map(|s| SpeakerCode::new(*s)).collect());
    let errors = ErrorCollector::new();
    utterance.validate(&ctx, &errors);
    errors.into_vec()
}

/// Validates utterance no participants.
fn validate_utterance_no_participants(
    utterance: &talkbank_model::Utterance,
) -> Vec<talkbank_model::ParseError> {
    validate_utterance_with_participants(utterance, vec![])
}

// Utterance/Main Tier Validation Errors (E3xx)

/// Runs validation error e308 utterance unknown speaker.
#[test]
fn validation_error_e308_utterance_unknown_speaker() -> Result<(), RoundtripError> {
    let input = "*MOT:\thello .";

    // Validate with only CHI as participant, so MOT is undeclared
    let result = true_roundtrip_tier_with_validation(input, parse_utterance_direct, |u| {
        validate_utterance_with_participants(u, vec!["CHI"])
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

/// Runs validation success utterance speaker in participants.
#[test]
fn validation_success_utterance_speaker_in_participants() -> Result<(), RoundtripError> {
    let input = "*MOT:\thello .";

    // Validate with MOT as participant
    true_roundtrip_tier_with_validation(input, parse_utterance_direct, |u| {
        validate_utterance_with_participants(u, vec!["MOT", "CHI"])
    })
}

/// Runs validation success speaker lowercase.
#[test]
fn validation_success_speaker_lowercase() -> Result<(), RoundtripError> {
    // Lowercase speakers are valid (lenient validation)
    let input = "*mot:\thello .";

    true_roundtrip_tier_with_validation(input, parse_utterance_direct, |u| {
        validate_utterance_with_participants(u, vec!["mot"])
    })
}

/// Runs validation error e308 speaker too long.
#[test]
fn validation_error_e308_speaker_too_long() -> Result<(), RoundtripError> {
    let input = "*VERYLONGSPEAKER:\thello .";

    let result = true_roundtrip_tier_with_validation(
        input,
        parse_utterance_direct,
        validate_utterance_no_participants,
    );
    match result {
        Err(RoundtripError::Validation { .. }) => Ok(()),
        Err(err) => Err(err),
        Ok(()) => Err(RoundtripError::Validation {
            count: 0,
            messages: Vec::new(),
        }),
    }
}
