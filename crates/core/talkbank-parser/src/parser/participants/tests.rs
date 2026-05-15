//! Tests for this subsystem.
//!

use super::build_participants;
use talkbank_model::model::{
    ChatDate, Header, IDHeader, ParticipantEntry, ParticipantName, ParticipantRole, SpeakerCode,
};

/// Tests build participants basic.
#[test]
fn test_build_participants_basic() -> Result<(), String> {
    let headers = vec![
        Header::Participants {
            entries: vec![ParticipantEntry {
                speaker_code: SpeakerCode::new("CHI"),
                name: Some(ParticipantName::new("Ruth")),
                role: ParticipantRole::new("Target_Child"),
            }]
            .into(),
        },
        Header::ID(IDHeader::new("eng", "CHI", "Target_Child")),
    ];

    let (participants, errors) = build_participants(&headers);

    assert_eq!(participants.len(), 1);
    assert!(errors.is_empty());

    let chi = participants
        .get("CHI")
        .ok_or_else(|| "CHI participant should exist".to_string())?;
    assert_eq!(chi.code.as_str(), "CHI");
    assert_eq!(chi.name.as_ref().map(|n| n.as_str()), Some("Ruth"));
    assert_eq!(chi.role.as_str(), "Target_Child");
    Ok(())
}

/// Tests build participants with birth.
#[test]
fn test_build_participants_with_birth() -> Result<(), String> {
    let headers = vec![
        Header::Participants {
            entries: vec![ParticipantEntry {
                speaker_code: SpeakerCode::new("CHI"),
                name: Some(ParticipantName::new("Ruth")),
                role: ParticipantRole::new("Target_Child"),
            }]
            .into(),
        },
        Header::ID(IDHeader::new("eng", "CHI", "Target_Child")),
        Header::Birth {
            participant: SpeakerCode::new("CHI"),
            date: ChatDate::new("28-JUN-2001"),
        },
    ];

    let (participants, errors) = build_participants(&headers);

    assert_eq!(participants.len(), 1);
    assert!(errors.is_empty());

    let chi = participants
        .get("CHI")
        .ok_or_else(|| "CHI participant should exist".to_string())?;
    assert_eq!(
        chi.birth_date.as_ref().map(|d| d.as_str()),
        Some("28-JUN-2001")
    );
    Ok(())
}

/// Tests e522 missing id.
#[test]
fn test_e522_missing_id() {
    let headers = vec![
        Header::Participants {
            entries: vec![ParticipantEntry {
                speaker_code: SpeakerCode::new("CHI"),
                name: Some(ParticipantName::new("Ruth")),
                role: ParticipantRole::new("Target_Child"),
            }]
            .into(),
        },
        // Missing @ID header for CHI
    ];

    let (participants, errors) = build_participants(&headers);

    assert_eq!(participants.len(), 0);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.to_string(), "E522");
    assert!(errors[0].message.contains("CHI"));
    assert!(errors[0].message.contains("no matching @ID header"));
}

/// Tests e523 orphan id.
#[test]
fn test_e523_orphan_id() {
    let headers = vec![
        Header::Participants {
            entries: vec![ParticipantEntry {
                speaker_code: SpeakerCode::new("CHI"),
                name: Some(ParticipantName::new("Ruth")),
                role: ParticipantRole::new("Target_Child"),
            }]
            .into(),
        },
        Header::ID(IDHeader::new("eng", "CHI", "Target_Child")),
        Header::ID(IDHeader::new("eng", "MOT", "Mother")), // Orphan - not in @Participants
    ];

    let (participants, errors) = build_participants(&headers);

    assert_eq!(participants.len(), 1);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.to_string(), "E523");
    assert!(errors[0].message.contains("MOT"));
    assert!(errors[0].message.contains("not in @Participants"));
}

/// Tests e524 orphan birth.
#[test]
fn test_e524_orphan_birth() {
    let headers = vec![
        Header::Participants {
            entries: vec![ParticipantEntry {
                speaker_code: SpeakerCode::new("CHI"),
                name: Some(ParticipantName::new("Ruth")),
                role: ParticipantRole::new("Target_Child"),
            }]
            .into(),
        },
        Header::ID(IDHeader::new("eng", "CHI", "Target_Child")),
        Header::Birth {
            participant: SpeakerCode::new("MOT"), // Orphan - MOT not a participant
            date: ChatDate::new("01-JAN-2000"),
        },
    ];

    let (participants, errors) = build_participants(&headers);

    assert_eq!(participants.len(), 1);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.to_string(), "E524");
    assert!(errors[0].message.contains("MOT"));
    assert!(errors[0].message.contains("not a declared participant"));
}

/// Tests multiple participants.
#[test]
fn test_multiple_participants() -> Result<(), String> {
    let headers = vec![
        Header::Participants {
            entries: vec![
                ParticipantEntry {
                    speaker_code: SpeakerCode::new("CHI"),
                    name: Some(ParticipantName::new("Ruth")),
                    role: ParticipantRole::new("Target_Child"),
                },
                ParticipantEntry {
                    speaker_code: SpeakerCode::new("INV"),
                    name: Some(ParticipantName::new("Chiat")),
                    role: ParticipantRole::new("Investigator"),
                },
            ]
            .into(),
        },
        Header::ID(IDHeader::new("eng", "CHI", "Target_Child").with_age("10;03.")),
        Header::ID(IDHeader::new("eng", "INV", "Investigator")),
        Header::Birth {
            participant: SpeakerCode::new("CHI"),
            date: ChatDate::new("28-JUN-2001"),
        },
    ];

    let (participants, errors) = build_participants(&headers);

    assert_eq!(participants.len(), 2);
    assert!(errors.is_empty());

    let chi = participants
        .get("CHI")
        .ok_or_else(|| "CHI participant should exist".to_string())?;
    assert_eq!(chi.code.as_str(), "CHI");
    assert_eq!(
        chi.birth_date.as_ref().map(|d| d.as_str()),
        Some("28-JUN-2001")
    );

    let inv = participants
        .get("INV")
        .ok_or_else(|| "INV participant should exist".to_string())?;
    assert_eq!(inv.code.as_str(), "INV");
    assert_eq!(inv.birth_date, None);
    Ok(())
}
