//! Integration tests for participant model
//!
//! Tests that participants are correctly built from @Participants + @ID + @Birth headers.

#[path = "participant_integration/basic.rs"]
mod basic;
#[path = "participant_integration/fixtures.rs"]
mod fixtures;
#[path = "participant_integration/helpers.rs"]
mod helpers;
#[path = "participant_integration/real_file.rs"]
mod real_file;
#[path = "test_utils/mod.rs"]
mod test_utils;
