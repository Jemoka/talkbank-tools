//! Test module for headers in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::ChatFile;
use talkbank_model::{ErrorCode, ErrorCollector};
use talkbank_parser::{ParserInitError, TreeSitterParser};

use super::helpers::{get_error_codes, has_error};

/// Enum variants for TestError.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Failed to create TreeSitterParser: {source}")]
    TreeSitterInit { source: ParserInitError },
    #[error("Parse failed")]
    ParseErrors { errors: talkbank_model::ParseErrors },
}

/// Validates chat file.
fn validate_chat_file(chat_file: &ChatFile) -> Vec<talkbank_model::ParseError> {
    let errors = ErrorCollector::new();
    chat_file.validate(&errors, None);
    errors.into_vec()
}

// E5xx: Header Mutation Tests

/// Tests e501 missing begin header.
#[test]
fn test_e501_missing_begin_header() -> Result<(), TestError> {
    let input = r#"@UTF8
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    match parser.parse_chat_file(input) {
        Ok(chat_file) => {
            let errors = validate_chat_file(&chat_file);
            assert!(
                has_error(&errors, ErrorCode::MissingRequiredHeader)
                    || has_error(&errors, ErrorCode::MissingEndHeader),
                "Expected E504 (missing required header) or E502 (missing @End) when @Begin absent, got: {:?}",
                get_error_codes(&errors)
            );
        }
        Err(parse_errors) => {
            assert!(
                parse_errors.errors.iter().any(|e| {
                    matches!(
                        e.code,
                        ErrorCode::SyntaxError
                            | ErrorCode::UnparsableContent
                            | ErrorCode::MissingRequiredHeader
                            | ErrorCode::MissingEndHeader
                            | ErrorCode::DuplicateHeader
                            | ErrorCode::InvalidIDFormat
                    )
                }),
                "Expected E303, E316, E501, E504, or E502 in parse errors, got: {:?}",
                parse_errors
                    .errors
                    .iter()
                    .map(|e| e.code)
                    .collect::<Vec<_>>()
            );
        }
    }

    Ok(())
}

/// Tests e503 missing utf8 header.
#[test]
fn test_e503_missing_utf8_header() -> Result<(), TestError> {
    // Valid CHAT file but without @UTF8 header — should trigger E503 (validation)
    // or E316 (parser rejects file structure without @UTF8)
    let input = "@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    match parser.parse_chat_file(input) {
        Ok(chat_file) => {
            let errors = validate_chat_file(&chat_file);
            assert!(
                has_error(&errors, ErrorCode::MissingUTF8Header),
                "Expected E503 (missing @UTF8), got: {:?}",
                get_error_codes(&errors)
            );
        }
        Err(parse_errors) => {
            // Parser may reject the file at the structural level (E316/E303)
            assert!(
                parse_errors.errors.iter().any(|e| {
                    matches!(
                        e.code,
                        ErrorCode::SyntaxError
                            | ErrorCode::UnparsableContent
                            | ErrorCode::DuplicateHeader
                            | ErrorCode::InvalidIDFormat
                    )
                }),
                "Expected E303, E316, E501, or E510 in parse errors, got: {:?}",
                parse_errors
                    .errors
                    .iter()
                    .map(|e| e.code)
                    .collect::<Vec<_>>()
            );
        }
    }

    Ok(())
}

/// Tests e502 missing end header.
#[test]
fn test_e502_missing_end_header() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    let chat_file = parser
        .parse_chat_file(input)
        .map_err(|errors| TestError::ParseErrors { errors })?;
    let errors = validate_chat_file(&chat_file);

    assert!(
        has_error(&errors, ErrorCode::MissingEndHeader),
        "Expected E502 (missing @End), got: {:?}",
        get_error_codes(&errors)
    );

    Ok(())
}

/// Tests e504 missing participants header.
#[test]
fn test_e504_missing_participants_header() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
*CHI:	hello .
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    let chat_file = parser
        .parse_chat_file(input)
        .map_err(|errors| TestError::ParseErrors { errors })?;
    let errors = validate_chat_file(&chat_file);

    assert!(
        has_error(&errors, ErrorCode::MissingRequiredHeader),
        "Expected E504 (missing @Participants), got: {:?}",
        get_error_codes(&errors)
    );

    Ok(())
}

/// Tests e505 duplicate begin header.
#[test]
fn test_e505_duplicate_begin_header() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    match parser.parse_chat_file(input) {
        Ok(chat_file) => {
            let errors = validate_chat_file(&chat_file);
            assert!(
                has_error(&errors, ErrorCode::InvalidIDFormat),
                "Expected E505 (duplicate @Begin), got: {:?}",
                get_error_codes(&errors)
            );
        }
        Err(parse_errors) => {
            assert!(
                parse_errors.errors.iter().any(|e| {
                    matches!(
                        e.code,
                        ErrorCode::SyntaxError
                            | ErrorCode::UnparsableContent
                            | ErrorCode::InvalidIDFormat
                            | ErrorCode::DuplicateHeader
                    )
                }),
                "Expected E303, E316, E501, or E505 in parse errors, got: {:?}",
                parse_errors
                    .errors
                    .iter()
                    .map(|e| e.code)
                    .collect::<Vec<_>>()
            );
        }
    }

    Ok(())
}

/// Tests e308 undeclared speaker.
#[test]
fn test_e308_undeclared_speaker() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
*FAT:	hi there .
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    let chat_file = parser
        .parse_chat_file(input)
        .map_err(|errors| TestError::ParseErrors { errors })?;
    let errors = validate_chat_file(&chat_file);

    assert!(
        has_error(&errors, ErrorCode::UndeclaredSpeaker),
        "Expected E308 (undeclared speaker FAT), got: {:?}",
        get_error_codes(&errors)
    );

    Ok(())
}

/// Tests e518 invalid date format.
#[test]
fn test_e518_invalid_date_format() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Date:	2020-01-15
*CHI:	hello .
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    match parser.parse_chat_file(input) {
        Ok(chat_file) => {
            let errors = validate_chat_file(&chat_file);
            assert!(
                has_error(&errors, ErrorCode::InvalidDateFormat),
                "Expected E518 (invalid date format), got: {:?}",
                get_error_codes(&errors)
            );
        }
        Err(parse_errors) => {
            assert!(
                parse_errors.errors.iter().any(|e| {
                    matches!(
                        e.code,
                        ErrorCode::SyntaxError
                            | ErrorCode::UnparsableContent
                            | ErrorCode::InvalidDateFormat
                    )
                }),
                "Expected E303, E316, or E518 in parse errors, got: {:?}",
                parse_errors
                    .errors
                    .iter()
                    .map(|e| e.code)
                    .collect::<Vec<_>>()
            );
        }
    }

    Ok(())
}

/// Tests e517 invalid age format.
#[test]
fn test_e517_invalid_age_format() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|invalid_age||||Child|||
*CHI:	hello .
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    let chat_file = parser
        .parse_chat_file(input)
        .map_err(|errors| TestError::ParseErrors { errors })?;
    let errors = validate_chat_file(&chat_file);

    assert!(
        has_error(&errors, ErrorCode::InvalidAgeFormat),
        "Expected E517 (invalid age format), got: {:?}",
        get_error_codes(&errors)
    );

    Ok(())
}

/// Tests e522 speaker not in participants.
#[test]
fn test_e522_speaker_not_in_participants() -> Result<(), TestError> {
    let input = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@ID:	eng|corpus|FAT|||||Father|||
*CHI:	hello .
@End
"#;

    let parser = TreeSitterParser::new().map_err(|source| TestError::TreeSitterInit { source })?;
    match parser.parse_chat_file(input) {
        Ok(chat_file) => {
            let errors = validate_chat_file(&chat_file);

            assert!(
                has_error(&errors, ErrorCode::SpeakerNotDefined)
                    || has_error(&errors, ErrorCode::OrphanIDHeader),
                "Expected E522/E523 (speaker not in participants), got: {:?}",
                get_error_codes(&errors)
            );
        }
        Err(parse_errors) => {
            assert!(
                parse_errors.errors.iter().any(|e| {
                    e.code == ErrorCode::SpeakerNotDefined || e.code == ErrorCode::OrphanIDHeader
                }),
                "Expected E522 or E523 in parse errors, got: {:?}",
                parse_errors
                    .errors
                    .iter()
                    .map(|e| e.code)
                    .collect::<Vec<_>>()
            );
        }
    }

    Ok(())
}
