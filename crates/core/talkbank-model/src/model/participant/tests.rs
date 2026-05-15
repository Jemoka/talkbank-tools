//! Integration-style tests for the unified `Participant` model.
//!
//! These tests ensure participant-level convenience accessors remain coherent
//! across `@Participants`, `@ID`, and optional birth-date metadata sources.

use super::super::{
    ChatDate, IDHeader, ParticipantEntry, ParticipantName, ParticipantRole, SpeakerCode,
};
use super::Participant;

/// Verifies baseline participant assembly from `@Participants` plus `@ID`.
///
/// This case ensures convenience fields are populated consistently when only
/// required participant metadata is available.
#[test]
fn test_participant_new() {
    let entry = ParticipantEntry {
        speaker_code: SpeakerCode::new("CHI"),
        name: Some(ParticipantName::new("Ruth")),
        role: ParticipantRole::new("Target_Child"),
    };

    let id = IDHeader::new("eng", "CHI", "Target_Child");

    let participant = Participant::new(entry, id);

    assert_eq!(participant.code.as_str(), "CHI");
    assert_eq!(participant.name.as_ref().map(|n| n.as_str()), Some("Ruth"));
    assert_eq!(participant.role.as_str(), "Target_Child");
    assert_eq!(participant.birth_date, None);
    assert_eq!(participant.speaker_code(), "CHI");
    assert!(!participant.has_birth_date());
}

/// Adds optional birth-date metadata from `@Birth of`.
#[test]
fn test_participant_with_birth_date() {
    let entry = ParticipantEntry {
        speaker_code: SpeakerCode::new("CHI"),
        name: Some(ParticipantName::new("Ruth")),
        role: ParticipantRole::new("Target_Child"),
    };

    let id = IDHeader::new("eng", "CHI", "Target_Child")
        .with_age("10;03.")
        .with_corpus("chiat");

    let participant = Participant::new(entry, id).with_birth_date(ChatDate::new("28-JUN-2001"));

    assert_eq!(
        participant.birth_date.as_ref().map(|d| d.as_str()),
        Some("28-JUN-2001")
    );
    assert!(participant.has_birth_date());
    assert_eq!(participant.age(), Some("10;03."));
    assert_eq!(participant.corpus(), Some("chiat"));
}

/// Exercises accessor helpers over embedded ID/participant fields.
#[test]
fn test_participant_convenience_methods() {
    let entry = ParticipantEntry {
        speaker_code: SpeakerCode::new("CHI"),
        name: None,
        role: ParticipantRole::new("Target_Child"),
    };

    let id = IDHeader::new("eng", "CHI", "Target_Child")
        .with_age("2;6.0")
        .with_sex(super::super::Sex::Female)
        .with_corpus("bates");

    let participant = Participant::new(entry, id);

    assert_eq!(participant.speaker_code(), "CHI");
    assert!(
        participant
            .languages()
            .0
            .iter()
            .any(|c| c.as_str() == "eng")
    );
    assert_eq!(participant.age(), Some("2;6.0"));
    assert_eq!(participant.sex(), Some(&super::super::Sex::Female));
    assert_eq!(participant.corpus(), Some("bates"));
}
