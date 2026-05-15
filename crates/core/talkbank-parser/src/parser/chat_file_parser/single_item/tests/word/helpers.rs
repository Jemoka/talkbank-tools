//! Test module for helpers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::super::with_snapshot_settings;

pub type WordParseResult = crate::error::ParseResult<crate::model::Word>;

/// Record one parse result snapshot using the shared insta settings.
pub fn snapshot(name: &str, result: &WordParseResult) {
    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!(name, result);
    });
}

// Re-export parse_word for use in other test modules
pub use super::super::parse_word;
