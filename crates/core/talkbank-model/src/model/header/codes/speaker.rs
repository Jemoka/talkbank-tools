//! `SpeakerCode` model and validation helpers.
//!
//! Speaker identifiers are interned (`Arc<str>`) for reuse across large corpora.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speaker_Codes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Line>

use crate::validation::{Validate, ValidationContext};
use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use talkbank_derive::{SemanticEq, SpanShift};

/// Interned speaker identifier used by main-tier prefixes and participant headers.
///
/// Speaker codes identify participants in:
/// - main tiers (`*CHI:`)
/// - participant-related headers (`@Participants`, `@ID`, `@Birth of ...`)
///
/// ## Memory Optimization
///
/// This type uses `Arc<str>` with interning for memory efficiency:
/// - All codes are interned through a global interner
/// - Standard codes (CHI, MOT, FAT, etc.) are pre-populated on first use
/// - Cloning is O(1) (atomic reference count increment)
/// - Multiple occurrences of the same code share a single Arc
///
/// This reduces memory usage by 10-50MB for large corpora.
///
/// # Common Usage
///
/// **In main tiers:**
/// - `*CHI:` - Utterance by child participant
/// - `*MOT:` - Utterance by mother
/// - `*FAT:` - Utterance by father
///
/// **In headers:**
/// - `@Participants:` - Lists all speaker codes with names and roles
/// - `@ID:` - Detailed participant information
/// - `@Birth of SPK:` - Participant-specific metadata
/// - `@Language of SPK:` - Participant language
///
/// # CHAT Format Examples
///
/// ```text
/// @Participants: CHI Target_Child, MOT Mother, FAT Father, INV Investigator
/// @ID: eng|Corpus|CHI|2;06.15|male|||Target_Child|||
/// @Birth of CHI: 15-JAN-2015
/// *CHI: I want cookie.
/// *MOT: here you go.
/// *FAT: what do you say?
/// *CHI: thank you!
/// ```
///
/// # Common Codes
///
/// **Family members:**
/// - `CHI` - Target child (primary participant)
/// - `MOT` - Mother
/// - `FAT` - Father
/// - `BRO` - Brother
/// - `SIS` - Sister
/// - `GRA` - Grandmother
/// - `GRF` - Grandfather
///
/// **Research roles:**
/// - `INV` - Investigator/Researcher
/// - `EXP` - Experimenter
/// - `OBS` - Observer
/// - `CAM` - Cameraman
///
/// **Other:**
/// - `TEA` - Teacher
/// - `DOC` - Doctor
/// - `NUR` - Nurse
/// - `UNK` - Unknown speaker
/// - `ENV` - Environment (background noise, etc.)
///
/// # Format Rules (validator)
///
/// - max length: 7 characters
/// - allowed chars: `A-Z`, `0-9`, `_`, `-`, `'`
///
/// # References
///
/// - [CHAT Manual: Speaker Codes](https://talkbank.org/0info/manuals/CHAT.html#Speaker_Codes)
/// - [Main Tier Format](https://talkbank.org/0info/manuals/CHAT.html#Main_Line)
/// - [Participants Header](https://talkbank.org/0info/manuals/CHAT.html#Participants_Header)
#[derive(
    Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift,
)]
#[serde(transparent)]
pub struct SpeakerCode(pub Arc<str>);

impl SpeakerCode {
    /// Construct and intern a speaker identifier.
    ///
    /// Interning keeps repeated speaker codes pointer-shared across the model,
    /// which materially reduces memory footprint for large corpora.
    pub fn new(value: impl AsRef<str>) -> Self {
        let s = value.as_ref();
        Self(crate::model::speaker_interner().intern(s))
    }

    /// Borrow as `&str`.
    ///
    /// This is the preferred accessor for validation and formatting code that
    /// should not depend on the internal `Arc<str>` representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl crate::model::WriteChat for SpeakerCode {
    /// Writes the raw speaker identifier exactly as stored.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(&self.0)
    }
}

impl std::fmt::Display for SpeakerCode {
    /// Displays the interned speaker identifier text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::ops::Deref for SpeakerCode {
    type Target = str;

    /// Exposes this code as `&str` for generic string APIs.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for SpeakerCode {
    /// Borrows this code as `&str`.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for SpeakerCode {
    /// Interns an owned identifier as a `SpeakerCode`.
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SpeakerCode {
    /// Interns a borrowed identifier as a `SpeakerCode`.
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl std::borrow::Borrow<str> for SpeakerCode {
    /// Supports hashmap/set lookups keyed by `str`.
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

/// Maximum allowed length for speaker IDs (`ChatFileUtils.MAX_WHO` compatibility).
///
/// Keeping this limit aligned with legacy tooling avoids subtle cross-tool
/// validation differences on shared corpora.
const MAX_SPEAKER_ID_LENGTH: usize = 7;

impl Validate for SpeakerCode {
    /// Enforces CHAT speaker-id constraints used by parser/validator compatibility checks.
    fn validate(&self, _context: &ValidationContext, errors: &impl crate::ErrorSink) {
        // E308: Check speaker ID length
        if self.0.len() > MAX_SPEAKER_ID_LENGTH {
            errors.report(
                ParseError::new(
                    ErrorCode::UndeclaredSpeaker,
                    Severity::Error,
                    SourceLocation::at_offset(0),
                    ErrorContext::new(self.as_str(), 0..self.0.len(), "speaker_code"),
                    format!(
                        "Speaker ID '{}' exceeds maximum length of {} characters (has {})",
                        self.0,
                        MAX_SPEAKER_ID_LENGTH,
                        self.0.len()
                    ),
                )
                .with_suggestion(
                    "Speaker IDs should be 7 characters or less (e.g., CHI, MOT, FAT)",
                ),
            );
        }

        // E302: Check for invalid characters in speaker ID
        // Valid characters: uppercase letters (A-Z), digits (0-9), underscore (_), hyphen (-), apostrophe (')
        // Note: Some legacy CHAT files use hyphens and apostrophes in speaker IDs (e.g., F_A'-T)
        if let Some(invalid_char) = self.0.chars().find(|c| {
            !c.is_ascii_uppercase() && !c.is_ascii_digit() && *c != '_' && *c != '-' && *c != '\''
        }) {
            errors.report(
                ParseError::new(
                    ErrorCode::MissingNode,
                    Severity::Error,
                    SourceLocation::at_offset(0),
                    ErrorContext::new(self.as_str(), 0..self.0.len(), "speaker_code"),
                    format!(
                        "Speaker ID '{}' contains invalid character '{}'",
                        self.0, invalid_char
                    ),
                )
                .with_suggestion(
                    "Speaker IDs should use uppercase letters (A-Z), digits (0-9), underscores (_), hyphens (-), or apostrophes (')",
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Standard speaker codes are interned to shared allocations.
    ///
    /// This verifies pre-seeded interner behavior for the most frequent labels.
    #[test]
    fn test_standard_codes_are_interned() {
        let chi1 = SpeakerCode::new("CHI");
        let chi2 = SpeakerCode::new("CHI");

        // Same Arc (pointer equality)
        assert!(Arc::ptr_eq(&chi1.0, &chi2.0));
        assert_eq!(chi1.as_str(), "CHI");
    }

    /// Unknown speaker codes are also interned after first insertion.
    ///
    /// Runtime-added values should still preserve pointer-sharing semantics.
    #[test]
    fn test_custom_codes_are_interned() {
        let custom1 = SpeakerCode::new("XYZ");
        let custom2 = SpeakerCode::new("XYZ");

        // Same Arc (runtime interned)
        assert!(Arc::ptr_eq(&custom1.0, &custom2.0));
        assert_eq!(custom1.as_str(), "XYZ");
    }

    /// Different speaker codes must remain distinct interned values.
    ///
    /// This avoids accidental aliasing between unrelated participants.
    #[test]
    fn test_different_codes() {
        let chi = SpeakerCode::new("CHI");
        let mot = SpeakerCode::new("MOT");

        // Different Arcs
        assert!(!Arc::ptr_eq(&chi.0, &mot.0));
        assert_ne!(chi, mot);
    }

    /// `From<String>` delegates to the same interning path as `new`.
    ///
    /// Owned-string callers should observe identical behavior to borrowed input.
    #[test]
    fn test_from_string() {
        let code = SpeakerCode::from("CHI".to_string());
        assert_eq!(code.as_str(), "CHI");
    }

    /// `From<&str>` constructs interned speaker codes from literals.
    ///
    /// This keeps tests and hand-built fixtures concise and consistent.
    #[test]
    fn test_from_str() {
        let code = SpeakerCode::from("MOT");
        assert_eq!(code.as_str(), "MOT");
    }
}
