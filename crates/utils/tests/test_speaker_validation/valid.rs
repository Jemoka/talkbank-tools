//! Test module for valid in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{TestError, parse_chat_file_streaming_or_err, parser_suite};
use talkbank_model::{ErrorCode, ErrorCollector};

/// Tests speaker validation with participant model.
#[test]
fn test_speaker_validation_with_participant_model() -> Result<(), TestError> {
    let valid_chat = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child, MOT Mother
@ID:	eng|corpus|CHI|||||Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello .
*MOT:	hi there .
@End
"#;

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_streaming_or_err(&parser, valid_chat)?;

        let validation_errors = ErrorCollector::new();
        chat_file.validate(&validation_errors, None);
        let errors = validation_errors.into_vec();

        let e308_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::UndeclaredSpeaker)
            .collect();
        assert_eq!(
            e308_errors.len(),
            0,
            "[{}] Should have no E308 errors for valid speakers",
            parser.name()
        );
    }

    Ok(())
}

/// Tests event speaker zero is allowed.
#[test]
fn test_event_speaker_zero_is_allowed() -> Result<(), TestError> {
    let event_chat = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
*0:	doorbell rings .
@End
"#;

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parse_chat_file_streaming_or_err(&parser, event_chat)?;

        let validation_errors = ErrorCollector::new();
        chat_file.validate(&validation_errors, None);
        let errors = validation_errors.into_vec();

        let e308_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.code == ErrorCode::UndeclaredSpeaker)
            .collect();
        assert_eq!(
            e308_errors.len(),
            0,
            "[{}] Should allow speaker '0' for events",
            parser.name()
        );
    }

    Ok(())
}
