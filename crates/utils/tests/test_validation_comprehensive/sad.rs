//! Test module for sad in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::helpers::{TestError, parser_suite};
use talkbank_model::{ErrorCode, ErrorCollector};

/// Tests sad path mixed valid invalid.
#[test]
fn test_sad_path_mixed_valid_invalid() -> Result<(), TestError> {
    let mixed_chat = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	this is valid .
*UNKNOWN:	this is invalid .
*CHI:	this is also valid .
*CHI:	missing terminator
@End
"#;

    // Test BOTH parsers
    for parser in parser_suite()? {
        let chat_file = parser.parse_chat_file_result(mixed_chat)?;

        let errors = ErrorCollector::new();
        chat_file.validate(&errors, None);
        let error_vec = errors.into_vec();

        let e308_errors: Vec<_> = error_vec
            .iter()
            .filter(|e| e.code == ErrorCode::UndeclaredSpeaker)
            .collect();
        assert!(
            !e308_errors.is_empty(),
            "[{}] Should have E308 for UNKNOWN speaker",
            parser.name()
        );

        let e304_errors: Vec<_> = error_vec
            .iter()
            .filter(|e| e.code == ErrorCode::MissingSpeaker)
            .collect();
        assert!(
            !e304_errors.is_empty(),
            "[{}] Should have E304 for missing terminator",
            parser.name()
        );
    }

    Ok(())
}
