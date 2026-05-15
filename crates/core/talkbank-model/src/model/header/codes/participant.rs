//! Participant-related code types (`@Participants` records).
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speaker_ID>

use crate::{interned_newtype, string_newtype};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

interned_newtype! {
    /// Participant role in conversation
    ///
    /// Roles describe the participant's function in the conversation.
    /// Common examples: "Target_Child", "Mother", "Father", "Investigator"
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
    pub struct ParticipantRole,
    interner: crate::model::participant_interner()
}

string_newtype!(
    /// Display name recorded in `@Participants`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
    pub struct ParticipantName;
);

/// Participant entry in @Participants header
///
/// Format: `SPEAKER_CODE [Optional Name] Role`
/// Example: `CHI Alex Target_Child` or `MOT Mother`
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct ParticipantEntry {
    /// Three-letter speaker code (e.g., "CHI", "MOT", "FAT")
    pub speaker_code: super::SpeakerCode,

    /// Optional participant name (e.g., "Alex", "Mary")
    pub name: Option<ParticipantName>,

    /// Participant role (e.g., "Target_Child", "Mother", "Father")
    pub role: ParticipantRole,
}

impl crate::validation::Validate for ParticipantEntry {
    /// Validates speaker-code formatting and role membership against CHAT role vocabulary.
    fn validate(
        &self,
        _context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        if self.speaker_code.as_str().is_empty() {
            errors.report(crate::ParseError::new(
                crate::ErrorCode::EmptyParticipantCode,
                crate::Severity::Error,
                crate::SourceLocation::at_offset(0),
                crate::ErrorContext::new(self.speaker_code.as_str(), 0..0, "speaker_code"),
                "Participant speaker code cannot be empty",
            ));
        }

        crate::validation::header::check_speaker_id(
            self.speaker_code.as_str(),
            "speaker_code",
            crate::Span::DUMMY,
            errors,
        );

        if self.role.as_str().is_empty() {
            errors.report(crate::ParseError::new(
                crate::ErrorCode::EmptyParticipantRole,
                crate::Severity::Error,
                crate::SourceLocation::at_offset(0),
                crate::ErrorContext::new(self.role.as_str(), 0..0, "role"),
                "Participant role cannot be empty",
            ));
        } else if !crate::validation::header::participant::is_allowed_participant_role(
            self.role.as_str(),
        ) {
            let suggested =
                crate::validation::header::participant::suggest_similar_role(self.role.as_str());
            errors.report(
                crate::ParseError::new(
                    crate::ErrorCode::InvalidParticipantRole,
                    crate::Severity::Error,
                    crate::SourceLocation::at_offset(0),
                    crate::ErrorContext::new(self.role.as_str(), 0..0, "role"),
                    format!("Invalid participant role: '{}'", self.role.as_str()),
                )
                .with_suggestion(format!("Use a valid CHAT role such as: {}", suggested)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Repeated participant-role labels reuse the same interned allocation.
    ///
    /// This test guards the memory-sharing contract for frequent role strings.
    #[test]
    fn test_participant_role_interning() {
        let mother1 = ParticipantRole::new("Mother");
        let mother2 = ParticipantRole::new("Mother");

        // Same Arc (pointer equality) - strings should be interned
        assert!(Arc::ptr_eq(&mother1.0, &mother2.0));
        assert_eq!(mother1.as_str(), "Mother");
        assert_eq!(mother2.as_str(), "Mother");
    }

    /// Distinct participant roles must not alias to the same interned pointer.
    ///
    /// Avoiding aliasing keeps equality and diagnostics semantically correct.
    #[test]
    fn test_participant_role_different_values() {
        let mother = ParticipantRole::new("Mother");
        let father = ParticipantRole::new("Father");

        // Different values - different Arcs
        assert!(!Arc::ptr_eq(&mother.0, &father.0));
        assert_eq!(mother.as_str(), "Mother");
        assert_eq!(father.as_str(), "Father");
    }

    /// Display output preserves the exact role label text.
    ///
    /// This ensures header serialization remains lossless for participant roles.
    #[test]
    fn test_participant_role_display() {
        let role = ParticipantRole::new("Target_Child");
        assert_eq!(role.to_string(), "Target_Child");
        assert_eq!(format!("{}", role), "Target_Child");
    }
}
