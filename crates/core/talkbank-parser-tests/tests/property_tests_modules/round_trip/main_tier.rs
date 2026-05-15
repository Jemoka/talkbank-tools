//! Test module for main tier in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use rstest::rstest;

use super::helpers::parser;
use talkbank_parser_tests::test_error::TestError;

/// Verifies representative main tiers round-trip through parser and serializer.
#[rstest]
#[case::simple("*CHI:\thello .")]
#[case::question("*MOT:\twhat ?")]
#[case::with_pause("*CHI:\tum (.) yes .")]
#[case::multiple_words("*CHI:\thello world .")]
#[case::with_filler("*MOT:\tyeah &-um ok .")]
#[case::with_media_bullet("*CHI:\thello . \u{0015}1000_2000\u{0015}")]
// skip bullet removed from the grammar (2026-03-31)
fn main_tier_round_trip(#[case] input: &str) -> Result<(), TestError> {
    let ts = parser()?;
    let tier = ts.parse_main_tier(input)?;
    let output = tier.to_chat();
    if output != input {
        return Err(TestError::Failure(format!(
            "Round-trip failed:
  input:  '{}'
  output: '{}'",
            input, output
        )));
    }
    Ok(())
}
