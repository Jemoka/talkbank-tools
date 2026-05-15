//! Test module for main tier in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::ErrorCode;
use talkbank_parser::{ParserInitError, TreeSitterParser};

use super::helpers::{get_error_codes, has_error, validate_chat_file};

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Failed to create TreeSitterParser: {source}")]
    TreeSitterInit { source: ParserInitError },
    #[error("Parse failed")]
    ParseErrors { errors: talkbank_model::ParseErrors },
}

// E3xx: Main Tier Mutation Tests

/// Tests e304 missing terminator.
#[test]
fn test_e304_missing_terminator() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    let chat_file = parser
        .parse_chat_file(input)
        .map_err(|errors| TestError::ParseErrors { errors })?;
    let errors = validate_chat_file(&chat_file);

    assert!(
        has_error(&errors, ErrorCode::MissingSpeaker)
            || has_error(&errors, ErrorCode::MissingMainTier),
        "Expected E304/E301 (missing terminator), got: {:?}",
        get_error_codes(&errors)
    );

    Ok(())
}

/// Tests e305 empty utterance.
#[test]
fn test_e305_empty_utterance() -> Result<(), TestError> {
    let input = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\t.\n@End\n";

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    let chat_file = parser
        .parse_chat_file(input)
        .map_err(|errors| TestError::ParseErrors { errors })?;
    let errors = validate_chat_file(&chat_file);

    assert!(
        has_error(&errors, ErrorCode::MissingTerminator)
            || has_error(&errors, ErrorCode::EmptyUtterance)
            || has_error(&errors, ErrorCode::SyllablePauseNotBetweenSpokenMaterial)
            || has_error(&errors, ErrorCode::EmptySpokenContent),
        "Expected E305/E306/E252/E209 (empty utterance), got: {:?}",
        get_error_codes(&errors)
    );

    Ok(())
}
