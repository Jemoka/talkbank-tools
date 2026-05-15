//! Test module for typed in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::Header;

use super::helpers::{TestError, parser_suite};

/// Tests parse languages header.
#[test]
fn test_parse_languages_header() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let header = parser.parse_header("@Languages:\teng")?;
        assert_eq!(
            header.name(),
            "Languages",
            "[{}] expected Languages",
            parser.name()
        );
        match header {
            Header::Languages { codes } => {
                assert_eq!(codes.len(), 1, "[{}] expected 1 code", parser.name());
                assert_eq!(
                    codes[0].as_str(),
                    "eng",
                    "[{}] expected 'eng'",
                    parser.name()
                );
            }
            _ => {
                return Err(TestError::UnexpectedHeader {
                    parser: parser.name(),
                    expected: "Languages",
                });
            }
        }
    }

    Ok(())
}

/// Tests parse participants header.
#[test]
fn test_parse_participants_header() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let header =
            parser.parse_header("@Participants:\tFAT Father, CHI Target_Child, MOT Mother")?;
        assert_eq!(
            header.name(),
            "Participants",
            "[{}] expected Participants",
            parser.name()
        );
        match header {
            Header::Participants { entries } => {
                assert_eq!(entries.len(), 3, "[{}] expected 3 entries", parser.name());
                assert_eq!(
                    entries[0].speaker_code.as_str(),
                    "FAT",
                    "[{}]",
                    parser.name()
                );
                assert_eq!(entries[0].role.as_str(), "Father", "[{}]", parser.name());
                assert_eq!(
                    entries[1].speaker_code.as_str(),
                    "CHI",
                    "[{}]",
                    parser.name()
                );
                assert_eq!(
                    entries[1].role.as_str(),
                    "Target_Child",
                    "[{}]",
                    parser.name()
                );
                assert_eq!(
                    entries[2].speaker_code.as_str(),
                    "MOT",
                    "[{}]",
                    parser.name()
                );
                assert_eq!(entries[2].role.as_str(), "Mother", "[{}]", parser.name());
            }
            _ => {
                return Err(TestError::UnexpectedHeader {
                    parser: parser.name(),
                    expected: "Participants",
                });
            }
        }
    }

    Ok(())
}

/// Tests parse id header.
#[test]
fn test_parse_id_header() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let header =
            parser.parse_header("@ID:\teng|MacWhinney|CHI|1;04.11|male|TD||Target_Child|||")?;
        assert_eq!(header.name(), "ID", "[{}] expected ID", parser.name());
        match header {
            Header::ID(id) => {
                assert!(
                    id.language.0.iter().any(|c| c.as_str() == "eng"),
                    "[{}]",
                    parser.name()
                );
                assert_eq!(
                    id.corpus.as_deref(),
                    Some("MacWhinney"),
                    "[{}]",
                    parser.name()
                );
                assert_eq!(id.speaker.as_str(), "CHI", "[{}]", parser.name());
                assert_eq!(
                    id.age.as_ref().map(|a| a.as_str()),
                    Some("1;04.11"),
                    "[{}]",
                    parser.name()
                );
                assert_eq!(
                    id.sex,
                    Some(talkbank_model::model::Sex::Male),
                    "[{}]",
                    parser.name()
                );
                assert_eq!(id.group.as_deref(), Some("TD"), "[{}]", parser.name());
                assert!(id.ses.is_none(), "[{}]", parser.name());
                assert_eq!(id.role.as_str(), "Target_Child", "[{}]", parser.name());
                assert!(id.education.is_none(), "[{}]", parser.name());
                assert!(id.custom_field.is_none(), "[{}]", parser.name());
            }
            _ => {
                return Err(TestError::UnexpectedHeader {
                    parser: parser.name(),
                    expected: "ID",
                });
            }
        }
    }

    Ok(())
}

/// Tests parse media header.
#[test]
fn test_parse_media_header() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let header = parser.parse_header("@Media:\t010411a, audio")?;
        assert_eq!(header.name(), "Media", "[{}] expected Media", parser.name());
        match header {
            Header::Media(media) => {
                assert_eq!(media.filename.as_str(), "010411a", "[{}]", parser.name());
                assert_eq!(
                    media.media_type,
                    talkbank_model::model::MediaType::Audio,
                    "[{}]",
                    parser.name()
                );
            }
            _ => {
                return Err(TestError::UnexpectedHeader {
                    parser: parser.name(),
                    expected: "Media",
                });
            }
        }
    }

    Ok(())
}

/// Tests parse date header.
#[test]
fn test_parse_date_header() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let header = parser.parse_header("@Date:\t06-MAY-1979")?;
        assert_eq!(header.name(), "Date", "[{}] expected Date", parser.name());
        match header {
            Header::Date { date } => {
                assert_eq!(date.as_str(), "06-MAY-1979", "[{}]", parser.name());
            }
            _ => {
                return Err(TestError::UnexpectedHeader {
                    parser: parser.name(),
                    expected: "Date",
                });
            }
        }
    }

    Ok(())
}

/// Tests parse situation header.
#[test]
fn test_parse_situation_header() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let header = parser.parse_header("@Situation:\tRoss giggling and laughing")?;
        assert_eq!(
            header.name(),
            "Situation",
            "[{}] expected Situation",
            parser.name()
        );
        match header {
            Header::Situation { text } => {
                assert_eq!(
                    text.as_str(),
                    "Ross giggling and laughing",
                    "[{}]",
                    parser.name()
                );
            }
            _ => {
                return Err(TestError::UnexpectedHeader {
                    parser: parser.name(),
                    expected: "Situation",
                });
            }
        }
    }

    Ok(())
}

/// Tests parse pid header.
#[test]
fn test_parse_pid_header() -> Result<(), TestError> {
    for parser in parser_suite()? {
        let header = parser.parse_header("@PID:\t11312/c-00016447-1")?;
        assert_eq!(header.name(), "PID", "[{}] expected PID", parser.name());
        match header {
            Header::Pid { pid } => {
                assert_eq!(pid.as_str(), "11312/c-00016447-1", "[{}]", parser.name());
            }
            _ => {
                return Err(TestError::UnexpectedHeader {
                    parser: parser.name(),
                    expected: "PID",
                });
            }
        }
    }

    Ok(())
}
