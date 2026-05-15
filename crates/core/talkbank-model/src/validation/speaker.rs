//! Speaker-ID validation helpers.
//!
//! This module provides lightweight character-level checks shared by parser and
//! validation flows that need to reject obviously malformed speaker IDs early.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Speaker_ID>

/// Legacy maximum speaker-ID length from Java `ChatFileUtils.MAX_WHO`.
#[allow(dead_code)]
const MAX_SPEAKER_ID_LENGTH: usize = 7;

/// Returns the first invalid speaker-ID character, if any.
///
/// Invalid characters:
/// - Colon (:) - reserved as speaker ID delimiter
/// - Whitespace (space, tab, newline, etc.)
///
/// All other characters are accepted: lowercase, uppercase, digits, punctuation, Unicode, etc.
/// This lenient approach supports international corpora and various naming conventions.
/// Returning the first offending character allows callers to produce targeted
/// diagnostics without re-scanning the whole identifier.
pub(crate) fn has_invalid_speaker_chars(speaker: &str) -> Option<char> {
    speaker.chars().find(|c| *c == ':' || c.is_whitespace())
}
