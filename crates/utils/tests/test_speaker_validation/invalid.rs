//! Test module for invalid in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{TestError, parse_chat_file_streaming_or_err, parser_suite};
use talkbank_model::{ErrorCode, ErrorCollector};

/// Tests speaker not in participants.
#[test]
fn test_speaker_not_in_participants() -> Result<(), TestError> {
    let invalid_chat = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
*MOT:	hi there .
@End
"#;

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_streaming_or_err(&parser, invalid_chat)?;

        let validation_errors = ErrorCollector::new();
        chat_file.validate(&validation_errors, None);
        let errors = validation_errors.into_vec();

        let e308_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::UndeclaredSpeaker)
            .collect();
        assert_eq!(
            e308_errors.len(),
            1,
            "[{}] Should have one E308 error for MOT",
            parser.name()
        );
    }

    Ok(())
}

/// Tests multiple unknown speakers.
#[test]
fn test_multiple_unknown_speakers() -> Result<(), TestError> {
    let invalid_chat = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
*MOT:	hi .
*FAT:	hello there .
@End
"#;

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_streaming_or_err(&parser, invalid_chat)?;

        let validation_errors = ErrorCollector::new();
        chat_file.validate(&validation_errors, None);
        let errors = validation_errors.into_vec();

        let e308_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::UndeclaredSpeaker)
            .collect();
        assert_eq!(
            e308_errors.len(),
            2,
            "[{}] Should have E308 errors for both MOT and FAT",
            parser.name()
        );
    }

    Ok(())
}
