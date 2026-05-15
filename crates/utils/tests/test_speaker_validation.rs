//! Test module for test speaker validation in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

#[path = "test_speaker_validation/helpers.rs"]
mod helpers;
#[path = "test_speaker_validation/invalid.rs"]
mod invalid;
#[path = "test_utils/mod.rs"]
mod test_utils;
#[path = "test_speaker_validation/valid.rs"]
mod valid;
