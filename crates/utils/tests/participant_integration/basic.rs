//! Test module for basic in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::fixtures::{BASIC_PARTICIPANTS, MIXED_BIRTH_DATES, NO_BIRTH_DATE};
use super::helpers::{TestError, parser_suite};
use talkbank_model::ChatFile;
use talkbank_model::ErrorCollector;

/// Parses chat file or err.
fn parse_chat_file_or_err(
    parser: &super::helpers::ParserImpl,
    input: &str,
) -> Result<ChatFile, TestError> {
    let errors = ErrorCollector::new();
    let chat_file = parser
        .parse_chat_file_streaming(input, &errors)
        .ok_or_else(|| TestError::ParseErrors {
            parser: parser.name(),
            errors: talkbank_model::ParseErrors::new(),
        })?;

    let error_vec = errors.into_vec();
    if error_vec.is_empty() {
        Ok(chat_file)
    } else {
        Err(TestError::ParseErrors {
            parser: parser.name(),
            errors: talkbank_model::ParseErrors { errors: error_vec },
        })
    }
}

/// Tests parse file builds participants.
#[test]
fn test_parse_file_builds_participants() -> Result<(), TestError> {
    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_or_err(&parser, BASIC_PARTICIPANTS)?;

        assert_eq!(
            chat_file.participant_count(),
            2,
            "[{}] Should have 2 participants",
            parser.name()
        );

        let chi = chat_file
            .get_participant("CHI")
            .ok_or(TestError::MissingParticipant {
                parser: parser.name(),
                participant: "CHI",
            })?;
        assert_eq!(chi.code.as_str(), "CHI");
        assert_eq!(chi.name.as_deref(), Some("Ruth"));
        assert_eq!(chi.role.as_str(), "Target_Child");
        assert_eq!(chi.age(), Some("10;03."));
        assert_eq!(
            chi.birth_date.as_ref().map(|d| d.as_str()),
            Some("28-JUN-2001")
        );
        assert!(chi.languages().0.iter().any(|c| c.as_str() == "eng"));
        assert_eq!(chi.corpus(), Some("chiat"));

        let inv = chat_file
            .get_participant("INV")
            .ok_or(TestError::MissingParticipant {
                parser: parser.name(),
                participant: "INV",
            })?;
        assert_eq!(inv.code.as_str(), "INV");
        assert_eq!(inv.name.as_deref(), Some("Chiat"));
        assert_eq!(inv.role.as_str(), "Investigator");
        assert_eq!(inv.birth_date, None);
        assert!(inv.languages().0.iter().any(|c| c.as_str() == "eng"));
    }

    Ok(())
}

/// Tests participant without birth date.
#[test]
fn test_participant_without_birth_date() -> Result<(), TestError> {
    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_or_err(&parser, NO_BIRTH_DATE)?;

        assert_eq!(
            chat_file.participant_count(),
            1,
            "[{}] participant count",
            parser.name()
        );

        let mot = chat_file
            .get_participant("MOT")
            .ok_or(TestError::MissingParticipant {
                parser: parser.name(),
                participant: "MOT",
            })?;
        assert_eq!(mot.code.as_str(), "MOT");
        assert_eq!(mot.birth_date, None);
        assert!(!mot.has_birth_date());
    }

    Ok(())
}

/// Tests multiple participants with mixed birth dates.
#[test]
fn test_multiple_participants_with_mixed_birth_dates() -> Result<(), TestError> {
    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_or_err(&parser, MIXED_BIRTH_DATES)?;

        assert_eq!(
            chat_file.participant_count(),
            3,
            "[{}] participant count",
            parser.name()
        );

        let chi = chat_file
            .get_participant("CHI")
            .ok_or(TestError::MissingParticipant {
                parser: parser.name(),
                participant: "CHI",
            })?;
        assert_eq!(
            chi.birth_date.as_ref().map(|d| d.as_str()),
            Some("15-MAR-2020")
        );

        let mot = chat_file
            .get_participant("MOT")
            .ok_or(TestError::MissingParticipant {
                parser: parser.name(),
                participant: "MOT",
            })?;
        assert_eq!(mot.birth_date, None);

        let fat = chat_file
            .get_participant("FAT")
            .ok_or(TestError::MissingParticipant {
                parser: parser.name(),
                participant: "FAT",
            })?;
        assert_eq!(
            fat.birth_date.as_ref().map(|d| d.as_str()),
            Some("10-JUN-1985")
        );
    }

    Ok(())
}
