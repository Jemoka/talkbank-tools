//! Test module for happy in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{TestError, parser_suite};
use talkbank_model::{ErrorCode, ErrorCollector};

/// Tests happy path complete valid file.
#[test]
fn test_happy_path_complete_valid_file() -> Result<(), TestError> {
    let valid_chat = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Ruth Target_Child, MOT Mother, FAT Father
@ID:	eng|corpus|CHI|2;06.00||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@ID:	eng|corpus|FAT|||||Father|||
@Date:	15-JAN-2020
@Comment:	This is a complete valid file
*CHI:	hello .
*MOT:	hi there .
*FAT:	good morning .
@End
"#;

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parser.parse_chat_file_result(valid_chat)?;

        let errors = ErrorCollector::new();
        chat_file.validate(&errors, None);
        let error_vec = errors.into_vec();

        assert_eq!(
            error_vec.len(),
            0,
            "[{}] Valid file should have no validation errors",
            parser.name()
        );
    }

    Ok(())
}

/// Tests happy path event speaker zero.
#[test]
fn test_happy_path_event_speaker_zero() -> Result<(), TestError> {
    let valid_chat = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
*0:	doorbell rings .
*CHI:	someone's here .
*0:	dog barks .
@End
"#;

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parser.parse_chat_file_result(valid_chat)?;

        let errors = ErrorCollector::new();
        chat_file.validate(&errors, None);
        let error_vec = errors.into_vec();

        let speaker_errors: Vec<_> = error_vec
            .iter()
            .filter(|e| e.code == ErrorCode::UndeclaredSpeaker)
            .collect();
        assert_eq!(
            speaker_errors.len(),
            0,
            "[{}] Event speaker '0' should be valid",
            parser.name()
        );
    }

    Ok(())
}

/// Tests happy path minimal with mor.
#[test]
fn test_happy_path_minimal_with_mor() -> Result<(), TestError> {
    let valid_chat = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%mor:	intj|hello noun|world .
@End
"#;

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parser.parse_chat_file_result(valid_chat)?;

        let errors = ErrorCollector::new();
        chat_file.validate(&errors, None);
        let error_vec = errors.into_vec();

        assert_eq!(
            error_vec.len(),
            0,
            "[{}] Valid file with %mor should have no errors",
            parser.name()
        );
    }

    Ok(())
}
